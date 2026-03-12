# Examples

AgentLang ships with six `.agent` examples in `examples/`. Each one exercises a distinct set of language features.

---

## `blog.agent` — sequential pipeline

**Features:** sequential `let` statements, two agents, field access

```agentlang
agent planner {
  model: "gpt-4.1"
  , tools: [web_search]
}

agent writer {
  model: "gpt-4.1-mini"
  , tools: []
}

task research(topic: String) -> Obj{notes: String} {}
task draft(notes: String) -> Obj{article: String} {}

pipeline blog_post(topic: String) -> String {
  let r = run research with { topic: topic } by planner;
  let d = run draft with { notes: r.notes } by writer;
  return d.article;
}
```

```bash
python main.py examples/blog.agent blog_post \
  --input '{"topic":"agent memory patterns"}'
```

```json
{
  "result": "[writer] Draft article:\n[planner] key points for 'agent memory patterns'"
}
```

---

## `compare.agent` — parallel execution

**Features:** `parallel { } join`, merging branch outputs downstream

```agentlang
agent planner {
  model: "gpt-4.1"
  , tools: [web_search]
}

agent reviewer {
  model: "gpt-4.1-mini"
  , tools: []
}

task research(topic: String) -> Obj{notes: String} {}
task compare(note_a: String, note_b: String) -> Obj{decision: String} {}

pipeline compare_options(query: String) -> String {
  parallel {
    let a = run research with { topic: query + " option A" } by planner;
    let b = run research with { topic: query + " option B" } by planner;
  } join;

  let c = run compare with { note_a: a.notes, note_b: b.notes } by reviewer;
  return c.decision;
}
```

```bash
python main.py examples/compare.agent compare_options \
  --input '{"query":"vector database"}'
```

```json
{
  "result": "[reviewer] Option A vs B\nA: [planner] key points for 'vector database option A'\nB: [planner] key points for 'vector database option B'"
}
```

---

## `support.agent` — multi-step routing

**Features:** three sequential tasks, field threading between steps

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

```bash
python main.py examples/support.agent support_reply \
  --input '{"message":"urgent refund request"}'
```

```json
{
  "result": "[triage] Routed as billing to billing-priority."
}
```

---

## `reliability.agent` — retry, fallback, conditional

**Features:** `retries`, `on_fail use`, `if/else`, string equality

```agentlang
agent ops {
  model: "gpt-4.1-mini"
  , tools: []
}

task flaky_fetch(key: String, failures_before_success: Number) -> Obj{data: String} {}
task draft(notes: String) -> Obj{article: String} {}

pipeline resilient_brief(topic: String, fail_count: Number) -> String {
  let f = run flaky_fetch
    with { key: topic, failures_before_success: fail_count }
    by ops
    retries 2
    on_fail use { data: "fallback for " + topic };

  if f.data == "fallback for " + topic {
    let d = run draft with { notes: "Fallback path: " + f.data } by ops;
    return d.article;
  } else {
    let d = run draft with { notes: "Fresh path: " + f.data } by ops;
    return d.article;
  }
}
```

Run — succeeds within retry budget (`fail_count: 1` < `retries 2`):

```bash
python main.py examples/reliability.agent resilient_brief \
  --input '{"topic":"api-status","fail_count":1}'
```

```json
{
  "result": "[ops] Draft article:\nFresh path: [ops] fetched payload for api-status"
}
```

Run — exhausts retries, uses fallback (`fail_count: 5` > `retries 2`):

```bash
python main.py examples/reliability.agent resilient_brief \
  --input '{"topic":"api-status","fail_count":5}'
```

```json
{
  "result": "[ops] Draft article:\nFallback path: fallback for api-status"
}
```

---

## `live_answer.agent` — minimal live adapter test

**Features:** single-task pipeline, direct LLM prompt, `llm_complete`

```agentlang
agent assistant {
  model: "gpt-4.1-mini"
  , tools: []
}

task llm_complete(prompt: String) -> Obj{text: String} {}

pipeline answer(question: String) -> String {
  let r = run llm_complete
    with { prompt: "Answer briefly: " + question }
    by assistant;
  return r.text;
}
```

Mock mode:

```bash
python main.py examples/live_answer.agent answer \
  --input '{"question":"What is an agentic workflow?"}'
```

```json
{
  "result": "[assistant] Answer briefly: What is an agentic workflow?"
}
```

Live mode:

```bash
export OPENAI_API_KEY="sk-..."

python main.py examples/live_answer.agent answer \
  --adapter live \
  --input '{"question":"What is an agentic workflow?"}'
```

---

## `complete_agent.agent` — fuller multi-agent workflow

**Features:** tool-enabled research agent, parallel research, downstream comparison, direct LLM prompt, final drafting step

```agentlang
agent scout {
  model: "gpt-4.1"
  , tools: [web_search]
}

agent analyst {
  model: "gpt-4.1-mini"
  , tools: []
}

agent writer {
  model: "gpt-4.1-mini"
  , tools: []
}

task research(topic: String) -> Obj{notes: String} {}
task compare(note_a: String, note_b: String) -> Obj{decision: String} {}
task llm_complete(prompt: String) -> Obj{text: String} {}
task draft(notes: String) -> Obj{article: String} {}

pipeline executive_brief(product: String, competitor_a: String, competitor_b: String) -> String {
  parallel {
    let a = run research with { topic: competitor_a + " strategy for " + product } by scout;
    let b = run research with { topic: competitor_b + " strategy for " + product } by scout;
  } join;

  let c = run compare with { note_a: a.notes, note_b: b.notes } by analyst;
  let o = run llm_complete with { prompt: "Write a concise executive brief outline for " + product + ".\nDecision context:\n" + c.decision } by analyst;
  let d = run draft with { notes: "Executive outline:\n" + o.text + "\n\nResearch A:\n" + a.notes + "\n\nResearch B:\n" + b.notes } by writer;
  return d.article;
}
```

Mock mode:

```bash
python main.py examples/complete_agent.agent executive_brief \
  --input '{"product":"team chat","competitor_a":"Slack","competitor_b":"Microsoft Teams"}'
```

This example is the closest thing in-tree to a "complete agent" workflow: a tool-enabled scout gathers context, an analyst synthesizes a decision, and a writer turns that into a final artifact.

---

## Authoring tips

- Keep task signatures small and explicit.
- Choose the pipeline return type intentionally — it's what `--input` validation checks against.
- Include both a happy-path and a failure-path input when testing retry behavior.
- Add a new example whenever you introduce a new language feature.
