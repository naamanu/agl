# AGL language roadmap

This document is the source of truth for evolving AGL after the Rust port. The goal is not to turn AGL into a general-purpose programming language. AGL should become the smallest language that makes agentic applications reliable, portable, auditable, and resumable.

The roadmap is complete only when every acceptance criterion below is implemented, documented, covered by conformance tests, and represented in the formal semantics where applicable.

## Design principles

1. **Reliability over convenience.** Make failure, cancellation, budgets, and side effects explicit.
2. **Provider independence.** Programs express required capabilities; deployment selects providers and models.
3. **Static guarantees where useful.** Reject unsafe retries, invalid data flow, non-exhaustive matches, and undeclared capabilities before execution.
4. **Deterministic orchestration.** Nondeterministic model and tool results are recorded as events around a deterministic workflow core.
5. **Structured execution.** Child work has an owner, deadline, cancellation path, and durable identity.
6. **Inspectability.** Source, lowered form, diagnostics, traces, and replay remain understandable without framework internals.
7. **A small language.** Prefer orthogonal primitives and library features over overlapping syntax.
8. **Compatibility by evidence.** Every language version has executable conformance fixtures and explicit migration notes.

## Completion rules

An item is not complete merely because its parser accepts new syntax. Completion requires:

- AST, parser, formatter, checker, and runtime support as applicable.
- Positive and negative conformance fixtures.
- Rust unit and integration tests.
- Differential tests when the feature belongs to the retained Python compatibility surface.
- Language-reference and conceptual documentation.
- Formal static or dynamic semantics for language-level behavior.
- Diagnostic source spans and stable error codes for rejected programs.
- A changelog entry and migration note.

## Phase 0 — specification and conformance foundation

This phase prevents the implementation, documentation, and paper from becoming competing definitions of AGL.

- [x] Create a normative, versioned AGL 0.2 specification.
- [x] Declare the Rust implementation normative and define the Python implementation's compatibility role.
- [x] Resolve whether objects are exact or use width/depth structural subtyping.
- [x] Reconcile current CLI names, adapter names, environment variables, and defaults.
- [x] Define lexical, grammar, name-resolution, typing, and execution behavior separately.
- [x] Add valid and invalid fixture directories with machine-readable expectations.
- [x] Add a conformance runner that checks parsing, diagnostics, lowering, and execution.
- [x] Add stable phase diagnostic codes.
- [x] Add golden source rendering as part of AGL 0.3 diagnostics.
- [x] Define a language-version mechanism and unsupported-version diagnostic.
- [x] Update the paper with a clear historical-versus-current specification boundary.

Acceptance criteria:

- `cargo test --test conformance` executes every fixture.
- Each rejected fixture declares the expected diagnostic code.
- Documentation no longer contradicts checker or runtime behavior.
- Language changes cannot merge without updating the versioned specification or explicitly declaring no semantic change.

## AGL 0.3 — typed outcomes and language integrity

### Named data types

- [x] Add nominal `record` declarations while retaining structural `Obj` values.
- [x] Define construction, field access, assignability, and serialization for records.
- [x] Preserve aliases as transparent names and document the distinction from records.
- [x] Add duplicate-field, recursive-type, and visibility diagnostics.

### Tagged unions and matching

- [x] Add tagged `union` declarations with unit and record variants.
- [x] Add constructors for union variants.
- [x] Add `match` statements with bindings.
- [x] Check exhaustiveness and unreachable cases.
- [x] Specify JSON encoding for records and unions.

### Structured outcomes and errors

- [x] Add built-in `Result[T, E]` alongside `Option[T]`.
- [x] Represent current task, timeout, assertion, and runtime faults through structured `Failure` catches.
- [x] Add tool/adapter-specific, cancellation, and budget failure categories as those runtime layers gain typed failure propagation.
- [x] Keep explicit `match`; 0.3 deliberately omits implicit propagation syntax.
- [x] Retain `try/catch` as a boundary for unexpected execution faults, not ordinary typed failures.
- [x] Make retry selection operate on typed error variants.

### Diagnostics and developer experience

- [x] Give lexer, parser, resolver, checker, and runtime errors stable codes.
- [x] Render filename, source span, offending line, caret, and concise remediation.
- [x] Detect unreachable statements and unused bindings.
- [x] Provide typo suggestions for declarations, fields, and enum/union variants.

Acceptance criteria:

- A task can return a typed domain error without throwing a runtime exception.
- Every `match` over a closed union is statically exhaustive.
- Record and union values round-trip through JSON with a documented stable encoding.
- Existing 0.2 programs either continue to run or receive an automated migration diagnostic.

## AGL 0.4 — agent contracts, effects, and evaluation

### Effect and capability system

- [x] Define built-in effects such as `model`, `network`, `filesystem`, `external_read`, `external_write`, `secret`, and `human`.
- [x] Let tasks and tools declare required effects.
- [x] Infer effects through pipeline calls and check that callers permit them.
- [x] Add user-defined capability names without allowing them to weaken built-in safety rules.
- [x] Surface declared effect sets in lowered output and the language documentation.
- [x] Include inferred effect sets in generated package/API documentation when documentation generation lands in 0.6.

### Retry and idempotency safety

- [x] Declare tasks and tools as pure, idempotent, keyed-idempotent, or non-idempotent.
- [x] Reject unsafe retry policies on side-effecting non-idempotent tasks and agent tools.
- [x] Support retry predicates over typed failures.
- [x] Add bounded exponential backoff and jitter policies with deterministic test control.
- [x] Attach stable invocation and idempotency keys to traces and adapter calls.

### Provider-independent agents

