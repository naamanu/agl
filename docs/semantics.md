# AgentLang v0 Semantics

## 1. Abstract Syntax

Program:

\[
P ::= \langle A, T, W \rangle
\]

- \(A\): agent definitions
- \(T\): task signatures
- \(W\): pipeline definitions

Task signature:

\[
t : (x_1:\tau_1, \dots, x_n:\tau_n) \to \tau_o
\]

Pipeline statement forms:

\[
s ::= \texttt{let }x=\texttt{run }t\ \texttt{with }\{k_i:e_i\}\ [\texttt{by }a]\ [\texttt{retries }n]\ [\texttt{on\_fail abort}]
\]
\[
\quad\mid\ \texttt{let }x=\texttt{run }t\ \texttt{with }\{k_i:e_i\}\ [\texttt{by }a]\ [\texttt{retries }n]\ \texttt{on\_fail use }e_f
\]
\[
\quad\mid\ \texttt{parallel }\{s_1;\dots;s_m;\}\ \texttt{join}
\]
\[
\quad\mid\ \texttt{if }e\ \{s^*\}\ [\texttt{else }\{s^*\}]
\]
\[
\quad\mid\ \texttt{return }e
\]

Expressions:

\[
e ::= c \mid x \mid x.f \mid \{k_i:e_i\} \mid [e^*] \mid e + e \mid e == e \mid e != e
\]

Types:

\[
\tau ::= \texttt{String} \mid \texttt{Number} \mid \texttt{Bool} \mid \texttt{List}[\tau] \mid \texttt{Obj}\{f_i:\tau_i\}
\]

## 2. Static Semantics

Typing environment:

\[
\Gamma : \text{Var} \to \tau
\]

Task table:

\[
\Sigma : \text{TaskName} \to (\vec{\tau}_{in}, \tau_{out})
\]

Agent table:

\[
\Delta : \text{AgentName} \to \text{AgentSpec}
\]

Task invocation typing:

\[
\frac{
\Sigma(t) = ((k_1:\tau_1,\dots,k_n:\tau_n)\to\tau_o)\quad
\Gamma \vdash e_i : \tau_i\ \forall i
}{
\Gamma \vdash \texttt{run }t\ \texttt{with }\{k_i:e_i\}\ [\texttt{by }a]\ [\texttt{retries }n]\ [\texttt{on\_fail abort}] : \tau_o
}
\]

with side-condition \(a \in dom(\Delta)\) when `by a` exists.

Fallback policy typing:

\[
\frac{
\Gamma \vdash e_f : \tau_o
}{
\Gamma \vdash \texttt{on\_fail use }e_f : \tau_o
}
\]

If typing:

\[
\frac{
\Gamma \vdash e_c : \texttt{Bool}\quad
\Gamma \vdash s_{then}^*\quad
\Gamma \vdash s_{else}^*
}{
\Gamma \vdash \texttt{if }e_c\{s_{then}^*\}\texttt{ else }\{s_{else}^*\} : \checkmark
}
\]

Return typing:

\[
\frac{\Gamma \vdash e : \tau_{ret}}{
\Gamma \vdash \texttt{return }e : \checkmark
}
\]

Pipeline well-typedness:
- every variable reference is bound,
- every task call argument type matches declared task input type,
- `return` expression type equals declared pipeline return type.

## 3. Dynamic Semantics

Runtime configuration:

\[
\langle S, E \rangle
\]

- \(S\): current statement stream
- \(E\): environment (runtime values)

Transitions (informal):

1. **Run**: evaluate args in \(E\), invoke task handler.
2. **Retry**: on failure, re-run up to `retries` budget.
3. **Failure Policy**:
   - `on_fail abort`: raise runtime error.
   - `on_fail use e_f`: evaluate fallback expression \(e_f\) and bind to target.
4. **Parallel Join**: execute run branches concurrently against the current \(E\) snapshot; merge outputs into \(E\) after all branches complete.
5. **If**: evaluate condition; execute only the selected branch.
6. **Return**: evaluate return expression in \(E\) and terminate.

## 4. Determinism

- Expression evaluation and branch selection are deterministic.
- Final outputs are deterministic only if task handlers are deterministic and side-effect free.
