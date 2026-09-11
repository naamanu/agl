# Formal Semantics

This page is a historical overview of AgentLang v0 with sketches of later
extensions. It is not a complete formal specification of AGL 0.6. The
[sequential core semantics](core-semantics.md) makes control-flow completions
and scope restoration explicit and identifies the remaining proof obligations.

## 1. Abstract Syntax

A program is a tuple of agent definitions, tool signatures, task signatures, pipeline definitions, type aliases, enum definitions, and test blocks:

\\[P ::= \\langle A, U, T, W, \\mathcal\{Y\}, \\mathcal\{N\}, \\mathcal\{B\} \\rangle\\]

where \\(A\\) is the set of agent definitions, \\(U\\) the set of tool signatures, \\(T\\) the set of task signatures, \\(W\\) the set of pipeline definitions, \\(\\mathcal\{Y\}\\) the set of type alias definitions, \\(\\mathcal\{N\}\\) the set of enum definitions, and \\(\\mathcal\{B\}\\) the set of test blocks.

A tool signature declares typed inputs and a typed output for runtime-executable tools:

\\[u : (x\_1\{:\}\\tau\_1,\\ \\dots,\\ x\_n\{:\}\\tau\_n) \\to \\tau\_o\\]

A task signature declares its input parameters and return type:

\\[t : (x\_1\{:\}\\tau\_1,\\ \\dots,\\ x\_n\{:\}\\tau\_n) \\to \\tau\_o\\]

Agent tasks add an execution marker:

\\[t^\{agent\} : (x\_1\{:\}\\tau\_1,\\ \\dots,\\ x\_n\{:\}\\tau\_n) \\to \\tau\_o\\]

**Pipeline statement forms:**

\\[\\begin\{align\*\}
s\\ ::=\\ & \\texttt\{let\}\\ x = \\texttt\{run\}\\ t\\ \\texttt\{with\}\\ \\\{k\_i\{:\}e\_i\\\}\\ \[\\texttt\{by\}\\ a\]\\ \[\\texttt\{retries\}\\ n\]\\ \[\\texttt\{on\\\_fail abort\}\] \\\\
  \\mid\\ & \\texttt\{let\}\\ x = \\texttt\{run\}\\ t\\ \\texttt\{with\}\\ \\\{k\_i\{:\}e\_i\\\}\\ \[\\texttt\{by\}\\ a\]\\ \[\\texttt\{retries\}\\ n\]\\ \\texttt\{on\\\_fail use\}\\ e\_f \\\\
  \\mid\\ & \\texttt\{parallel\}\\ \[\\texttt\{max\\\_concurrency\}\\ n\]\\ \\\{r\_1;\\dots;r\_m\\\}\\ \\texttt\{join\} \\\\
  \\mid\\ & \\texttt\{if\}\\ e\\ \\\{s^\*\\\}\\ \[\\texttt\{else\}\\ \\\{s^\*\\\}\] \\\\
  \\mid\\ & \\texttt\{while\}\\ e\\ \\\{s^\*\\\} \\\\
  \\mid\\ & \\texttt\{break\} \\\\
  \\mid\\ & \\texttt\{continue\} \\\\
  \\mid\\ & \\texttt\{if let\}\\ x = e\\ \\\{s^\*\\\}\\ \[\\texttt\{else\}\\ \\\{s^\*\\\}\] \\\\
  \\mid\\ & \\texttt\{try\}\\ \\\{s^\*\\\}\\ \\texttt\{catch\}\\ x\\ \\\{s^\*\\\} \\\\
  \\mid\\ & \\texttt\{assert\}\\ e,\\ c \\\\
  \\mid\\ & \\texttt\{return\}\\ e
\\end\{align\*\}\\]

Each \\(r\_i\\) inside a `parallel` block is restricted to the run statement form (`let x = run ...`). General statements (`if`, `return`, nested `parallel`) are not permitted inside `parallel`.

**Test block form:**

\\[b\\ ::=\\ \\texttt\{test\}\\ c\\ \\\{s^\*\\\}\\]

