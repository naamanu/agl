# Plugins

AgentLang supports a plugin system for registering custom task and tool handlers at runtime, without modifying the core codebase.

## Plugin contract

A plugin is a Python module that exports a `register(registry)` function. The `registry` argument is a `PluginRegistry` instance.

```python
def register(registry):
    def my_handler(args, agent):
        return {"result": f"handled: {args['input']}"}

    registry.register_task("my_task", my_handler)
```

## PluginRegistry API

| Method | Description |
|---|---|
| `register_task(name, handler)` | Register a task handler callable |
| `register_tool(name, handler)` | Register a tool handler callable |
| `get_task_handlers()` | Returns `dict[str, callable]` of registered task handlers |
| `get_tool_handlers()` | Returns `dict[str, callable]` of registered tool handlers |

### Handler signatures

Task handlers receive two arguments:

```python
def handler(args: dict[str, Any], agent: str | None) -> dict:
    ...
```

- `args` — dictionary of task arguments (keys match DSL parameter names)
- `agent` — the agent name string, or `None` if no agent binding

The return value must be a dictionary matching the task's declared return type.

## Loading plugins

Use the `--plugin` flag to load a plugin at startup:

```bash
python main.py examples/showcase_all_features.agent produce \
  --input '{"topic":"AI safety"}' \
  --plugin examples/showcase_plugin.py
```

The `--plugin` flag accepts either a file path or a dotted Python module name. It may be repeated to load multiple plugins.

## Precedence

Plugin-registered handlers take precedence over built-in handlers in `stdlib.py`. This allows plugins to override default task behavior.

## Example plugin

From `examples/showcase_plugin.py`:

```python
def register(registry):
    def merge_drafts(args, _agent):
        a = args["draft_a"]
        b = args["draft_b"]
        wa = args["word_count_a"]
        wb = args["word_count_b"]
        merged = f"{a}\n\n{b}".strip() if b else a
        sections = [s for s in [a, b] if s]
        return {
            "article": merged,
            "sections": sections,
            "total_words": wa + wb,
        }

    def risky_enrich(args, _agent):
        topic = args["topic"]
        if "fail" in topic.lower():
            raise RuntimeError(f"Enrichment service unavailable for: {topic}")
        return {"extra": f"[Enriched context for '{topic}']"}

    def fallback_enrich(args, _agent):
        return {"extra": f"[Fallback content: {args['query'][:80]}]"}

    registry.register_task("merge_drafts", merge_drafts)
    registry.register_task("risky_enrich", risky_enrich)
    registry.register_task("fallback_enrich", fallback_enrich)
```

## Next: [Observability](observability.md)
