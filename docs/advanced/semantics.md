# Formal Semantics

This page gives a precise mathematical account of AgentLang v0: abstract syntax, static typing rules, and dynamic (small-step) semantics.

## 1. Abstract Syntax

A program is a tuple of agent definitions, tool signatures, task signatures, pipeline definitions, type aliases, enum definitions, and test blocks:

$$P ::= \langle A, U, T, W, \mathcal{Y}, \mathcal{N}, \mathcal{B} \rangle$$

where \(A\) is the set of agent definitions, \(U\) the set of tool signatures, \(T\) the set of task signatures, \(W\) the set of pipeline definitions, \(\mathcal{Y}\) the set of type alias definitions, \(\mathcal{N}\) the set of enum definitions, and \(\mathcal{B}\) the set of test blocks.

A tool signature declares typed inputs and a typed output for runtime-executable tools:

$$u : (x_1{:}\tau_1,\ \dots,\ x_n{:}\tau_n) \to \tau_o$$

A task signature declares its input parameters and return type:

$$t : (x_1{:}\tau_1,\ \dots,\ x_n{:}\tau_n) \to \tau_o$$

Agent tasks add an execution marker:

$$t^{agent} : (x_1{:}\tau_1,\ \dots,\ x_n{:}\tau_n) \to \tau_o$$

**Pipeline statement forms:**

$$\begin{align*}
s\ ::=\ & \texttt{let}\ x = \texttt{run}\ t\ \texttt{with}\ \{k_i{:}e_i\}\ [\texttt{by}\ a]\ [\texttt{retries}\ n]\ [\texttt{on\_fail abort}] \\
  \mid\ & \texttt{let}\ x = \texttt{run}\ t\ \texttt{with}\ \{k_i{:}e_i\}\ [\texttt{by}\ a]\ [\texttt{retries}\ n]\ \texttt{on\_fail use}\ e_f \\
  \mid\ & \texttt{parallel}\ [\texttt{max\_concurrency}\ n]\ \{r_1;\dots;r_m\}\ \texttt{join} \\
  \mid\ & \texttt{if}\ e\ \{s^*\}\ [\texttt{else}\ \{s^*\}] \\
  \mid\ & \texttt{while}\ e\ \{s^*\} \\
  \mid\ & \texttt{break} \\
  \mid\ & \texttt{continue} \\
  \mid\ & \texttt{if let}\ x = e\ \{s^*\}\ [\texttt{else}\ \{s^*\}] \\
  \mid\ & \texttt{try}\ \{s^*\}\ \texttt{catch}\ x\ \{s^*\} \\
  \mid\ & \texttt{assert}\ e,\ c \\
  \mid\ & \texttt{return}\ e
\end{align*}$$

Each \(r_i\) inside a `parallel` block is restricted to the run statement form (`let x = run ...`). General statements (`if`, `return`, nested `parallel`) are not permitted inside `parallel`.

**Test block form:**

$$b\ ::=\ \texttt{test}\ c\ \{s^*\}$$

Test blocks are top-level declarations that run only under `--test`. Each has its own scope.

**Expression forms:**

Variable references and field access chains are unified into a single form \(x.f_1{\cdots}f_n\) where \(n \geq 0\) (\(n = 0\) is a plain variable reference, \(n \geq 1\) is a field access chain of depth \(n\)):

$$\begin{align*}
e\ ::=\ & c \mid \texttt{null} \mid x.f_1{\cdots}f_n \mid (e) \mid \{k_i{:}e_i\} \mid [e^*] \\
  \mid\ & e + e \mid e\ {==}\ e \mid e\ {!=}\ e
\end{align*}$$

**Types:**

$$\tau\ ::=\ \texttt{String} \mid \texttt{Number} \mid \texttt{Bool} \mid \texttt{List}[\tau] \mid \texttt{Option}[\tau] \mid \texttt{Obj}\{f_i{:}\tau_i\} \mid \texttt{Enum}[n]$$

where \(\texttt{Enum}[n]\) denotes an enum type with \(n\) declared variants. Enum values are assignable to \(\texttt{String}\). Type aliases are resolved at parse time and do not appear in the type grammar.

---

## 2. Static Semantics

### Environments

