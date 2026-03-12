# Examples

AgentLang ships with twelve `.agent` examples in `examples/`. Each one exercises a distinct set of language features.

---

## `blog.agent` — sequential pipeline

**Features:** sequential `let` statements, two agents, field access

```agentlang
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

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
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

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
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

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

## `tool_declarations.agent` — first-class tool declarations

**Features:** declared `tool`, agent tool reference validation

```agentlang
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

agent researcher {
  model: "gpt-4.1-mini"
  , tools: [web_search]
}

pipeline declared_tool_name(query: String) -> String {
  return "declared " + query;
}
```

```bash
python main.py examples/tool_declarations.agent declared_tool_name \
  --input '{"query":"web_search"}'
```

```json
{
  "result": "declared web_search"
}
```

---

## `tool_backed_research.agent` — tool-backed research task

**Features:** declared tool, agent capability binding, tool-aware `research` task

```agentlang
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

agent scout {
  model: "gpt-4.1"
  , tools: [web_search]
}

task research(topic: String) -> Obj{notes: String} {}

pipeline notes(topic: String) -> String {
  let r = run research with { topic: topic } by scout;
  return r.notes;
}
```

```bash
python main.py examples/tool_backed_research.agent notes \
  --input '{"topic":"incident response"}'
```

```json
{
  "result": "[scout] key points for 'incident response'"
}
```

Live mode uses the declared `web_search` tool when the model decides it needs external grounding:

```bash
python main.py examples/tool_backed_research.agent notes \
  --adapter live \
  --input '{"topic":"incident response"}'
```

---

## `agent_task.agent` — task declared for agent execution

**Features:** `task ... by agent {}`, explicit agent binding, declared tool access

```agentlang
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

agent researcher {
  model: "gpt-4.1"
  , tools: [web_search]
}

task investigate(topic: String) -> Obj{summary: String, sources: List[String]} by agent {}

pipeline brief(topic: String) -> String {
  let r = run investigate with { topic: topic } by researcher;
  return r.summary;
}
```

Mock mode:

```bash
python main.py examples/agent_task.agent brief \
  --input '{"topic":"incident response"}'
```

```json
{
  "result": "[researcher:investigate.summary] incident response"
}
```

Live mode:

```bash
python main.py examples/agent_task.agent brief \
  --adapter live \
  --input '{"topic":"incident response"}'
```

---

## `multiagent_blog.agent` — multi-agent blog pipeline with bounded review loop

**Features:** multiple agents, multiple tools, agent tasks, explicit planner-reviewer revision cycle, editor and publisher handoff

```agentlang
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}
tool fetch_url(url: String) -> Obj{content: String} {}

agent planner {
  model: "gpt-4.1"
  , tools: [web_search, fetch_url]
}

agent reviewer {
  model: "gpt-4.1-mini"
  , tools: [web_search]
}

agent editor {
  model: "gpt-4.1-mini"
  , tools: []
}

agent publisher {
  model: "gpt-4.1-mini"
  , tools: []
}
```

This example uses a real review loop. The planner produces an outline, the reviewer approves or rejects it, and the pipeline keeps revising until the outline is approved or the revision budget is exhausted.

```bash
python main.py examples/multiagent_blog.agent publish_topic_blog \
  --input '{"topic":"agent memory systems"}'
```

---

## `while_loop.agent` — first-class looping construct

**Features:** `while`, variable rebinding, deterministic loop progress

```agentlang
agent ops {
  model: "gpt-4.1-mini"
  , tools: []
}

task countdown(current: Number) -> Obj{next: Number, done: Bool} {}

pipeline loop_to_zero(start: Number) -> Number {
  let state = run countdown with { current: start } by ops;

  while state.done == false {
    let state = run countdown with { current: state.next } by ops;
  }

  return state.next;
}
```

```bash
python main.py examples/while_loop.agent loop_to_zero \
  --input '{"start":3}'
```

```json
{
  "result": 0
}
```

---

## `break_continue.agent` — loop control flow

**Features:** `while`, `break`, `continue`, variable rebinding

```bash
python main.py examples/break_continue.agent stop_early \
  --input '{"start":4}'
python main.py examples/break_continue.agent skip_once \
  --input '{"start":4}'
```

```json
{
  "result": 2
}
```

```json
{
  "result": 0
}
```

---

## Authoring tips

- Keep task signatures small and explicit.
- Choose the pipeline return type intentionally — it's what `--input` validation checks against.
- Include both a happy-path and a failure-path input when testing retry behavior.
- Add a new example whenever you introduce a new language feature.
