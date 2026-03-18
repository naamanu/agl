# Observability

AgentLang provides structured execution tracing via the `ExecutionContext` and the `--output-trace` CLI flag.

## ExecutionContext

The `ExecutionContext` (defined in `agentlang/context.py`) records structured events throughout pipeline execution. It tracks:

- Task lifecycle (start, end, error)
- Parallel block boundaries
- Retry attempts
- Pipeline-calls-pipeline events

## Enabling tracing

Pass `--output-trace PATH` to write the trace after execution:

```bash
python main.py examples/showcase_all_features.agent produce \
  --input '{"topic":"AI safety"}' \
  --output-trace trace.json
```

## Trace JSON format

The trace file contains a JSON object with a `trace` array of events:

```json
{
  "trace": [
    {
      "type": "task_start",
      "task": "research",
      "args": {"topic": "AI safety — technical deep-dive"},
      "timestamp": 1710400000.123,
      "id": "task:research:1"
    },
    {
      "type": "task_end",
      "task": "research",
      "result": {"notes": "...", "sources": ["..."]},
      "duration_s": 0.0042,
      "timestamp": 1710400000.127,
      "id": "task:research:1"
    },
    {
      "type": "parallel_start",
      "branch_count": 2,
      "timestamp": 1710400000.128
    },
    {
      "type": "parallel_end",
      "branch_count": 2,
      "timestamp": 1710400000.135
    },
    {
      "type": "retry",
      "task": "risky_enrich",
      "attempt": 1,
      "error": "RuntimeError: Enrichment service unavailable",
      "timestamp": 1710400000.140,
      "id": "task:risky_enrich:3"
    },
    {
      "type": "task_error",
      "task": "risky_enrich",
      "error": "RuntimeError: Enrichment service unavailable",
      "timestamp": 1710400000.141,
      "id": "task:risky_enrich:3"
    },
    {
      "type": "pipeline_call",
      "pipeline": "research_and_draft",
      "args": {"topic": "AI safety", "angle": "technical deep-dive"},
      "timestamp": 1710400000.128
    }
  ]
}
```

## Event types

| Type | Fields | Description |
|---|---|---|
| `task_start` | `task`, `args`, `timestamp`, `id` | Recorded before a task handler is invoked |
| `task_end` | `task`, `result`, `duration_s`, `timestamp`, `id` | Recorded after successful task completion |
| `task_error` | `task`, `error`, `timestamp`, `id` | Recorded when a task handler raises an exception |
| `parallel_start` | `branch_count`, `timestamp` | Recorded at the start of a parallel block |
| `parallel_end` | `branch_count`, `timestamp` | Recorded after all parallel branches complete |
| `retry` | `task`, `attempt`, `error`, `timestamp`, `id` | Recorded on each retry attempt |
| `pipeline_call` | `pipeline`, `args`, `timestamp` | Recorded when one pipeline calls another |

## Correlation IDs

Each call to `record_task_start` returns a unique correlation key (e.g. `task:research:1`). All subsequent `retry`, `task_end`, and `task_error` events for that invocation carry the same `id` field. This makes concurrent same-name tasks distinguishable — for example, two parallel branches both running `research` will have different `id` values, so their events can be paired unambiguously.

## Use cases

- **Debugging:** Trace task failures and see exact arguments/errors
- **Performance:** Use `duration_s` to identify slow tasks
- **Auditing:** Record which tasks ran, in what order, with what inputs
- **Visualization:** Parse the JSON trace to build execution timelines

## Next: [Language Reference](../reference/language.md)
