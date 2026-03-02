# CLI Reference

AgentLang is invoked via `main.py`. There are two subcommands: `run` executes a pipeline from a file, `repl` starts an interactive session.

## `run` — execute a pipeline

```
python main.py <source> <pipeline> [options]
```

### Positional arguments

| Argument | Description |
|---|---|
| `source` | Path to a `.agent` file |
| `pipeline` | Name of the pipeline to execute |

### Options

| Flag | Default | Description |
|---|---|---|
| `--input '<json>'` | `{}` | JSON object mapped to pipeline input params |
| `--workers N` | `8` | Max threads for `parallel` blocks. Must be `>= 1`. |
| `--adapter mock\|live` | `mock` | Task execution mode |

### Examples

Run in mock mode (default):

```bash
python main.py examples/blog.agent blog_post \
  --input '{"topic":"agent memory patterns"}'
```

```json
{
  "result": "[writer] Draft article:\n[planner] key points for 'agent memory patterns'"
}
```

Run in live mode:

```bash
export OPENAI_API_KEY="sk-..."

python main.py examples/blog.agent blog_post \
  --adapter live \
  --input '{"topic":"agent memory patterns"}'
```

Limit parallel workers:

```bash
python main.py examples/compare.agent compare_options \
  --input '{"query":"vector database"}' \
  --workers 2
```

### Input validation

`--input` is validated before execution:

```bash
# Missing required input
python main.py examples/blog.agent blog_post --input '{}'
Execution error: Pipeline 'blog_post' missing inputs: ['topic'].

# Unknown extra key
python main.py examples/blog.agent blog_post \
  --input '{"topic":"x","extra":"bad"}'
Execution error: Pipeline 'blog_post' received unknown inputs: ['extra'].

# Wrong pipeline name
python main.py examples/blog.agent nonexistent_pipeline --input '{}'
Execution error: Unknown pipeline 'nonexistent_pipeline'.
```

Values are type-checked against declared DSL types. Booleans in JSON are checked against `Bool`; integers and floats are `Number`; `true`/`false` are **not** accepted as `Number`.

---

## `repl` — interactive session

```
python main.py repl [--adapter mock|live]
```

Starts an interactive prompt for exploring pipelines.

```bash
python main.py repl --adapter mock
```

```
AgentLang REPL (adapter=mock). Type 'exit' to quit.
>
```

---

## Environment variables

These are read at startup and affect live mode behavior:

| Variable | Default | Description |
|---|---|---|
| `OPENAI_API_KEY` | — | Required for `--adapter live` |
| `OPENAI_BASE_URL` | `https://api.openai.com/v1` | Override API base URL |
| `AGENTLANG_ADAPTER` | `mock` | Default adapter if `--adapter` is not passed |
| `AGENTLANG_DEFAULT_MODEL` | — | Fallback model when no `by agent` binding exists |
| `AGENTLANG_WEB_RESULTS` | `5` | Number of DuckDuckGo results to inject for `research` |
| `AGENTLANG_HTTP_TIMEOUT_S` | `20` | HTTP timeout in seconds for live adapter calls |

!!! warning "Never commit secrets"
    Use environment variables or a shell profile for `OPENAI_API_KEY`. Do not hardcode keys in `.agent` files or source code.

---

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Pipeline executed successfully |
| `1` | Runtime error (bad input, failed task, missing key) |
| `2` | Argument parse error (bad CLI flags) |
