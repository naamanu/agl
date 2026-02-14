# AgentLang (Minimal v0)

`agentlang` is a tiny language for agentic workflows:
- define `agent`s (model + tools),
- define typed `task` signatures,
- define `pipeline`s that run tasks sequentially or in `parallel ... join`.

This repo includes:
- a lexer/parser,
- a static type checker,
- a runtime executor with parallel branches,
- five runnable `.agent` examples.

## Formal Core

See `docs/semantics.md` for grammar, typing judgments, and small-step runtime transitions.

## Documentation

Start at `docs/README.md` for full docs:

- `docs/getting-started.md`
- `docs/language-reference.md`
- `docs/runtime-and-typing.md`
- `docs/adapters.md`
- `docs/examples.md`
- `docs/contributing.md`
- `docs/semantics.md`

## DSL Example

```agentlang
agent planner {
  model: "gpt-4.1"
  , tools: [web_search]
}

task research(topic: String) -> Obj{notes: String} {}
task draft(notes: String) -> Obj{article: String} {}

pipeline blog_post(topic: String) -> String {
  let r = run research with { topic: topic } by planner;
  let d = run draft with { notes: r.notes } by planner;
  return d.article;
}
```

## Run

```bash
python main.py examples/blog.agent blog_post --input '{"topic":"agent memory patterns"}'
python main.py examples/compare.agent compare_options --input '{"query":"vector database"}'
python main.py examples/support.agent support_reply --input '{"message":"urgent refund request"}'
python main.py examples/reliability.agent resilient_brief --input '{"topic":"api-status","fail_count":1}'
python main.py examples/reliability.agent resilient_brief --input '{"topic":"api-status","fail_count":5}'
python main.py examples/live_answer.agent answer --input '{"question":"What is an agentic workflow?"}'
```

## Live Adapters (OpenAI + Tools)

By default, tasks run in deterministic `mock` mode.  
To use real adapters:

```bash
export OPENAI_API_KEY="..."
python main.py examples/blog.agent blog_post \
  --adapter live \
  --input '{"topic":"agent memory patterns"}'
```

How `live` mode works:
- Agent model comes from the DSL `agent ... { model: "..." }`.
- If an agent has `web_search` in its tools list, the `research` task enriches prompts with DuckDuckGo results.
- `research`, `draft`, `compare`, `respond`, and `llm_complete` can use OpenAI in `live` mode.
- `extract_intent`, `route`, and `flaky_fetch` stay deterministic.

Optional env vars:
- `AGENTLANG_ADAPTER` (`mock` or `live`)
- `AGENTLANG_DEFAULT_MODEL` (fallback model when no agent is bound)
- `AGENTLANG_WEB_RESULTS` (default `5`)
- `AGENTLANG_HTTP_TIMEOUT_S` (default `20`)
- `OPENAI_BASE_URL` (default `https://api.openai.com/v1`)

## Current Semantics (v0 Scope)

- Task declarations are signatures only; execution comes from Python task handlers.
- Statements supported in pipelines:
  - `let x = run task with { ... } by agent retries N on_fail abort;`
  - `let x = run task with { ... } retries N on_fail use <expr>;`
  - `parallel { ... } join;` (run statements only inside)
  - `if <bool-expr> { ... } else { ... }`
  - `return expr;`
- Expressions:
  - string/number/bool literals
  - object/list literals (`{ x: 1 }`, `[1, 2]`)
  - variable and field access (`x`, `x.field`)
  - string/number `+`
  - equality/inequality (`==`, `!=`)

## Project Layout

```text
agentlang/
  ast.py
  lexer.py
  parser.py
  checker.py
  runtime.py
  stdlib.py
examples/
  blog.agent
  compare.agent
  live_answer.agent
  reliability.agent
  support.agent
docs/
  README.md
  getting-started.md
  language-reference.md
  runtime-and-typing.md
  adapters.md
  examples.md
  contributing.md
  semantics.md
main.py
```
