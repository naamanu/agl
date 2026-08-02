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
| OpenAI and Anthropic adapters | Python compatibility CLI |
| Web tool adapters | Python compatibility CLI |
| Dynamic Python plugins | Python compatibility CLI |
| Interactive REPL and lowered-IR printer | Python compatibility CLI |

All checked-in `.agent` examples are parsed and checked by the Rust integration suite. Deterministic blog, support-routing, and retry paths have also been compared directly with the Python results.

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

The Python suite remains required until live adapters and plugin migration are finished.
