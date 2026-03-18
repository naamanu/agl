# Repository Guidelines

## Project Structure & Module Organization
Core implementation lives in `agentlang/`: parser and language model (`ast.py`, `lexer.py`, `parser.py`), validation (`checker.py`), execution (`runtime.py`, `stdlib.py`), observability (`context.py`), extensibility (`plugins.py`), and integrations in `agentlang/adapters/`.  
`main.py` is the CLI entrypoint (direct pipeline execution plus the `repl` subcommand).  
`examples/` contains runnable `.agent` programs used for smoke testing and demos.  
`docs/` contains language, runtime, adapter, and contribution docs.  
`tests/` contains automated coverage for parser, checker, and runtime behavior.

## Build, Test, and Development Commands
- `python -m py_compile main.py agentlang/*.py agentlang/adapters/*.py`  
  Fast syntax validation for core modules.
- `python main.py run examples/blog.agent blog_post --input '{"topic":"agent memory patterns"}'`  
  Run a standard mock-mode pipeline.
- `python main.py run examples/reliability.agent resilient_brief --input '{"topic":"api-status","fail_count":5}'`  
  Exercise retry/failure behavior.
- `python main.py repl --adapter mock`  
  Start the interactive REPL.
- `OPENAI_API_KEY=... python main.py run examples/blog.agent blog_post --adapter live --input '{"topic":"agent memory patterns"}'`
  Verify live adapter behavior.
- `python main.py examples/showcase_all_features.agent --test --plugin examples/showcase_plugin.py`
  Run in-language test blocks.
- `python main.py examples/showcase_all_features.agent produce --input '{"topic":"AI safety"}' --output-trace trace.json --plugin examples/showcase_plugin.py`
  Run with execution trace output.

## Coding Style & Naming Conventions
Use Python 3.14+ with 4-space indentation and explicit type hints on public functions.  
Use `snake_case` for functions/variables and `PascalCase` for classes.  
Keep changes small and composable; prefer explicit errors over silent fallbacks.  
For language features, update parser/checker/runtime together and keep docs synchronized.

## Testing Guidelines
Minimum validation for each change:
1. Run `py_compile` checks.
2. Run `python -m unittest discover -s tests`.
3. Run at least one happy-path example and one failure/retry example from `examples/`.
4. Run `python main.py examples/showcase_all_features.agent --test --plugin examples/showcase_plugin.py`.
5. If DSL/runtime semantics change, update relevant docs (`docs/reference/language.md`, `docs/reference/runtime.md`, `docs/advanced/semantics.md`).

## Commit & Pull Request Guidelines
Follow Conventional Commit style seen in history: `feat: ...`, `fix: ...` (imperative, concise subject).  
PRs should include: problem statement, scope of changes, modules touched, commands executed for validation, and required doc updates.  
Include CLI output snippets when behavior changes.

## Security & Configuration Tips
Do not commit secrets. Use environment variables (`OPENAI_API_KEY`, optional `OPENAI_BASE_URL`) for live adapter runs.  
Avoid logging sensitive values in adapter or CLI error paths.
