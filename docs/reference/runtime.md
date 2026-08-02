# Compiler and runtime

The Rust implementation is normative for AGL 0.6. The versioned language contracts are in [`spec/agl-0.2.md`](https://github.com/naamanu/agl/blob/main/spec/agl-0.2.md) through [`spec/agl-0.6.md`](https://github.com/naamanu/agl/blob/main/spec/agl-0.6.md); the Python implementation is retained as a compatibility oracle and plugin bridge.

## Execution pipeline

```text
AGL source
  │
  ├─ lexer and parser ──> AST with source spans
  ├─ module loader ─────> imported program + cached public interfaces
  ├─ checker ───────────> types, effects, deployment requirements
  ├─ analyzer ──────────> warnings and suggestions
  └─ runtime ───────────> deterministic or live execution + trace events
```

The CLI performs these phases in order. `--check`, `--format`, `--lower`, `--effects`, `--summary`, `--docs`, and `--api` stop after their respective inspection phase.

## Public Rust API

The `agl` crate exposes compiler phases without shelling out:

```rust
use agl::{check_program, execute_pipeline, parse_program};
use std::collections::BTreeMap;

let program = parse_program(source)?;
check_program(&program)?;
let value = execute_pipeline(&program, "pipeline_name", BTreeMap::new(), &registry, &context)?;
```

Important public types include `Program`, `Registry`, `Invocation`, `TaskOutput`, `HandlerFailure`, `ExecutionContext`, `CancellationToken`, `EventStore`, and the extension traits in `agl::extension`.

## Handlers and invocations

Task declarations provide signatures; behavior comes from a Rust `Registry`, a native provider adapter, or the process-isolated Python compatibility bridge. Contextual handlers receive an `Invocation` containing:

- stable execution, invocation, and idempotency identifiers;
- attempt number and optional execution context;
- a cooperative `CancellationToken`;
- usage reporting through `TaskOutput`.

Handlers should check cancellation at meaningful points and return `HandlerFailure` with an explicit kind and retryability rather than throwing an untyped exception.

## Retries, timeouts, and effects

Retries are bounded by the source declaration and resource budgets. Backoff supports initial delay, maximum delay, multiplier, and deterministic jitter. A retry preserves the logical invocation identity and idempotency key while incrementing the attempt number. A timeout cancels the invocation cooperatively and is non-retryable unless the program explicitly models another recovery path.

The checker infers transitive effects from task, tool, pipeline, and agent declarations. `--effects` exposes the result; `--summary` additionally reports external writes and approval boundaries. Retry safety is checked against declared idempotency and effects.

## Structured concurrency

AGL 0.5/0.6 provides bounded `parallel`, ordered `parallel map`, and `race` statements. Child scopes are joined before their parent continues. `race` cancels losing branches. Scheduler groups, concurrency limits, rate limits, and pipeline budgets are enforced by the runtime.

## Durable execution

`ExecutionContext::durable` and `SqliteEventStore` persist task attempts, results, checkpoints, approvals, and trace events. The CLI equivalent is:

```bash
cargo run -- examples/showcase_all_features.agent produce \
  --event-store run.sqlite --execution-id incident-42
cargo run -- examples/showcase_all_features.agent produce \
  --event-store run.sqlite --execution-id incident-42 --resume
```

Replay requires the same source/deployment contract. The runtime records content fingerprints and stable IDs so a resumed run cannot silently consume incompatible results.

## Observability and redaction

`ExecutionContext::events()` returns structured `TraceEvent` values. The CLI writes them with `--output-trace`. Durable persistence and trace output redact configured secret values before storage. Event records include execution, invocation, attempt, idempotency, provider, usage, and failure information where available.

## Diagnostics

Lexing, parsing, checking, and analysis failures carry stable codes and source spans. The CLI renders a human-readable diagnostic; the JSON-lines protocol and LSP expose machine-readable diagnostics for editor/build integration.
