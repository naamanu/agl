# Structured parallel execution

AGL 0.6 has three structured-concurrency forms. Every child is owned by its parent scope and is joined before the enclosing statement continues.

## Parallel branches

```agentlang
parallel max_concurrency 2 {
  let a = run research with { topic: "angle A" } by planner;
  let b = run research with { topic: "angle B" } by planner;
} join;
```

Branches receive a snapshot of the environment. Their bindings become available after `join`; one branch cannot observe another branch's writes. `max_concurrency` bounds the number of active branches in that block. There is no global `--workers` flag: use per-block limits, task `concurrency_group` limits, rate limits, and pipeline budgets.

## Ordered parallel map

```agentlang
let answers = parallel map item in questions {
  let answer = run answer with { question: item } by researcher;
} max_concurrency 4 failure collect_all;
```

Results preserve input order even when branches finish out of order. `failure fail_fast` stops scheduling later chunks after a failure; `failure collect_all` completes the dataset and reports the aggregate failure.

## Race

```agentlang
let answer = race {
  let fast = run answer_fast with { question: question } by local;
  let deep = run answer_deep with { question: question } by remote;
};
```

The first successful branch wins. Losing branches receive a cooperative cancellation token and are joined before the race statement returns. If every branch fails, the race returns an aggregate failure.

## Cancellation and limits

Contextual Rust handlers receive `Invocation.cancellation` and should check it during network, tool, and long-running work. Runtime limits include task/tool concurrency groups, rate limits, pipeline concurrency budgets, and per-map/per-block bounds. These limits are distinct from provider-side quotas.

See [Structured Concurrency](structured-concurrency.md), [Retries & Budgets](retries-and-budgets.md), and [Runtime & Typing](../reference/runtime.md).