Test blocks are top-level declarations that run only under `--test`. Each has its own scope.

**Expression forms:**

Variable references and field access chains are unified into a single form \\(x.f\_1\{\\cdots\}f\_n\\) where \\(n \\geq 0\\) (\\(n = 0\\) is a plain variable reference, \\(n \\geq 1\\) is a field access chain of depth \\(n\\)):

\\[\\begin\{align\*\}
e\\ ::=\\ & c \\mid \\texttt\{null\} \\mid x.f\_1\{\\cdots\}f\_n \\mid (e) \\mid \\\{k\_i\{:\}e\_i\\\} \\mid \[e^\*\] \\\\
  \\mid\\ & e + e \\mid e\\ \{==\}\\ e \\mid e\\ \{!=\}\\ e
\\end\{align\*\}\\]

**Types:**

\\[\\tau\\ ::=\\ \\texttt\{String\} \\mid \\texttt\{Number\} \\mid \\texttt\{Bool\} \\mid \\texttt\{List\}\[\\tau\] \\mid \\texttt\{Option\}\[\\tau\] \\mid \\texttt\{Obj\}\\\{f\_i\{:\}\\tau\_i\\\} \\mid \\texttt\{Enum\}\[n\]\\]

where \\(\\texttt\{Enum\}\[n\]\\) denotes an enum type with \\(n\\) declared variants. Enum values are assignable to \\(\\texttt\{String\}\\). Type aliases are resolved at parse time and do not appear in the type grammar.

---

## 2. Static Semantics

### Environments

| Symbol | Definition |
|---|---|
| \\(\\Gamma : \\text\{Var\} \\to \\tau\\) | Typing environment — maps variable names to their types |
| \\(\\Upsilon : \\text\{ToolName\} \\to (\\vec\{\\tau\}\_\{in\}, \\tau\_\{out\})\\) | Tool table — maps tool names to their signatures |
| \\(\\Sigma : \\text\{TaskName\} \\to (\\vec\{\\tau\}\_\{in\}, \\tau\_\{out\})\\) | Task table — maps task names to their signatures |
| \\(\\Delta : \\text\{AgentName\} \\to \\text\{AgentSpec\}\\) | Agent table — maps agent names to their specs |

### Expression typing

\\[\\dfrac\{\}\{\\Gamma \\vdash c : \\text\{type\}(c)\} \\quad \\text\{(literal)\}\\]

\\[\\dfrac\{x \\in \\text\{dom\}(\\Gamma)\}\{\\Gamma \\vdash x : \\Gamma(x)\} \\quad \\text\{(variable)\}\\]

Field access chains are typed by iterating the one-step rule:

\\[\\dfrac\{\\Gamma \\vdash e : \\texttt\{Obj\}\\\{f\_i\{:\}\\tau\_i\\\} \\qquad f\_j \\in \\\{f\_i\\\}\}\{\\Gamma \\vdash e.f\_j : \\tau\_j\} \\quad \\text\{(field access)\}\\]

For a chain \\(x.f\_1\{\\cdots\}f\_n\\), this rule is applied \\(n\\) times in sequence starting from \\(\\Gamma \\vdash x : \\Gamma(x)\\).

\\[\\dfrac\{\\Gamma \\vdash e\_1 : \\tau \\quad \\Gamma \\vdash e\_2 : \\tau \\quad \\tau \\in \\\{\\texttt\{String\},\\ \\texttt\{Number\}\\\}\}\{\\Gamma \\vdash e\_1 + e\_2 : \\tau\} \\quad \\text\{(addition)\}\\]

\\[\\dfrac\{\\Gamma \\vdash e\_1 : \\tau\_1 \\quad \\Gamma \\vdash e\_2 : \\tau\_2 \\quad \\text\{comparable\}(\\tau\_1, \\tau\_2)\}\{\\Gamma \\vdash e\_1\\ \{==\}\\ e\_2 : \\texttt\{Bool\}\} \\quad \\text\{(equality)\}\\]

