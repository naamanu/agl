# Your First Pipeline

This tutorial walks through building an AgentLang pipeline from scratch — a support triage workflow that classifies a message, routes it to the right queue, and generates a response.

By the end you'll have a working `.agent` file and understand the relationship between agents, tasks, and pipelines.

## The goal

Given a user message like `"urgent refund request"`, we want to:

1. Extract the intent and urgency
2. Route to the correct support queue
3. Generate a reply

This mirrors the included `examples/support.agent` — but we'll build it step by step.

## Step 1: Declare an agent

Create a new file `my_support.agent`:

```agentlang
agent triage {
  model: "gpt-4.1-mini"
  , tools: []
}
```

An `agent` block binds a name to a model and tool list. You reference it later with `by triage` in run statements.

## Step 2: Declare tasks

Tasks are typed signatures — they declare what goes in and what comes out:

```agentlang
agent triage {
  model: "gpt-4.1-mini"
  , tools: []
}

task extract_intent(message: String) -> Obj{intent: String, urgency: String} {}
task route(intent: String, urgency: String) -> Obj{queue: String} {}
task respond(intent: String, queue: String) -> Obj{reply: String} {}
```

!!! note "Task bodies are always empty"
    The `{}` body is intentional. Task *signatures* live in the DSL; task *behavior* is supplied by Python handlers at runtime. This separation keeps the language simple and the runtime extensible.

## Step 3: Write the pipeline

```agentlang
agent triage {
  model: "gpt-4.1-mini"
  , tools: []
}

task extract_intent(message: String) -> Obj{intent: String, urgency: String} {}
task route(intent: String, urgency: String) -> Obj{queue: String} {}
task respond(intent: String, queue: String) -> Obj{reply: String} {}

pipeline support_reply(message: String) -> String {
  let i = run extract_intent with { message: message } by triage;
  let q = run route with { intent: i.intent, urgency: i.urgency } by triage;
  let r = run respond with { intent: i.intent, queue: q.queue } by triage;
  return r.reply;
}
```

The pipeline:

- takes a `message: String` as input
- runs three tasks in sequence, threading outputs into subsequent inputs
- returns `r.reply` which has the declared return type `String`

## Step 4: Run it

```bash
python main.py my_support.agent support_reply \
  --input '{"message":"urgent refund request"}'
```

```json
{
  "result": "[triage] Routed as billing to billing-priority."
}
```

## Step 5: Add error handling

What if `route` fails? Add a retry and fallback:

```agentlang
let q = run route
  with { intent: i.intent, urgency: i.urgency }
  by triage
  retries 2
  on_fail use { queue: "general" };
```

If `route` fails after 3 total attempts, execution continues with `queue: "general"` instead of aborting.

## What the type checker catches

The type checker runs before execution. Try breaking something — for example, passing the wrong field:

```agentlang
-- wrong: respond expects intent: String, not message: String
let r = run respond with { intent: message, queue: q.queue } by triage;
```

```
TypeError: argument 'intent' expected String, got String  ✓
-- (this actually would pass since message is String)
```

Try a real type mismatch:

```agentlang
let r = run respond with { intent: 42, queue: q.queue } by triage;
```

```
TypeError: argument 'intent' expected String, got Number
```

The pipeline never executes — the error is caught statically.

## Next steps

- Add a `parallel` block to run tasks concurrently → [Parallel Execution](../concepts/parallel.md)
- Use `if/else` to branch on task output → [Pipelines](../concepts/pipelines.md)
- Connect to real LLMs → [Adapters](../reference/adapters.md)
