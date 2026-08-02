# AGL 0.4 language specification

Status: draft normative

Language version: `0.4`

AGL 0.4 extends [AGL 0.3](agl-0.3.md) with operational effect contracts and retry-safe idempotency. Unchanged behavior is inherited from 0.3.

## 1. Effects

Tasks and tools may declare the capabilities their host implementation can exercise:

```agl
task fetch(url: String) -> String
  effects [network, external_read]
  idempotency idempotent {}
```

The built-in effect names are `model`, `network`, `filesystem`, `external_read`, `external_write`, `secret`, and `human`. Other identifiers are permitted as application capabilities. A custom effect cannot replace or suppress a built-in effect required by a safety rule.

Calling an agent task contributes `model`. Calling through an agent also contributes the effects of every tool available to that agent. Pipeline calls transitively contribute the callee's inferred effects. Inference reaches a fixed point across the pipeline call graph.

A pipeline may omit its effect clause and expose its inferred set, or declare an allowed ceiling:

```agl
pipeline report(url: String) -> String
  effects [network, external_read] {
  let content = fetch(url);
  return content;
}
```

Every inferred effect must belong to the declared set. A declaration may contain additional effects so deployment policy can reserve capabilities for dynamically selected handlers. Canonical lowering prints declared effects in lexical order.

## 2. Idempotency

Task and tool declarations accept one idempotency contract:

```text
idempotency pure
idempotency idempotent
idempotency keyed_by parameter
idempotency non_idempotent
```

- `pure` permits no declared effects.
- `idempotent` promises that repeated identical calls have the same externally visible effect.
- `keyed_by parameter` requires a declared `String` parameter whose value is the host idempotency key.
- `non_idempotent` explicitly forbids automatic retry.
- An omitted declaration is `unspecified`.

The checker rejects retries of `non_idempotent` tasks. A task with `external_write` is retryable only when pure, idempotent, or keyed. The same rule applies when an agent can invoke an external-write tool during a retried task.

These checks are static promises: hosts must implement the declared idempotency behavior. Each logical invocation receives stable execution, invocation, and idempotency IDs. Attempts of the same invocation reuse those values.

An optional `backoff { initial_ms, max_ms, multiplier, jitter }` clause specifies bounded exponential retry delay. `multiplier` is at least one and `jitter` is in `[0, 1]`. Jitter is derived from the execution seed, invocation ID, and attempt so deterministic contexts reproduce it exactly.

## 3. Agent requirements and deployment

An agent may declare provider-neutral requirements:

```agl
agent researcher {
  tools: [search],
  requires: [reasoning, tool_calling],
  min_context: 100000,
  max_latency_ms: 5000,
  quality: "high"
}
```

`requires` is an application-defined capability set. The remaining constraints are optional. Quality tiers are ordered `low < medium < high < frontier`.

Provider, model, endpoint, reasoning effort, supplied capabilities, context window, expected latency, and quality live in an external deployment binding. When a deployment is applied, every source agent must have exactly one known binding. The binding must match the selected provider and satisfy every source constraint before execution.

The source `model` field remains an explicit compatibility escape hatch when no deployment is supplied. Deployment model and reasoning settings take precedence over environment and source defaults. A deployment contains configuration, never API credentials.

## 4. Resource budgets

A pipeline may declare `budget { ... }` with any of `time_ms`, `tokens`, `cost_usd`, `tool_calls`, `retries`, and `concurrency`. Limits are non-negative; concurrency is positive. Counters are scoped to that pipeline execution and atomically shared by parallel branches. A nested pipeline creates a nested scope.

Calls and retries are reserved before work begins. Provider usage is charged after an adapter result. Time is checked at orchestration boundaries. Crossing a ceiling raises a structured, non-retryable `Failure` whose kind is `budget` and whose operation identifies the exhausted resource.

Trace events contain invocation identity, retry delay, and provider usage. Usage may be actual or estimated and adapters identify which in extension-level metadata.

## 5. Evaluation

An `eval` declaration names a pipeline, JSONL dataset, positive trial count, and optional baseline. Assertions include runtime return-schema validation, exact expected-value predicates, maximum latency, maximum cost, and a `Bool`-returning semantic-grader pipeline. Each dataset row supplies an input object and optional expected value.

Evaluation uses a deterministic seed per case and trial. Implementations report pass rate and latency/cost distributions. Recorded `task_result` events can construct replay handlers, so replay never contacts providers. A baseline sets minimum pass rate and maximum mean latency and cost; crossing a baseline is an evaluation failure.

## 6. Compatibility

Effect and idempotency clauses require `language "0.4";`. Existing 0.2 and 0.3 declarations retain their unspecified contracts. Pipelines without explicit effect ceilings continue to compile through inference.
