# CLI Reference

AgentLang is invoked via `main.py`. The default command executes a named pipeline or workflow from a file, and `repl` starts an interactive session.

## `run` — execute a pipeline or workflow

```
cargo run -- <source> <pipeline> [options]
```

### Positional arguments

| Argument | Description |
|---|---|
| `source` | Path to a `.agent` file |
| `pipeline` | Name of the pipeline or workflow to execute |

### Options

| Flag | Default | Description |
|---|---|---|
| `--input '<json>'` | `{}` | JSON object mapped to pipeline input params |
| `--workers N` | `8` | Max threads for `parallel` blocks. Must be `>= 1`. |
| `--adapter mock\|live\|anthropic` | `mock` | Task execution mode |
| `--lower` | off | Print the lowered pipeline IR for the selected pipeline or workflow and exit |
| `--effects` | off | Print inferred transitive pipeline effects as JSON and exit |
| `--deployment PATH` | off | Validate and apply provider-neutral agent bindings from JSON |
| `--eval NAME` | off | Run a declared dataset evaluation and emit a JSON report |
| `--update-baseline` | off | Replace the selected evaluation's baseline with the current report |
| `--trace-live` | off | Emit live model/tool tracing to `stderr` when running with `--adapter live` or `--adapter anthropic` |
| `--output-trace PATH` | off | Write a structured JSON execution trace to `PATH` after execution |
| `--plugin MODULE` | — | Load a plugin module (Python file path or dotted module name). May be repeated. |
| `--test` | off | Run all `test` blocks in the source file instead of executing a pipeline |

### Examples

Run in mock mode (default):

```bash
cargo run -- examples/blog.agent blog_post \
  --input '{"topic":"agent memory patterns"}'
```

```json
{
  "result": "[writer] Draft article:\n[planner] key points for 'agent memory patterns'"
}
```

Run in live mode (OpenAI):

```bash
export OPENAI_API_KEY="sk-..."

cargo run -- examples/blog.agent blog_post \
  --adapter live \
  --input '{"topic":"agent memory patterns"}'
```

Run in anthropic mode (Claude):

```bash
export ANTHROPIC_API_KEY="sk-ant-..."

cargo run -- examples/blog.agent blog_post \
  --adapter anthropic \
  --input '{"topic":"agent memory patterns"}'
```

Inspect lowered workflow IR:

```bash
cargo run -- examples/multiagent_blog.agent publish_topic_blog --lower
```

Trace live model and tool activity:

```bash
cargo run -- examples/incident_runbook.agent respond_to_incident \
  --adapter live \
  --trace-live \
  --input '{"incident":"database failover drill"}'
```

Limit parallel workers:

```bash
cargo run -- examples/compare.agent compare_options \
  --input '{"query":"vector database"}' \
  --workers 2
```

Write an execution trace to a file:

```bash
cargo run -- examples/showcase_all_features.agent produce \
  --input '{"topic":"AI safety"}' \
  --output-trace trace.json
```

Run with a plugin:

```bash
cargo run -- examples/showcase_all_features.agent produce \
  --input '{"topic":"AI safety"}' \
  --plugin examples/showcase_plugin.py
```

Run test blocks:

```bash
cargo run -- examples/showcase_all_features.agent --test
```

### Input validation

`--input` is validated before execution:

```bash
# Missing required input
cargo run -- examples/blog.agent blog_post --input '{}'
Execution error: Pipeline 'blog_post' missing inputs: ['topic'].

# Unknown extra key
cargo run -- examples/blog.agent blog_post \
  --input '{"topic":"x","extra":"bad"}'
Execution error: Pipeline 'blog_post' received unknown inputs: ['extra'].

# Wrong pipeline/workflow name
cargo run -- examples/blog.agent nonexistent_pipeline --input '{}'
Execution error: Unknown pipeline 'nonexistent_pipeline'.
```

Values are type-checked against declared DSL types. Booleans in JSON are checked against `Bool`; integers and floats are `Number`; `true`/`false` are **not** accepted as `Number`.

---

## `repl` — interactive session

```
cargo run -- repl [--adapter mock|live|anthropic]
```

Starts an interactive prompt for exploring pipelines and workflows.

```bash
cargo run -- repl --adapter mock
```

```
AgentLang REPL (adapter=mock). Type 'exit' to quit.
>
```

---

## Environment variables

These are read at startup and affect live/anthropic mode behavior:

| Variable | Default | Description |
|---|---|---|
| `OPENAI_API_KEY` | — | Required for `--adapter openai` (`live` is an alias) |
| `ANTHROPIC_API_KEY` | — | Required for `--adapter anthropic` |
| `AGL_OPENAI_MODEL` | `gpt-5.6-sol` | Global OpenAI model override |
| `AGL_ANTHROPIC_MODEL` | provider default | Global Anthropic model override |

!!! warning "Never commit secrets"
    Use environment variables or a shell profile for `OPENAI_API_KEY` and `ANTHROPIC_API_KEY`. Do not hardcode keys in `.agent` files or source code.

---

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Pipeline or workflow executed successfully; all tests passed (with `--test`) |
| `1` | Runtime error (bad input, failed task, missing key); test failure (with `--test`) |
| `2` | Argument parse error (bad CLI flags) |
