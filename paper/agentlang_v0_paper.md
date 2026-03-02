# AgentLang v0: A Small, Typed DSL for Agentic Workflows with Formal Operational Semantics

**Author:** Nana Manu
**Date:** March 2, 2026

## Abstract

Agent orchestration code is often embedded in general-purpose languages and framework-specific APIs, which makes execution behavior difficult to reason about and validate statically. AgentLang is a compact domain-specific language (DSL) that makes workflow structure explicit through typed task signatures, sequential and parallel composition, conditional branching, and built-in retry/fallback policies. This paper presents AgentLang v0 as a full source-to-execution stack: lexer, parser, static checker, and runtime, together with a formal static and dynamic semantics. The implementation is intentionally small (1,567 LOC across CLI and core modules) and dependency-light, while still supporting deterministic mock execution and optional live OpenAI-backed adapters. We describe the language design, semantic model, implementation architecture, and empirical artifact checks. Results show that static checks reject common orchestration errors before execution, while parallel blocks deliver near-ideal speedup in an I/O-bound microbenchmark (1.97x for two equal-latency tasks).

## 1. Introduction

Agent workflows typically combine typed data movement, control flow, failure handling, and calls to external models/tools. In many systems these concerns are split across application code and framework conventions, making behavior implicit and difficult to verify. AgentLang takes the opposite position: workflows should be first-class programs with explicit syntax and formal semantics.

AgentLang v0 is designed around three constraints.

1. Small enough to read end-to-end quickly.
2. Strong enough to catch common orchestration errors before runtime.
3. Practical enough to execute real mock/live task handlers from a CLI.

The project currently ships as a self-contained Python 3.14+ implementation with five example programs, a formal semantics, and an interactive REPL.

## 2. Language Overview

An AgentLang source file contains declarations for:

1. `agent` definitions (model/tool configuration),
2. `task` signatures (typed inputs and output),
3. `pipeline` definitions (executable statement blocks).

Core statement forms are:

1. Task invocation with binding: `let x = run t with {...} ...;`
2. Parallel join block: `parallel { ... } join;`
3. Conditional: `if e { ... } else { ... }`
4. Return: `return e;`

Types are structural and static:

1. Primitive: `String`, `Number`, `Bool`
2. Composite: `List[T]`, `Obj{field: Type, ...}`

Expressions include literals, references, chained field access (`x.f1.f2`), object/list literals, `+`, `==`, and `!=`.

## 3. Formal Semantics and Safety Model

AgentLang v0 includes a formal account of:

1. Abstract syntax (program, statement, expression, and type forms),
2. Static typing judgments over environments,
3. Dynamic (small-step) execution rules.

### 3.1 Static semantics

Typing environments map variables to types. Task and agent tables map identifiers to signatures/specifications. The checker enforces:

1. existence of referenced tasks/agents,
2. exact argument key matching and argument type compatibility,
3. `Bool` condition type for `if`,
4. fallback type equality for `on_fail use`,
5. pipeline return type consistency.

For path sensitivity, branch environments are merged after `if` using a **branch merge** operation. The merge keeps only variables present in both branches with identical types. Variables re-bound with incompatible types across branches, or introduced in only one branch, are excluded from the post-merge environment, preventing unsound field access in subsequent statements.

Parallel typing requires fresh, pairwise-distinct targets for all branch bindings, ensuring disjoint environment extension at join.

### 3.2 Dynamic semantics

Runtime configurations use a statement stream and value environment. Execution is sequential except within `parallel`, where branch runs are dispatched concurrently against a snapshot of the environment and merged at `join`.

Retry semantics are explicit: each run has `retries + 1` total attempts. On exhaustion:

1. `on_fail abort` raises an execution error,
2. `on_fail use e_f` evaluates and binds the fallback expression.

The determinism guarantee is conditional: expression evaluation is deterministic, and whole-pipeline determinism holds when task handlers are deterministic.

## 4. Implementation

AgentLang is implemented in pure Python 3.14+ with a small, explicit architecture:

1. `agentlang/lexer.py` — tokenization and string escape decoding.
2. `agentlang/parser.py` — recursive-descent parser into AST.
3. `agentlang/checker.py` — static type and binding checker.
4. `agentlang/runtime.py` — executor over statement blocks and expressions.
5. `agentlang/stdlib.py` — built-in task handlers plus mock/live adapter wiring.
6. `main.py` — CLI with `run` and interactive `repl` modes.

Core size (from `wc -l`):

1. `main.py`: 135 LOC
2. `ast.py`: 123 LOC
3. `lexer.py`: 172 LOC
4. `parser.py`: 432 LOC
5. `checker.py`: 240 LOC
6. `runtime.py`: 226 LOC
7. `stdlib.py`: 239 LOC
8. Total (above): 1,567 LOC

### 4.1 Runtime isolation for handler arguments

The runtime evaluates argument expressions per run statement and passes deep-copied arguments to handlers (`copy.deepcopy`). This prevents handler-side mutation from leaking into pipeline environment state or sibling `parallel` branches through shared mutable objects.

### 4.2 Interactive REPL

`main.py repl [--adapter mock|live]` starts a read-eval-print loop. Each prompt accepts `<source_file> <pipeline_name> [json_input]`, parses and type-checks the source, executes the named pipeline, and prints the JSON result. Errors are reported inline and the loop continues without restarting.

