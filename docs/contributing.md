# Contributing

This guide explains how to extend AgentLang — adding language features, new tasks, or adapter changes.

## Development workflow

1. Edit source files.
2. Run syntax validation:

    ```bash
    python -m py_compile main.py agentlang/*.py agentlang/adapters/*.py
    ```

3. Run a representative example:

    ```bash
    python main.py examples/blog.agent blog_post \
      --input '{"topic":"agent memory patterns"}'
    ```

4. Run the retry/fallback example to exercise the failure path:

    ```bash
    python main.py examples/reliability.agent resilient_brief \
      --input '{"topic":"api-status","fail_count":5}'
    ```

5. If you changed DSL or runtime semantics, update the relevant docs (see below).

## Project layout

```text
agentlang/
  ast.py          -- AST node dataclasses
  lexer.py        -- tokenizer + string decoder
  parser.py       -- recursive-descent parser
  checker.py      -- static type checker
  runtime.py      -- pipeline executor
  stdlib.py       -- built-in task handlers + task registry
  adapters/
    openai.py     -- OpenAI Responses API client
    tools.py      -- web search and other tool adapters
examples/
  *.agent         -- runnable example programs
docs/             -- this documentation
main.py           -- CLI entrypoint
```

## Extending the language

Adding a new syntax feature touches every layer. Update all of these:

| File | What to change |
|---|---|
| `agentlang/ast.py` | Add new AST node dataclass(es) |
| `agentlang/lexer.py` | Add new tokens or keywords |
| `agentlang/parser.py` | Add parsing logic for the new syntax |
| `agentlang/checker.py` | Add type-checking rules |
| `agentlang/runtime.py` | Add execution semantics |
| `docs/reference/language.md` | Update syntax reference |
| `docs/reference/runtime.md` | Update execution phase docs |
| `docs/advanced/semantics.md` | Update formal rules |

## Adding a new task

1. Declare the task signature in a `.agent` file:

    ```agentlang
    task my_task(input: String) -> Obj{result: String} {}
    ```

2. Add a handler function in `agentlang/stdlib.py`:

    ```python
    def my_task_handler(args: dict[str, Any], agent: str | None) -> dict[str, str]:
        return {"result": f"handled: {args['input']}"}
    ```

3. Register it in `default_task_registry()`:

    ```python
    "my_task": my_task_handler,
    ```

4. Add or update an example in `examples/`.

5. Document the task in `docs/reference/examples.md` and `docs/reference/adapters.md` if it has live behavior.

## Adapter changes

The OpenAI adapter lives in `agentlang/adapters/openai.py`. Tool adapters (web search) are in `agentlang/adapters/tools.py`.

Guidelines:

- Keep adapter modules dependency-light.
- Wrap all external errors with clear, user-readable messages.
- Never log or surface secrets in error messages or stack traces.

## Style guidelines

- 4-space indentation, explicit type hints on public functions.
- `snake_case` for functions/variables, `PascalCase` for classes.
- Keep changes small and composable — prefer explicit errors over silent fallbacks.
- Keep docs synchronized with behavior changes.
- Follow Conventional Commit style: `feat:`, `fix:`, `docs:` prefixes with an imperative, concise subject.

## Commit and PR checklist

Before opening a PR:

- [ ] `py_compile` passes on all core modules
- [ ] At least one happy-path example runs correctly
- [ ] At least one failure-path example runs correctly (if relevant)
- [ ] Docs updated for any DSL/runtime/adapter changes
- [ ] No secrets in source, examples, or docs