\\[\\dfrac\{\\Gamma \\vdash e\_1 : \\tau\_1 \\quad \\Gamma \\vdash e\_2 : \\tau\_2 \\quad \\text\{comparable\}(\\tau\_1, \\tau\_2)\}\{\\Gamma \\vdash e\_1\\ \{!=\}\\ e\_2 : \\texttt\{Bool\}\} \\quad \\text\{(inequality)\}\\]

where \\(\\text\{comparable\}(\\tau\_1, \\tau\_2)\\) holds when \\(\\tau\_1\\) and \\(\\tau\_2\\) are mutually assignable, or when one is \\(\\texttt\{Null\}\\) and the other is \\(\\texttt\{Option\}\[\\tau\]\\), or when one is \\(\\texttt\{Enum\}\[E\]\\) and the other is \\(\\texttt\{String\}\\).

\\[\\dfrac\{\\forall\\, i\\colon\\ \\Gamma \\vdash e\_i : \\tau\_i\}\{\\Gamma \\vdash \\\{k\_i\{:\}e\_i\\\} : \\texttt\{Obj\}\\\{k\_i\{:\}\\tau\_i\\\}\} \\quad \\text\{(object literal)\}\\]

\\[\\dfrac\{\\forall\\, i\\colon\\ \\Gamma \\vdash e\_i : \\tau\}\{\\Gamma \\vdash \[e\_1,\\dots,e\_n\] : \\texttt\{List\}\[\\tau\]\} \\quad \\text\{(list literal — all items same type)\}\\]

\\[\\dfrac\{\}\{\\Gamma \\vdash \\texttt\{null\} : \\texttt\{Null\}\} \\quad \\text\{(null literal)\}\\]

### Statement typing and completion environments

