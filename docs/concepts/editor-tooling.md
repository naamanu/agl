# Editor and build tooling

- `agl file.agent --format` prints canonical source.
- `agl file.agent --docs api.md` generates exported API docs with inferred effects.
- `agl protocol` serves the JSON-lines compiler protocol.
- `agl lsp` serves LSP over stdio with diagnostics, completion, hover, definition, references, rename, and formatting.
- `agl completions bash|zsh|fish` emits shell completions.
- `tree-sitter-agl` contains the incremental grammar and highlight query.

The protocol is intentionally independent from an editor so build systems can consume the same structured check, lower, format, and effects results.
