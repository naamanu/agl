# Effects and idempotency

AGL 0.4 makes operational capabilities visible to the compiler. Types describe data; effects describe what executing a task may do.

```agentlang
language "0.4";

task publish(id: String, article: String) -> String
  effects [network, external_write]
  idempotency keyed_by id {}

pipeline release(id: String, article: String) -> String
  effects [network, external_write] {
  let publication = publish(id, article) retries 2;
  return publication;
}
```

## Built-in effects

| Effect | Meaning |
|---|---|
| `model` | Calls a generative model |
| `network` | Uses the network |
| `filesystem` | Reads or writes host files |
| `external_read` | Reads external state |
| `external_write` | Mutates external state |
| `secret` | Reads secret material |
| `human` | Suspends for human input or approval |

Applications may introduce capability names for deployment policy. Custom names do not weaken built-in rules.

Pipeline effects are transitive through task, agent-tool, and pipeline calls. An omitted pipeline clause is inferred. When a pipeline declares `effects [...]`, the list is an allowed ceiling and missing capabilities are a static error.

## Retry safety

`pure`, `idempotent`, `keyed_by`, and `non_idempotent` document whether repeated execution is safe. A retried external write must be idempotent or keyed. The key parameter must be a `String`.

`retry_on` remains orthogonal: it selects which typed domain errors cause another attempt, while idempotency determines whether another attempt is safe at all.
