# Adapters

Adapters determine how task handlers are executed. AgentLang ships with two: `mock` for deterministic local development, and `live` for real LLM calls via OpenAI plus typed tool calling.

## Mock adapter (default)

Mock mode uses local Python handlers that return structured placeholder values. No external API calls are made.

```bash
python main.py examples/blog.agent blog_post \
  --input '{"topic":"agent memory patterns"}'
```

```json
{
  "result": "[writer] Draft article:\n[planner] key points for 'agent memory patterns'"
}
```

**Use mock mode for:**

- Local development and iteration
- Testing pipeline structure and routing logic
- CI without API keys

## Live adapter

Live mode routes LLM-backed tasks to the OpenAI Responses API and activates typed tool integrations.

```bash
export OPENAI_API_KEY="sk-..."

python main.py examples/blog.agent blog_post \
  --adapter live \
  --input '{"topic":"agent memory patterns"}'
```

### Task behavior by adapter

| Task | Mock | Live |
|---|---|---|
| `research` | Placeholder notes | OpenAI call, optionally enriched with web search |
| `draft` | Placeholder article | OpenAI call |
| `compare` | Placeholder decision | OpenAI call |
| `respond` | Placeholder reply | OpenAI call |
| `llm_complete` | Echoes prompt | OpenAI call |
| `extract_intent` | Fixed deterministic output | Fixed deterministic output |
| `route` | Fixed deterministic output | Fixed deterministic output |
| `flaky_fetch` | Fails N times then succeeds | Fails N times then succeeds |
| `task ... by agent {}` | Deterministic placeholder object | OpenAI call with schema-constrained JSON output |

`extract_intent`, `route`, and `flaky_fetch` behave identically in both modes.

## Web search tool

When an agent declares `tools: [web_search]`, live model tasks expose that declared tool through the runtime tool registry. The model can call `web_search` during task execution.

```agentlang
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

agent planner {
  model: "gpt-4.1"
  , tools: [web_search]
}
```

```bash
export AGENTLANG_WEB_RESULTS=10   # fetch 10 results instead of the default 5
```

In mock mode, `web_search` in the tools list is parsed and stored but has no effect.

## Fetch URL tool

When an agent declares `tools: [fetch_url]`, live model tasks can request raw page text for a URL through the runtime tool registry.

```agentlang
tool fetch_url(url: String) -> Obj{content: String} {}
```

In mock mode, `fetch_url` returns a deterministic placeholder string instead of performing a network request.

## Agent tasks and tool calling

Agent tasks are declared with `by agent`:

```agentlang
task investigate(topic: String) -> Obj{summary: String, sources: List[String]} by agent {}
```

In live mode, the runtime:

1. Selects the model from the bound `agent`
2. Exposes the agent's declared tools to the model
3. Executes tool calls through the runtime tool registry
4. Requires the final model output to decode as JSON matching the declared return type

This keeps orchestration in the DSL while provider-specific tool mechanics stay in the adapter/runtime layer.

## Live tracing

Use `--trace-live` or `AGENTLANG_TRACE_LIVE=1` to print live execution trace lines to `stderr`.

```bash
python main.py examples/incident_runbook.agent respond_to_incident \
  --adapter live \
  --trace-live \
  --input '{"incident":"database failover drill"}'
```

Example trace lines:

```text
[trace] task=draft_response_plan agent=researcher model=gpt-4.1 start args={"incident":"database failover drill"}
[trace] task=review_response_plan agent=reviewer tool=web_search call args={"query":"best practices for database failover drills site:aws.amazon.com"}
[trace] task=publish_runbook agent=commander model=gpt-4.1-mini result={"runbook":"Runbook for Database Failover Drill: ..."}
```

## Model resolution

The model used for a task is determined by:

1. The `agent` bound via `by agent_name` → uses `agent_name.model` from the DSL
2. No `by` clause → uses `AGENTLANG_DEFAULT_MODEL` env var (live mode only)

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `OPENAI_API_KEY` | — | Required for live mode |
| `OPENAI_BASE_URL` | `https://api.openai.com/v1` | Override for custom endpoints or proxies |
| `AGENTLANG_ADAPTER` | `mock` | Set default adapter without `--adapter` flag |
| `AGENTLANG_DEFAULT_MODEL` | — | Fallback model when no `by` binding |
| `AGENTLANG_WEB_RESULTS` | `5` | DuckDuckGo results for `research` in live mode |
| `AGENTLANG_HTTP_TIMEOUT_S` | `20` | Timeout in seconds for HTTP calls |
| `AGENTLANG_TRACE_LIVE` | `0` | Enable live tracing without passing `--trace-live` |

## Error handling

Adapter errors are caught and surfaced as runtime failures:

```text
Execution error: OPENAI_API_KEY is required when adapter mode is 'live'.
Execution error: LLM call failed: <http error>
Execution error: <adapter timeout message>
Execution error: Task 'draft_response_plan' by agent 'researcher' failed after 1 attempts. Last error: RuntimeError: LLM call failed: ...
```

The `--adapter live` flag enables the live adapter for a single run without setting `AGENTLANG_ADAPTER` permanently.

!!! warning "Security"
    Never hardcode `OPENAI_API_KEY` in `.agent` files, source code, or documentation. Use environment variables or your shell profile.
