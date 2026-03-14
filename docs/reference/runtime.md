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
    Lowering (agentlang/lowering.py)
        │  Compiles workflows into explicit pipelines
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

- Recognizes keywords: `agent`, `task`, `pipeline`, `let`, `run`, `with`, `by`, `retries`, `on_fail`, `abort`, `use`, `parallel`, `join`, `if`, `else`, `return`, `true`, `false`, `type`, `enum`, `try`, `catch`, `assert`, `test`, `max_concurrency`
- Recognizes workflow keywords such as `workflow`, `stage`, `review`, `checks`, `revise`, `using`, and `max_rounds`
- Decodes string escape sequences: `\n`, `\t`, `\r`, `\\`, `\"`, `\'`, `\0`, `\uXXXX`, `\UXXXXXXXX`
- Rejects unknown escapes and invalid Unicode code points (surrogates, values above `U+10FFFF`)

## Parser

The parser builds the AST and enforces structural uniqueness constraints.

Guarantees:

- Unique agent, task, and pipeline names
- Unique workflow names
- Unique type alias and enum names
- Unique enum variant names within each enum
- Unique parameter names within each task/pipeline
- Unique field names in object type declarations and object literals
- Unique tool names within an agent's tool list
- Unique argument keys in `run` call argument maps
- Well-formed `parallel`, `if`, `return`, `try`/`catch`, `assert`, and `test` blocks

## Lowering

When a file contains `workflow` declarations, the lowering pass compiles them into ordinary `PipelineDef` nodes before type-checking and execution.

Current workflow lowering handles:

- `stage` steps
- declarative `review ... revise ... max_rounds ...` loops
- hidden countdown-based loop budgets
- workflow returns

`python main.py <file> <workflow_name> --lower` prints the lowered pipeline IR.

## Type checker

The type checker walks the AST and verifies semantic correctness before any task executes.

Verifies:

- Every variable reference is bound
- `run` target task or pipeline exists in the task/pipeline table
- `by` agent exists in the agent table (when specified)
- Argument names exactly match task parameter names
- Argument expression types match declared task input types
- `if` condition has type `Bool`
- `while` condition has type `Bool`
- `break` / `continue` only occur inside loops
- `if let` only unwraps `Option[T]`
- `on_fail use` fallback type matches the task's return type
- `return` expression type equals the pipeline's declared return type
- `try`/`catch` block well-formedness; error variable typed as `String`
- `assert` expression has type `Bool`
- Enum values are valid variants of the declared enum
- Type aliases are resolved to their underlying types
- Pipeline-as-run-target parameter and return types match
- workflow-lowered pipelines remain well-typed after compilation

Path sensitivity:

- `if/else`: the checker tracks variable bindings introduced in both branches
- `if` without `else`: the checker does not assume a guaranteed return from the `if` block alone — a `return` must exist outside it, or in an `else`
- `try/catch`: variables bound before `try` may be re-bound in both blocks; the merged environment is available after

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

### While / break / continue

1. Evaluates the loop condition.
2. Executes the body while the condition remains `true`.
3. `break` exits the nearest loop.
4. `continue` skips to the next iteration.

### Agent tasks

1. Selects the model from the bound agent.
2. Exposes the agent's declared tools, if any.
3. Executes tool calls through the runtime tool registry.
4. Decodes final model output as JSON.
5. Validates the resulting runtime value against the declared DSL type.

### Return

1. Evaluates the return expression.
2. Exits the pipeline immediately, returning the value.

### Try / catch

1. Executes the `try` block statements in order.
2. If any statement raises a runtime error, execution jumps to the `catch` block.
3. The error variable is bound as a `String` containing the error message.
4. After either block completes, execution continues with the merged environment.

### Assert

1. Evaluates the expression (must be `Bool`).
2. If `false`, halts execution with an assertion error containing the message string.
3. If `true`, continues to the next statement.

### Pipeline call

1. Evaluates argument expressions against the current environment.
2. Creates a new scope for the target pipeline with the arguments bound as parameters.
3. Executes the target pipeline.
4. Binds the result to the declared variable name.

### ExecutionContext

When `--output-trace PATH` is passed, an `ExecutionContext` records structured events throughout execution:

- `task_start` / `task_end` / `task_error` — per-task lifecycle
- `parallel_start` / `parallel_end` — parallel block boundaries
- `retry` — retry attempts with error details
- `pipeline_call` — pipeline-calls-pipeline events

After execution, the trace is written as JSON to the specified path.

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
| Unhandled task exception | `Task 'flaky_fetch' failed after 3 attempts. Last error: RuntimeError: ...` |
| Invalid pipeline inputs | `Pipeline 'p' missing inputs: ['topic']` |
| Invalid live agent output | `Agent task 'investigate' returned non-JSON output: '...'` |
