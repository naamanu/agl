# AgentLang

AgentLang (AGL) is a typed language for agentic workflows, implemented as a Rust
library and CLI. Define agents and tasks, compose them into workflows or pipelines,
and run them with deterministic mock handlers or native OpenAI and Anthropic adapters.

## Quick start

The repository pins Rust 1.94.1 in [rust-toolchain.toml](rust-toolchain.toml).
From the repository root, run a pipeline in mock mode—no API key required:

```bash
cargo run --locked -- examples/blog.agent blog_post \
  --input '{"topic":"agent memory patterns"}'
```

The [blog example](examples/blog.agent) connects two typed tasks:

```agentlang
agent planner { model: "gpt-4.1", tools: [web_search] }
agent writer { model: "gpt-4.1-mini", tools: [] }

tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}
task research(topic: String) -> Obj{notes: String} {}
task draft(notes: String) -> Obj{article: String} {}

pipeline blog_post(topic: String) -> String {
  let r = run research with { topic: topic } by planner;
  let d = run draft with { notes: r.notes } by writer;
  return d.article;
}
```

Check a program without executing it, or run its embedded test blocks:

```bash
cargo run --locked -- examples/showcase_all_features.agent --check
cargo run --locked -- examples/showcase_all_features.agent --test
```

For live execution, see [provider setup](docs/reference/adapters.md).
To install the CLI locally, run `cargo install --path . --locked`.

## Language and tooling

- **Typed programs:** records, enums, unions, `Result`, pattern matching, and static checks for task arguments and pipeline returns.
- **Workflow composition:** declarative stages and review loops, explicit pipelines, parallel calls, and pipeline-to-pipeline calls.
- **Error handling:** retries, fallback values, and `try`/`catch`.
- **Testing and inspection:** assertions, embedded test blocks, deterministic mock execution, and JSON traces.
- **Native integration:** Rust task and tool registries, plus OpenAI and Anthropic adapters.
- **Developer tools:** formatter, REPL, language server, tree-sitter grammar, and module/package support.

## Examples

| Example | Demonstrates |
| --- | --- |
| [Blog](examples/blog.agent) | Research followed by drafting |
| [Newsletter](examples/newsletter.agent) | Declarative workflow stages |
| [Comparison](examples/compare.agent) | Parallel research and a combined result |
| [Reliability](examples/reliability.agent) | Retries and fallback values |
| [Native embedding](examples/native_embed.rs) | Registering Rust handlers |

See the [example guide](docs/reference/examples.md) for inputs and commands.

## Documentation

Read the [documentation](https://nanamanu.com/agl), or browse the sources:

- [Quick start](docs/tutorial/quickstart.md) and [language reference](docs/reference/language.md)
- [CLI reference](docs/reference/cli.md) and [native extensions](docs/native-extensions.md)
- [Versioned specifications](spec/) and [sequential core semantics](docs/advanced/core-semantics.md)
- [Language roadmap](docs/roadmap.md) and [contributing guide](CONTRIBUTING.md)

The documentation uses mdBook 0.5.4. Follow the
[installation instructions](docs/contributing.md#building-the-documentation), then
build or preview from the repository root:

```bash
mdbook build
mdbook serve --open
```

## Development

The compiler, runtime, and CLI live in `src/`; Rust tests are in `tests/`.
Run the standard checks with:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked
```

Licensed under [MIT](LICENSE).