- [x] Separate source-level agent requirements from deployment bindings.
- [x] Express capability, context, tool, latency, and quality requirements in source.
- [x] Move provider, model, endpoint, and reasoning settings into deployment configuration.
- [x] Keep an explicit source-level model override as a documented escape hatch.
- [x] Validate deployment configuration against agent requirements before execution.

### Resource budgets

- [x] Add scoped budgets for time, tokens, cost, tool calls, retries, and concurrency.
- [x] Define reservation and accounting semantics for parallel work.
- [x] Return typed budget-exhaustion outcomes.
- [x] Record provider usage and estimated/actual cost in traces.

### Evaluation

- [x] Extend `test` with deterministic model/tool mocks and recorded replay.
- [x] Add dataset-driven `eval` declarations.
- [x] Support schema, predicate, latency, cost, and semantic-grader assertions.
- [x] Run repeated trials and report distributions for nondeterministic evaluations.
- [x] Compare evaluation results against a checked-in baseline in CI.

Acceptance criteria:

- The checker rejects retrying an unkeyed external write.
- The same source program can run against multiple provider bindings without modification.
- A pipeline cannot exceed a declared budget without a typed, traceable outcome.
- Evaluation regressions can fail CI without making ordinary unit tests nondeterministic.

## AGL 0.5 — structured concurrency and durable execution

### Async runtime and cancellation

- [x] Replace abandoned timeout threads with an async execution engine.
- [x] Propagate cooperative cancellation through pipelines, tasks, tools, and adapters.
- [x] Define scoped deadlines and cleanup behavior.
- [x] Ensure every spawned operation is awaited, cancelled, or durably detached by explicit policy.

### Concurrency language

- [x] Generalize parallel branches beyond direct task calls where semantics remain clear.
- [x] Add bounded `parallel map` with stable result ordering.
- [x] Add `race` with explicit winner and loser-cancellation semantics.
- [x] Add fail-fast and collect-all policies.
- [x] Add provider/tool concurrency groups and rate limits.
- [x] Define nested concurrency-budget behavior.

### Durable workflows

- [x] Assign stable execution and invocation identities.
- [x] Introduce an event-store abstraction and a local SQLite implementation.
- [x] Checkpoint completed operations and pipeline state.
- [x] Resume after process failure without repeating completed side effects.
- [x] Record nondeterministic model, tool, time, and external-input events for replay.
- [x] Detect incompatible source or deployment changes during resume.
- [x] Add retention, redaction, and schema-migration policies for stored histories.

### Human interaction

- [x] Add typed approval/input suspension points.
- [x] Resume suspended workflows through CLI and embedding APIs.
- [x] Define approval expiry, rejection, delegation, and audit behavior.

Acceptance criteria:

- Timeout and race losers stop cooperatively without continuing billable work when the provider supports cancellation.
- A killed process resumes from its last durable event without repeating a completed external write.
- Replay can reproduce orchestration decisions without contacting providers.
- Human approval is a persisted state transition rather than a blocked process.

## AGL 0.6 — modules, tooling, and ecosystem

### Modules and visibility

- [x] Add imports, qualified names, and explicit public/private declarations.
- [x] Specify deterministic local module resolution and cycle diagnostics.
- [x] Support separate compilation and cache validated module interfaces.
- [x] Add documentation generation from exported declarations.

### Packages and compatibility

- [x] Add an AGL package manifest with language and package versions.
- [x] Support local-path and pinned Git dependencies first.
- [x] Add a deterministic lockfile and content verification.
- [x] Define semantic-version compatibility rules for exported AGL APIs.
- [x] Defer a public package registry until real reuse patterns justify it.

### Language tooling

- [x] Publish a tree-sitter grammar or equivalent incremental parser integration.
- [x] Implement an LSP with diagnostics, hover, completion, definition, references, and rename.
- [x] Add canonical formatting and format-on-save support.
- [x] Add a machine-readable compiler protocol for editor and build-tool integration.
- [x] Provide shell completion and structured CLI output.

### Stable embedding and extension API

- [x] Define versioned Rust traits and data contracts for hosts, task handlers, tools, event stores, policy resolvers, and graders.
- [x] Add compatibility tests for extension implementations.
- [x] Keep compile-time Rust registration as the default native extension model.
- [x] Version the Python subprocess protocol and document its deprecation criteria.
- [x] Evaluate a process-isolated WASI component boundary only after the native contracts stabilize.

Acceptance criteria:

- A multi-module application type-checks incrementally and has reproducible dependencies.
- Editors can provide precise incremental diagnostics and navigation.
- Public package compatibility can be checked without executing the package.
- Extension API upgrades have explicit compatibility and migration tests.

## Cross-cutting security and provenance

These requirements apply throughout phases 0.3–0.6 rather than being deferred to one release.

- [x] Redact secrets from diagnostics, traces, event histories, and evaluation artifacts.
- [x] Restrict tools by declared capabilities and deployment policy.
- [x] Track model, prompt, tool, source, and human-approval provenance.
- [x] Add configurable network-host and filesystem-path allowlists.
- [x] Make external writes and approval requirements visible in static summaries.
- [x] Define trust boundaries for native, Python, and future process-isolated extensions.

## Deliberate non-goals

- General mutable state, classes, inheritance, or arbitrary metaprogramming.
- Provider-specific prompt syntax in the core language.
- Transparent retries of unknown side effects.
- Unbounded implicit concurrency.
- Dynamic in-process Rust library loading across unstable ABIs.
- A public package registry before modules and compatibility rules mature.
- Replacing host languages for tool implementation or user-interface code.

## Release gates for every phase

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
python3 -m unittest discover -s tests
cargo bench --bench runtime
cargo package --locked
```

Provider-related releases additionally require both manually triggered live-provider smoke tests. Durable-runtime releases require crash/resume, duplicate-side-effect, cancellation, and replay integration tests.
