# Pipelines

A **pipeline** is the executable unit in AgentLang. It takes typed inputs, runs a sequence of statements, and returns a typed value. Pipelines are invoked from the CLI.

## Syntax

```agentlang
pipeline <name>(param: Type, ...) -> ReturnType {
  <statements>
}
```

## Example

```agentlang
pipeline support_reply(message: String) -> String {
  let i = run extract_intent with { message: message } by triage;
  let q = run route with { intent: i.intent, urgency: i.urgency } by triage;
  let r = run respond with { intent: i.intent, queue: q.queue } by triage;
  return r.reply;
}
```

## Rules

- Pipeline names must be unique within a file.
- Every pipeline must have at least one reachable `return`.
- Input params are bound from the CLI `--input` JSON.
- The `return` expression type must match the declared return type.

## Statements

### `let` — run a task

```agentlang
let x = run task_name with { key: expr } by agent_name;
```

Runs a task, binds the result to `x`. See [retry & fallback](retry.md) for optional clauses.

### `parallel { } join` — concurrent tasks

```agentlang
parallel {
  let a = run research with { topic: query + " A" } by planner;
  let b = run research with { topic: query + " B" } by planner;
} join;
```

Runs all branches concurrently. After `join`, both `a` and `b` are in scope. See [Parallel Execution](parallel.md).

### `if / else` — conditional branching

```agentlang
if f.data == "fallback for " + topic {
  let d = run draft with { notes: "Fallback path: " + f.data } by ops;
  return d.article;
} else {
  let d = run draft with { notes: "Fresh path: " + f.data } by ops;
  return d.article;
}
```

Rules:

- The condition must type-check to `Bool`.
- `else` is optional.
- The type checker does **not** assume a guaranteed return from an `if` without `else` — you must have a `return` reachable outside the `if` block, or include `else`.

### `return` — exit the pipeline

```agentlang
return expr;
```

Evaluates `expr` and exits immediately. The type must match the pipeline's declared return type.

## Invoking a pipeline from the CLI

```bash
python main.py <source.agent> <pipeline_name> --input '<json>'
```

Example:

```bash
python main.py examples/support.agent support_reply \
  --input '{"message":"urgent refund request"}'
```

```json
{
  "result": "[triage] Routed as billing to billing-priority."
}
```

## Input validation

AgentLang validates `--input` strictly before execution:

```bash
# Missing required input
python main.py examples/blog.agent blog_post --input '{}'
# Execution error: Pipeline 'blog_post' missing inputs: ['topic'].

# Unknown extra key
python main.py examples/blog.agent blog_post \
  --input '{"topic":"x","extra":"bad"}'
# Execution error: Pipeline 'blog_post' received unknown inputs: ['extra'].
```

Values are also type-checked against declared DSL types (`String`, `Number`, `Bool`, `List[...]`, `Obj{...}`).

## Next: [The Type System](types.md)
