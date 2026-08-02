# Structured concurrency

AGL 0.5 owns concurrent work as a scope. Ordinary `parallel`, ordered `parallel map`, and `race` join their children before leaving the statement. `race` sends cooperative cancellation to losers.

```agentlang
let summaries = parallel map document in documents max_concurrency 4 fail_fast {
  let summary = summarize(document);
};

let answer = race {
  let primary = ask_primary(question);
  let backup = ask_backup(question);
};
```

`parallel map` preserves input ordering regardless of completion order. `fail_fast` stops scheduling later chunks after a failure; `collect_all` completes the dataset and aggregates failures. Direct task calls keep cancellation and durable identity unambiguous in 0.5.

Task/tool declarations can share a scheduler contract with `concurrency_group api concurrency_limit 4 rate_limit 10`. Pipeline `budget { concurrency: ... }` still caps a local scope, while named groups coordinate calls across scopes.
