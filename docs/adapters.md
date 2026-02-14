# Adapters

AgentLang supports two runtime adapter modes.

## Modes

### `mock` (default)

- no external API calls
- deterministic local handlers
- best for local development and tests

### `live`

- OpenAI Responses API for LLM-backed tasks
- optional web search enrichment for `research` when agent has `web_search`

## Enable Live Mode

```bash
export OPENAI_API_KEY="..."
python main.py examples/blog.agent blog_post --adapter live --input '{"topic":"agent memory patterns"}'
```

## Environment Variables

- `OPENAI_API_KEY`
  - required in `live` mode
- `OPENAI_BASE_URL`
  - default: `https://api.openai.com/v1`
- `AGENTLANG_ADAPTER`
  - default: `mock`
- `AGENTLANG_DEFAULT_MODEL`
  - fallback model if no `by` agent binding exists
- `AGENTLANG_WEB_RESULTS`
  - number of DuckDuckGo hits used in context (default `5`)
- `AGENTLANG_HTTP_TIMEOUT_S`
  - adapter timeout in seconds (default `20`)

## Agent Model And Tool Resolution

- model source:
  - from `agent` declaration when `by agent_name` is used
  - else `AGENTLANG_DEFAULT_MODEL`
- tool source:
  - from `agent.tools`
  - if includes `web_search`, `research` pulls DDG results and injects them into prompt context

## LLM-backed Task Behavior

In `live` mode, these tasks call OpenAI:

- `research`
- `draft`
- `compare`
- `respond`
- `llm_complete`

These remain local deterministic handlers:

- `extract_intent`
- `route`
- `flaky_fetch`

## Error Handling

Adapter errors are wrapped and surfaced as runtime failures.

Examples:

- missing API key
- HTTP/network failures
- malformed adapter response
- timeout

CLI output:

```text
Execution error: <reason>
```

## Security Notes

- never hardcode API keys in source files
- use environment variables or your shell profile
- avoid committing secrets in docs/examples

