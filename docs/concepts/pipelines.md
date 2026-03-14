# Pipelines

A **pipeline** is the explicit execution IR in AgentLang. It takes typed inputs, runs a sequence of statements, and returns a typed value. Pipelines are invoked from the CLI directly, and workflows are lowered into pipelines before type-checking and execution.

## Syntax

```agentlang
pipeline <name>(param: Type, ...) -> ReturnType {
  <statements>
}
```

## Example

```agentlang
pipeline support_reply(message: String) -> String {
  let i = run extract_intent with { message: message } by triage;
  let q = run route with { intent: i.intent, urgency: i.urgency } by triage;
  let r = run respond with { intent: i.intent, queue: q.queue } by triage;
  return r.reply;
}
```

## Relationship to workflows

If `workflow` is the high-level authoring model, `pipeline` is the explicit lowered form.

Use `pipeline` when you want direct control over:

- `parallel { } join` (with optional `max_concurrency`)
- `while`, `break`, and `continue`
- `if` / `if let`
- `try` / `catch`
- `assert` quality gates
- retry/fallback clauses on individual runs
- pipeline composition (pipeline-calls-pipeline)

Use `workflow` when you want to declare stage handoffs and review loops without writing the underlying control flow.

## Rules

- Pipeline names must be unique within a file.
- Every pipeline must have at least one reachable `return`.
- Input params are bound from the CLI `--input` JSON.
- The `return` expression type must match the declared return type.

## Statements

### `let` — run a task

```agentlang
let x = run task_name with { key: expr } by agent_name;
```

Runs a task, binds the result to `x`. See [retry & fallback](retry.md) for optional clauses.

Shorthand syntax is also available:

```agentlang
let x = task_name(expr1, expr2) by agent_name;
```

Arguments are positional and matched to declared parameter names in order.

### `parallel { } join` — concurrent tasks

```agentlang
parallel {
  let a = run research with { topic: query + " A" } by planner;
  let b = run research with { topic: query + " B" } by planner;
} join;
```

Runs all branches concurrently. After `join`, both `a` and `b` are in scope. See [Parallel Execution](parallel.md).

### `if / else` — conditional branching

```agentlang
if f.data == "fallback for " + topic {
  let d = run draft with { notes: "Fallback path: " + f.data } by ops;
  return d.article;
} else {
  let d = run draft with { notes: "Fresh path: " + f.data } by ops;
  return d.article;
}
```

Rules:

- The condition must type-check to `Bool`.
- `else` is optional.
- The type checker does **not** assume a guaranteed return from an `if` without `else` — you must have a `return` reachable outside the `if` block, or include `else`.

### `if let` — option unwrap

```agentlang
if let value = maybe_result {
  return value.name;
} else {
  return "missing";
}
```

`if let` unwraps `Option[T]` values and binds the inner value only inside the success branch.

### `while`, `break`, `continue` — explicit looping

```agentlang
while state.done == false {
  if state.next == 2 {
    break;
  }
  let state = run countdown with { current: state.next } by ops;
}
```

Use these only when you want direct low-level control. Workflow review loops lower into these constructs automatically.

### `try` / `catch` — error recovery

```agentlang
try {
  let result = run risky_task with { input: data };
} catch err {
  let fallback = run safe_task with { query: err };
}
```

If any statement in the `try` block raises a runtime error, execution jumps to `catch`. The error variable is bound as a `String`. See [Error Handling](error-handling.md) for details.

### `assert` — quality gates

```agentlang
assert final.title != "", "Title must not be empty";
```

Halts execution with an assertion error if the expression is `false`. See [Testing](testing.md) for usage in test blocks.

### Pipeline composition (pipeline-calls-pipeline)

A `run` statement can target another pipeline:

```agentlang
pipeline sub_task(topic: String, angle: String) -> DraftResult {
  let notes = run research with { topic: topic + " — " + angle } by researcher;
  let article = draft(notes.notes) by writer;
  return article;
}

pipeline main(topic: String) -> String {
  let result = run sub_task with { topic: topic, angle: "deep-dive" };
  return result.article;
}
```

### `return` — exit the pipeline

```agentlang
return expr;
```

Evaluates `expr` and exits immediately. The type must match the pipeline's declared return type.

## Invoking a pipeline from the CLI

```bash
python main.py <source.agent> <pipeline_name> --input '<json>'
```

Example:

```bash
python main.py examples/support.agent support_reply \
  --input '{"message":"urgent refund request"}'
```

```json
{
  "result": "[triage] Routed as billing to billing-priority."
}
```

## Input validation

AgentLang validates `--input` strictly before execution:

```bash
# Missing required input
python main.py examples/blog.agent blog_post --input '{}'
# Execution error: Pipeline 'blog_post' missing inputs: ['topic'].

# Unknown extra key
python main.py examples/blog.agent blog_post \
  --input '{"topic":"x","extra":"bad"}'
# Execution error: Pipeline 'blog_post' received unknown inputs: ['extra'].
```

Values are also type-checked against declared DSL types (`String`, `Number`, `Bool`, `List[...]`, `Obj{...}`).

To inspect lowered workflow IR, use:

```bash
python main.py examples/multiagent_blog.agent publish_topic_blog --lower
```

## Next: [The Type System](types.md)
