# Workflows

A **workflow** is the high-level authoring surface in AgentLang. It lets you declare stage handoffs and review loops without spelling out the lowered `run`, `while`, `break`, and rebinding logic yourself.

## Syntax

```agentlang
workflow <name>(param: Type, ...) -> ReturnType {
  <workflow-steps>
}
```

## Step forms

### `stage`

```agentlang
stage artifact = agent_name does task_name(expr1, expr2);
```

- Arguments are positional
- The argument count must match the task signature
- The stage result becomes a named artifact for later steps

### `review`

```agentlang
review approved_artifact = reviewer checks draft_artifact
  revise with reviser using revise_task
  max_rounds 2;
```

This is a declarative review loop. The compiler lowers it to:

- an initial review task call
- a hidden revision budget using `countdown`
- a `while` loop
- a revise task call plus re-review on each failed review round

Current rule:

- the review task name is inferred as `review_<artifact>`

So the example above requires a task named `review_approved_artifact(...)`.

## Example

```agentlang
agent planner {
  model: "gpt-4.1"
  , tools: []
}

agent reviewer {
  model: "gpt-4.1-mini"
  , tools: []
}

task plan(topic: String) -> Obj{outline: String, sources: List[String]} by agent {}
task review_approved_outline(topic: String, outline: String, sources: List[String]) -> Obj{approved: Bool, feedback: String} by agent {}
task revise_outline(topic: String, outline: String, sources: List[String], feedback: String) -> Obj{outline: String, sources: List[String]} by agent {}

workflow publish(topic: String) -> String {
  stage draft = planner does plan(topic);
  review approved_outline = reviewer checks draft revise with planner using revise_outline max_rounds 2;
  return approved_outline.outline;
}
```

## Lowering

Workflows are compiled into ordinary pipelines before type-checking and execution.

Use:

```bash
python main.py examples/multiagent_blog.agent publish_topic_blog --lower
```

to inspect the lowered pipeline IR.

## When to use workflows vs pipelines

Use `workflow` when:

- users should author stage handoffs declaratively
- review/revise loops should be implicit
- you want a cleaner source language and an inspectable lowered IR

Use `pipeline` when:

- you need explicit low-level control flow
- you want to author `parallel`, `while`, `break`, `continue`, or custom fallback behavior directly
- you are debugging or experimenting with the core execution model

## Next: [Pipelines](pipelines.md)
