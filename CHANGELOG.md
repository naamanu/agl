# Changelog

## 0.5.0 — Unreleased

- Add async embedding, cooperative timeout/race cancellation, ordered bounded parallel map, failure policies, concurrency groups, and rate limits.
- Add stable execution/invocation identity, event-store traits, local SQLite histories, checkpoint replay, compatibility fingerprints, retention, and redaction.
- Add persisted typed approval, rejection, delegation, expiry, and CLI resume workflows.

- Add deterministic bounded retry backoff, stable invocation/idempotency keys, contextual handler usage, and scoped pipeline resource budgets for AGL 0.4.
- Add dataset evaluations, repeated-trial distributions, semantic graders, trace replay, and CI baselines for AGL 0.4.

## 0.4.0 — Unreleased

- Added transitive effect inference and declared pipeline effect ceilings.
- Added task/tool idempotency contracts and static retry-safety enforcement.
- Added provider-neutral agent requirements and validated JSON deployment bindings.

## 0.3.0 — 2026-08-02

- Added version-gated nominal records, tagged unions, `Result[T, E]`, and exhaustive `match`.
- Added stable tagged JSON encodings for unions and typed outcomes.
- Added source-rendered diagnostics, unused/unreachable warnings, name suggestions, and exhaustive return-path checking.
- Added versioned normative specifications and executable conformance fixtures.

## 0.2.0 — 2026-08-02

- Reimplemented the AGL lexer, parser, workflow lowering, checker, runtime, and CLI in Rust.
- Added native OpenAI Responses and Anthropic Messages adapters with client-side tool calling.
- Added validated DuckDuckGo search and URL-fetch tools.
- Added structured traces, retry/fallback/timeout behavior, bounded parallelism, test blocks, REPL, and lowered-IR output.
- Added native handler registries and a subprocess bridge for existing Python task/tool plugins.
- Added complete example checking and deterministic Python/Rust differential tests.
- Retained the Python implementation as a compatibility oracle during the 0.2 release cycle.
