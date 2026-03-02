# Runtime & Typing

This page describes the AgentLang execution pipeline — from source text to final output — and the guarantees each phase provides.

## Execution phases

```
Source text (.agent file)
        │
        ▼
    Lexer (agentlang/lexer.py)
        │  Tokenizes source, decodes escape sequences
        ▼
    Parser (agentlang/parser.py)
        │  Builds AST, enforces structural uniqueness
        ▼
    Type Checker (agentlang/checker.py)
        │  Verifies types, bindings, and return paths
        ▼
    Runtime (agentlang/runtime.py)
        │  Executes pipeline, calls task handlers
        ▼
    Output (JSON to stdout)
```

## Lexer

The lexer converts source text into a flat token stream.

- Recognizes keywords: `agent`, `task`, `pipeline`, `let`, `run`, `with`, `by`, `retries`, `on_fail`, `abort`, `use`, `parallel`, `join`, `if`, `else`, `return`, `true`, `false`
- Decodes string escape sequences: `\n`, `\t`, `\r`, `\\`, `\"`, `\'`, `\0`, `\uXXXX`, `\UXXXXXXXX`
- Rejects unknown escapes and invalid Unicode code points (surrogates, values above `U+10FFFF`)

## Parser

The parser builds the AST and enforces structural uniqueness constraints.

Guarantees:

- Unique agent, task, and pipeline names
- Unique parameter names within each task/pipeline
- Unique field names in object type declarations and object literals
- Unique tool names within an agent's tool list
- Unique argument keys in `run` call argument maps
- Well-formed `parallel`, `if`, and `return` blocks

## Type checker

The type checker walks the AST and verifies semantic correctness before any task executes.

Verifies:

- Every variable reference is bound
- `run` target task exists in the task table
- `by` agent exists in the agent table (when specified)
- Argument names exactly match task parameter names
- Argument expression types match declared task input types
- `if` condition has type `Bool`
- `on_fail use` fallback type matches the task's return type
- `return` expression type equals the pipeline's declared return type

Path sensitivity:

- `if/else`: the checker tracks variable bindings introduced in both branches
- `if` without `else`: the checker does not assume a guaranteed return from the `if` block alone — a `return` must exist outside it, or in an `else`

## Runtime

The runtime walks pipeline statements in order, maintaining an environment map of `name → value`.

### Run statement

1. Evaluates argument expressions against the current environment.
2. Looks up the task handler from the registry.
3. Executes the handler (up to `retries + 1` times on failure).
4. On exhausted retries:
   - `on_fail abort`: raises `RuntimeError`, pipeline stops.
   - `on_fail use <expr>`: evaluates the fallback expression and binds the result.
5. Binds the result to the declared variable name.

### Parallel block

1. Takes a **read-only snapshot** of the current environment.
2. Submits each branch as a `ThreadPoolExecutor` future.
3. Waits for all futures to complete.
4. Merges all branch results into the main environment.
5. Continues sequentially after `join`.

### Conditional

1. Evaluates the condition expression.
2. Executes exactly one branch (then or else).

### Return

1. Evaluates the return expression.
2. Exits the pipeline immediately, returning the value.

## Determinism

| Component | Deterministic? |
|---|---|
| Lexer + parser | Always |
| Type checker | Always |
| Expression evaluation | Always |
| Mock task handlers | Always |
| `parallel` branch *ordering* | Not guaranteed |
| Live task outputs | Depends on LLM/network |

The final output is deterministic only if all task handlers are deterministic. Mock mode is always deterministic; live mode depends on the LLM.

## Failure sources

Failures during execution surface as:

```text
Execution error: <message>
```

Common sources:

| Source | Example message |
|---|---|
| Missing task handler | `Unknown task 'my_task'` |
| Adapter/network error | `LLM call failed: <http error>` |
| Invalid field access | `KeyError: 'missing_field'` |
| Unhandled task exception | `Task 'flaky_fetch' failed after 3 attempts.` |
| Invalid pipeline inputs | `Pipeline 'p' missing inputs: ['topic']` |
