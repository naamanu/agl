# Changelog

## 0.2.0 — 2026-08-02

- Reimplemented the AGL lexer, parser, workflow lowering, checker, runtime, and CLI in Rust.
- Added native OpenAI Responses and Anthropic Messages adapters with client-side tool calling.
- Added validated DuckDuckGo search and URL-fetch tools.
- Added structured traces, retry/fallback/timeout behavior, bounded parallelism, test blocks, REPL, and lowered-IR output.
- Added native handler registries and a subprocess bridge for existing Python task/tool plugins.
- Added complete example checking and deterministic Python/Rust differential tests.
- Retained the Python implementation as a compatibility oracle during the 0.2 release cycle.
