# Retry & Fallback

AgentLang has first-class syntax for handling transient task failures. You can declare a retry budget and a fallback policy directly in the run statement — no try/except in Python, no wrapper logic.

## Syntax

```agentlang
let x = run task_name
  with { key: expr }
  by agent_name
  retries N
  on_fail abort;
```

```agentlang
let x = run task_name
  with { key: expr }
  by agent_name
  retries N
  on_fail use <fallback_expr>;
```

All clauses are optional. Defaults: `retries 0`, `on_fail abort`.

## Retry budget

`retries N` means the task will be attempted up to `N + 1` times total.

| `retries N` | Total attempts |
|---|---|
| `retries 0` (default) | 1 |
| `retries 1` | 2 |
| `retries 2` | 3 |

## Failure policies

### `on_fail abort` (default)

If the task exhausts its retry budget, a runtime error is raised and the pipeline stops:

```
Execution error: Task 'flaky_fetch' failed after 3 attempts.
```

### `on_fail use <expr>`

If the task fails after all retries, the fallback expression is evaluated and bound to `x` instead:

```agentlang
let f = run flaky_fetch
  with { key: topic, failures_before_success: fail_count }
  by ops
  retries 2
  on_fail use { data: "fallback for " + topic };
```

!!! warning "Type must match"
    The fallback expression type must match the task's declared return type. This is enforced statically by the type checker.

## Full example

From `examples/reliability.agent`:

```agentlang
pipeline resilient_brief(topic: String, fail_count: Number) -> String {
  let f = run flaky_fetch
    with { key: topic, failures_before_success: fail_count }
    by ops
    retries 2
    on_fail use { data: "fallback for " + topic };

  if f.data == "fallback for " + topic {
    let d = run draft with { notes: "Fallback path: " + f.data } by ops;
    return d.article;
  } else {
    let d = run draft with { notes: "Fresh path: " + f.data } by ops;
    return d.article;
  }
}
```

Run it — succeeds within retry budget:

```bash
python main.py examples/reliability.agent resilient_brief \
  --input '{"topic":"api-status","fail_count":1}'
```

```json
{
  "result": "[ops] Draft article:\nFresh path: [ops] fetched payload for api-status"
}
```

Run it — exhausts retries, uses fallback:

```bash
python main.py examples/reliability.agent resilient_brief \
  --input '{"topic":"api-status","fail_count":5}'
```

```json
{
  "result": "[ops] Draft article:\nFallback path: fallback for api-status"
}
```

## Detecting fallback use

After `on_fail use`, the bound variable holds the fallback value. You can inspect it with `==` to detect which path was taken:

```agentlang
if f.data == "fallback for " + topic {
  -- fallback path
} else {
  -- success path
}
```

This is a common pattern — compute a sentinel fallback value and check for it downstream.

## Next: [Error Handling](error-handling.md)
