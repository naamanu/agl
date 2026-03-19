# Agents

An **agent** is a named execution profile that binds a model name and a list of tools. Agents don't do anything on their own — they are referenced in pipeline `run` statements or selected implicitly by workflow `stage`/`review` steps to declare *which model and tools* should handle a task.

## Syntax

```agentlang
agent <name> {
  [ model: "<model-name>" ]
  , tools: [tool_a, tool_b]
}
```

The `model` field is optional. When omitted, the agent uses the runtime default.

## Example

```agentlang
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

agent planner {
  model: "gpt-4.1"
  , tools: [web_search]
}

-- model omitted — uses AGENTLANG_DEFAULT_MODEL or mock default
agent writer {
  tools: []
}
```

## Rules

- `model` is optional. When omitted, it defaults to `None` and the runtime falls back to `AGENTLANG_DEFAULT_MODEL` (live/anthropic mode) or the default mock handler. In anthropic mode, OpenAI model names are automatically mapped to Claude equivalents (see [Model mapping](../reference/adapters.md#model-mapping)).
- `tools` is required. Use `[]` for an empty tool list.
- Tool names are identifiers and must be declared with `tool` definitions in the DSL.
- Agent names must be unique within a file.
- Duplicate tool names in the list are a parse error.

## Using an agent

Reference an agent in a pipeline run statement with `by`:

```agentlang
let r = run research with { topic: topic } by planner;
```

When `by` is omitted, the runtime falls back to the `AGENTLANG_DEFAULT_MODEL` environment variable (in live/anthropic mode) or the default mock handler.

In workflows, the agent is attached directly to the step:

```agentlang
stage plan = planner does draft_outline(topic);
```

## Model resolution

| Situation | Model used |
|---|---|
| `by agent_name` present, `model` declared | `agent_name.model` from the DSL (mapped in anthropic mode) |
| `by agent_name` present, `model` omitted | `AGENTLANG_DEFAULT_MODEL` env var (live/anthropic) or mock default |
| `by` omitted, live/anthropic mode | `AGENTLANG_DEFAULT_MODEL` env var |
| `by` omitted, mock mode | no model needed |

## Tools

Tools are identifiers that activate additional adapter behavior in `live` and `anthropic` mode.

Currently supported tools:

| Tool | Effect in live/anthropic mode |
|---|---|
| `web_search` | Exposes a typed search tool that model tasks may call at runtime |
| `fetch_url` | Fetches page content and exposes extracted text to model tasks |

In `mock` mode, tools are parsed and stored but do not trigger external network calls.

## Next: [Tasks](tasks.md)
