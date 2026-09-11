# Adapters

Adapters determine how task handlers are executed. AgentLang ships with deterministic `mock`, OpenAI (`openai`, with `live` as a compatibility alias), and Anthropic (`anthropic`) modes. Both provider adapters support typed tool calling.

## Mock adapter (default)

Mock mode uses deterministic handlers in the Rust standard library and runtime. No external API calls are made.

```bash
cargo run -- examples/blog.agent blog_post \
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

cargo run -- examples/blog.agent blog_post \
  --adapter openai \
  --input '{"topic":"agent memory patterns"}'
```

## Anthropic adapter (Claude)

Anthropic mode routes LLM-backed tasks to the Anthropic Messages API using Claude models. It supports the same tool calling and tracing features as the OpenAI live adapter.

```bash
export ANTHROPIC_API_KEY="sk-ant-..."

cargo run -- examples/blog.agent blog_post \
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

The native built-in currently returns at most five results per call. Applications that need different search behavior can supply their own `ToolRegistry` handler.

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

Use `--trace-live` to print live execution trace lines to `stderr`. This works with both `--adapter openai` and `--adapter anthropic`.

```bash
cargo run -- examples/incident_runbook.agent respond_to_incident \
  --adapter openai \
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

1. `AGL_OPENAI_MODEL` or `AGL_ANTHROPIC_MODEL`, when set for the selected provider.
2. The model declared by the agent bound with `by agent_name`.
3. The native provider default when the declaration omits a model.

Legacy model declarations are mapped by provider and capability tier. Deployment overrides allow model upgrades without editing `.agent` source.

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `OPENAI_API_KEY` | — | Required for `--adapter openai` (`live` is a compatibility alias) |
| `ANTHROPIC_API_KEY` | — | Required for `--adapter anthropic` |
| `AGL_OPENAI_MODEL` | `gpt-5.6-sol` | Global OpenAI model override |
| `AGL_ANTHROPIC_MODEL` | provider default | Global Anthropic model override |

## Error handling

Adapter errors are caught and surfaced as runtime failures:

```text
Execution error: OPENAI_API_KEY is required when adapter mode is 'live'.
Execution error: ANTHROPIC_API_KEY is required when adapter mode is 'anthropic'.
Execution error: LLM call failed: <http error>
Execution error: <adapter timeout message>
Execution error: Task 'draft_response_plan' by agent 'researcher' failed after 1 attempts. Last error: RuntimeError: LLM call failed: ...
```

The `--adapter openai` or `--adapter anthropic` flag enables the respective adapter. `--adapter live` remains an alias for OpenAI compatibility.

> [!WARNING]
> **Security**
>
> Never hardcode `OPENAI_API_KEY` or `ANTHROPIC_API_KEY` in `.agent` files, source code, or documentation. Use environment variables or your shell profile.
