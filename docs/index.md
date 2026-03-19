# AgentLang

**A tiny, self-contained DSL for agentic workflows.**

Define agents, typed tasks, declarative workflows, and explicit pipelines — then run them with deterministic mock adapters or live LLM backends (OpenAI or Anthropic/Claude). No framework. No magic. Everything compiles from source to execution in Python.

```agentlang
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

agent planner {
  model: "gpt-4.1"
  , tools: [web_search]
}

agent reviewer {
  model: "gpt-4.1-mini"
  , tools: [web_search]
}

task plan_blog(topic: String) -> Obj{outline: String, sources: List[String]} by agent {}
task review_outline(topic: String, outline: String, sources: List[String]) -> Obj{approved: Bool, feedback: String} by agent {}
task revise_outline(topic: String, outline: String, sources: List[String], feedback: String) -> Obj{outline: String, sources: List[String]} by agent {}

workflow publish_topic_blog(topic: String) -> String {
  stage plan = planner does plan_blog(topic);
  review outline = reviewer checks plan revise with planner using revise_outline max_rounds 2;
  return outline.outline;
}
```

```bash
$ python main.py examples/blog.agent blog_post \
    --input '{"topic":"agent memory patterns"}'
{
  "result": "[writer] Draft article:\n[planner] key points for 'agent memory patterns'"
}
```

---

## Why AgentLang?

Most "agent frameworks" hide the execution model behind layers of abstraction. AgentLang does the opposite — the language has a formal grammar, a static type checker, and a runtime you can read in an afternoon.

| | AgentLang |
|---|---|
| External dependencies | None (core) |
| Type checking | Static, structural |
  | High-level authoring | `workflow`, `stage`, `review` |
  | Parallel execution | Built-in `parallel { } join` |
  | Looping | `while`, `break`, `continue` |
  | Retry / fallback | First-class syntax |
  | LLM backend | Optional (`--adapter live` or `--adapter anthropic`) |

---

## Where to start

<div class="grid cards" markdown>

-   **New to AgentLang?**

    ---

    Follow the tutorial to run your first pipeline or workflow in under five minutes.

    [:octicons-arrow-right-24: Quick Start](tutorial/quickstart.md)

-   **Learn the language**

    ---

    Understand agents, tasks, workflows, pipelines, the type system, and parallel execution.

    [:octicons-arrow-right-24: Concepts](concepts/agents.md)

-   **Connect to OpenAI or Anthropic**

    ---

    Switch from deterministic mock mode to live LLM adapters.

    [:octicons-arrow-right-24: Adapters](reference/adapters.md)

-   **Read the full reference**

    ---

    Complete syntax, CLI flags, and runtime semantics.

    [:octicons-arrow-right-24: Language Reference](reference/language.md)

</div>
