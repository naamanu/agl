# Agents

An **agent** is a named execution profile that binds a model name and a list of tools. Agents don't do anything on their own — they are referenced in run statements to declare *which model and tools* should handle a task.

## Syntax

```agentlang
agent <name> {
  model: "<model-name>"
  , tools: [tool_a, tool_b]
}
```

## Example

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
```

## Rules

- `model` is required and must be a string literal.
- `tools` is required. Use `[]` for an empty tool list.
- Tool names are identifiers and must be declared with `tool` definitions in the DSL.
- Agent names must be unique within a file.
- Duplicate tool names in the list are a parse error.

## Using an agent

Reference an agent in a run statement with `by`:

```agentlang
let r = run research with { topic: topic } by planner;
```

When `by` is omitted, the runtime falls back to the `AGENTLANG_DEFAULT_MODEL` environment variable (in live mode) or the default mock handler.

## Model resolution

| Situation | Model used |
|---|---|
| `by agent_name` present | `agent_name.model` from the DSL |
| `by` omitted, live mode | `AGENTLANG_DEFAULT_MODEL` env var |
| `by` omitted, mock mode | no model needed |

## Tools

Tools are identifiers that activate additional adapter behavior in `live` mode.

Currently supported tools:

| Tool | Effect in live mode |
|---|---|
| `web_search` | Exposes a typed search tool that live model tasks may call at runtime |
| `fetch_url` | Fetches page content and exposes extracted text to live model tasks |

In `mock` mode, tools are parsed and stored but do not trigger external network calls.

## Next: [Tasks](tasks.md)
