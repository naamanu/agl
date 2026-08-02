# Durable workflows and approvals

Use a SQLite event store and stable execution ID when a workflow may outlive one process:

```bash
agl release.agent release --event-store runs.sqlite --execution-id release-42
```

If the pipeline reaches `approve production "Release?"`, it persists a `human_suspended` event and exits with a structured suspension. Resume it with the same source, deployment, store, and ID:

```bash
agl release.agent release --event-store runs.sqlite --execution-id release-42 \
  --resume --approval production=true
```

Completed task results are replayed by invocation ID, so the work before the approval is not repeated. A source or deployment fingerprint mismatch stops resume. SQLite histories are schema-versioned, configured secrets are redacted before append, and hosts can apply retention with `EventStore::prune_before`.

Approval decisions are typed booleans and audited with actor/time metadata. `expires` bounds how long a suspended decision remains valid; `delegate` records the intended decision role.