Statement checking separates normal completion, return, break, continue, and
failure. See the [static completion analysis](core-semantics.md#static-completion-analysis)
for the implemented rules shared by all versions. A single post-statement
environment is insufficient: returned branches do not feed the next statement,
and a catch begins from the possible failure prefixes rather than the try entry.

Calls validate arguments and fallback assignability against their declared
signature, then bind the declared output type on normal completion. Agent tasks
require a declared `by` agent. Pipeline calls use fresh callee parameter scopes.
Assertions require Bool. Optional, match, and catch names are lexical bindings
restored before any outgoing environments are merged.

Loops preserve entry types on normal/continue back-edges and merge zero-iteration
and break paths at exit. Break and continue are valid only in loop bodies.

### Enum typing

\\[\\dfrac\{
  v \\in \\text\{variants\}(\\mathcal\{N\}(E))
\}\{
  \\Gamma \\vdash v : \\texttt\{Enum\}\[E\]
\}\\]

Enum literals are string constants validated against the declared variant set. A string literal that matches a variant is inferred as the corresponding `Enum[E]` type. Enum types are assignable to `String` (safe widening via the subtyping relation), but `String` is **not** assignable to `Enum[E]` — this ensures that only known-valid variants pass the static checker. Enum variant names must be globally unique across all enum declarations.

### Subtyping

AgentLang uses a structural subtyping relation \\(\\tau\_a \<: \\tau\_e\\) ("actual is assignable to expected"):

\\[\\dfrac\{\}\{\\tau \<: \\tau\} \\quad \\text\{(reflexivity)\}\\]

\\[\\dfrac\{\}\{\\texttt\{Enum\}\[E\] \<: \\texttt\{String\}\} \\quad \\text\{(enum widening)\}\\]

\\[\\dfrac\{\\tau\_a \<: \\tau\_e\}\{\\texttt\{List\}\[\\tau\_a\] \<: \\texttt\{List\}\[\\tau\_e\]\} \\quad \\text\{(list covariance)\}\\]

\\[\\dfrac\{\\text\{dom\}(\\sigma\_e) \\subseteq \\text\{dom\}(\\sigma\_a) \\qquad \\forall\\, f \\in \\text\{dom\}(\\sigma\_e)\\colon\\ \\sigma\_a(f) \<: \\sigma\_e(f)\}\{\\texttt\{Obj\}\\\{\\sigma\_a\\\} \<: \\texttt\{Obj\}\\\{\\sigma\_e\\\}\} \\quad \\text\{(width + depth subtyping)\}\\]

\\[\\dfrac\{\}\{\\texttt\{Null\} \<: \\texttt\{Option\}\[\\tau\]\} \\quad \\text\{(null assignable to option)\}\\]

\\[\\dfrac\{\\tau\_a \<: \\tau\_e\}\{\\texttt\{Option\}\[\\tau\_a\] \<: \\texttt\{Option\}\[\\tau\_e\]\} \\quad \\text\{(option covariance)\}\\]

**Note on covariance:** List and Obj subtyping are covariant. This is classically unsound for mutable containers, but AgentLang has no mutation operators (no assignment, no list append, no field update). The runtime's `deepcopy` at handler boundaries prevents handler-side mutation from leaking. If mutation is ever added, these rules must be revised.

### Test block typing

\\[\\dfrac\{
  \\Gamma\_0 = \\emptyset \\qquad \\Gamma\_0 \\vdash s^\*\\ \\dashv \\Gamma'
\}\{
  \\vdash \\texttt\{test\}\\ c\\ \\\{s^\*\\\} : \\checkmark
\}\\]

Test blocks are checked in an empty initial environment (they have their own scope). They do not contribute to the program's pipeline environments.

### Parallel typing

\\[\\dfrac\{
  \\forall\\, i \\neq j\\colon\\ x\_i \\neq x\_j \\qquad
  \\forall\\, i\\colon\\ x\_i \\notin \\text\{dom\}(\\Gamma) \\qquad
  \\forall\\, i\\colon\\ \\Gamma \\vdash \\texttt\{run\}\\ t\_i\\ \\texttt\{with\}\\ \\vec\{a\}\_i\\ \\cdots : \\tau\_i
\}\{
  \\Gamma \\vdash \\texttt\{parallel\}\\ \\\{\\ \\texttt\{let\}\\ x\_i = \\texttt\{run\}\\ t\_i\\ \\vec\{a\}\_i\\ \\\}\\ \\texttt\{join\}\\ \\dashv\\ \\Gamma\[x\_1 \\mapsto \\tau\_1,\\ \\dots,\\ x\_m \\mapsto \\tau\_m\]
\}\\]

All targets must be fresh (not already in \\(\\Gamma\\)) and pairwise distinct. This ensures the result is a true disjoint extension of \\(\\Gamma\\).

### Return typing and pipeline well-typedness

Every reachable return expression must be assignable to the pipeline's declared
result type. Checking starts with the declared parameter types and propagates
only normal completion to subsequent statements. All versions require a
reachable return. AGL 0.2 permits normal fallthrough; later versions reject it.
Unreachable statements still receive analysis warnings but do not contribute
return types or environments.

---

## 3. Dynamic Semantics

### Runtime configuration

A runtime configuration is a pair \\(\\langle S,\\ E \\rangle\\) where:

- \\(S\\) is the remaining statement stream
- \\(E\\) is the environment mapping variable names to runtime values

### Transition rules

**Run (success):**

\\[\\langle \[\\texttt\{let\}\\ x = \\texttt\{run\}\\ t\\ \\vec\{a\}\] \\cdot S,\\ E \\rangle \\;\\longrightarrow\\; \\langle S,\\ E\[x \\mapsto \\text\{handler\}(t,\\ \\mathcal\{A\}\\lbrack\\!\\lbrack\\vec\{a\}\\rbrack\\!\\rbrack\_E)\] \\rangle\\]

For agent tasks, the handler is synthesized by the runtime: the bound agent selects a model and tool set, the model may emit tool calls, tool calls are executed against the runtime tool registry, and the final model output is decoded into a runtime value that must match the declared task return type.

**Retry:** on failure, re-submits the run statement with the retry counter decremented. When the budget reaches zero, the failure policy applies.

**Failure policy — abort:**

\\[\\langle \[\\texttt\{let\}\\ x = \\texttt\{run\}\\ t\\ \\vec\{a\}\\ \\texttt\{on\\\_fail abort\}\] \\cdot S,\\ E \\rangle \\;\\xrightarrow\{\\text\{fail\}\}\\; \\textbf\{Error\}\\]

**Failure policy — use:**

\\[\\langle \[\\texttt\{let\}\\ x = \\texttt\{run\}\\ t\\ \\vec\{a\}\\ \\texttt\{on\\\_fail use\}\\ e\_f\] \\cdot S,\\ E \\rangle \\;\\xrightarrow\{\\text\{fail\}\}\\; \\langle S,\\ E\[x \\mapsto \\mathcal\{E\}\\lbrack\\!\\lbrack e\_f \\rbrack\\!\\rbrack\_E\] \\rangle\\]

**Parallel join:**

Each \\(r\_i \\equiv \\texttt\{let\}\\ x\_i = \\texttt\{run\}\\ t\_i\\ \\vec\{a\}\_i\\). All branches execute concurrently against a snapshot of \\(E\\), producing values \\(v\_i = \\text\{handler\}(t\_i,\\ \\mathcal\{A\}\\lbrack\\!\\lbrack\\vec\{a\}\_i\\rbrack\\!\\rbrack\_E)\\):

\\[\\langle \[\\texttt\{parallel\}\\\{r\_1;\\dots;r\_m\\\}\\texttt\{ join\}\] \\cdot S,\\ E \\rangle \\;\\longrightarrow\\; \\langle S,\\ E \\cup \\\{x\_1 \\mapsto v\_1,\\ \\dots,\\ x\_m \\mapsto v\_m\\\} \\rangle\\]

All \\(x\_i\\) are distinct and not in \\(\\text\{dom\}(E)\\) (enforced statically), so the extension is disjoint.

**If (true branch, with else):**

\\[\\langle \[\\texttt\{if\}\\ e\_c\\ \\\{s\_\{then\}^\*\\\}\\ \\texttt\{else\}\\ \\\{s\_\{else\}^\*\\\}\] \\cdot S,\\ E \\rangle \\;\\longrightarrow\\; \\langle s\_\{then\}^\* \\cdot S,\\ E \\rangle \\quad \\text\{when\}\\ \\mathcal\{E\}\\lbrack\\!\\lbrack e\_c \\rbrack\\!\\rbrack\_E = \\textit\{true\}\\]

**If (false branch, with else):**

\\[\\langle \[\\texttt\{if\}\\ e\_c\\ \\\{s\_\{then\}^\*\\\}\\ \\texttt\{else\}\\ \\\{s\_\{else\}^\*\\\}\] \\cdot S,\\ E \\rangle \\;\\longrightarrow\\; \\langle s\_\{else\}^\* \\cdot S,\\ E \\rangle \\quad \\text\{when\}\\ \\mathcal\{E\}\\lbrack\\!\\lbrack e\_c \\rbrack\\!\\rbrack\_E = \\textit\{false\}\\]

**If (true branch, without else):**

\\[\\langle \[\\texttt\{if\}\\ e\_c\\ \\\{s\_\{then\}^\*\\\}\] \\cdot S,\\ E \\rangle \\;\\longrightarrow\\; \\langle s\_\{then\}^\* \\cdot S,\\ E \\rangle \\quad \\text\{when\}\\ \\mathcal\{E\}\\lbrack\\!\\lbrack e\_c \\rbrack\\!\\rbrack\_E = \\textit\{true\}\\]

**If (false branch, without else — skip):**

\\[\\langle \[\\texttt\{if\}\\ e\_c\\ \\\{s\_\{then\}^\*\\\}\] \\cdot S,\\ E \\rangle \\;\\longrightarrow\\; \\langle S,\\ E \\rangle \\quad \\text\{when\}\\ \\mathcal\{E\}\\lbrack\\!\\lbrack e\_c \\rbrack\\!\\rbrack\_E = \\textit\{false\}\\]

**While:**

\\[\\langle \[\\texttt\{while\}\\ e\_c\\ \\\{s^\*\\\}\] \\cdot S,\\ E \\rangle \\;\\longrightarrow\\; \\langle s^\* \\cdot \[\\texttt\{while\}\\ e\_c\\ \\\{s^\*\\\}\] \\cdot S,\\ E \\rangle \\quad \\text\{when\}\\ \\mathcal\{E\}\\lbrack\\!\\lbrack e\_c \\rbrack\\!\\rbrack\_E = \\textit\{true\}\\]

\\[\\langle \[\\texttt\{while\}\\ e\_c\\ \\\{s^\*\\\}\] \\cdot S,\\ E \\rangle \\;\\longrightarrow\\; \\langle S,\\ E \\rangle \\quad \\text\{when\}\\ \\mathcal\{E\}\\lbrack\\!\\lbrack e\_c \\rbrack\\!\\rbrack\_E = \\textit\{false\}\\]

**Break:**

\\[\\langle \[\\texttt\{break\}\] \\cdot S,\\ E \\rangle \\;\\longrightarrow\\; \\langle S\_\{post\\text\{-\}loop\},\\ E \\rangle\\]

where \\(S\_\{post\\text\{-\}loop\}\\) is the statement stream after removing the enclosing `while` and its remaining body. Equivalently, `break` raises a control-flow signal that unwinds to the nearest enclosing loop, which then terminates.

**Continue:**

\\[\\langle \[\\texttt\{continue\}\] \\cdot S,\\ E \\rangle \\;\\longrightarrow\\; \\langle \[\\texttt\{while\}\\ e\_c\\ \\\{s^\*\\\}\] \\cdot S\_\{post\\text\{-\}loop\},\\ E \\rangle\\]

`continue` raises a control-flow signal that skips the remaining statements in the current iteration and re-evaluates the loop condition.

**If-let (some):**

\\[\\langle \[\\texttt\{if let\}\\ x = e\_o\\ \\\{s\_\{then\}^\*\\\}\\ \\texttt\{else\}\\ \\\{s\_\{else\}^\*\\\}\] \\cdot S,\\ E \\rangle \\;\\longrightarrow\\; \\langle s\_\{then\}^\* \\cdot S,\\ E\[x \\mapsto v\] \\rangle \\quad \\text\{when\}\\ \\mathcal\{E\}\\lbrack\\!\\lbrack e\_o \\rbrack\\!\\rbrack\_E = v \\neq \\texttt\{null\}\\]

The binding \\(x\\) is scoped to the `then` branch only. After the branch completes, \\(x\\) is removed from the environment (or restored to its prior value if it existed before the `if let`).

**If-let (none):**

\\[\\langle \[\\texttt\{if let\}\\ x = e\_o\\ \\\{s\_\{then\}^\*\\\}\\ \\texttt\{else\}\\ \\\{s\_\{else\}^\*\\\}\] \\cdot S,\\ E \\rangle \\;\\longrightarrow\\; \\langle s\_\{else\}^\* \\cdot S,\\ E \\rangle \\quad \\text\{when\}\\ \\mathcal\{E\}\\lbrack\\!\\lbrack e\_o \\rbrack\\!\\rbrack\_E = \\texttt\{null\}\\]

When `else` is absent and the value is `null`, execution skips to the next statement.

**Try/catch:**

The try body propagates normal completion, return, break, and continue unchanged.
Only an execution error enters the catch. The handler begins with the environment
at the failure point, with its error variable temporarily bound. The previous
binding is restored (or the name removed if fresh) on every handler exit. Other
bindings from partial execution persist. Handler errors propagate outward.
The [core exception rules](core-semantics.md#exceptions) make these completions
and environments explicit.

**Timeout (non-retryable):**

When a task handler exceeds its `timeout` deadline, a non-retryable timeout `Failure` is raised and its cooperative cancellation token is set. Cancellation-aware handlers and adapters stop and are joined; legacy synchronous handlers retain compatibility but cannot promise prompt cancellation.

**Assert (pass):**

\\[\\langle \[\\texttt\{assert\}\\ e,\\ c\] \\cdot S,\\ E \\rangle \\;\\longrightarrow\\; \\langle S,\\ E \\rangle \\quad \\text\{when\}\\ \\mathcal\{E\}\\lbrack\\!\\lbrack e \\rbrack\\!\\rbrack\_E = \\textit\{true\}\\]

**Assert (fail):**

\\[\\langle \[\\texttt\{assert\}\\ e,\\ c\] \\cdot S,\\ E \\rangle \\;\\longrightarrow\\; \\textbf\{AssertionError\}(c) \\quad \\text\{when\}\\ \\mathcal\{E\}\\lbrack\\!\\lbrack e \\rbrack\\!\\rbrack\_E = \\textit\{false\}\\]

**Pipeline call:**

\\[\\langle \[\\texttt\{let\}\\ x = \\texttt\{run\}\\ W'\\ \\texttt\{with\}\\ \\\{k\_i\{:\}e\_i\\\}\] \\cdot S,\\ E \\rangle \\;\\longrightarrow\\; \\langle S,\\ E\[x \\mapsto \\text\{exec\}(W',\\ \\\{k\_i \\mapsto \\mathcal\{E\}\\lbrack\\!\\lbrack e\_i \\rbrack\\!\\rbrack\_E\\\})\] \\rangle\\]

The target pipeline \\(W'\\) is executed in a fresh scope with its parameters bound from the arguments. The result is the pipeline's return value.

**Return:**

\\[\\langle \[\\texttt\{return\}\\ e\] \\cdot S,\\ E \\rangle \\;\\longrightarrow\\; \\mathcal\{E\}\\lbrack\\!\\lbrack e \\rbrack\\!\\rbrack\_E\\]

---

## 4. Determinism

**Expression determinism (conditional).** For a fixed environment and a
deterministic expression evaluator, successful evaluation has at most one value.
This does not assert that every expression evaluates successfully.

Pure task handlers alone do not guarantee deterministic pipeline outcomes:
timeouts, shared budgets, and races can expose scheduling differences. The
sequential core has a conditional determinism result when expression and call
outcomes are fixed; it does not establish termination.

For an ordinary parallel join whose branches all succeed with fixed values,
disjoint result bindings make the merged environment independent of completion
order. This conditional property does not imply deterministic failure behavior.

The runtime additionally enforces that task handler outputs and final pipeline return values conform to their declared DSL types; malformed runtime values are execution errors even if the surrounding program parsed and type-checked successfully.

Declared tools are also validated dynamically: tool call arguments and tool results must conform to their DSL signatures.

---

## 5. AGL 0.3 nominal and sum types

AGL 0.3 extends the type grammar with nominal records, closed unions, and results:

\\[\\tau ::= \\cdots \\mid \\texttt\{Record\}\[R\] \\mid \\texttt\{Union\}\[U\] \\mid \\texttt\{Result\}\[\\tau\_o,\\tau\_e\]\\]

A record environment \\(\\mathcal\{R\}\\) maps each record name to its field row, and a union environment \\(\\mathcal\{U\}\\) maps each union name and variant to a field row.

**Record construction:**

\\[\\dfrac\{\\mathcal\{R\}(R)=\\\{f\_i\{:\}\\tau\_i\\\}\\quad \\forall i.\\ \\Gamma\\vdash e\_i:\\tau\_i\}\{\\Gamma\\vdash R\\\{f\_i:e\_i\\\}:\\texttt\{Record\}\[R\]\}\\]

The provided field domain must equal the declared field domain. `Record[R]` is assignable only to itself (including through transparent aliases); equal field structure does not make two record names interchangeable.

**Union construction:**

\\[\\dfrac\{\\mathcal\{U\}(U,V)=\\\{f\_i\{:\}\\tau\_i\\\}\\quad \\forall i.\\ \\Gamma\\vdash e\_i:\\tau\_i\}\{\\Gamma\\vdash U::V\\\{f\_i:e\_i\\\}:\\texttt\{Union\}\[U\]\}\\]

**Result construction:**

\\[\\dfrac\{\\Gamma\\vdash e:\\tau\}\{\\Gamma\\vdash \\texttt\{Ok\}(e):\\texttt\{Result\}\[\\tau,\\alpha\]\}\\qquad
\\dfrac\{\\Gamma\\vdash e:\\epsilon\}\{\\Gamma\\vdash \\texttt\{Err\}(e):\\texttt\{Result\}\[\\alpha,\\epsilon\]\}\\]

Here \\(\\alpha\\) is a contextual inference placeholder assignable to the corresponding expected result component. It cannot be named in source or escape a checked use site.

### Exhaustive match typing

For a union, the arm set must contain each declared variant exactly once, with
exactly its payload bindings. Each arm is checked with those names temporarily
bound. The same rule applies to `Result::Ok { value }` and
`Result::Err { error }`.

Restore the pattern bindings in every outgoing environment, then merge arms
separately by completion kind. Only normally completing arms contribute to the
post-match normal environment; returns, breaks, continues, and errors propagate
independently. See the [static completion analysis](core-semantics.md#static-completion-analysis).

At runtime, evaluation selects the unique arm whose `$type` and `$variant` tags equal the scrutinee tags, binds its payload fields, and executes that body. Exhaustiveness guarantees that every statically valid tagged value has an arm.

Typed `Err` values are ordinary results. They do not take an execution-error transition and are therefore not intercepted by `try/catch`.

### Total return paths

AGL 0.3 strengthens pipeline well-typedness by rejecting a normal fallthrough outcome. Errors need not produce a return. Every reachable return must have a compatible type, and at least one reachable return must exist. Branches and exhaustive matches merge only normal continuations; loops always include a possible zero-iteration exit. This analysis does not prove termination.

---

## 6. AGL 0.4 effect judgments

Let \\(\\Phi(d)\\) be the declared direct effect set of task or tool \\(d\\). Agent execution adds `model`; an agent's available tools add their declared effects. Pipeline effect inference is the least fixed point of:

\\[\\Phi(W)=\\bigcup\_\{r\\in runs(W)\}\\left(\\Phi(r)\\cup\\Phi(callee(r))\\right)\\]

For a pipeline with declared ceiling \\(C\\), well-typedness additionally requires \\(\\Phi(W)\\subseteq C\\).

Let \\(safe(i)\\) hold for idempotency contracts `pure`, `idempotent`, and `keyed_by`. A retried call with effect `external_write` is well typed only when \\(safe(i)\\) holds. A call explicitly declared `non_idempotent` is never retryable. When the call is agent-bound, this condition also applies to every available tool carrying `external_write`.

---

## 7. AGL 0.5 structured and durable transitions

For `parallel map`, if \\(E(x\_s)=\[v\_1\\ldots v\_n\]\\) and every mapped task produces \\(u\_i\\), the resulting environment is \\(E\[x\_t\\mapsto\[u\_1\\ldots u\_n\]\]\\); scheduler order does not alter index order. A race returns the first successful \\(u\_i\\), sends cancellation to every \\(j\\ne i\\), and joins the complete child set before continuing.

Durable execution adds history \\(H\\) to the configuration. Before invoking operation identity \\(i\\), replay checks \\(H\\) for `task_result(i,v)`. If present, the transition binds \\(v\\) without an external call. Otherwise the runtime appends an attempt, invokes, and appends the result checkpoint. Resume is defined only when the stored program/deployment fingerprint equals the current fingerprint.

An approval without a supplied decision appends `human_suspended` and yields a typed suspension. Resume with a non-expired decision binds its Boolean value and appends an audited approval event.

## 8. AGL 0.6 module judgments

Let \\(M\\) map canonical module paths to interfaces and \\(I\\) be the directed import graph. Resolution succeeds iff \\(I\\) is acyclic and every qualified reference \\(a::x\\) names \\(x\\) in the public interface of alias \\(a\\). Private declarations remain available only while checking their owning namespace.

An interface fingerprint is computed from canonical serialized public signatures and effects. Package/API compatibility is structural over these interfaces: removal or signature/effect change is breaking; addition is compatible.
