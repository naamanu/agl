# AGL 0.5 language specification

Status: draft normative

Language version: `0.5`

AGL 0.5 extends [AGL 0.4](agl-0.4.md) with structured concurrency, durable execution, and persisted human interaction.

## 1. Cooperative execution

The host owns every spawned operation until it completes or receives cancellation. Contextual handlers receive an `Invocation` containing a cancellation token. Timeout and race cancellation is cooperative: a handler or adapter that advertises cancellation support must stop billable work when the token is set. Legacy synchronous handlers remain compatible but cannot promise prompt cancellation.

The Rust embedding API provides `execute_pipeline_async`; the synchronous API remains a compatibility wrapper. A task timeout is a scoped deadline and produces `Failure.kind = "timeout"`. Cancellation produces `Failure.kind = "cancelled"`.

## 2. Structured concurrency

Bounded ordered mapping has the form:

```agl
let outputs = parallel map item in items max_concurrency 4 collect_all {
  let output = transform(item);
};
```

The input must be `List[T]`; the body is one direct task call accepting the bound item; the result is `List[U]` in input order. `fail_fast` stops scheduling later chunks after a failing chunk. `collect_all` runs every item and reports the aggregate failure after all children finish.

`let winner = race { ... };` requires at least two same-typed direct task calls. The first successful result wins, all loser tokens are cancelled, and every child is joined before the scope exits. If every branch fails, the race fails.

Tasks and tools may declare `concurrency_group`, `concurrency_limit`, and `rate_limit`. Members of a named group share host scheduling limits. Enclosing pipeline concurrency budgets cap the width of parallel and race scopes; nested pipelines create nested resource scopes.

## 3. Durable execution

An execution has a caller-supplied stable ID. Each call receives a stable invocation ID allocated before concurrent scheduling. An `EventStore` appends versioned trace events; `SqliteEventStore` is the normative local implementation.

`task_result` is the operation checkpoint. Resume loads prior events and substitutes the recorded value for a matching invocation before consulting a handler. Thus a completed external write is not repeated. The original idempotency key remains the fallback protection if an external effect succeeds but its checkpoint cannot be persisted.

The program fingerprint covers source AST and deployment bindings. Resume fails with `resume_incompatible` if it changes. Model/task results, usage, retry timing, human input, and orchestration decisions are events and can be replayed without providers. Event stores expose retention pruning, redact configured secrets before persistence, and carry a schema version for migration.

## 4. Human interaction

```agl
let approved = approve production "Release?" expires 3600 delegate operator;
```

Approval yields `Bool`. Without a host decision, execution appends `human_suspended` and returns `Failure.kind = "suspended"`; it does not block a process. Resume supplies an approval or rejection. Decisions record actor, timestamp, optional delegate, and expiry. An expired decision fails with `approval_expired`.

## 5. Compatibility

All constructs in this specification require `language "0.5";`. AGL 0.2–0.4 programs retain their earlier meaning.
