# Retries, invocation identity, and budgets

AGL 0.4 treats a retry as another attempt at one logical invocation. Every invocation receives an execution ID, invocation ID, and idempotency key. Those values remain stable across attempts and are emitted in `task_attempt` and `task_retry` trace events. A `keyed_by` task uses the declared string argument as its adapter-visible idempotency key.

```agentlang
let receipt = publish(order_id)
  retries 3
  backoff { initial_ms: 100, max_ms: 2000, multiplier: 2, jitter: 0.2 };
```

Backoff is capped exponential delay. Jitter is a fraction from zero to one. Production contexts sleep for the computed delay; deterministic test/evaluation contexts use a seed and record the same delay without sleeping.

A pipeline can place a scoped ceiling on resources:

```agentlang
pipeline release(order_id: String) -> String
  budget {
    time_ms: 10000,
    tokens: 20000,
    cost_usd: 0.50,
    tool_calls: 8,
    retries: 3,
    concurrency: 2
  } {
  let receipt = publish(order_id);
  return receipt;
}
```

Parallel branches atomically share their enclosing pipeline's counters. A nested pipeline starts a nested scope. Runtime crossings reserve calls and retries before starting work and account reported provider tokens and cost after a response. Exceeding any ceiling produces a structured `Failure` with `kind = "budget"` and the resource name in `operation`; it is also visible in the trace.

Native handlers that can report usage should use `Registry::register_contextual` and return `TaskOutput`. Legacy handlers remain supported and report zero usage.
