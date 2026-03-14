# AgentLang

A tiny, self-contained DSL for agentic workflows. Define agents, typed tasks, declarative workflows, and low-level pipelines — then run them with deterministic mock adapters or live OpenAI backends.

```agentlang
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

agent researcher {
  model: "gpt-4.1"
  , tools: [web_search]
}

agent reviewer {
  model: "gpt-4.1-mini"
  , tools: [web_search]
}

agent writer {
  model: "gpt-4.1-mini"
  , tools: []
}

task draft_outline(topic: String) -> Obj{outline: String, sources: List[String]} by agent {}
task review_approved_outline(topic: String, outline: String, sources: List[String]) -> Obj{approved: Bool, feedback: String} by agent {}
task revise_outline(topic: String, outline: String, sources: List[String], feedback: String) -> Obj{outline: String, sources: List[String]} by agent {}
task write_post(topic: String, outline: String) -> Obj{article: String} by agent {}

workflow blog_post(topic: String) -> String {
  stage draft = researcher does draft_outline(topic);
  review approved_outline = reviewer checks draft revise with researcher using revise_outline max_rounds 2;
  stage post = writer does write_post(topic, approved_outline.outline);
  return post.article;
}
```

![blog pipeline](docs/assets/screenshots/blog.png)

---

## Features

- **Static type checker** — catches bad arguments, wrong field access, and return type mismatches before execution
- **Type aliases & enums** — `type Notes = Obj{...};` and `enum Tone { formal, casual };` for cleaner signatures
- **Declarative workflows** — `workflow`, `stage`, and `review` compile into explicit pipeline IR
- **Parallel execution** — `parallel { } join` with optional per-block `max_concurrency`
- **Shorthand syntax** — `let r = task(args) by agent;` as concise alternative to `run ... with`
- **Loop control** — `while`, `break`, and `continue` are available in lowered pipelines and low-level authoring
- **Retry & fallback** — `retries N on_fail use <expr>` as first-class syntax
- **Try/catch** — `try { ... } catch err { ... }` for multi-step error recovery
- **Pipeline composition** — pipelines can call other pipelines with `run sub_pipeline with {...}`
- **Assert & test blocks** — `assert expr, "msg";` and `test "name" { ... }` for in-language testing
- **Typed agent tasks** — `task ... by agent {}` enforces declared output shapes at runtime; `model` is optional
- **Plugin system** — `--plugin` loads custom task/tool handlers at runtime
- **Observability** — `--output-trace` writes structured JSON execution traces
- **Live tracing** — `--trace-live` prints model round trips and tool calls to `stderr`
- **Two adapter modes** — `mock` (deterministic, no API key) and `live` (OpenAI + tool calling)
- **No framework dependencies** — lexer, parser, checker, and runtime are all pure Python

---

## Examples

### Parallel comparison

Two research tasks run concurrently, results merged for a downstream compare step.

![compare pipeline](docs/assets/screenshots/compare.png)

### Retry with fallback

`fail_count: 1` — succeeds within the retry budget:

![reliability success](docs/assets/screenshots/reliability_success.png)

`fail_count: 5` — exhausts retries, uses fallback value:

![reliability fallback](docs/assets/screenshots/reliability_fallback.png)

### Input validation

Strict validation before execution runs:

![error missing input](docs/assets/screenshots/error_missing_input.png)

---

## Quick start

```bash
# mock mode — no API key needed
python main.py examples/blog.agent blog_post --input '{"topic":"agent memory patterns"}'
python main.py examples/multiagent_blog.agent publish_topic_blog --input '{"topic":"agent memory systems"}'
python main.py examples/multiagent_blog.agent publish_topic_blog --lower
python main.py examples/compare.agent compare_options --input '{"query":"vector database"}'
python main.py examples/support.agent support_reply --input '{"message":"urgent refund request"}'
python main.py examples/reliability.agent resilient_brief --input '{"topic":"api-status","fail_count":1}'
python main.py examples/reliability.agent resilient_brief --input '{"topic":"api-status","fail_count":5}'

# run test blocks
python main.py examples/showcase_all_features.agent --test --plugin examples/showcase_plugin.py

# write execution trace
python main.py examples/showcase_all_features.agent produce --input '{"topic":"AI safety"}' --output-trace trace.json --plugin examples/showcase_plugin.py

# live mode — requires OPENAI_API_KEY
export OPENAI_API_KEY="..."
python main.py examples/incident_runbook.agent respond_to_incident --adapter live --trace-live --input '{"incident":"database failover drill"}'
```

---

## Project layout

```text
agentlang/
  ast.py        -- AST node dataclasses
  lexer.py      -- tokenizer + string decoder
  parser.py     -- recursive-descent parser
  lowering.py   -- workflow-to-pipeline lowering
  checker.py    -- static type checker
  runtime.py    -- pipeline executor
  stdlib.py     -- built-in task handlers
  context.py    -- ExecutionContext + observability
  plugins.py    -- PluginRegistry for extensibility
  adapters/     -- OpenAI + tool adapters
examples/       -- seventeen runnable .agent programs
docs/           -- full documentation (MkDocs)
main.py         -- CLI entrypoint
```

## Documentation

Full docs at **https://nanamanu.com/agl**

Covers: [Quick Start](docs/tutorial/quickstart.md) · [Language Reference](docs/reference/language.md) · [Adapters](docs/reference/adapters.md) · [Formal Semantics](docs/advanced/semantics.md) · [Contributing](docs/contributing.md)
