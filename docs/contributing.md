# Contributing

This guide explains how to extend AgentLang — adding language features, new tasks, or adapter changes.

## Development workflow

1. Edit the Rust source in `src/`; it is the normative implementation.
2. Format and run the native suite:

    ```bash
    cargo fmt --all -- --check
    cargo test
    ```

3. Run a representative example:

    ```bash
    cargo run -- examples/blog.agent blog_post \
      --input '{"topic":"agent memory patterns"}'
    ```

4. Run the retry/fallback example to exercise the failure path:

    ```bash
    cargo run -- examples/reliability.agent resilient_brief \
      --input '{"topic":"api-status","fail_count":5}'
    ```

5. Check the largest language fixture and run its test blocks:

    ```bash
    cargo run -- examples/showcase_all_features.agent --check
    cargo run -- examples/showcase_all_features.agent --test
    ```

6. If you changed DSL or runtime semantics, update the relevant docs (see below).

## Building the documentation

Documentation uses mdBook 0.5.4, matching the documentation CI workflow:

```bash
cargo install mdbook --version 0.5.4 --locked
mdbook build
mdbook serve --open
```

Run these commands from the repository root. `book.toml` configures the book,
`docs/SUMMARY.md` defines chapter navigation, and generated HTML goes into
`book/`. Add new pages to the summary so mdBook renders them. Pull requests
build the book; pushes to `main` publish it through GitHub Pages.

Use Markdown admonitions (`> [!NOTE]`, `> [!WARNING]`) for callouts. Math uses
mdBook's [MathJax syntax](https://rust-lang.github.io/mdBook/format/mathjax.html):
escaped delimiters `\\(` … `\\)` for inline equations and `\\[` … `\\]` for
display equations. Preserve TeX backslashes through Markdown escaping, as in
the semantics chapters.

## Project layout

```text
src/               -- primary Rust compiler and runtime
  adapters/        -- provider clients and native web tools
  formatter.rs     -- canonical source and lowered pipeline formatters
  extension.rs     -- native extension contracts
examples/
  *.agent         -- runnable example programs
docs/             -- this documentation
```

## Extending the language

Adding a new syntax feature touches every layer. Update all of these:

| File | What to change |
|---|---|
| `src/ast.rs` | Add or extend AST types |
| `src/lexer.rs` | Add tokens or lexical rules |
| `src/parser.rs` | Add parsing logic and source diagnostics |
| `src/checker.rs` | Add type/effect rules |
| `src/runtime.rs` | Add execution semantics and trace events |
| `docs/reference/language.md` | Update syntax reference |
| `docs/reference/runtime.md` | Update execution phase docs |
| `docs/advanced/semantics.md` | Update formal rules |

## Adding a new task

1. Declare the task signature in a `.agent` file:

    ```agentlang
    task my_task(input: String) -> Obj{result: String} {}
    ```

2. Register a Rust handler in a `Registry`:

    ```rust
    registry.register("my_task", |args, _agent| {
        Ok(serde_json::json!({"result": args["input"]}))
    });
    ```

3. Add or update an example in `examples/`.

4. Document the task in `docs/reference/examples.md` and `docs/reference/adapters.md` if it has live behavior.

## Adapter changes

The native provider clients live in `src/adapters/`; native web tools live in `src/adapters/tools.rs`.

Guidelines:

- Keep adapter modules dependency-light.
- Wrap all external errors with clear, user-readable messages.
- Never log or surface secrets in error messages or stack traces.

## Style guidelines

- Follow `rustfmt` and Clippy for Rust code; use descriptive public API names and explicit error types.
- Keep changes small and composable — prefer explicit errors over silent fallbacks.
- Keep docs synchronized with behavior changes.
- Follow Conventional Commit style: `feat:`, `fix:`, `docs:` prefixes with an imperative, concise subject.

## Commit and PR checklist

Before opening a PR:

- [ ] At least one happy-path example runs correctly
- [ ] At least one failure-path example runs correctly (if relevant)
- [ ] `--test` passes on `showcase_all_features.agent`
- [ ] Docs updated for any DSL/runtime/adapter changes
- [ ] No secrets in source, examples, or docs
