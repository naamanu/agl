# AGL 0.2 language specification

Status: normative

Language version: `0.2`

Normative implementation: the Rust crate and CLI in `src/`

This specification defines the source-language contract for AGL 0.2. When examples, conceptual documentation, the historical paper, or the retained Python implementation disagree with this document, this document and its Rust conformance suite take precedence. The Python implementation is a compatibility oracle for the subset exercised by differential tests; it is not the normative definition of new behavior.

## 1. Source files and versioning

An AGL source file is UTF-8 text. The optional first declaration is:

```agl
language "0.2";
```

When absent, the compiler interprets the file as AGL 0.2. Any explicit unsupported version is rejected with `AGL1003`. A `language` declaration anywhere except the beginning is a syntax error.

Declarations following the version header may appear in any order except that type aliases and enums must be declared before a type expression refers to them.

## 2. Lexical structure

Identifiers begin with an ASCII letter or underscore and continue with ASCII letters, digits, or underscores. Keywords are reserved and cannot be used as identifiers.

Number literals are unsigned decimal integers or decimals. Negative-number syntax is not part of 0.2. String literals use double quotes and support `\n`, `\t`, `\r`, `\\`, `\"`, `\'`, `\0`, `\uXXXX`, and `\UXXXXXXXX` escapes. Strings cannot contain an unescaped newline.

`--` begins a line comment. A comment ends immediately before the next newline or at end of file. AGL 0.2 has no block comments.

The punctuation and operators are:

```text
{ } ( ) [ ] : , ; = + . -> == !=
```

Lexical failures use diagnostic code `AGL0001`.

## 3. Program declarations

A program contains disjoint tables of aliases, enums, agents, tools, tasks, and pipelines, plus an ordered list of tests. A workflow is lowered to a pipeline during parsing and therefore does not remain a separate runtime declaration.

### 3.1 Type aliases

```agl
type Name = String;
type Notes = Obj{text: String, sources: List[String]};
```

Aliases are transparent and resolved recursively by the checker and runtime. Recursive aliases are not supported. An alias must be declared before use.

### 3.2 Enums

```agl
enum Status { pending, approved, rejected };
```

An enum has at least one unique variant. Enum variants are represented as JSON strings. A string literal equal to a declared variant is inferred as that enum type. Variant names must be globally unambiguous in 0.2. An enum value widens to `String`; arbitrary strings do not narrow to enums.

### 3.3 Agents

```agl
agent researcher {
  model: "gpt-5.6-sol",
  tools: [web_search]
}
```

`tools` is required and may be empty. `model` is optional. Every listed tool must be declared. The deployment adapter interprets model names; model strings have no static semantics in 0.2.

### 3.4 Tools and tasks

```agl
tool web_search(query: String) -> List[Obj{title: String, url: String}] {}
task summarize(text: String) -> Obj{summary: String} {}
task investigate(topic: String) -> Obj{answer: String} by agent {}
```

Tool and task bodies are empty because behavior is supplied by the host runtime. Parameters are ordered, uniquely named, and statically typed. `by agent` marks a task as model-executed and requires every call to select an agent explicitly.

### 3.5 Pipelines

```agl
pipeline name(parameter: Type, ...) -> Type {
  statement*
}
```

A pipeline has a unique name, unique parameter names, a declared result type, and at least one statically compatible return. Pipelines may call other pipelines, subject to the recursion limit at runtime.

### 3.6 Workflows

A workflow is a higher-level authoring form containing `stage`, `review`, and `return` steps. It is deterministically lowered to an ordinary pipeline before static checking. The `--lower` CLI option exposes this representation. Review lowering may synthesize a compatible `countdown` task.

### 3.7 Tests

```agl
test "description" {
  statement*
}
```

Tests have isolated empty scopes and run only when requested by the host or CLI. `assert` failures are execution failures.

## 4. Types

The 0.2 type grammar is:

```text
Type ::= String | Number | Bool
       | List[Type]
       | Option[Type]
       | Obj{ Field* }
       | EnumName
       | AliasName
Field ::= Identifier : Type
```

Runtime values use JSON representations. `Number` includes JSON integers and finite decimals but excludes booleans. `Option[T]` accepts `null` or a value of `T`.

### 4.1 Structural assignability

AGL 0.2 object types are open structural requirements, not exact shapes. A value with additional fields is assignable to an object type when every required field exists and is assignable. Extra fields remain present at runtime.

Assignability is reflexive and additionally permits:

