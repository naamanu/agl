# Tasks

A **task** is a typed signature that declares the inputs and output of a callable unit of work. Tasks are the nouns of AgentLang — they describe *what* can be run, but not *how* it runs.

## Syntax

```agentlang
task <name>(param1: Type, param2: Type) -> ReturnType {}
```

or

```agentlang
task <name>(param1: Type, param2: Type) -> ReturnType by agent {}
```

## Example

```agentlang
task research(topic: String) -> Obj{notes: String} {}
task compare(note_a: String, note_b: String) -> Obj{decision: String} {}
task investigate(topic: String) -> Obj{summary: String, sources: List[String]} by agent {}
```

## Rules

- The body is always `{}`.
- Standard tasks are supplied by Python handlers at runtime.
- `task ... by agent {}` declares a model-executed task that must be run with an explicit `by <agent>` binding.
- Task names must be unique within a file.
- Parameter names must be unique within a task.
- All parameters and the return type must use supported [types](types.md).

## Why empty bodies?

This separation between signature and behavior is intentional:

- The DSL stays simple and statically checkable.
- Behavior can be swapped between `mock` and `live` adapters without changing the `.agent` source.
- Deterministic tasks and agent-executed tasks can share the same workflow and pipeline language.

## Built-in task handlers

The following tasks are available out of the box:

| Task name | Mock behavior | Live/Anthropic behavior |
|---|---|---|
| `research` | Returns placeholder notes | Calls LLM with optional web search |
| `draft` | Returns placeholder article | Calls LLM to write a draft |
| `compare` | Returns placeholder decision | Calls LLM to compare two notes |
| `respond` | Returns placeholder reply | Calls LLM to generate a reply |
| `llm_complete` | Echoes the prompt | Calls LLM with the given prompt |
| `extract_intent` | Returns fixed `intent`/`urgency` | Deterministic local handler |
| `route` | Returns routing decision | Deterministic local handler |
| `flaky_fetch` | Fails N times, then succeeds | Deterministic local handler |

!!! note
    `extract_intent`, `route`, and `flaky_fetch` are always deterministic regardless of adapter mode.

## Agent tasks

Agent tasks are declared in the DSL and executed by the model bound in the run statement.

```agentlang
task investigate(topic: String) -> Obj{summary: String, sources: List[String]} by agent {}
```

- They may use the bound agent's declared tools in live/anthropic mode.
- They must still return values that match the declared DSL return type.
- In mock mode, AgentLang returns deterministic placeholder values that satisfy the declared type.
- In live/anthropic mode, the final model output is decoded as JSON and validated against the task's declared return type.

## Adding a new task

1. Declare the signature in your `.agent` file.
2. For deterministic tasks, add a Python handler in `agentlang/stdlib.py`.
3. For model-executed tasks, declare `by agent` and rely on the runtime-generated handler path.

See [Contributing → Adding a Task](../contributing.md#adding-a-new-task) for the full checklist.

## Next: [Workflows](workflows.md)
