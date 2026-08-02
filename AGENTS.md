# Repository Guidelines

## Project Structure & Module Organization

The production implementation is Rust-first. Core language code lives in `src/`: AST, lexer, parser, checker, workflow lowering, runtime, standard library, formatter, plugins, and provider adapters. `src/main.rs` provides the `agl` CLI and `src/lib.rs` exposes the embedding API.

`agentlang/` and `main.py` retain the original Python implementation as the compatibility oracle during migration. Keep it behaviorally aligned when changing DSL semantics. `examples/` contains `.agent` programs and native embedding examples, `docs/` contains reference and migration guidance, `tests/` contains Rust integration/differential/live tests plus the Python suite, and `benches/` contains executable performance checks.

## Build, Test, and Development Commands

- `cargo fmt --all -- --check` checks Rust formatting.
- `cargo clippy --all-targets --all-features -- -D warnings` enforces lint cleanliness.
- `cargo test --locked` runs Rust unit, integration, and Python/Rust differential tests.
- `python3 -m unittest discover -s tests` runs the Python compatibility suite.
- `cargo run --locked -- run examples/blog.agent blog_post --input '{"topic":"agent memory patterns"}'` runs a mock-mode pipeline.
- `cargo run --locked --example native_embed` demonstrates a compile-time native Rust extension.
- `cargo bench --bench runtime` runs the runtime concurrency benchmark.
- `cargo package --locked --allow-dirty` verifies the distributable crate.

Live OpenAI and Anthropic tests are ignored by default because they are billable. Run them explicitly with the appropriate key, or dispatch `.github/workflows/live-smoke.yml` after configuring repository secrets.

## Coding Style & Naming Conventions

Use the pinned stable Rust toolchain, standard `rustfmt`, `snake_case` for functions and modules, and `PascalCase` for types and traits. Prefer typed errors and explicit state transitions over silent fallbacks. Keep public APIs documented where intent is not obvious.

For language changes, update the versioned normative specification in `spec/`, the Rust AST/parser/checker/runtime, and relevant documentation together. Until the Python compatibility layer is formally retired, update it too and extend the differential tests whenever both implementations should agree.

## Testing Guidelines

Minimum validation for each change:

1. Run formatting, strict Clippy, and `cargo test --locked`.
2. Run the Python compatibility suite when changing shared semantics.
3. Run at least one representative CLI or embedding example for user-visible behavior.
4. Update differential tests for parser, checker, lowering, or runtime changes.
5. Update conformance fixtures, the normative specification, reference docs, and migration notes when syntax, semantics, providers, or extension APIs change.

Provider changes additionally require the relevant opt-in live smoke workflow before release. Do not turn billable network tests into default CI jobs.

## Commit & Pull Request Guidelines

Follow Conventional Commit style used in history: `feat: ...`, `fix: ...`, `test: ...`, or `chore: ...`, with a concise imperative subject. PRs should state the problem, scope, affected modules, validation commands, documentation changes, and any credential-dependent checks still pending. Include concise CLI output when behavior changes.

## Security & Configuration Tips

Never commit provider credentials. Live adapters read `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`; optional model overrides are `AGL_OPENAI_MODEL` and `AGL_ANTHROPIC_MODEL`. OpenAI-compatible endpoints may also use `OPENAI_BASE_URL`. Avoid logging secret values, authorization headers, or sensitive tool payloads.
