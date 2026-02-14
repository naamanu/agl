# Runtime And Typing

This document maps the implementation to expected behavior.

## High-Level Flow

Execution path:

1. Parse source text (`agentlang/parser.py`)
2. Type-check program (`agentlang/checker.py`)
3. Build task registry (`agentlang/stdlib.py`)
4. Execute selected pipeline (`agentlang/runtime.py`)

## Parser Behavior

Parser guarantees:

- uniqueness of agent/task/pipeline names
- well-formed blocks and clauses
- support for retry/failure policy clauses in run statements

Notable parser constraints:

- task bodies are `{}` only
- parallel blocks accept run statements only

## Type Checker Behavior

Checker verifies:

- run target task exists
- `by` agent exists (if specified)
- argument names exactly match task params
- argument expression types match declared task input types
- `if` condition is `Bool`
- `on_fail use` fallback type equals task return type
- returned expression type equals pipeline return type

Path-sensitivity:

- `if` with `else`: checker tracks common bindings from both branches
- `if` without `else`: checker does not assume branch return guarantees whole-pipeline return

## Runtime Behavior

The runtime walks statements in order and maintains an environment map.

### Run Statement

- evaluates argument expressions against current env
- invokes matching task handler
- applies retry loop: `retries + 1` max attempts
- applies fail policy:
  - `abort`: raise runtime error
  - `use`: evaluate fallback expression and bind result

### Parallel Block

- takes a snapshot of current environment
- executes each run branch concurrently via `ThreadPoolExecutor`
- merges branch outputs into main env after all futures complete

### If Statement

- evaluates condition expression
- condition must evaluate to Python `bool`
- executes exactly one branch

### Return Statement

- evaluates expression and exits pipeline immediately

## Determinism Notes

Deterministic:

- parser and checker behavior
- expression evaluation
- branch selection with deterministic data

Potentially non-deterministic:

- order/timing in parallel branch execution
- external adapter outputs (`live` mode)
- network/tool responses

## Failure Semantics

Failures can originate from:

- missing task handler
- adapter API/network errors
- invalid field access at runtime
- unhandled task exceptions

CLI surfaces these as:

```text
Execution error: <message>
```

