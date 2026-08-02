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

These checks are static promises: hosts must implement the declared idempotency behavior. Future stable extension contracts carry the resolved key to handlers and adapters.

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

## 4. Compatibility

Effect and idempotency clauses require `language "0.4";`. Existing 0.2 and 0.3 declarations retain their unspecified contracts. Pipelines without explicit effect ceilings continue to compile through inference.
