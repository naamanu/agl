# AGL 0.3 language specification

Status: normative

Language version: `0.3`

Normative implementation: the Rust crate and CLI in `src/`

AGL 0.3 is a backward-compatible extension of [AGL 0.2](agl-0.2.md). All unchanged lexical, declaration, typing, control-flow, and execution rules are inherited from 0.2. This document specifies the 0.3 additions and semantic corrections.

Files without a `language` declaration use the current language version. Programs that require 0.3 features should declare `language "0.3";`. An explicit `language "0.2";` file cannot use the declarations, types, constructors, or matching syntax defined here.

## 1. Named records

A record introduces a nominal static type:

```agl
record Person {
  name: String,
  active: Bool,
};
```

Records must be declared before use. A record name shares the type namespace with aliases, enums, unions, and built-in type constructors. Duplicate names and duplicate fields are rejected. Recursive and mutually recursive records are not supported in 0.3.

A record is constructed explicitly and must provide every declared field exactly once with no undeclared fields:

```agl
Person {name: "Nana", active: true}
```

Two differently named records are not assignable even when their fields are identical. A structural object literal is not implicitly promoted to a record. Fields are accessed with the same `value.field` syntax as structural objects.

Records encode as ordinary JSON objects containing their declared fields. Their nominal identity is a compile-time property and is not emitted as a JSON tag. Host-supplied values are validated against all record fields at runtime; additional host fields are tolerated but are not statically accessible through the record type.

Aliases remain transparent names. `type User = Person;` denotes `Person`; it does not create a second nominal type.

## 2. Tagged unions

A union declares a non-empty closed set of variants. Variants may be units or carry named fields:

```agl
union FetchError {
  Network { message: String },
  InvalidResponse { raw: String },
  Cancelled,
};
```

Variant and field names are unique within their declaration. Union types are nominal. Recursive unions are not supported in 0.3.

Constructors are qualified:

```agl
FetchError::Network {message: "offline"}
FetchError::Cancelled
```

A constructor must provide exactly the fields declared by its variant.

### JSON encoding

Union values use a stable adjacent-field tagged object:

```json
{
  "$type": "FetchError",
  "$variant": "Network",
  "message": "offline"
}
```

Unit variants contain only `$type` and `$variant`. Variant payload field names cannot collide with the reserved tags because `$` is not valid in an AGL identifier. Adapter JSON schemas use the same encoding.

## 3. Result

`Result[T, E]` is a built-in closed outcome type with `Ok` and `Err` constructors:

```agl
Ok("completed")
Err(FetchError::Network {message: "offline"})
```

Constructor inference leaves the unconstructed side context-polymorphic. An `Ok(String)` value is therefore assignable to `Result[String, E]` for the expected `E`, and an `Err(E)` value is assignable to `Result[T, E]` for the expected `T`.

The JSON encoding is:

```json
{"$type":"Result","$variant":"Ok","value":"completed"}
{"$type":"Result","$variant":"Err","error":{"$type":"FetchError","$variant":"Cancelled"}}
```

A task handler may return an encoded `Err` as an ordinary successful host call. This is a typed domain outcome and does not activate `try/catch`. Host invocation failures, panics, invalid returned values, and orchestration faults remain execution errors.

## 4. Exhaustive matching

`match` branches on a union or `Result` value:

```agl
match result {
  Result::Ok { value } => {
    return value;
  },
  Result::Err { error } => {
    return describe(error);
  }
}
```

Patterns are qualified and list payload fields by name. Each listed name introduces a same-named immutable binding scoped to that arm. Unit variants omit the binding block.

The checker requires:

- The scrutinee is a union or `Result`.
- Every arm uses the scrutinee type.
- Every declared variant appears exactly once.
- Bindings exactly match the selected variant fields.
- Every arm body is well typed.

Missing variants are non-exhaustive. Duplicate variants are unreachable and rejected. Pattern bindings shadow outer bindings only within the arm and cannot escape the match.

AGL 0.3 deliberately has no wildcard pattern and no propagation operator. Explicit exhaustive matches preserve local visibility of every domain failure. A future propagation form requires evidence that it improves real programs without hiding error policy.

## 5. Return paths and diagnostics

Pipelines must return on every statically reachable continuation. A return in only one conditional branch is insufficient. Exhaustive matches and conditionals with terminating branches count as terminating statements; loops are conservatively assumed to execute zero times.

Compiler diagnostics retain the stable phase-code families introduced by 0.2 and render filename, line, column, source line, and caret when a span is available. Static analysis additionally reports:

| Code | Meaning |
|---|---|
| `AGLW2001` | Unused local or pattern binding |
| `AGLW2002` | Unreachable statement |

Warnings do not prevent execution. Unknown references, fields, callables, agents, and variants suggest a close visible name when edit distance provides a useful candidate.

## 6. Typed retry selection

A task returning `Result[T, E]`, where `E` is a declared union, may restrict retries to named error variants:

```agl
let result = fetch(url)
  retries 3
  retry_on [FetchError::Network, FetchError::RateLimited];
```

`retry_on` requires a positive retry budget. Every selector must name a variant of the task's declared error union and may occur only once. An `Err` matching a selector consumes another attempt while budget remains. A non-matching `Err`, an `Ok`, or the last attempt returns the typed result normally. Host execution failures retain the existing retry and `on_fail` behavior.

## 7. Structured execution failures

AGL 0.3 retains unannotated `catch error` with its legacy string binding and adds a structured boundary:

```agl
try {
  let value = risky();
  return value;
} catch failure: Failure {
  return failure.kind + ": " + failure.message;
}
```

`Failure` is a built-in immutable record with fields:

| Field | Type | Meaning |
|---|---|---|
| `kind` | `String` | Stable broad category such as `task`, `timeout`, `assertion`, or `runtime` |
| `message` | `String` | Human-readable detail |
| `operation` | `Option[String]` | Associated task/tool/operation when known |
| `retryable` | `Bool` | Whether the originating failure was eligible for retry |

Typed domain `Err` values do not enter `catch`. Structured catch is reserved for invocation, validation, timeout, assertion, and orchestration faults. Later runtime phases extend the same value with cancellation and budget categories without changing catch syntax.

Match payload bindings, like optional and catch bindings, are restored on normal completion, return, break, continue, and execution error. Returning a payload preserves the already-evaluated value. Arms with non-normal completions do not contribute to the post-match normal environment.

## 8. Compatibility

The shared completion-sensitive checking and binding-restoration corrections apply to every language version. Previously accepted programs with incompatible reachable returns, parallel sibling dependencies, or unsafe loop/catch bindings are rejected. All checked-in AGL 0.2 examples remain accepted when the version header is absent or explicitly `0.2`. New syntax in an explicitly 0.2 file produces a migration diagnostic directing the author to opt into 0.3. The 0.2 JSON representations are unchanged.

The conformance fixtures under `tests/conformance/` and typed-domain-error integration test in `tests/agl03.rs` are executable requirements of this specification.