| Symbol | Definition |
|---|---|
| \(\Gamma : \text{Var} \to \tau\) | Typing environment — maps variable names to their types |
| \(\Upsilon : \text{ToolName} \to (\vec{\tau}_{in}, \tau_{out})\) | Tool table — maps tool names to their signatures |
| \(\Sigma : \text{TaskName} \to (\vec{\tau}_{in}, \tau_{out})\) | Task table — maps task names to their signatures |
| \(\Delta : \text{AgentName} \to \text{AgentSpec}\) | Agent table — maps agent names to their specs |

### Expression typing

$$\dfrac{}{\Gamma \vdash c : \text{type}(c)} \quad \text{(literal)}$$

$$\dfrac{x \in \text{dom}(\Gamma)}{\Gamma \vdash x : \Gamma(x)} \quad \text{(variable)}$$

Field access chains are typed by iterating the one-step rule:

$$\dfrac{\Gamma \vdash e : \texttt{Obj}\{f_i{:}\tau_i\} \qquad f_j \in \{f_i\}}{\Gamma \vdash e.f_j : \tau_j} \quad \text{(field access)}$$

For a chain \(x.f_1{\cdots}f_n\), this rule is applied \(n\) times in sequence starting from \(\Gamma \vdash x : \Gamma(x)\).

$$\dfrac{\Gamma \vdash e_1 : \tau \quad \Gamma \vdash e_2 : \tau \quad \tau \in \{\texttt{String},\ \texttt{Number}\}}{\Gamma \vdash e_1 + e_2 : \tau} \quad \text{(addition)}$$

$$\dfrac{\Gamma \vdash e_1 : \tau_1 \quad \Gamma \vdash e_2 : \tau_2 \quad \text{comparable}(\tau_1, \tau_2)}{\Gamma \vdash e_1\ {==}\ e_2 : \texttt{Bool}} \quad \text{(equality)}$$

$$\dfrac{\Gamma \vdash e_1 : \tau_1 \quad \Gamma \vdash e_2 : \tau_2 \quad \text{comparable}(\tau_1, \tau_2)}{\Gamma \vdash e_1\ {!=}\ e_2 : \texttt{Bool}} \quad \text{(inequality)}$$

where \(\text{comparable}(\tau_1, \tau_2)\) holds when \(\tau_1\) and \(\tau_2\) are mutually assignable, or when one is \(\texttt{Null}\) and the other is \(\texttt{Option}[\tau]\), or when one is \(\texttt{Enum}[E]\) and the other is \(\texttt{String}\).

$$\dfrac{\forall\, i\colon\ \Gamma \vdash e_i : \tau_i}{\Gamma \vdash \{k_i{:}e_i\} : \texttt{Obj}\{k_i{:}\tau_i\}} \quad \text{(object literal)}$$

$$\dfrac{\forall\, i\colon\ \Gamma \vdash e_i : \tau}{\Gamma \vdash [e_1,\dots,e_n] : \texttt{List}[\tau]} \quad \text{(list literal — all items same type)}$$

$$\dfrac{}{\Gamma \vdash \texttt{null} : \texttt{Null}} \quad \text{(null literal)}$$

### Statement typing and environment extension

Statements are typed with a sequencing judgment \(\Gamma \vdash s \dashv \Gamma'\), where \(\Gamma'\) is the environment available to subsequent statements.

**Run statement:**

$$\dfrac{
  \Sigma(t) = ((k_1{:}\tau_1,\dots,k_n{:}\tau_n) \to \tau_o) \qquad \Gamma \vdash e_i : \tau_i \quad \forall\, i
}{
  \Gamma \vdash \texttt{let}\ x = \texttt{run}\ t\ \texttt{with}\ \{k_i{:}e_i\}\ [\texttt{by}\ a]\ [\texttt{retries}\ n]\ \cdots \dashv \Gamma[x \mapsto \tau_o]
}$$

with side-condition \(a \in \text{dom}(\Delta)\) when `by a` is present.

If \(t\) is an agent task, then `by a` is required.

**Fallback policy:**

$$\dfrac{\Gamma \vdash e_f : \tau_o}{\Gamma \vdash \texttt{on\_fail use}\ e_f : \tau_o}$$

The fallback expression type must equal the task's return type \(\tau_o\). Checked statically.

### Conditional typing and environment merging

Both branches receive a copy of \(\Gamma\) and may independently extend it. The environment after the `if` is computed by the **branch merge** \(\Gamma \sqcap (\Gamma_1, \Gamma_2)\):

$$\Gamma \sqcap (\Gamma_1, \Gamma_2)\ =\ \{\ x \mapsto \tau\ \mid\ x \in \text{dom}(\Gamma_1) \cap \text{dom}(\Gamma_2),\ \Gamma_1(x) = \Gamma_2(x) = \tau\ \}$$