- `Enum[E]` to `String`.
- `List[A]` to `List[B]` when `A` is assignable to `B`.
- `Option[A]` to `Option[B]` when `A` is assignable to `B`.
- `null` to every `Option[T]`.
- Object width and depth subtyping.

Values are immutable within AGL 0.2. This makes covariance safe at the language boundary; handlers receive owned JSON values and cannot mutate the pipeline environment through an AGL reference.

## 5. Expressions

Expressions include JSON-like literals, references, chained field access, parentheses, object and list literals, addition, and equality/inequality.

`+` accepts two strings or two numbers. `==` and `!=` accept mutually assignable types, enum/string pairs, and option/null pairs. List literals must be non-empty so their element type can be inferred, and every element must be assignable to the inferred element type.

Field access is valid only when every path segment exists in the statically resolved object type.

## 6. Statements

### 6.1 Calls

The canonical form is:

```agl
let result = run task with {argument: expression}
  by agent
  retries 2
  timeout 30
  on_fail use fallback;
```

Clauses are optional and may appear in any order, but each may appear at most once. Argument names must exactly match the called task or pipeline parameters. The shorthand `let result = task(expression, ...);` maps positional expressions to declared parameter order.

Pipeline calls cannot use agents, retry, timeout, or fallback clauses. A run makes its target available to subsequent statements.

`retries N` means at most `N + 1` total attempts. `on_fail abort` is the default. A fallback must be assignable to the call result type. Timeout values are positive seconds. In 0.2 a timed-out handler thread can remain alive; this limitation is intentionally removed by the structured-concurrency roadmap.

### 6.2 Conditionals and loops

`if` and `while` conditions have type `Bool`. `if let name = option` unwraps `Option[T]` and binds `name: T` only in the non-null branch. `break` and `continue` are valid only within a loop.

After conditional control flow, only variables available with mutually assignable types on every possible continuation remain in scope. A loop body may execute zero times, so it cannot introduce a definitely available post-loop binding.

### 6.3 Parallel blocks

```agl
parallel max_concurrency 4 {
  let a = run first with {...};
  let b = run second with {...};
} join;
```

Every branch is a direct run statement evaluated against the same pre-block environment snapshot. Targets must be fresh and pairwise distinct. Results become available after all branches join. Result bindings are deterministic even though completion and trace-event order are not. `max_concurrency` must be at least one.

### 6.4 Try, assertions, and return

`try/catch` catches execution failures and binds their rendered message as `String`. It does not intercept `return`, `break`, or `continue`. `assert condition, "message";` requires a boolean and raises an execution failure when false.

Every returned expression must be assignable to the pipeline result type. AGL 0.2 return-path analysis is conservative: a pipeline must contain at least one compatible return, while runtime still rejects a path that reaches the end without returning.

## 7. Static checking

The checker validates declaration references, tool membership, call arguments, agent requirements, expression types, fallback types, control-flow conditions, branch environments, parallel freshness, loop control placement, and compatible returns. Static failures use `AGL2001` in 0.2. Later language versions may introduce more specific codes without reusing the meaning of an existing code.

## 8. Execution

Pipeline inputs are validated before execution. Expressions are deterministic. Task handlers and model/tool adapters may be nondeterministic. Whole-pipeline execution is deterministic only when handlers are deterministic and scheduling order is unobserved.

Parallel branches receive cloned environments. Retry invokes the same registered handler again. Pipeline calls recurse through the same program and registry and are limited to 128 nested calls. Returned task and pipeline values are validated against declared types.

Execution failures use `AGL3001`. Structured domain failures are not part of 0.2; they are planned for AGL 0.3.

## 9. Diagnostics

Diagnostic code families are stable public API:

| Code | Meaning |
|---|---|
| `AGL0001` | Lexical failure |
| `AGL1001` | Syntactic failure |
| `AGL1002` | Parse-time semantic/lowering failure |
| `AGL1003` | Unsupported language version |
| `AGL2001` | Static checking failure |
| `AGL3001` | Runtime orchestration failure |

Human-readable wording may improve within a language version. Tools must use codes rather than matching message text.

## 10. Conformance

The fixtures under `tests/conformance/` are executable examples of this specification. Their manifests declare whether a source must parse/check, lower, execute to a JSON value, or fail with a diagnostic code. Run them with:

```bash
cargo test --locked --test conformance
```

The fixtures supplement this prose; they do not silently extend it. Any discovered disagreement requires a specification correction, an implementation correction, or an explicit language-version change.
