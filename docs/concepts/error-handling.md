# Error Handling

AgentLang provides `try`/`catch` blocks for recovering from runtime errors within a pipeline.

## Syntax

```agentlang
try {
  <statements>
} catch <error_var> {
  <statements>
}
```

## How it works

1. The `try` block executes its statements in order.
2. If any statement raises a runtime error (task failure, assertion, etc.), execution immediately jumps to the `catch` block.
3. The error variable is bound as a `String` containing the error message.
4. If the `try` block completes without error, the `catch` block is skipped entirely.

## Scope rules

- The error variable (`<error_var>`) is only in scope inside the `catch` block.
- Variables bound before the `try` block can be re-bound inside either the `try` or `catch` block. The merged environment after the block reflects whichever branch executed.

## Example

From `examples/showcase_all_features.agent`:

```agentlang
try {
  let enrichment = run risky_enrich with { topic: topic };
  let merged = run merge_drafts with {
    draft_a: merged.article,
    draft_b: enrichment.extra,
    word_count_a: merged.total_words,
    word_count_b: 100
  };
} catch err {
  -- gracefully degrade: use fallback enrichment
  let fallback = run fallback_enrich with { query: err };
  let merged = run merge_drafts with {
    draft_a: merged.article,
    draft_b: fallback.extra,
    word_count_a: merged.total_words,
    word_count_b: 50
  };
}
```

Here, `risky_enrich` may fail at runtime. If it does, the `catch` block uses `fallback_enrich` with the error message as input, and re-binds `merged` so downstream code sees the fallback result.

## When to use try/catch vs retries/on_fail

| Mechanism | Use when |
|---|---|
| `retries N` | The same task might succeed on retry (transient failures) |
| `on_fail use <expr>` | You have a static fallback value for a single task |
| `try`/`catch` | You need to run different logic on failure, or the recovery involves multiple steps |

`try`/`catch` is more powerful — it wraps an arbitrary block of statements, not just a single task call. Use `retries`/`on_fail` for simple cases and `try`/`catch` for complex error recovery.

## Next: [Testing](testing.md)
