# AgentLang

**A tiny, self-contained DSL for agentic workflows.**

Define agents, typed tasks, declarative workflows, and explicit pipelines — then run them with deterministic mock adapters or live OpenAI backends. No framework. No magic. Everything compiles from source to execution in Python.

```agentlang
tool web_search(query: String) -> List[Obj{title: String, url: String, snippet: String}] {}

agent planner {
  model: "gpt-4.1"
  , tools: [web_search]
}

task research(topic: String) -> Obj{notes: String} {}
task draft(notes: String)    -> Obj{article: String} {}

pipeline blog_post(topic: String) -> String {
  let r = run research with { topic: topic } by planner;
  let d = run draft    with { notes: r.notes } by planner;
  return d.article;
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
| Parallel execution | Built-in `parallel { } join` |
| Retry / fallback | First-class syntax |
| LLM backend | Optional (`--adapter live`) |

---

## Where to start

<div class="grid cards" markdown>

-   **New to AgentLang?**

    ---

    Follow the tutorial to run your first pipeline in under five minutes.

    [:octicons-arrow-right-24: Quick Start](tutorial/quickstart.md)

-   **Learn the language**

    ---

    Understand agents, tasks, pipelines, the type system, and parallel execution.

    [:octicons-arrow-right-24: Concepts](concepts/agents.md)

-   **Connect to OpenAI**

    ---

    Switch from deterministic mock mode to live LLM adapters.

    [:octicons-arrow-right-24: Adapters](reference/adapters.md)

-   **Read the full reference**

    ---

    Complete syntax, CLI flags, and runtime semantics.

    [:octicons-arrow-right-24: Language Reference](reference/language.md)

</div>
