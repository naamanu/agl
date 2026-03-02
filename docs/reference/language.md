# Language Reference

Complete syntax reference for AgentLang v0.

## File structure

An `.agent` file contains any number of `agent`, `task`, and `pipeline` declarations in any order. Names must be unique across each declaration type.

```agentlang
-- agent declarations
agent planner { model: "gpt-4.1", tools: [web_search] }
agent writer  { model: "gpt-4.1-mini", tools: [] }

-- task signatures
task research(topic: String) -> Obj{notes: String} {}
task draft(notes: String) -> Obj{article: String} {}

-- pipeline definitions
pipeline blog_post(topic: String) -> String {
  let r = run research with { topic: topic } by planner;
  let d = run draft with { notes: r.notes } by writer;
  return d.article;
}
```

## `agent` declaration

```
agent <name> {
  model: "<string>"
  , tools: [ <id>, ... ]
}
```

| Field | Required | Description |
|---|---|---|
| `model` | Yes | Model name string literal |
| `tools` | Yes | List of tool identifiers (may be empty `[]`) |

Constraints: agent names unique, tool names unique within the list.

## `task` declaration

```
task <name>( <params> ) -> <type> {}
```

| Part | Description |
|---|---|
| `<name>` | Unique identifier |
| `<params>` | Comma-separated `name: Type` pairs (may be empty) |
| `-> <type>` | Return type |
| `{}` | Always empty — behavior is provided by runtime handlers |

Constraints: task names unique, parameter names unique within a task.

## `pipeline` declaration

```
pipeline <name>( <params> ) -> <type> {
  <statements>
}
```

Constraints: pipeline names unique, at least one reachable `return`.

## Statements

### Run statement

```
let <x> = run <task>
  with { <key>: <expr>, ... }
  [ by <agent> ]
  [ retries <N> ]
  [ on_fail abort | on_fail use <expr> ]
  ;
```

| Clause | Default | Description |
|---|---|---|
| `by <agent>` | none | Agent binding for model + tool resolution |
| `retries N` | `0` | Retry budget (N+1 total attempts) |
| `on_fail abort` | default | Raise error on exhaustion |
| `on_fail use <expr>` | — | Use fallback value on exhaustion |

Constraints: argument keys must exactly match task parameter names. Duplicate argument keys are a parse error.

### Parallel block

```
parallel {
  let <a> = run <task> with { ... } [ by <agent> ] [ retries N ] [ on_fail ... ];
  let <b> = run <task> with { ... } [ by <agent> ] [ retries N ] [ on_fail ... ];
} join;
```

Only `let ... = run ...;` statements are permitted inside. All bindings from inside the block are available after `join`.

### Conditional

```
if <expr> {
  <statements>
} [ else {
  <statements>
} ]
```

Condition must have type `Bool`. `else` is optional.

### Return

```
return <expr> ;
```

Expression type must match the pipeline's declared return type.

## Expressions

| Form | Description |
|---|---|
| `"string"` | String literal |
| `123`, `3.14` | Number literal |
| `true`, `false` | Bool literal |
| `x` | Variable reference |
| `x.field` | Object field access |
| `{ key: expr, ... }` | Object literal |
| `[expr, expr]` | List literal |
| `expr + expr` | String or number addition |
| `expr == expr` | Equality (same type) |
| `expr != expr` | Inequality (same type) |

Duplicate keys in object literals are a parse error.

## Types

| Type | Syntax |
|---|---|
| String | `String` |
| Number | `Number` |
| Bool | `Bool` |
| List | `List[T]` |
| Object | `Obj{field: Type, field: Type}` |

Duplicate field names in `Obj` types are a parse error.

## Comments

```agentlang
-- this is a single-line comment
```

Only `--` line comments are supported. Block comments are not supported in v0.
