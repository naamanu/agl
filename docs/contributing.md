# Contributing

This document describes how to extend AgentLang safely.

## Development Workflow

1. Edit source files.
2. Run syntax checks:

```bash
python -m py_compile main.py agentlang/*.py agentlang/adapters/*.py
```

3. Run representative examples:

```bash
python main.py examples/blog.agent blog_post --input '{"topic":"agent memory patterns"}'
python main.py examples/reliability.agent resilient_brief --input '{"topic":"api-status","fail_count":5}'
```

## Extending The Language

When adding a syntax feature, update all of:

1. AST (`agentlang/ast.py`)
2. lexer tokens/keywords (`agentlang/lexer.py`)
3. parser (`agentlang/parser.py`)
4. type checker (`agentlang/checker.py`)
5. runtime execution semantics (`agentlang/runtime.py`)
6. docs:
   - `docs/language-reference.md`
   - `docs/runtime-and-typing.md`
   - `docs/semantics.md`

## Adding A New Task

1. Declare task signature in `.agent` source.
2. Add runtime handler in `agentlang/stdlib.py`.
3. Register handler in `default_task_registry(...)`.
4. Add or update example in `examples/`.
5. Document behavior in `docs/examples.md` (and `docs/adapters.md` if live-backed).

## Adapter Changes

OpenAI adapter lives in:

- `agentlang/adapters/openai.py`

Tool adapters live in:

- `agentlang/adapters/tools.py`

Guidelines:

- keep adapter modules dependency-light
- wrap external errors with clear messages
- avoid leaking secrets in logs or docs

## Style Guidelines

- keep changes small and composable
- prefer explicit errors over silent fallbacks
- preserve deterministic mock behavior for local development
- keep docs synchronized with behavior changes