Variables that appear in both branches but with incompatible types, and variables introduced in only one branch, are excluded from the merged environment and cannot be referenced by subsequent statements.

**With else:**

$$\dfrac{
  \Gamma \vdash e_c : \texttt{Bool} \qquad
  \Gamma \vdash s_{then}^*\ \dashv \Gamma_{then} \qquad
  \Gamma \vdash s_{else}^*\ \dashv \Gamma_{else}
}{
  \Gamma \vdash \texttt{if}\ e_c\ \{s_{then}^*\}\ \texttt{else}\ \{s_{else}^*\}\ \dashv\ \Gamma \sqcap (\Gamma_{then},\ \Gamma_{else})
}$$

**Without else:**

$$\dfrac{
  \Gamma \vdash e_c : \texttt{Bool} \qquad
  \Gamma \vdash s_{then}^*\ \dashv \Gamma_{then}
}{
  \Gamma \vdash \texttt{if}\ e_c\ \{s_{then}^*\}\ \dashv\ \Gamma \sqcap (\Gamma_{then},\ \Gamma)
}$$

When `else` is absent, the merge treats the implicit false branch as leaving \(\Gamma\) unchanged. A variable re-bound only in the `if` branch is therefore dropped from the merged environment.

**If-let:**

$$\dfrac{
  \Gamma \vdash e_o : \texttt{Option}[\tau] \qquad
  \Gamma[x \mapsto \tau] \vdash s_{then}^*\ \dashv \Gamma_{then} \qquad
  \Gamma \vdash s_{else}^*\ \dashv \Gamma_{else}
}{
  \Gamma \vdash \texttt{if let}\ x = e_o\ \{s_{then}^*\}\ \texttt{else}\ \{s_{else}^*\}\ \dashv\ \Gamma \sqcap (\Gamma_{then},\ \Gamma_{else})
}$$

The binding \(x\) is available only inside the successful unwrap branch and is excluded from the merged environment unless both branches independently bind the same name with the same type.

**While:**

The loop condition must have type `Bool`. The post-loop environment is conservatively merged with the pre-loop environment just like an `if` without `else`, because the loop body may execute zero or many times.

`break` and `continue` are only well-typed inside loop bodies.

### Try/catch typing

$$\dfrac{
  \Gamma \vdash s_{try}^*\ \dashv \Gamma_{try} \qquad
  \Gamma[x_{err} \mapsto \texttt{String}] \vdash s_{catch}^*\ \dashv \Gamma_{catch}
}{
  \Gamma \vdash \texttt{try}\ \{s_{try}^*\}\ \texttt{catch}\ x_{err}\ \{s_{catch}^*\}\ \dashv\ \Gamma \sqcap (\Gamma_{try},\ \Gamma_{catch})
}$$

The error variable \(x_{err}\) is bound as `String` only inside the catch block. The post-block environment merges both branches, as either may execute.

### Assert typing

$$\dfrac{
  \Gamma \vdash e : \texttt{Bool}
}{
  \Gamma \vdash \texttt{assert}\ e,\ c\ \dashv \Gamma
}$$

The assertion expression must have type `Bool`. If `false` at runtime, execution halts. The environment is unchanged.

### Pipeline-as-run-target typing