```
AgentLang REPL (adapter=mock). Type 'exit' to quit.
> examples/blog.agent blog_post {"topic":"agent memory"}
{
  "result": "[writer] Draft article:\n[planner] key points for 'agent memory'"
}
> exit
```

## 5. Evaluation

This section reports artifact-level checks executed against the current repository state. All commands require Python 3.14+ and must be run from the repository root.

### 5.1 Build and example execution

Syntax/build sanity:

```bash
python3.14 -m py_compile main.py agentlang/*.py agentlang/adapters/*.py
```

Observed: pass.

Happy-path examples (mock adapter):

```bash
python3.14 main.py examples/blog.agent blog_post --input '{"topic":"agent memory patterns"}'
python3.14 main.py examples/compare.agent compare_options --input '{"query":"vector database"}'
python3.14 main.py examples/support.agent support_reply --input '{"message":"urgent refund request"}'
```

Observed:

1. Blog pipeline produced a draft string from research notes.
2. Compare pipeline produced a combined A/B decision output.
3. Support pipeline produced a routed billing response.

### 5.2 Reliability behavior (retry and fallback)

```bash
python3.14 main.py examples/reliability.agent resilient_brief \
  --input '{"topic":"api-status","fail_count":1}'
python3.14 main.py examples/reliability.agent resilient_brief \
  --input '{"topic":"api-status","fail_count":5}'
```

Observed:

1. `fail_count=1` succeeds within retry budget (`Fresh path` output).
2. `fail_count=5` exhausts retries and uses fallback (`Fallback path` output).

Abort-mode error surface check using `examples/retry_abort.agent`, a pipeline that uses `on_fail abort` with `retries 2` (3 total attempts):

```bash
python3.14 main.py examples/retry_abort.agent abort_on_failure \
  --input '{"topic":"api-status","fail_count":5}'
```

Observed (written to stderr, exit code 1):

```text
Execution error: Task 'flaky_fetch' failed after 3 attempts.
```

### 5.3 Input validation checks

```bash
python3.14 main.py examples/blog.agent blog_post --input '{}'
python3.14 main.py examples/blog.agent blog_post --input '{"topic":42}'
```

Observed:

1. Missing key rejected before execution.
2. Wrong input type rejected before execution.

### 5.4 Parallelism microbenchmark

The benchmark script is `paper/benchmark.py`. It parses a self-contained AgentLang source that defines two pipelines over the same `slow_task` handler (which sleeps for 0.1s to simulate I/O-bound work): one sequential (`a` then `b`), one using `parallel { a; b } join`. Both are run 10 times and wall-clock means are compared.

```bash
python3.14 paper/benchmark.py
```

Observed:

1. Sequential mean: `0.2076s` (stdev `0.0018s`)
2. Parallel mean: `0.1054s` (stdev `0.0005s`)
3. Speedup: `1.97x`

The small shortfall from the theoretical 2.00x is attributable to thread scheduling overhead from `ThreadPoolExecutor`. For independent, balanced I/O-bound branches, the runtime achieves near-ideal two-way overlap.

## 6. Discussion

### 6.1 What the design gets right

1. **Inspectability:** the full execution path from text to output is explicit and compact.
2. **Static safety:** many workflow errors are shifted left to check time.
3. **Operational clarity:** retry/fallback and parallel composition are language-level constructs, not ad hoc host-language patterns.

### 6.2 Current limitations

1. No dedicated unit-test framework in-tree yet (artifact checks are command-based).
2. Type system is intentionally minimal (no unions/generics beyond `List[T]`).
3. Parallel runtime uses thread pools and is tuned for I/O-bound handlers.
4. Live adapter behavior remains nondeterministic due to model/network factors.

### 6.3 Future work

1. Add a formalized effect/exception layer for richer failure semantics.
2. Extend types (sum types, optional types, schema aliases).
3. Add model-checking or symbolic analysis for return-path and policy properties.
4. Provide a benchmark suite across larger workflow graphs and mixed latency profiles.

## 7. Reproducibility

Environment assumptions:

1. Python 3.14+
2. Repository root as working directory
3. Mock mode for deterministic local runs (default)

All commands in Section 5 can be executed directly from the project root. No external services are required unless `--adapter live` is used.

## 8. Conclusion

AgentLang v0 demonstrates that agent workflow semantics can be made fully explicit in a small, self-contained system. The design deliberately avoids embedding workflow logic in a general-purpose host language or framework API: tasks, pipelines, agents, retry policies, and parallel composition all have dedicated syntax and formal treatment. The combination of typed signatures, a sound static checker, and a documented operational semantics yields a system that is both understandable and auditable — properties that are difficult to recover after the fact in framework-native code. The artifact checks confirm that static analysis catches real classes of orchestration errors before execution, and that parallel composition delivers the expected performance benefit for I/O-bound workloads. At 1,567 LOC, the entire stack fits in a single reading session, making it a viable foundation for more ambitious extensions.

## References

[1] Chase, H. et al. *LangChain: Building applications with LLMs through composability.* GitHub, 2022. https://github.com/langchain-ai/langchain

[2] Liu, J. et al. *LlamaIndex: A data framework for LLM applications.* GitHub, 2022. https://github.com/run-llama/llama_index

[3] Temporal Technologies. *Temporal: Durable execution for the modern developer.* Technical documentation, 2023. https://docs.temporal.io

[4] Pierce, B. C. *Types and Programming Languages.* MIT Press, 2002.

[5] Wirth, N. *What can we do about the unnecessary diversity of notation for syntactic definitions?* Communications of the ACM, 1977.
