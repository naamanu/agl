# Observability

The Rust runtime records structured `TraceEvent` values in `ExecutionContext`. Each event has a `kind`, a millisecond timestamp, and a JSON `fields` object.

## CLI tracing

Write a trace after a run:

```bash
cargo run -- examples/blog.agent blog_post \
  --input '{"topic":"AI safety"}' --output-trace trace.json
```

Live provider request/tool activity goes to stderr with `--trace-live`. This is separate from the persisted execution trace.

## Event kinds

The runtime emits kinds including:

| Kind | Typical fields |
|---|---|
| `task_attempt` | task, agent, attempt, execution_id, invocation_id, idempotency_key |
| `task_result` | task, attempt, execution_id, invocation_id, result |
| `task_retry` | task, attempt, reason, delay_ms, invocation_id, idempotency_key |
| `task_replayed` | task, execution_id, invocation_id, idempotency_key |
| `task_cancelled` | task, execution_id, invocation_id, reason |
| `provider_usage` | task, attempt, execution_id, invocation_id, usage |
| `race_winner` | branch, cancelled_losers |
| `human_suspended` | approval, prompt, delegate, expires_seconds |
| `human_approval` | approval, decision, actor, decided_at_ms |

The schema is intentionally an extensible `(kind, fields)` envelope. Consumers should ignore unknown kinds and use the event schema version recorded by durable stores.

## Durable history and redaction

With `--event-store PATH`, events and checkpoints are persisted in the local SQLite-backed event store. `--resume` loads prior task results by stable invocation identity. Secret values configured on an `ExecutionContext` are redacted before trace and durable persistence.

## Rust embedding

```rust
let context = agl::context::ExecutionContext::default();
let value = agl::execute_pipeline(&program, "pipeline", inputs, &registry, &context)?;
for event in context.events() {
    println!("{} {} {}", event.kind, event.timestamp_ms, event.fields);
}
```

Use `ExecutionContext::deterministic(seed)` for non-sleeping, reproducible retry/evaluation runs and `ExecutionContext::durable(execution_id, store, resume)` for replayable execution.

## What to measure

- task attempts, retries, cancellations, and replay hits;
- provider usage and cost inputs returned in `TaskOutput`;
- approval suspension, decision, delegation, and expiry;
- race winners and parallel-map failures;
- source/deployment fingerprints at durable-run boundaries.
