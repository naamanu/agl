# Formal Semantics

This page gives a precise mathematical account of AgentLang v0: abstract syntax, static typing rules, and dynamic (small-step) semantics.

## 1. Abstract Syntax

A program is a triple of agent definitions, task signatures, and pipeline definitions:

$$P ::= \langle A, T, W \rangle$$

where \(A\) is the set of agent definitions, \(T\) the set of task signatures, and \(W\) the set of pipeline definitions.

A task signature declares its input parameters and return type:

$$t : (x_1{:}\tau_1,\ \dots,\ x_n{:}\tau_n) \to \tau_o$$

**Pipeline statement forms:**

$$\begin{align*}
s\ ::=\ & \texttt{let}\ x = \texttt{run}\ t\ \texttt{with}\ \{k_i{:}e_i\}\ [\texttt{by}\ a]\ [\texttt{retries}\ n]\ [\texttt{on\_fail abort}] \\
  \mid\ & \texttt{let}\ x = \texttt{run}\ t\ \texttt{with}\ \{k_i{:}e_i\}\ [\texttt{by}\ a]\ [\texttt{retries}\ n]\ \texttt{on\_fail use}\ e_f \\
  \mid\ & \texttt{parallel}\ \{r_1;\dots;r_m\}\ \texttt{join} \\
  \mid\ & \texttt{if}\ e\ \{s^*\}\ [\texttt{else}\ \{s^*\}] \\
  \mid\ & \texttt{if let}\ x = e\ \{s^*\}\ [\texttt{else}\ \{s^*\}] \\
  \mid\ & \texttt{return}\ e
\end{align*}$$

Each \(r_i\) inside a `parallel` block is restricted to the run statement form (`let x = run ...`). General statements (`if`, `return`, nested `parallel`) are not permitted inside `parallel`.

**Expression forms:**

Variable references and field access chains are unified into a single form \(x.f_1{\cdots}f_n\) where \(n \geq 0\) (\(n = 0\) is a plain variable reference, \(n \geq 1\) is a field access chain of depth \(n\)):

$$\begin{align*}
e\ ::=\ & c \mid \texttt{null} \mid x.f_1{\cdots}f_n \mid (e) \mid \{k_i{:}e_i\} \mid [e^*] \\
  \mid\ & e + e \mid e\ {==}\ e \mid e\ {!=}\ e
\end{align*}$$

**Types:**

$$\tau\ ::=\ \texttt{String} \mid \texttt{Number} \mid \texttt{Bool} \mid \texttt{List}[\tau] \mid \texttt{Option}[\tau] \mid \texttt{Obj}\{f_i{:}\tau_i\}$$

---

## 2. Static Semantics

### Environments

| Symbol | Definition |
|---|---|
| \(\Gamma : \text{Var} \to \tau\) | Typing environment — maps variable names to their types |
| \(\Sigma : \text{TaskName} \to (\vec{\tau}_{in}, \tau_{out})\) | Task table — maps task names to their signatures |
| \(\Delta : \text{AgentName} \to \text{AgentSpec}\) | Agent table — maps agent names to their specs |

### Expression typing

$$\dfrac{}{\Gamma \vdash c : \text{type}(c)} \quad \text{(literal)}$$

$$\dfrac{x \in \text{dom}(\Gamma)}{\Gamma \vdash x : \Gamma(x)} \quad \text{(variable)}$$

Field access chains are typed by iterating the one-step rule:

$$\dfrac{\Gamma \vdash e : \texttt{Obj}\{f_i{:}\tau_i\} \qquad f_j \in \{f_i\}}{\Gamma \vdash e.f_j : \tau_j} \quad \text{(field access)}$$

For a chain \(x.f_1{\cdots}f_n\), this rule is applied \(n\) times in sequence starting from \(\Gamma \vdash x : \Gamma(x)\).

$$\dfrac{\Gamma \vdash e_1 : \tau \quad \Gamma \vdash e_2 : \tau \quad \tau \in \{\texttt{String},\ \texttt{Number}\}}{\Gamma \vdash e_1 + e_2 : \tau} \quad \text{(addition)}$$

$$\dfrac{\Gamma \vdash e_1 : \tau \quad \Gamma \vdash e_2 : \tau}{\Gamma \vdash e_1\ {==}\ e_2 : \texttt{Bool}} \quad \text{(equality)}$$

$$\dfrac{\Gamma \vdash e_1 : \tau \quad \Gamma \vdash e_2 : \tau}{\Gamma \vdash e_1\ {!=}\ e_2 : \texttt{Bool}} \quad \text{(inequality)}$$

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

**If-let:**

If \(\mathcal{E}\llbracket e_o \rrbracket_E = v \neq \texttt{null}\), execute the `then` branch with \(x\) bound to \(v\). If it is `null`, execute the `else` branch if present, otherwise skip.

**Return:**

$$\langle [\texttt{return}\ e] \cdot S,\ E \rangle \;\longrightarrow\; \mathcal{E}\llbracket e \rrbracket_E$$

---

## 4. Determinism

**Theorem (expression determinism).** For any closed expression \(e\) and environment \(E\), \(\mathcal{E}\llbracket e \rrbracket_E\) is uniquely defined.

**Corollary.** A pipeline whose task handlers are all pure functions produces a deterministic output for any given input.

The parallel join is deterministic in its *result set* but not in *execution order* — branch interleavings are unspecified. Final outputs are deterministic because branches bind disjoint names and the merge is a union.

The runtime additionally enforces that task handler outputs and final pipeline return values conform to their declared DSL types; malformed runtime values are execution errors even if the surrounding program parsed and type-checked successfully.
