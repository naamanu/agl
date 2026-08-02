# Language Reference

Complete syntax reference for AGL 0.2 through 0.6. The Rust parser and checker are the executable reference; the versioned normative contracts live in `spec/agl-0.2.md` through `spec/agl-0.6.md`.

## File structure

An `.agent` file may begin with `language "0.2";` through `language "0.6";`, then contains declarations. When omitted, the current language version is assumed. Unsupported versions are rejected rather than silently reinterpreted. AGL 0.3 adds named outcomes, 0.4 effects/deployments/evaluation, 0.5 durable structured concurrency, and 0.6 modules/packages/tooling.

```agentlang
language "0.2";

-- type aliases and enums
type Notes = Obj{notes: String};
enum Tone { formal, casual };

-- tool declarations
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

-- agent declarations (model is optional)
agent planner { model: "gpt-4.1", tools: [web_search] }
agent writer  { tools: [] }

-- task signatures
task research(topic: String) -> Notes {}
task draft(notes: String) -> Obj{article: String} {}

-- pipeline definitions
pipeline blog_post(topic: String) -> String {
  let r = run research with { topic: topic } by planner;
  let d = run draft with { notes: r.notes } by writer;
  return d.article;
}

-- workflow definitions
workflow publish_topic_blog(topic: String) -> String {
  stage plan = planner does research(topic);
  stage draft = writer does draft(plan.notes);
  return draft.article;
}

-- test blocks
test "research returns notes" {
  let r = run research with { topic: "test" };
  assert r.notes != "", "notes should not be empty";
}
```

For new code, start with `language "0.6";`. The versioned normative specifications live in `spec/agl-0.2.md` through `spec/agl-0.6.md`.

## AGL 0.3 records, unions, and matching

```agentlang
language "0.3";

record Article { title: String, body: String };
union PublishError {
  Rejected { reason: String },
  Unavailable,
};

task publish(article: Article) -> Result[String, PublishError] {}

pipeline publish_title(article: Article) -> String {
  let outcome = publish(article);
  match outcome {
    Result::Ok { value } => { return value; }
    Result::Err { error } => {
      match error {
        PublishError::Rejected { reason } => { return reason; }
        PublishError::Unavailable => { return "unavailable"; }
      }
    }
  }
}
```

Record constructors provide every declared field. Union constructors use `Type::Variant` and carry declared fields. `match` must cover every variant exactly once. Pattern fields bind same-named variables within the selected arm.

## `type` alias declaration

```
type <Name> = <type>;
```

Defines a named alias for a type expression. Aliases are resolved at parse time — everywhere `<Name>` appears in a type position, it is replaced by the underlying type.

```agentlang
type Notes = Obj{notes: String, sources: List[String]};
type Profile = Obj{name: String, score: Number};
```

Constraints: alias names must be unique. Aliases cannot be recursive.

## `enum` declaration

```
enum <Name> { variant1, variant2, ... };
```

Defines a closed set of string variants. Enum values are assignable to `String` parameters. At runtime, the value is validated to be one of the declared variants.

```agentlang
enum Tone { formal, conversational, technical };
enum Status { pending, approved, rejected };
```

Constraints: enum names must be unique in the type namespace. Variant names are globally unique in 0.2/0.3 so string-literal inference is unambiguous.

## `agent` declaration

```
agent <name> {
  [ model: "<string>" ]
  , tools: [ <id>, ... ]
}
```

| Field | Required | Description |
|---|---|---|
| `model` | No | Model name string literal (defaults to `None`) |
| `tools` | Yes | List of tool identifiers (may be empty `[]`) |

When `model` is omitted, the native runtime uses its provider default. Set `AGL_OPENAI_MODEL` or `AGL_ANTHROPIC_MODEL` to override model routing globally for a deployment. Legacy OpenAI model declarations are mapped by capability tier when using a native provider adapter.

```agentlang
-- with explicit model
agent planner { model: "gpt-4.1", tools: [web_search] }

-- without model (uses default)
agent writer { tools: [] }
```

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

## `workflow` declaration

```
workflow <name>( <params> ) -> <type> {
  <workflow-steps>
}
```

`workflow` is the high-level authoring surface. It compiles to an ordinary `pipeline` before type-checking and execution. Use `cargo run -- <file> <name> --lower` to inspect the lowered pipeline IR.

Constraints: workflow names unique, at least one `return`, and workflow names may not collide with pipeline names.

## Workflow steps

### Stage step

```
stage <artifact> = <agent> does <task>( <expr>, ... );
```

Arguments are positional and must match the task's declared parameter order.

### Review step

```
review <artifact> = <reviewer> checks <source>
  revise with <reviser> using <task>
  max_rounds <N>;
```

This is a declarative review loop:

- The workflow compiler infers the review task name as `review_<artifact>`.
- The inferred review task is expected to return an object with at least `approved: Bool` and `feedback: String`.
- The revise task must return the same object type as `<source>`.
- The source artifact is consumed by the review step and replaced by the final reviewed artifact name.
- `max_rounds` sets the revision budget; looping is handled internally in the lowered pipeline.

### Workflow return

```
return <expr> ;
```

Workflow return expressions use the same expression grammar as pipelines.

## Statements

### Run statement

