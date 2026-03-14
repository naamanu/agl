# Parallel Execution

AgentLang has first-class syntax for running tasks concurrently. The `parallel { } join` block executes multiple run statements at the same time and makes all their results available after the join.

## Syntax

```agentlang
parallel [ max_concurrency <N> ] {
  let a = run task_name with { ... } by agent;
  let b = run task_name with { ... } by agent;
} join;
```

## Example

From `examples/compare.agent`:

```agentlang
pipeline compare_options(query: String) -> String {
  parallel {
    let a = run research with { topic: query + " option A" } by planner;
    let b = run research with { topic: query + " option B" } by planner;
  } join;

  let c = run compare with { note_a: a.notes, note_b: b.notes } by reviewer;
  return c.decision;
}
```

```bash
python main.py examples/compare.agent compare_options \
  --input '{"query":"vector database"}'
```

```json
{
  "result": "[reviewer] Option A vs B\nA: [planner] key points for 'vector database option A'\nB: [planner] key points for 'vector database option B'"
}
```

The two `research` calls ran at the same time. After `join`, both `a` and `b` are available for the downstream `compare` step.

## How it works

1. The runtime takes a **snapshot** of the current environment at the start of the block.
2. Each branch runs in its own thread via `ThreadPoolExecutor`.
3. After all futures complete, branch outputs are **merged** into the main environment.
4. Execution continues sequentially after `join`.

## Rules

- Only `let ... = run ...;` statements are permitted inside a `parallel` block. `if`, `return`, and nested `parallel` blocks are not allowed.
- All branch bindings become available after `join`.
- Branches share a read-only snapshot of the environment — they cannot see each other's outputs until after `join`.

## Controlling concurrency

There are two levels of concurrency control:

**Global:** The `--workers` flag sets the maximum number of concurrent threads across all parallel blocks:

```bash
python main.py examples/compare.agent compare_options \
  --input '{"query":"vector database"}' \
  --workers 4
```

The default is `8`. The value must be `>= 1`.

**Per-block:** The optional `max_concurrency` clause limits how many branches run simultaneously within a single parallel block:

```agentlang
parallel max_concurrency 2 {
  let a = run research with { topic: "angle A" } by planner;
  let b = run research with { topic: "angle B" } by planner;
  let c = run research with { topic: "angle C" } by planner;
} join;
```

With `max_concurrency 2`, at most 2 of the 3 branches execute at the same time. This is useful for rate-limiting API calls or controlling resource usage within a specific block without affecting other parallel blocks.

## Thread safety

Task handlers in `agentlang/stdlib.py` that share mutable state use a `threading.Lock`. If you write a custom task handler that mutates shared state, protect it with a lock.

## Next: [Retry & Fallback](retry.md)
