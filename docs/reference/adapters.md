# Adapters

Adapters determine how task handlers are executed. AgentLang ships with two: `mock` for deterministic local development, and `live` for real LLM calls via OpenAI.

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

Live mode routes LLM-backed tasks to the OpenAI Responses API and activates tool integrations.

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

`extract_intent`, `route`, and `flaky_fetch` behave identically in both modes.

## Web search tool

When an agent declares `tools: [web_search]`, the `research` task in live mode exposes that declared tool to the model through the runtime tool registry. The model can then call `web_search` during task execution.

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

## Error handling

Adapter errors are caught and surfaced as runtime failures:

```text
Execution error: OPENAI_API_KEY is required when adapter mode is 'live'.
Execution error: LLM call failed: <http error>
Execution error: <adapter timeout message>
```

The `--adapter live` flag enables the live adapter for a single run without setting `AGENTLANG_ADAPTER` permanently.

!!! warning "Security"
    Never hardcode `OPENAI_API_KEY` in `.agent` files, source code, or documentation. Use environment variables or your shell profile.
