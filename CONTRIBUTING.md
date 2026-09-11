# Contributing to AgentLang

Thanks for your interest in contributing.

## How this repo is laid out

AgentLang is implemented in Rust:

- **`src/`** — the Rust library and CLI. This is the primary implementation.
- **`spec/`** — the language specification. Versioned contracts run from
  `spec/agl-0.2.md` through `spec/agl-0.6.md`.
- **`tests/`**, **`examples/`**, **`benches/`**, **`docs/`**, **`tree-sitter-agl/`**

A language change updates the versioned specification, Rust implementation,
and relevant conformance or regression tests together.

## Prerequisites

- Rust `1.94.1` — pinned in `rust-toolchain.toml`, so `rustup` picks it up automatically

## Getting started

```bash
git clone https://github.com/your-username/agl.git
cd agl
cargo build
cargo run -- --help
```

## Before you open a PR

Run what CI runs:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
cargo package --locked
```

Clippy warnings are denied, so a warning fails the build.

For documentation changes, install mdBook with
`cargo install mdbook --version 0.5.4 --locked`, then run `mdbook build`.
Use `mdbook serve --open` for a local preview. Navigation lives in
`docs/SUMMARY.md`; configuration lives in `book.toml`.

## Changing the language

If your change affects syntax, types, or error codes:

1. Update the relevant versioned document in `spec/` first — the spec is the contract, not the implementation.
2. Error codes are structured (`AGL1003`, `AGL3001`, …). Reuse an existing code where
   one fits; if you add one, document it in the spec.
3. Add an example under `examples/` when you add user-visible syntax.
4. Add Rust regression tests and conformance fixtures for the behavior.

## Making changes

1. Branch off `main`: `git checkout -b feat/your-change`
2. Add tests alongside the change — this repo is well covered and PRs are expected to keep it that way.
3. Open a PR against `main`, linking any related issue.

## Reporting issues

Please include:

- Whether you hit it via the CLI or the Rust embedding API
- A minimal `.agent` program that reproduces it
- The error code, if one was printed
- Expected vs actual behaviour

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