When a `run` statement targets a pipeline \(W'\) instead of a task:

$$\dfrac{
  W'.\text{params} = ((k_1{:}\tau_1,\dots,k_n{:}\tau_n)) \qquad W'.\text{return} = \tau_o \qquad \Gamma \vdash e_i : \tau_i \quad \forall\, i
}{
  \Gamma \vdash \texttt{let}\ x = \texttt{run}\ W'\ \texttt{with}\ \{k_i{:}e_i\}\ \dashv \Gamma[x \mapsto \tau_o]
}$$

The same argument-matching and return-type rules apply as for task targets.

### Enum typing

$$\dfrac{
  v \in \text{variants}(\mathcal{N}(E))
}{
  \Gamma \vdash v : \texttt{Enum}[E]
}$$

Enum literals are string constants validated against the declared variant set. A string literal that matches a variant is inferred as the corresponding `Enum[E]` type. Enum types are assignable to `String` (safe widening via the subtyping relation), but `String` is **not** assignable to `Enum[E]` — this ensures that only known-valid variants pass the static checker. Enum variant names must be globally unique across all enum declarations.

### Subtyping

AgentLang uses a structural subtyping relation \(\tau_a <: \tau_e\) ("actual is assignable to expected"):

$$\dfrac{}{\tau <: \tau} \quad \text{(reflexivity)}$$

$$\dfrac{}{\texttt{Enum}[E] <: \texttt{String}} \quad \text{(enum widening)}$$

$$\dfrac{\tau_a <: \tau_e}{\texttt{List}[\tau_a] <: \texttt{List}[\tau_e]} \quad \text{(list covariance)}$$

$$\dfrac{\text{dom}(\sigma_e) \subseteq \text{dom}(\sigma_a) \qquad \forall\, f \in \text{dom}(\sigma_e)\colon\ \sigma_a(f) <: \sigma_e(f)}{\texttt{Obj}\{\sigma_a\} <: \texttt{Obj}\{\sigma_e\}} \quad \text{(width + depth subtyping)}$$

$$\dfrac{}{\texttt{Null} <: \texttt{Option}[\tau]} \quad \text{(null assignable to option)}$$

$$\dfrac{\tau_a <: \tau_e}{\texttt{Option}[\tau_a] <: \texttt{Option}[\tau_e]} \quad \text{(option covariance)}$$

**Note on covariance:** List and Obj subtyping are covariant. This is classically unsound for mutable containers, but AgentLang has no mutation operators (no assignment, no list append, no field update). The runtime's `deepcopy` at handler boundaries prevents handler-side mutation from leaking. If mutation is ever added, these rules must be revised.

### Test block typing

$$\dfrac{
  \Gamma_0 = \emptyset \qquad \Gamma_0 \vdash s^*\ \dashv \Gamma'
}{
  \vdash \texttt{test}\ c\ \{s^*\} : \checkmark
}$$

Test blocks are checked in an empty initial environment (they have their own scope). They do not contribute to the program's pipeline environments.

### Parallel typing

$$\dfrac{
  \forall\, i \neq j\colon\ x_i \neq x_j \qquad
  \forall\, i\colon\ x_i \notin \text{dom}(\Gamma) \qquad
  \forall\, i\colon\ \Gamma \vdash \texttt{run}\ t_i\ \texttt{with}\ \vec{a}_i\ \cdots : \tau_i
}{
  \Gamma \vdash \texttt{parallel}\ \{\ \texttt{let}\ x_i = \texttt{run}\ t_i\ \vec{a}_i\ \}\ \texttt{join}\ \dashv\ \Gamma[x_1 \mapsto \tau_1,\ \dots,\ x_m \mapsto \tau_m]
}$$

All targets must be fresh (not already in \(\Gamma\)) and pairwise distinct. This ensures the result is a true disjoint extension of \(\Gamma\).

### Return typing

$$\dfrac{
  \Gamma \vdash e : \tau_{ret}
}{
  \Gamma \vdash \texttt{return}\ e : \checkmark
}$$

where \(\tau_{ret}\) is the pipeline's declared return type.

### Pipeline well-typedness

A pipeline \(W = \langle \text{name},\ \vec{p},\ \tau_{ret},\ \vec{s} \rangle\) is well-typed under \(\Sigma, \Delta\) when:

1. The initial environment \(\Gamma_0 = \{p_i : \tau_i\}\) is constructed from declared params
2. Every statement in \(\vec{s}\) type-checks under the accumulated \(\Gamma\), with each statement extending \(\Gamma\) for subsequent statements via the \(\dashv\) judgment
3. Every reachable `return` expression has type \(\tau_{ret}\)

---

## 3. Dynamic Semantics

### Runtime configuration

A runtime configuration is a pair \(\langle S,\ E \rangle\) where:

- \(S\) is the remaining statement stream
- \(E\) is the environment mapping variable names to runtime values

### Transition rules

**Run (success):**

$$\langle [\texttt{let}\ x = \texttt{run}\ t\ \vec{a}] \cdot S,\ E \rangle \;\longrightarrow\; \langle S,\ E[x \mapsto \text{handler}(t,\ \mathcal{A}\llbracket\vec{a}\rrbracket_E)] \rangle$$

For agent tasks, the handler is synthesized by the runtime: the bound agent selects a model and tool set, the model may emit tool calls, tool calls are executed against the runtime tool registry, and the final model output is decoded into a runtime value that must match the declared task return type.

**Retry:** on failure, re-submits the run statement with the retry counter decremented. When the budget reaches zero, the failure policy applies.

**Failure policy — abort:**

$$\langle [\texttt{let}\ x = \texttt{run}\ t\ \vec{a}\ \texttt{on\_fail abort}] \cdot S,\ E \rangle \;\xrightarrow{\text{fail}}\; \textbf{Error}$$

**Failure policy — use:**

$$\langle [\texttt{let}\ x = \texttt{run}\ t\ \vec{a}\ \texttt{on\_fail use}\ e_f] \cdot S,\ E \rangle \;\xrightarrow{\text{fail}}\; \langle S,\ E[x \mapsto \mathcal{E}\llbracket e_f \rrbracket_E] \rangle$$

**Parallel join:**

Each \(r_i \equiv \texttt{let}\ x_i = \texttt{run}\ t_i\ \vec{a}_i\). All branches execute concurrently against a snapshot of \(E\), producing values \(v_i = \text{handler}(t_i,\ \mathcal{A}\llbracket\vec{a}_i\rrbracket_E)\):

$$\langle [\texttt{parallel}\{r_1;\dots;r_m\}\texttt{ join}] \cdot S,\ E \rangle \;\longrightarrow\; \langle S,\ E \cup \{x_1 \mapsto v_1,\ \dots,\ x_m \mapsto v_m\} \rangle$$

All \(x_i\) are distinct and not in \(\text{dom}(E)\) (enforced statically), so the extension is disjoint.

**If (true branch, with else):**

$$\langle [\texttt{if}\ e_c\ \{s_{then}^*\}\ \texttt{else}\ \{s_{else}^*\}] \cdot S,\ E \rangle \;\longrightarrow\; \langle s_{then}^* \cdot S,\ E \rangle \quad \text{when}\ \mathcal{E}\llbracket e_c \rrbracket_E = \textit{true}$$

**If (false branch, with else):**

$$\langle [\texttt{if}\ e_c\ \{s_{then}^*\}\ \texttt{else}\ \{s_{else}^*\}] \cdot S,\ E \rangle \;\longrightarrow\; \langle s_{else}^* \cdot S,\ E \rangle \quad \text{when}\ \mathcal{E}\llbracket e_c \rrbracket_E = \textit{false}$$

**If (true branch, without else):**

$$\langle [\texttt{if}\ e_c\ \{s_{then}^*\}] \cdot S,\ E \rangle \;\longrightarrow\; \langle s_{then}^* \cdot S,\ E \rangle \quad \text{when}\ \mathcal{E}\llbracket e_c \rrbracket_E = \textit{true}$$

**If (false branch, without else — skip):**

$$\langle [\texttt{if}\ e_c\ \{s_{then}^*\}] \cdot S,\ E \rangle \;\longrightarrow\; \langle S,\ E \rangle \quad \text{when}\ \mathcal{E}\llbracket e_c \rrbracket_E = \textit{false}$$

**While:**

$$\langle [\texttt{while}\ e_c\ \{s^*\}] \cdot S,\ E \rangle \;\longrightarrow\; \langle s^* \cdot [\texttt{while}\ e_c\ \{s^*\}] \cdot S,\ E \rangle \quad \text{when}\ \mathcal{E}\llbracket e_c \rrbracket_E = \textit{true}$$

$$\langle [\texttt{while}\ e_c\ \{s^*\}] \cdot S,\ E \rangle \;\longrightarrow\; \langle S,\ E \rangle \quad \text{when}\ \mathcal{E}\llbracket e_c \rrbracket_E = \textit{false}$$

**Break:**

$$\langle [\texttt{break}] \cdot S,\ E \rangle \;\longrightarrow\; \langle S_{post\text{-}loop},\ E \rangle$$

where \(S_{post\text{-}loop}\) is the statement stream after removing the enclosing `while` and its remaining body. Equivalently, `break` raises a control-flow signal that unwinds to the nearest enclosing loop, which then terminates.

**Continue:**

$$\langle [\texttt{continue}] \cdot S,\ E \rangle \;\longrightarrow\; \langle [\texttt{while}\ e_c\ \{s^*\}] \cdot S_{post\text{-}loop},\ E \rangle$$

`continue` raises a control-flow signal that skips the remaining statements in the current iteration and re-evaluates the loop condition.

**If-let (some):**

$$\langle [\texttt{if let}\ x = e_o\ \{s_{then}^*\}\ \texttt{else}\ \{s_{else}^*\}] \cdot S,\ E \rangle \;\longrightarrow\; \langle s_{then}^* \cdot S,\ E[x \mapsto v] \rangle \quad \text{when}\ \mathcal{E}\llbracket e_o \rrbracket_E = v \neq \texttt{null}$$

The binding \(x\) is scoped to the `then` branch only. After the branch completes, \(x\) is removed from the environment (or restored to its prior value if it existed before the `if let`).

**If-let (none):**

$$\langle [\texttt{if let}\ x = e_o\ \{s_{then}^*\}\ \texttt{else}\ \{s_{else}^*\}] \cdot S,\ E \rangle \;\longrightarrow\; \langle s_{else}^* \cdot S,\ E \rangle \quad \text{when}\ \mathcal{E}\llbracket e_o \rrbracket_E = \texttt{null}$$

When `else` is absent and the value is `null`, execution skips to the next statement.

**Try/catch (success):**

$$\langle [\texttt{try}\ \{s_{try}^*\}\ \texttt{catch}\ x_{err}\ \{s_{catch}^*\}] \cdot S,\ E \rangle \;\longrightarrow\; \langle s_{try}^* \cdot S,\ E \rangle$$

When the try block completes without error, execution continues after the try/catch with the environment extended by the try block.

**Try/catch (failure):**

$$\langle [\texttt{try}\ \{s_{try}^*\}\ \texttt{catch}\ x_{err}\ \{s_{catch}^*\}] \cdot S,\ E \rangle \;\xrightarrow{\text{fail}(m)}\; \langle s_{catch}^* \cdot S,\ E[x_{err} \mapsto m] \rangle$$

When a statement in the try block raises an `ExecutionError` with message \(m\), execution jumps to the catch block with the error variable bound as a `String`.

**Try/catch (control-flow passthrough):**

Control-flow signals (`return`, `break`, `continue`) raised inside a `try` block are **not** caught — they propagate through the try/catch boundary as if it were not present. Only `ExecutionError` exceptions (task failures, assertion failures, etc.) are caught. This ensures that `return` inside `try` exits the enclosing pipeline, and `break`/`continue` inside `try` affect the enclosing loop.

**Timeout (non-retryable):**

When a task handler exceeds its `timeout` deadline, a `HandlerTimeoutError` (subclass of `ExecutionError`) is raised. Unlike other execution errors, timeout errors are **not retryable** — the retry loop immediately falls through to the failure policy. This is because the timed-out handler thread may still be running in the background, and retrying would risk overlapping side-effects.

**Assert (pass):**

$$\langle [\texttt{assert}\ e,\ c] \cdot S,\ E \rangle \;\longrightarrow\; \langle S,\ E \rangle \quad \text{when}\ \mathcal{E}\llbracket e \rrbracket_E = \textit{true}$$

**Assert (fail):**

$$\langle [\texttt{assert}\ e,\ c] \cdot S,\ E \rangle \;\longrightarrow\; \textbf{AssertionError}(c) \quad \text{when}\ \mathcal{E}\llbracket e \rrbracket_E = \textit{false}$$

**Pipeline call:**

$$\langle [\texttt{let}\ x = \texttt{run}\ W'\ \texttt{with}\ \{k_i{:}e_i\}] \cdot S,\ E \rangle \;\longrightarrow\; \langle S,\ E[x \mapsto \text{exec}(W',\ \{k_i \mapsto \mathcal{E}\llbracket e_i \rrbracket_E\})] \rangle$$

The target pipeline \(W'\) is executed in a fresh scope with its parameters bound from the arguments. The result is the pipeline's return value.

**Return:**

$$\langle [\texttt{return}\ e] \cdot S,\ E \rangle \;\longrightarrow\; \mathcal{E}\llbracket e \rrbracket_E$$

---

## 4. Determinism

**Theorem (expression determinism).** For any closed expression \(e\) and environment \(E\), \(\mathcal{E}\llbracket e \rrbracket_E\) is uniquely defined.

**Corollary.** A pipeline whose task handlers are all pure functions produces a deterministic output for any given input.

The parallel join is deterministic in its *result set* but not in *execution order* — branch interleavings are unspecified. Final outputs are deterministic because branches bind disjoint names and the merge is a union.

The runtime additionally enforces that task handler outputs and final pipeline return values conform to their declared DSL types; malformed runtime values are execution errors even if the surrounding program parsed and type-checked successfully.

Declared tools are also validated dynamically: tool call arguments and tool results must conform to their DSL signatures.
