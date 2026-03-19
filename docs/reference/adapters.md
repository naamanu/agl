# Adapters

Adapters determine how task handlers are executed. AgentLang ships with three: `mock` for deterministic local development, `live` for real LLM calls via OpenAI, and `anthropic` for real LLM calls via Anthropic/Claude. Both live adapters support typed tool calling.

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

## Live adapter (OpenAI)

Live mode routes LLM-backed tasks to the OpenAI Responses API and activates typed tool integrations.

```bash
export OPENAI_API_KEY="sk-..."

python main.py examples/blog.agent blog_post \
  --adapter live \
  --input '{"topic":"agent memory patterns"}'
```

## Anthropic adapter (Claude)

Anthropic mode routes LLM-backed tasks to the Anthropic Messages API using Claude models. It supports the same tool calling and tracing features as the OpenAI live adapter.

```bash
export ANTHROPIC_API_KEY="sk-ant-..."

python main.py examples/blog.agent blog_post \
  --adapter anthropic \
  --input '{"topic":"agent memory patterns"}'
```

### Model mapping

When using `--adapter anthropic`, OpenAI model names in `.agent` files are automatically mapped to Claude equivalents:

| OpenAI model | Claude model |
|---|---|
| `gpt-4.1` | `claude-sonnet-4-20250514` |
| `gpt-4.1-mini` | `claude-haiku-4-5-20251001` |
| `gpt-4o` | `claude-sonnet-4-20250514` |
| `gpt-4o-mini` | `claude-haiku-4-5-20251001` |

If the model name already starts with `claude-`, it is passed through unchanged. This means you can write `.agent` files that work with both backends without modification, or use Claude model names directly when targeting Anthropic specifically.

### Task behavior by adapter

| Task | Mock | Live (OpenAI) | Anthropic (Claude) |
|---|---|---|---|
| `research` | Placeholder notes | OpenAI call, optionally enriched with web search | Claude call, optionally enriched with web search |
| `draft` | Placeholder article | OpenAI call | Claude call |
| `compare` | Placeholder decision | OpenAI call | Claude call |
| `respond` | Placeholder reply | OpenAI call | Claude call |
| `llm_complete` | Echoes prompt | OpenAI call | Claude call |
| `extract_intent` | Fixed deterministic output | Fixed deterministic output | Fixed deterministic output |
| `route` | Fixed deterministic output | Fixed deterministic output | Fixed deterministic output |
| `flaky_fetch` | Fails N times then succeeds | Fails N times then succeeds | Fails N times then succeeds |
| `task ... by agent {}` | Deterministic placeholder object | OpenAI call with schema-constrained JSON output | Claude call with schema-constrained JSON output |

`extract_intent`, `route`, and `flaky_fetch` behave identically in all modes.

## Web search tool

When an agent declares `tools: [web_search]`, live and anthropic mode tasks expose that declared tool through the runtime tool registry. The model can call `web_search` during task execution.

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

When an agent declares `tools: [fetch_url]`, live and anthropic mode tasks can request raw page text for a URL through the runtime tool registry.

```agentlang
tool fetch_url(url: String) -> Obj{content: String} {}
```

In mock mode, `fetch_url` returns a deterministic placeholder string instead of performing a network request.

## Agent tasks and tool calling

Agent tasks are declared with `by agent`:

```agentlang
task investigate(topic: String) -> Obj{summary: String, sources: List[String]} by agent {}
```

In live and anthropic mode, the runtime:

1. Selects the model from the bound `agent` (with automatic model mapping in anthropic mode)
2. Exposes the agent's declared tools to the model
3. Executes tool calls through the runtime tool registry
4. Requires the final model output to decode as JSON matching the declared return type

This keeps orchestration in the DSL while provider-specific tool mechanics stay in the adapter/runtime layer.

### JSON extraction

Claude models sometimes wrap JSON output in markdown code fences or add conversational text around the JSON object. The runtime handles this automatically by:

1. Stripping markdown fences (`` ```json ... ``` `` or `` ``` ... ``` ``)
2. Attempting direct JSON parse
3. If that fails, extracting the first `{` to last `}` substring and parsing that

This means agent tasks work reliably with both OpenAI and Claude without any changes to your `.agent` files.

## Live tracing

Use `--trace-live` or `AGENTLANG_TRACE_LIVE=1` to print live execution trace lines to `stderr`. This works with both `--adapter live` and `--adapter anthropic`.

```bash
python main.py examples/incident_runbook.agent respond_to_incident \
  --adapter live \
  --trace-live \
  --input '{"incident":"database failover drill"}'
```

Example trace lines (OpenAI):

```text
[trace] task=draft_response_plan agent=researcher model=gpt-4.1 start args={"incident":"database failover drill"}
[trace] task=review_response_plan agent=reviewer tool=web_search call args={"query":"best practices for database failover drills site:aws.amazon.com"}
[trace] task=publish_runbook agent=commander model=gpt-4.1-mini result={"runbook":"Runbook for Database Failover Drill: ..."}
```

Example trace lines (Anthropic):

```text
[trace] task=draft_response_plan agent=researcher model=claude-sonnet-4-20250514 start args={"incident":"database failover drill"}
[trace] task=draft_response_plan agent=researcher anthropic request mode=complete model=claude-sonnet-4-20250514
[trace] task=draft_response_plan agent=researcher anthropic response mode=complete text=...
```

## Model resolution

The model used for a task is determined by:

1. The `agent` bound via `by agent_name` → uses `agent_name.model` from the DSL
2. No `by` clause → uses `AGENTLANG_DEFAULT_MODEL` env var (live/anthropic mode only)
3. In anthropic mode, the resolved model name is mapped from OpenAI to Claude equivalents (see [Model mapping](#model-mapping) above)

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `OPENAI_API_KEY` | — | Required for `--adapter live` |
| `OPENAI_BASE_URL` | `https://api.openai.com/v1` | Override for custom OpenAI endpoints or proxies |
| `ANTHROPIC_API_KEY` | — | Required for `--adapter anthropic` |
| `ANTHROPIC_BASE_URL` | `https://api.anthropic.com` | Override for custom Anthropic endpoints or proxies |
| `AGENTLANG_ADAPTER` | `mock` | Set default adapter without `--adapter` flag |
| `AGENTLANG_DEFAULT_MODEL` | `gpt-4.1-mini` | Fallback model when no `by` binding (mapped automatically in anthropic mode) |
| `AGENTLANG_WEB_RESULTS` | `5` | DuckDuckGo results for `research` in live/anthropic mode |
| `AGENTLANG_HTTP_TIMEOUT_S` | `20` | Timeout in seconds for HTTP calls |
| `AGENTLANG_TRACE_LIVE` | `0` | Enable live tracing without passing `--trace-live` |

## Error handling

Adapter errors are caught and surfaced as runtime failures:

```text
Execution error: OPENAI_API_KEY is required when adapter mode is 'live'.
Execution error: ANTHROPIC_API_KEY is required when adapter mode is 'anthropic'.
Execution error: LLM call failed: <http error>
Execution error: <adapter timeout message>
Execution error: Task 'draft_response_plan' by agent 'researcher' failed after 1 attempts. Last error: RuntimeError: LLM call failed: ...
```

The `--adapter live` or `--adapter anthropic` flag enables the respective adapter for a single run without setting `AGENTLANG_ADAPTER` permanently.

!!! warning "Security"
    Never hardcode `OPENAI_API_KEY` or `ANTHROPIC_API_KEY` in `.agent` files, source code, or documentation. Use environment variables or your shell profile.
