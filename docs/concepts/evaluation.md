# Deterministic evaluation

AGL 0.4 keeps ordinary `test` blocks fast and deterministic and adds dataset-driven `eval` declarations for quality and operational regressions.

```agentlang
eval answer_regression {
  pipeline: answer,
  dataset: "eval/answers.jsonl",
  trials: 5,
  baseline: "eval/answers.baseline.json",
  assert_schema: true,
  assert_expected: true,
  max_latency_ms: 2000,
  max_cost_usd: 0.02,
  semantic_grader: grade_answer
}
```

The dataset is JSON Lines. Each row contains an `input` object and may contain an `expected` value. `assert_expected` is the built-in exact predicate. A semantic grader names a `Bool`-returning pipeline and receives `actual` and `expected` inputs.

Run an evaluation with `agl program.agent --eval answer_regression`. A report contains pass rate plus mean, p50, p95, minimum, and maximum latency and cost. Trials use deterministic seeds, so mock handlers and retry jitter are reproducible. `Registry::from_trace` creates replay handlers from recorded task results without contacting a provider.

Baseline files declare `min_pass_rate`, `max_mean_latency_ms`, and `max_mean_cost_usd`. Any regression makes the command fail, which makes it suitable for CI. Use `--update-baseline` only when intentionally accepting the current report.
