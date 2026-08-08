# Contributing to AgentLang

Thanks for your interest in contributing.

## How this repo is laid out

AgentLang has two implementations, and that shapes almost every contribution:

- **`src/`** — the Rust library and CLI. This is the primary implementation.
- **`agentlang/`** — the original Python implementation, kept as a **compatibility
  oracle**. CI runs it against the same tests, so the two must agree on observable
  behaviour.
- **`spec/`** — the language specification. `spec/agl-0.2.md` is normative for the
  current language version.
- **`tests/`**, **`examples/`**, **`benches/`**, **`docs/`**, **`tree-sitter-agl/`**

A language change is therefore usually a **three-part PR**: spec, Rust, and Python
oracle. Say so in your PR description if you are intentionally changing only one.

## Prerequisites

- Rust `1.94.1` — pinned in `rust-toolchain.toml`, so `rustup` picks it up automatically
- Python 3.14 for the compatibility oracle

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
python3 -m unittest discover -s tests   # compatibility oracle
cargo package --locked
```

Clippy warnings are denied, so a warning fails the build.

## Changing the language

If your change affects syntax, types, or error codes:

1. Update `spec/agl-0.2.md` first — the spec is the contract, not the implementation.
2. Error codes are structured (`AGL1003`, `AGL3001`, …). Reuse an existing code where
   one fits; if you add one, document it in the spec.
3. Add an example under `examples/` when you add user-visible syntax.
4. Keep the Rust implementation and the Python oracle in agreement.

## Making changes

1. Branch off `main`: `git checkout -b feat/your-change`
2. Add tests alongside the change — this repo is well covered and PRs are expected to keep it that way.
3. Open a PR against `main`, linking any related issue.

## Reporting issues

Please include:

- Whether you hit it via the Rust CLI or the Python implementation
- A minimal `.agl` program that reproduces it
- The error code, if one was printed
- Expected vs actual behaviour

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
