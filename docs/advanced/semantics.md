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
  \mid\ & \texttt{parallel}\ \{s_1;\dots;s_m\}\ \texttt{join} \\
  \mid\ & \texttt{if}\ e\ \{s^*\}\ [\texttt{else}\ \{s^*\}] \\
  \mid\ & \texttt{return}\ e
\end{align*}$$

**Expression forms:**

$$\begin{align*}
e\ ::=\ & c \mid x \mid x.f \mid \{k_i{:}e_i\} \mid [e^*] \\
  \mid\ & e + e \mid e\ {==}\ e \mid e\ {!=}\ e
\end{align*}$$

**Types:**

$$\tau\ ::=\ \texttt{String} \mid \texttt{Number} \mid \texttt{Bool} \mid \texttt{List}[\tau] \mid \texttt{Obj}\{f_i{:}\tau_i\}$$

---

## 2. Static Semantics

### Environments

| Symbol | Definition |
|---|---|
| \(\Gamma : \text{Var} \to \tau\) | Typing environment — maps variable names to their types |
| \(\Sigma : \text{TaskName} \to (\vec{\tau}_{in}, \tau_{out})\) | Task table — maps task names to their signatures |
| \(\Delta : \text{AgentName} \to \text{AgentSpec}\) | Agent table — maps agent names to their specs |

### Expression typing

Selected rules:

$$\dfrac{}{\Gamma \vdash c : \text{type}(c)} \quad \text{(literal)}$$

$$\dfrac{x \in \text{dom}(\Gamma)}{\Gamma \vdash x : \Gamma(x)} \quad \text{(variable)}$$

$$\dfrac{\Gamma \vdash e : \texttt{Obj}\{f_i{:}\tau_i\} \quad f_j \in \{f_i\}}{\Gamma \vdash e.f_j : \tau_j} \quad \text{(field access)}$$

$$\dfrac{\Gamma \vdash e_1 : \tau \quad \Gamma \vdash e_2 : \tau}{\Gamma \vdash e_1\ {==}\ e_2 : \texttt{Bool}} \quad \text{(equality)}$$

### Task invocation typing

$$\dfrac{
  \Sigma(t) = ((k_1{:}\tau_1,\dots,k_n{:}\tau_n) \to \tau_o) \qquad
  \Gamma \vdash e_i : \tau_i \quad \forall\, i
}{
  \Gamma \vdash \texttt{run}\ t\ \texttt{with}\ \{k_i{:}e_i\}\ [\texttt{by}\ a]\ [\texttt{retries}\ n]\ [\texttt{on\_fail abort}] : \tau_o
}$$

with side-condition \(a \in \text{dom}(\Delta)\) when `by a` is present.

### Fallback policy typing

$$\dfrac{
  \Gamma \vdash e_f : \tau_o
}{
  \Gamma \vdash \texttt{on\_fail use}\ e_f : \tau_o
}$$

The fallback expression type must equal the task's return type \(\tau_o\).

### Conditional typing

$$\dfrac{
  \Gamma \vdash e_c : \texttt{Bool} \qquad
  \Gamma \vdash s_{then}^* \qquad
  \Gamma \vdash s_{else}^*
}{
  \Gamma \vdash \texttt{if}\ e_c\ \{s_{then}^*\}\ \texttt{else}\ \{s_{else}^*\} : \checkmark
}$$

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
2. Every statement in \(\vec{s}\) type-checks under the accumulated \(\Gamma\)
3. Every reachable `return` expression has type \(\tau_{ret}\)

---

## 3. Dynamic Semantics

### Runtime configuration

A runtime configuration is a pair \(\langle S,\ E \rangle\) where:

- \(S\) is the remaining statement stream
- \(E\) is the environment mapping variable names to runtime values

### Transition rules

**Run (success):**

$$\langle [\texttt{let}\ x = \texttt{run}\ t\ \vec{a}] \cdot S,\ E \rangle \;\longrightarrow\; \langle S,\ E[x \mapsto \text{handler}(t,\ \vec{a})] \rangle$$

**Retry:** on failure, re-submits the run statement with the retry counter decremented. When the budget reaches zero, the failure policy applies.

**Failure policy — abort:**

$$\langle [\texttt{let}\ x = \texttt{run}\ t\ \vec{a}\ \texttt{on\_fail abort}] \cdot S,\ E \rangle \;\xrightarrow{\text{fail}}\; \textbf{Error}$$

**Failure policy — use:**

$$\langle [\texttt{let}\ x = \texttt{run}\ t\ \vec{a}\ \texttt{on\_fail use}\ e_f] \cdot S,\ E \rangle \;\xrightarrow{\text{fail}}\; \langle S,\ E[x \mapsto \mathcal{E}\llbracket e_f \rrbracket_E] \rangle$$

**Parallel join:**

$$\langle [\texttt{parallel}\{s_1;\dots;s_m\}\texttt{ join}] \cdot S,\ E \rangle \;\longrightarrow\; \left\langle S,\ E \cup \bigsqcup_{i=1}^{m} E_i \right\rangle$$

where each \(E_i\) is the result of running \(s_i\) against a snapshot of \(E\), and \(\bigsqcup\) denotes disjoint union (all bound names are distinct by the uniqueness constraint).

**If (true branch):**

$$\langle [\texttt{if}\ e_c\ \{s_{then}^*\}\ \texttt{else}\ \{s_{else}^*\}] \cdot S,\ E \rangle \;\longrightarrow\; \langle s_{then}^* \cdot S,\ E \rangle \quad \text{when}\ \mathcal{E}\llbracket e_c \rrbracket_E = \textit{true}$$

**Return:**

$$\langle [\texttt{return}\ e] \cdot S,\ E \rangle \;\longrightarrow\; \mathcal{E}\llbracket e \rrbracket_E$$

---

## 4. Determinism

**Theorem (expression determinism).** For any closed expression \(e\) and environment \(E\), \(\mathcal{E}\llbracket e \rrbracket_E\) is uniquely defined.

**Corollary.** A pipeline whose task handlers are all pure functions produces a deterministic output for any given input.

The parallel join is deterministic in its *result set* but not in *execution order* — branch interleavings are unspecified. Final outputs are deterministic because branches bind disjoint names and the merge is a union.
