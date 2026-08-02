# Rust port status

AGL 0.2 introduces a native Rust implementation while preserving the Python implementation as an executable specification during migration.

## Native parity

| Capability | Rust 0.2 |
|---|---|
| Lexer, escapes, source spans | Complete |
| Tasks, tools, agents, aliases, enums | Complete |
| Pipelines and shorthand calls | Complete |
| Workflows, stages, review/revision lowering | Complete |
| Static structural type checking | Complete |
| `if`, `if let`, `while`, break/continue | Complete |
| Parallel execution and `max_concurrency` | Complete |
| Retry, fallback, timeout, try/catch | Complete |
| Pipeline composition, assertions, test blocks | Complete |
| Deterministic mock handlers | Complete |
| Structured JSON traces | Complete |
| Native embedding/handler registration | Complete |
| OpenAI and Anthropic adapters | Complete |
| Web tool adapters | Complete |
| Dynamic Python task plugins | Complete via migration bridge |
| Dynamic Python tool plugins | Complete via migration bridge |
| Interactive REPL and lowered-IR printer | Complete |

All checked-in `.agent` examples are parsed and checked by the Rust integration suite. A differential test executes the blog, support-routing, comparison, and retry pipelines through both implementations and requires identical JSON results.

The native runtime benchmark uses two 25 ms handlers. On an Apple Silicon development machine it measured 57.77 ms sequentially and 29.23 ms in parallel, a 1.98x speedup over 12 iterations. Treat this as a reproducible smoke benchmark rather than a general performance claim; run `cargo bench --bench runtime` on each target platform.

## Provider configuration

OpenAI defaults to `gpt-5.6-sol` with medium reasoning effort. Legacy `gpt-4.1`/`gpt-4o` declarations map to Sol, while legacy mini declarations map to `gpt-5.6-luna`. Set `AGL_OPENAI_MODEL` to override routing globally. Anthropic can be overridden with `AGL_ANTHROPIC_MODEL`.

Real-provider tests are ignored during ordinary local and pull-request test runs because they are billable. Use `.github/workflows/live-smoke.yml` or run the ignored tests explicitly after setting the relevant API key.

## Embedding

The crate is library-first:

```rust
use agl::{check_program, execute_pipeline, parse_program, Registry};
use agl::context::ExecutionContext;
```

Use `Registry::register` to supply application task handlers. A handler receives evaluated JSON arguments and the optional agent name, and returns a JSON value or a concise error string. This replaces Python module loading with a compile-time-safe native extension point.

## Validation

```bash
cargo fmt --all -- --check
cargo test
cargo run -- examples/showcase_all_features.agent --check
python3 -m unittest discover -s tests
```

The Python suite remains required while the reference implementation is retained. Python task and tool plugins run through an isolated JSON subprocess bridge; native applications should use the Rust `Registry` and `ToolRegistry` APIs.

See [Native extensions and plugin migration](native-extensions.md) and [Releasing](releasing.md).
