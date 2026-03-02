# Tasks

A **task** is a typed signature that declares the inputs and output of a callable unit of work. Tasks are the nouns of AgentLang — they describe *what* can be run, but not *how* it runs.

## Syntax

```agentlang
task <name>(param1: Type, param2: Type) -> ReturnType {}
```

## Example

```agentlang
task research(topic: String) -> Obj{notes: String} {}
task draft(notes: String) -> Obj{article: String} {}
task compare(note_a: String, note_b: String) -> Obj{decision: String} {}
```

## Rules

- The body is always `{}`. Task behavior is supplied by Python handlers at runtime, not inline code.
- Task names must be unique within a file.
- Parameter names must be unique within a task.
- All parameters and the return type must use supported [types](types.md).

## Why empty bodies?

This separation between signature and behavior is intentional:

- The DSL stays simple and statically checkable.
- Behavior can be swapped between `mock` and `live` adapters without changing the `.agent` source.
- New tasks can be added to `agentlang/stdlib.py` without touching the language grammar.

## Built-in task handlers

The following tasks are available out of the box:

| Task name | Mock behavior | Live behavior |
|---|---|---|
| `research` | Returns placeholder notes | Calls OpenAI with optional web search |
| `draft` | Returns placeholder article | Calls OpenAI to write a draft |
| `compare` | Returns placeholder decision | Calls OpenAI to compare two notes |
| `respond` | Returns placeholder reply | Calls OpenAI to generate a reply |
| `llm_complete` | Echoes the prompt | Calls OpenAI with the given prompt |
| `extract_intent` | Returns fixed `intent`/`urgency` | Deterministic local handler |
| `route` | Returns routing decision | Deterministic local handler |
| `flaky_fetch` | Fails N times, then succeeds | Deterministic local handler |

!!! note
    `extract_intent`, `route`, and `flaky_fetch` are always deterministic regardless of adapter mode.

## Adding a new task

1. Declare the signature in your `.agent` file.
2. Add a Python handler in `agentlang/stdlib.py`.
3. Register it in `default_task_registry()`.

See [Contributing → Adding a Task](../contributing.md#adding-a-new-task) for the full checklist.

## Next: [Pipelines](pipelines.md)
