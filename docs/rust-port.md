# Rust implementation status

AGL 0.6 is implemented by the native Rust compiler, checker, runtime, CLI, adapters, and embedding API. The Python implementation remains available as a compatibility oracle and as a migration bridge for existing task/tool plugins; it is not the normative runtime.

## Implemented surface

| Area | Status |
|---|---|
| Lexer, parser, source spans, diagnostics | Complete |
| AGL 0.2–0.6 language versions | Complete |
| Types, records, unions, `Result`, exhaustive matching | Complete |
| Workflows and pipeline lowering | Complete |
| Effects, idempotency, retries, budgets | Complete |
| Provider-neutral deployment bindings and policy | Complete |
| Parallel, `parallel map`, `race`, cancellation | Complete |
| Durable SQLite-backed events, replay, approvals | Complete |
| Evaluations, distributions, baselines, replay | Complete |
| Modules, visibility, package locks, API comparison | Complete |
| Formatter, JSON-lines protocol, LSP, completions | Complete |
| OpenAI and Anthropic adapters plus native tools | Complete |
| Rust handler/tool/adapter extension traits | Complete |
| Python task/tool plugin bridge | Compatibility support |

## Compatibility boundary

Rust accepts the checked-in `.agent` examples and is the implementation used by the conformance and differential suites. Python plugins run in a separate process through a versioned JSON envelope; native applications should prefer `Registry`, `ToolRegistry`, and the extension traits in `agl::extension`.

## Embedding

```rust
use agl::{check_program, execute_pipeline, parse_program, Registry};
use agl::context::ExecutionContext;
use std::collections::BTreeMap;

let program = parse_program(source)?;
check_program(&program)?;
let registry = Registry::default();
let value = execute_pipeline(
    &program,
    "pipeline_name",
    BTreeMap::new(),
    &registry,
    &ExecutionContext::default(),
)?;
```

## Validation

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --offline -- -D warnings
cargo test --locked --offline
python3 -m unittest discover -s tests
cargo package --locked --offline
```

The live-provider tests are intentionally ignored in ordinary runs because they require credentials and make billable network calls. Run them manually or through `.github/workflows/live-smoke.yml` before a provider release.
