# CLI reference

The native executable is `agl`. During development, invoke it with `cargo run --`; after installation, use `agl` directly. Mock mode is deterministic and does not require credentials.

## Run, check, and inspect a source file

```text
agl <source.agent> [pipeline-or-workflow] [options]
```

The source file is parsed, imported modules are loaded, diagnostics are emitted, and the program is statically checked before execution. A pipeline/workflow name is required for execution, `--lower`, and `--effects`; it is not required for `--check`, `--test`, `--format`, `--docs`, `--api`, or `--summary`.

| Flag | Description |
|---|---|
| `--input JSON` | Object containing pipeline inputs; default `{}`. |
| `--check` | Parse, load imports, analyze, and type-check without executing. |
| `--test` | Run every `test` block in the source file. |
| `--format` | Print canonical AGL source and exit. |
| `--lower` | Print the selected pipeline/workflow after workflow lowering. |
| `--effects` | Print inferred transitive effects as JSON. |
| `--summary` | Print effects, external writes, and approval boundaries as JSON. |
| `--docs PATH` | Write generated Markdown API documentation. |
| `--api PATH` | Write a machine-readable public API snapshot. |
| `--deployment PATH` | Load provider-neutral agent bindings from JSON. |
| `--policy PATH` | Enforce effect, tool, network, and filesystem policy. |
| `--adapter mock\|openai\|live\|anthropic` | Select the task/model adapter. `live` is an OpenAI compatibility alias. |
| `--trace-live` | Emit model/tool trace lines to stderr. |
| `--output-trace PATH` | Write the structured execution trace to a JSON file. |
| `--event-store PATH` | Persist durable events and checkpoints in the local SQLite-backed store. |
| `--execution-id ID` | Choose a stable durable execution identity; requires `--event-store`. |
| `--resume` | Replay checkpoints for the selected durable execution; requires `--event-store`. |
| `--approval NAME=BOOL` | Supply an approval or rejection; may be repeated. |
| `--eval NAME` | Run a declared dataset evaluation. |
| `--update-baseline` | Replace the selected evaluation baseline; requires `--eval`. |

Examples:

```bash
cargo run -- examples/blog.agent blog_post \
  --input '{"topic":"agent memory patterns"}'
cargo run -- examples/showcase_all_features.agent --check
cargo run -- examples/showcase_all_features.agent --test
cargo run -- examples/multiagent_blog.agent publish_topic_blog --lower
cargo run -- examples/blog.agent blog_post --effects
cargo run -- examples/blog.agent blog_post --summary
cargo run -- examples/blog.agent blog_post --output-trace trace.json
```

Input keys and values are checked against the declared pipeline signature before execution. Errors use stable diagnostic codes and include source locations where available.

## REPL

```bash
cargo run -- repl --adapter mock
```

The REPL is stateful. Load a source file, then run a named pipeline:

```text
AGL REPL (adapter=mock). Type 'help' for commands, 'exit' to quit.
> load examples/blog.agent
Loaded 'examples/blog.agent': 2 agents, 2 tasks, 1 pipelines.
> run blog_post {"topic":"agent memory"}
{
  "result": "[writer] Draft article:\n[planner] key points for 'agent memory'"
}
```

Commands are `load <path>`, `run <name> [json]`, `lower <name>`, `list`, `clear`, `help`, and `exit`. REPL options also accept `--deployment` and `--trace-live`.

## Tooling subcommands

These commands are intercepted by the native binary and do not use the source-file CLI:

```bash
agl protocol                 # JSON-lines compiler protocol over stdin/stdout
agl lsp                      # LSP server over stdio
agl completions bash         # bash completion script (also zsh and fish)
agl package lock agl.json    # write agl.lock
agl api-compare old.json new.json
```

The protocol accepts structured parse/check/format/lower/effects requests. The LSP server supports diagnostics, completion, hover, definition, references, rename, and formatting.

## Live adapters and environment

```bash
export OPENAI_API_KEY="sk-..."
cargo run -- examples/incident_runbook.agent respond_to_incident \
  --adapter openai --trace-live \
  --input '{"incident":"database failover drill"}'

export ANTHROPIC_API_KEY="sk-ant-..."
cargo run -- examples/incident_runbook.agent respond_to_incident \
  --adapter anthropic --trace-live \
  --input '{"incident":"database failover drill"}'
```

| Variable | Meaning |
|---|---|
| `OPENAI_API_KEY` | Credential for the OpenAI adapter. |
| `ANTHROPIC_API_KEY` | Credential for the Anthropic adapter. |
| `AGL_OPENAI_MODEL` | Optional OpenAI model override. |
| `AGL_ANTHROPIC_MODEL` | Optional Anthropic model override. |

Never put credentials in `.agent` files or deployment/policy JSON.

## Exit status

`0` means the requested operation succeeded. `1` means a source, type, runtime, provider, policy, or evaluation failure. `2` means invalid CLI arguments or an unsupported completion shell.
