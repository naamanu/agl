# Language Reference

This document describes the practical syntax and rules for AgentLang v0.

## Core Concepts

- `agent`: execution profile (model + tools)
- `task`: typed signature for callable work
- `pipeline`: executable workflow block

## Syntax Overview

```agentlang
agent planner {
  model: "gpt-4.1"
  , tools: [web_search]
}

task research(topic: String) -> Obj{notes: String} {}

pipeline run(topic: String) -> String {
  let r = run research with { topic: topic } by planner;
  return r.notes;
}
```

## `agent` Declaration

```agentlang
agent <name> {
  model: "<model-name>"
  , tools: [tool_a, tool_b]
}
```

Rules:

- `model` is required and must be a string literal.
- `tools` is required and must be a list of identifiers.
- agent names must be unique in a file.

## `task` Declaration

```agentlang
task <name>(arg1: Type, arg2: Type) -> ReturnType {}
```

Rules:

- task body is currently signature-only (`{}`).
- task names must be unique in a file.
- runtime behavior is supplied by Python task handlers.

## `pipeline` Declaration

```agentlang
pipeline <name>(param: Type) -> ReturnType {
  <statements>
}
```

Rules:

- must include at least one reachable `return`.
- pipeline names must be unique in a file.
- input params are bound from CLI `--input` JSON.

## Statements

### 1) Run Statement

```agentlang
let x = run task_name with { a: expr1, b: expr2 } by some_agent retries 2 on_fail abort;
```

Supported clauses:

- `by <agent>`: optional agent binding
- `retries N`: optional, default `0`
- `on_fail abort`: optional, default policy
- `on_fail use <expr>`: optional fallback value

Fallback expression type must match task return type.

### 2) Parallel Block

```agentlang
parallel {
  let a = run research with { topic: q + " A" } by planner;
  let b = run research with { topic: q + " B" } by planner;
} join;
```

Rules:

- only `let ... = run ...;` statements are allowed inside the block.
- all branch outputs become available after `join`.

### 3) Conditional

```agentlang
if expr {
  ...
} else {
  ...
}
```

Rules:

- condition must type-check to `Bool`.
- `else` is optional, but missing `else` does not imply guaranteed return.

### 4) Return

```agentlang
return expr;
```

Rules:

- expression type must equal declared pipeline return type.

## Expressions

Supported forms:

- literals:
  - string: `"text"`
  - number: `1`, `3.14`
  - bool: `true`, `false`
- variable reference: `x`
- object field access: `x.field`
- object literal: `{ key: expr, other: expr }`
- list literal: `[expr1, expr2]`
- operators:
  - `+` for `String + String` or `Number + Number`
  - `==`, `!=` for same-type equality checks

## Types

Supported types:

- `String`
- `Number`
- `Bool`
- `List[T]`
- `Obj{field: Type, ...}`

Type checker enforces:

- task argument compatibility
- known variable/field references
- fallback compatibility for `on_fail use`
- pipeline return type correctness

## Errors

Typical parse/type errors:

- unknown task or agent
- missing/extra task arguments
- field access on non-object
- non-bool `if` condition
- fallback expression type mismatch
- pipeline missing `return`