```
let <x> = run <task>
  with { <key>: <expr>, ... }
  [ by <agent> ]
  [ retries <N> ]
  [ retry_on [<ErrorType>::<Variant>, ...] ]
  [ timeout <N> ]
  [ on_fail abort | on_fail use <expr> ]
  ;
```

| Clause | Default | Description |
|---|---|---|
| `by <agent>` | none | Agent binding for model + tool resolution |
| `retries N` | `0` | Retry budget (N+1 total attempts) |
| `retry_on [...]` | all host failures | Retry only the listed typed `Err` variants; requires `Result[T, E]` and `retries N` |
| `timeout N` | none | Deadline in seconds; handler is abandoned if exceeded |
| `on_fail abort` | default | Raise error on exhaustion |
| `on_fail use <expr>` | — | Use fallback value on exhaustion |

Constraints: argument keys must exactly match task parameter names. Duplicate argument keys are a parse error.

### Shorthand run

```
let <x> = <task>( <expr>, ... ) [ by <agent> ];
```

Syntactic sugar for `let <x> = run <task> with { ... }`. Arguments are positional and matched to declared parameter names in order.

```agentlang
let article = draft(notes.notes) by writer;
-- equivalent to:
let article = run draft with { notes: notes.notes } by writer;
```

### Parallel block

```
parallel [ max_concurrency <N> ] {
  let <a> = run <task> with { ... } [ by <agent> ] [ retries N ] [ on_fail ... ];
  let <b> = run <task> with { ... } [ by <agent> ] [ retries N ] [ on_fail ... ];
} join;
```

Only `let ... = run ...;` statements are permitted inside. All bindings from inside the block are available after `join`.

The optional `max_concurrency N` limits how many branches run simultaneously within this block. There is no global worker flag; task concurrency groups, rate limits, and pipeline budgets provide broader controls.

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

### Try / catch

```
try {
  <statements>
} catch <error_var> {
  <statements>
}
```

AGL 0.3 can bind a structured execution failure instead of a legacy message string:

```agentlang
try {
  let result = risky_task(input);
  return result;
} catch failure: Failure {
  return failure.kind + ": " + failure.message;
}
```

`Failure` exposes `kind`, `message`, `operation: Option[String]`, and `retryable`. Typed `Result::Err` values are ordinary domain outcomes and are not caught.

If any statement in the `try` block raises a runtime error, execution jumps to the `catch` block. The `<error_var>` is bound as a `String` containing the error message. Variables bound inside `try` that were also bound before `try` are available after the block (the catch block may re-bind them).

```agentlang
try {
  let result = run risky_task with { input: data };
} catch err {
  let fallback = run safe_task with { query: err };
}
```

### Assert

```
assert <expr>, "<message>";
```

Evaluates `<expr>` — if it is `false`, execution halts with an assertion error containing `<message>`. The expression must have type `Bool`.

```agentlang
assert final.title != "", "Title must not be empty";
```

### Test block

```
test "<name>" {
  <statements>
}
```

Test blocks are top-level declarations that run only when the `--test` flag is passed. Each test block has its own scope. Tests may contain `run`, `let`, `assert`, and other pipeline statements. Failed assertions cause test failure.

```agentlang
test "draft produces article" {
  let d = run draft with { notes: "test notes" };
  assert d.article != "", "Article should not be empty";
}
```

### Pipeline call (pipeline-calls-pipeline)

A `run` statement can target another pipeline instead of a task:

```agentlang
let result = run sub_pipeline with { topic: topic, angle: "deep-dive" };
```

The target pipeline's parameters and return type are checked identically to task calls. This enables modular composition of pipelines.

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
| Enum | `EnumName` (declared via `enum`) |
| Type alias | `AliasName` (declared via `type`) |

Enum values are assignable to `String` parameters. Type aliases are resolved at parse time.

Duplicate field names in `Obj` types are a parse error.

## Comments

```agentlang
-- this is a single-line comment
```

Only `--` line comments are supported. Block comments are not supported in v0.

## AGL 0.4 effects, idempotency, and deployment

Tasks and pipelines may declare operational capabilities and retry identity:

```agentlang
language "0.4";
task publish(id: String, body: String) -> String
  effects [network, external_write]
  idempotency keyed_by id {}
```

Built-in effects include `model`, `network`, `filesystem`, `external_read`, `external_write`, `secret`, and `human`. Effects are inferred transitively and can be inspected with `--effects`.

## AGL 0.5 structured concurrency and approvals

AGL 0.5 adds ordered `parallel map`, `race`, scheduler groups, resource budgets, and typed human approvals:

```agentlang
language "0.5";
let approved = approve release "Release to production?" expires 3600 delegate oncall;
let results = parallel map item in inputs max_concurrency 4 collect_all {
  let result = run classify with { value: item };
};
```

Approvals suspend a durable run until a host supplies `--approval release=true` or `--approval release=false`.

## AGL 0.6 modules and packages

Imports are relative to the importing file. Declarations are private by default; imported declarations must be `public` and are referenced through an alias:

```agentlang
language "0.6";
import text from "text.agent";

public pipeline report(input: String) -> String {
  return text::normalize(input);
}
```

Package manifests use `agl.json`; `agl package lock agl.json` creates a deterministic `agl.lock`. Use `--api` to snapshot the public interface and `agl api-compare` to check compatibility before a release.
