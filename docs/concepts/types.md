# The Type System

AgentLang has a static structural type system. The type checker runs after parsing, before execution — type errors are caught before any task handler is invoked.

## Primitive types

| Type | Description | Example literals |
|---|---|---|
| `String` | UTF-8 text | `"hello"`, `"gpt-4.1"` |
| `Number` | Integer or float | `1`, `3.14`, `0` |
| `Bool` | Boolean | `true`, `false` |

## Composite types

### `List[T]`

A list where every element has type `T`:

```agentlang
List[String]
List[Number]
List[Obj{name: String}]
```

### `Obj{field: Type, ...}`

A structural object type with named fields:

```agentlang
Obj{notes: String}
Obj{intent: String, urgency: String}
Obj{article: String}
```

Object types are **exact** — a value must have exactly the declared fields, no more and no fewer.

## String escape sequences

String literals support the following escape sequences:

| Escape | Character |
|---|---|
| `\n` | Newline |
| `\t` | Tab |
| `\r` | Carriage return |
| `\\` | Backslash |
| `\"` | Double quote |
| `\'` | Single quote |
| `\0` | Null |
| `\uXXXX` | Unicode code point (4 hex digits) |
| `\UXXXXXXXX` | Unicode code point (8 hex digits) |

## Operators and type rules

| Operator | Operand types | Result type |
|---|---|---|
| `+` | `String + String` | `String` |
| `+` | `Number + Number` | `Number` |
| `==` | same type on both sides | `Bool` |
| `!=` | same type on both sides | `Bool` |

## What the type checker verifies

- Every variable reference is bound before use.
- Task argument names and types exactly match the task signature.
- `if` conditions have type `Bool`.
- `on_fail use` fallback expression type matches the task return type.
- `return` expression type matches the pipeline's declared return type.
- Field access (`x.field`) is only allowed on values with `Obj` type.

## Type errors

Examples of errors the type checker catches:

```
TypeError: argument 'intent' expected String, got Number
TypeError: field 'missing_field' not found on Obj{notes: String}
TypeError: if condition has type String, expected Bool
TypeError: return expression has type Number, pipeline declares String
TypeError: on_fail fallback type Obj{article: String} does not match task return Obj{data: String}
```

## Type aliases

A type alias gives a name to a type expression. Aliases are resolved at parse time — the alias name can be used anywhere a type is expected.

```agentlang
type ResearchNotes = Obj{notes: String, sources: List[String]};
type DraftResult = Obj{article: String, word_count: Number};
```

Use aliases to avoid repeating complex object types across multiple task signatures:

```agentlang
task research(topic: String) -> ResearchNotes by agent {}
task draft(notes: String) -> DraftResult by agent {}
```

Alias names must be unique. Aliases cannot be recursive.

## Enum types

An enum declares a closed set of string variants:

```agentlang
enum ContentTone { formal, conversational, technical };
enum FilingStatus { single, married_joint, married_separate, head_of_household };
```

Enum values are assignable to `String` parameters. At runtime, values are validated to be one of the declared variants.

```agentlang
task review_article(article: String, tone: ContentTone) -> ReviewVerdict by agent {}

-- in a pipeline, pass enum values as string literals:
let v = run review_article with { article: merged.article, tone: "formal" } by reviewer;
```

Enum names must be unique. Variant names must be unique within an enum.

## Runtime input type checking

At runtime, the `--input` JSON is also validated against declared pipeline param types before execution begins:

```bash
python main.py examples/blog.agent blog_post \
  --input '{"topic": 42}'
# Execution error: Pipeline 'blog_post' input 'topic' has invalid value 42 for type String.
```

!!! note "`Bool` vs `Number`"
    `true` and `false` in JSON are Python `bool` values. AgentLang's `Number` type **excludes** booleans — passing `true` where a `Number` is expected is a type error.

## Next: [Parallel Execution](parallel.md)
