# Quick Start

This guide gets you from zero to a running AgentLang pipeline or workflow in five minutes.

## Prerequisites

- Rust 1.94.1+ (the repository pins the supported toolchain)
- This repository cloned locally

Verify your Rust toolchain:

```bash
rustc --version
# rustc 1.94.1 or newer
```

## 1. Verify the install

Build and test the native implementation to confirm everything is wired up:

```bash
cargo test --locked
```

No output means success.

## 2. Run your first pipeline

The `blog.agent` example defines a simple two-step pipeline: research a topic, then draft an article.

```bash
cargo run -- examples/blog.agent blog_post \
  --input '{"topic":"agent memory patterns"}'
```

Output:

```json
{
  "result": "[writer] Draft article:\n[planner] key points for 'agent memory patterns'"
}
```

> [!NOTE]
> **Mock mode**
>
> By default, pipelines run in **mock mode** — all task handlers are deterministic local functions that return structured placeholders. No API key required.

## 3. Run your first workflow

The higher-level `workflow` surface is the recommended authoring model for multi-agent handoffs and review loops.

```bash
cargo run -- examples/multiagent_blog.agent publish_topic_blog \
  --input '{"topic":"agent memory systems"}'
```

Lower it to explicit pipeline IR:

```bash
cargo run -- examples/multiagent_blog.agent publish_topic_blog --lower
```

This shows the generated `run`, `while`, and `break` structure that the runtime actually executes.

## 4. Try parallel execution

The `compare.agent` pipeline runs two research tasks in parallel, then merges results:

```bash
cargo run -- examples/compare.agent compare_options \
  --input '{"query":"vector database"}'
```

Output:

```json
{
  "result": "[reviewer] Option A vs B\nA: [planner] key points for 'vector database option A'\nB: [planner] key points for 'vector database option B'"
}
```

The two `research` calls ran concurrently — you'll see both results merged before the `compare` step executes.

## 5. See retry and fallback

The `reliability.agent` pipeline uses `retries` and `on_fail use` to handle transient failures gracefully.

Run with a low failure count (succeeds before fallback kicks in):

```bash
cargo run -- examples/reliability.agent resilient_brief \
  --input '{"topic":"api-status","fail_count":1}'
```

```json
{
  "result": "[ops] Draft article:\nFresh path: [ops] fetched payload for api-status"
}
```

Force the fallback by exceeding the retry budget:

```bash
cargo run -- examples/reliability.agent resilient_brief \
  --input '{"topic":"api-status","fail_count":5}'
```

```json
{
  "result": "[ops] Draft article:\nFallback path: fallback for api-status"
}
```

> [!TIP]
> **What just happened?**
>
> `fail_count: 5` exceeds the `retries 2` budget in the pipeline, so the `on_fail use` clause provides a fallback value. The `if/else` block then routes execution based on whether the fallback was used.

## 6. Start the REPL

For interactive exploration:

```bash
cargo run -- repl --adapter mock
```

```
AGL REPL (adapter=mock). Type 'help' for commands, 'exit' to quit.
> load examples/blog.agent
Loaded 'examples/blog.agent': 2 agents, 2 tasks, 1 pipelines.
> run blog_post {"topic":"agent memory"}
{
  "result": "[writer] Draft article:\n[planner] key points for 'agent memory'"
}
> exit
```

The REPL is stateful: use `load <path>`, then `run <pipeline-or-workflow> [json]`. `lower`, `list`, `clear`, and `help` are also available. Errors are printed and the session continues.

## 7. Trace a live run

When debugging live agent behavior, enable tracing. This works with both `--adapter live` (OpenAI) and `--adapter anthropic` (Claude):

```bash
cargo run -- examples/incident_runbook.agent respond_to_incident \
  --adapter live \
  --trace-live \
  --input '{"incident":"database failover drill"}'
```

Or with Anthropic/Claude:

```bash
cargo run -- examples/incident_runbook.agent respond_to_incident \
  --adapter anthropic \
  --trace-live \
  --input '{"incident":"database failover drill"}'
```

Trace lines are printed to `stderr` and show:

- agent task start/end
- LLM request mode (OpenAI or Anthropic)
- tool calls and tool results
- final structured task outputs

## Next steps

- **Understand the language** → [Agents](../concepts/agents.md), [Tasks](../concepts/tasks.md), [Workflows](../concepts/workflows.md), [Pipelines](../concepts/pipelines.md)
- **Write your own pipeline** → [Your First Pipeline](first-pipeline.md)
- **Connect to OpenAI or Anthropic** → [Adapters](../reference/adapters.md)
