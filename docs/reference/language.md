# Language Reference

Complete syntax reference for AgentLang v0.

## File structure

An `.agent` file contains any number of `agent`, `tool`, `task`, and `pipeline` declarations in any order. Names must be unique across each declaration type.

```agentlang
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

-- agent declarations
agent planner { model: "gpt-4.1", tools: [web_search] }
agent writer  { model: "gpt-4.1-mini", tools: [] }

-- tool declarations
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

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

Agent tool names must refer to declared `tool` definitions.

## `tool` declaration

```
tool <name>( <params> ) -> <type> {}
```

| Part | Description |
|---|---|
| `<name>` | Unique identifier |
| `<params>` | Comma-separated `name: Type` pairs (may be empty) |
| `-> <type>` | Return type |
| `{}` | Reserved for runtime-provided tool behavior |

Constraints: tool names unique, parameter names unique within a tool.

## `task` declaration

```
task <name>( <params> ) -> <type> {}
```

or

```
task <name>( <params> ) -> <type> by agent {}
```

| Part | Description |
|---|---|
| `<name>` | Unique identifier |
| `<params>` | Comma-separated `name: Type` pairs (may be empty) |
| `-> <type>` | Return type |
| `by agent` | Marks the task as model-executed instead of handler-executed |
| `{}` | Always empty — behavior is provided by runtime handlers |

Constraints: task names unique, parameter names unique within a task.
Agent tasks must be run with an explicit `by <agent>` binding in pipeline statements.

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

### While loop

```
while <expr> {
  <statements>
}
```

Condition must have type `Bool`. The loop body may contain the same statement forms as a pipeline block.

### Break / Continue

```
break;
continue;
```

`break` exits the nearest enclosing `while`. `continue` skips to the next iteration of the nearest enclosing `while`. Both are only valid inside loop bodies.

### Option unwrap conditional

```
if let <x> = <expr> {
  <statements>
} [ else {
  <statements>
} ]
```

`<expr>` must have type `Option[T]`. In the `then` branch, `<x>` is bound as `T`. If the option value is `null`, the `else` branch runs instead. `else` is optional.

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
| `null` | Null literal (assignable to `Option[T]`) |
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
| Option | `Option[T]` |
| Object | `Obj{field: Type, field: Type}` |

Duplicate field names in `Obj` types are a parse error.

## Comments

```agentlang
-- this is a single-line comment
```

Only `--` line comments are supported. Block comments are not supported in v0.
