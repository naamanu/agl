# Sequential core semantics

Status: sequential control-flow model with Rust regression coverage.
This formalization covers the core shared by AGL 0.2–0.6 and refines the
[semantics overview](semantics.md). Control-flow propagation and lexical
restoration are implemented and tested. Expressions and calls remain abstract;
this is not a complete correspondence or soundness proof for AGL.

## Scope and observations

We model sequencing, task-result binding, conditionals, optional binding, loops,
exceptions, assertions, and return. Programs are already parsed and names resolved.
Expressions and calls are explicit parameters of the semantics. Concurrency,
retry scheduling, budgets, cancellation, approval suspension, persistence, and
module resolution require additional judgments and are outside this core.

The observable result is a completion together with an environment. An execution
error retains the environment at the point of failure: `try` is not a transaction.
External effects are not rolled back either, but their histories are not modeled
here. `Err(v)` is an ordinary tagged value, distinct from an execution error.

## Syntax and judgments

Let names be \\(x\\), values be \\(v\\), execution errors be \\(\\epsilon\\), and finite
environments be \\(\\rho : \\mathrm\{Name\} \\rightharpoonup \\mathrm\{Value\}\\).
Values include Booleans, numbers, strings, null, lists, and objects (including
tagged records, unions, and results). A block \\(B\\) is a finite statement sequence:

\\[
\\begin\{aligned\}
s ::= \{\}& \\operatorname\{bind\}(x,r)
  \\mid \\operatorname\{if\}(e,B\_t,B\_f)
  \\mid \\operatorname\{iflet\}(x,e,B\_t,B\_f) \\\\
 &\\mid \\operatorname\{while\}(e,B)
  \\mid \\operatorname\{break\} \\mid \\operatorname\{continue\} \\\\
 &\\mid \\operatorname\{try\}(B\_t,x,h,B\_c)
  \\mid \\operatorname\{assert\}(e,m) \\mid \\operatorname\{return\}(e).
\\end\{aligned\}
\\]

Here \\(r\\) is a complete `run` statement without its target binding; \\(h\\) selects
string or structured catch payloads. Missing `else` means the empty block.

Completions are disjoint constructors:

\\[
q ::= N \\mid R(v) \\mid Bk \\mid Ct \\mid X(\\epsilon).
\\]

They mean normal completion, return, break, continue, and execution error.
We use the terminating big-step judgments

\\[
\\langle s,\\rho\\rangle\\Downarrow(q,\\rho')
\\qquad
\\langle B,\\rho\\rangle\\Downarrow(q,\\rho').
\\]

The expression interface is \\(\\operatorname\{eval\}(e,\\rho)\\in
\\operatorname\{Val\}(v)+\\operatorname\{Error\}(\\epsilon)\\), a deterministic partial
function. Undefined evaluation denotes divergence, not an arbitrary value.
The abstract call relation is \\(\\langle r,\\rho\\rangle\\Downarrow\_\{call\}o\\),
where \\(o\\) is a value or execution error. Calls cannot mutate the caller's
environment. They may have external effects and need not be deterministic.
Nested pipelines use fresh parameter environments and convert \\(R(v)\\) to a call
value; escaping \\(N\\), \\(Bk\\), or \\(Ct\\) is a call error. Return values must pass the
declared runtime type check before crossing the call boundary.

## Sequencing and primitive statements

\\[
\\frac\{\}\{\\langle \[\],\\rho\\rangle\\Downarrow(N,\\rho)\}
\\qquad
\\frac\{\\langle s,\\rho\\rangle\\Downarrow(N,\\rho\_1)\\quad
      \\langle B,\\rho\_1\\rangle\\Downarrow(q,\\rho\_2)\}
     \{\\langle s::B,\\rho\\rangle\\Downarrow(q,\\rho\_2)\}
\\]

\\[
\\frac\{\\langle s,\\rho\\rangle\\Downarrow(q,\\rho')\\quad q\\ne N\}
     \{\\langle s::B,\\rho\\rangle\\Downarrow(q,\\rho')\}
\\]

For a successful call, binding replaces an existing name if present:

\\[
\\frac\{\\langle r,\\rho\\rangle\\Downarrow\_\{call\}\\operatorname\{Val\}(v)\}
 \{\\langle\\operatorname\{bind\}(x,r),\\rho\\rangle\\Downarrow(N,\\rho\[x\\mapsto v\])\}
\\qquad
\\frac\{\\langle r,\\rho\\rangle\\Downarrow\_\{call\}\\operatorname\{Error\}(\\epsilon)\}
 \{\\langle\\operatorname\{bind\}(x,r),\\rho\\rangle\\Downarrow(X(\\epsilon),\\rho)\}
\\]

`return e` completes with \\(R(v)\\) when evaluation gives \\(\\operatorname\{Val\}(v)\\).
`break` and `continue` complete with \\(Bk\\) and \\(Ct\\). All three leave \\(\\rho\\)
unchanged. `assert true, m` completes with \\(N\\); `assert false, m` completes
with \\(X(\\operatorname\{assertion\}(m))\\).

For every statement that first evaluates an expression, an evaluation error
immediately gives \\((X(\\epsilon),\\rho)\\) without entering a branch or body.
Conditions are required to evaluate to Booleans; another value produces a
condition-type error in this proposed contract. This deliberately does not
adopt the Rust runtime's truthiness coercion for unchecked inputs.

## Conditionals and temporary bindings

An `if` evaluates its condition once, then has exactly the completion and
environment of the selected block (\\(B\_t\\) for true, \\(B\_f\\) for false).

Define restoration of one name, leaving all other final bindings unchanged:

\\[
\\operatorname\{restore\}\_x(\\rho\_0,\\rho\_1)=
\\begin\{cases\}
\\rho\_1\[x\\mapsto\\rho\_0(x)\] & x\\in\\operatorname\{dom\}(\\rho\_0),\\\\
\\rho\_1\\setminus\\\{x\\\} & \\text\{otherwise\}.
\\end\{cases\}
\\]

An `if let` whose expression is null executes \\(B\_f\\) in \\(\\rho\\).
For a non-null value:

\\[
\\frac\{\\operatorname\{eval\}(e,\\rho)=\\operatorname\{Val\}(v)\\quad v\\ne\\mathrm\{null\}
 \\quad\\langle B\_t,\\rho\[x\\mapsto v\]\\rangle\\Downarrow(q,\\rho')\}
 \{\\langle\\operatorname\{iflet\}(x,e,B\_t,B\_f),\\rho\\rangle
  \\Downarrow(q,\\operatorname\{restore\}\_x(\\rho,\\rho'))\}
\\]

Restoration applies to **every** completion, including errors. Returning a value
does not undo that value; restoration changes the environment only.

## Loops

Write \\(W=\\operatorname\{while\}(e,B)\\). A false condition gives \\((N,\\rho)\\).
For a true condition, the complete body case analysis is:

| Body completion from \\(\\rho\\) | Loop behavior |
|---|---|
| \\((N,\\rho\_1)\\) or \\((Ct,\\rho\_1)\\) | Evaluate \\(W\\) again in \\(\\rho\_1\\); use its result |
| \\((Bk,\\rho\_1)\\) | Complete with \\((N,\\rho\_1)\\) |
| \\((R(v),\\rho\_1)\\) | Complete with \\((R(v),\\rho\_1)\\) |
| \\((X(\\epsilon),\\rho\_1)\\) | Complete with \\((X(\\epsilon),\\rho\_1)\\) |

For example, the iteration rule is:

\\[
\\frac\{\\operatorname\{eval\}(e,\\rho)=\\operatorname\{Val\}(\\mathrm\{true\})\\quad
 \\langle B,\\rho\\rangle\\Downarrow(q\_1,\\rho\_1)\\quad q\_1\\in\\\{N,Ct\\\}\\quad
 \\langle W,\\rho\_1\\rangle\\Downarrow(q,\\rho\_2)\}
 \{\\langle W,\\rho\\rangle\\Downarrow(q,\\rho\_2)\}.
\\]

Each loop consumes only its own body's break/continue. An inner loop consumes
those signals before its containing block sees them. An infinite loop has no
finite derivation; lack of a derivation alone does not prove divergence because
the call relation is abstract.

## Exceptions

Let \\(\\operatorname\{payload\}\_h(\\epsilon)\\) encode an execution error as a string
or a `Failure` value. A try block's non-error completion passes through:

\\[
\\frac\{\\langle B\_t,\\rho\\rangle\\Downarrow(q,\\rho')\\quad
       q\\in\\\{N,R(v),Bk,Ct\\\}\}
 \{\\langle\\operatorname\{try\}(B\_t,x,h,B\_c),\\rho\\rangle\\Downarrow(q,\\rho')\}.
\\]

On error, the catch starts from the **failure environment**, not the entry
environment. Only the catch variable is temporary:

\\[
\\frac\{\\langle B\_t,\\rho\\rangle\\Downarrow(X(\\epsilon),\\rho\_1)\\quad
 \\langle B\_c,\\rho\_1\[x\\mapsto\\operatorname\{payload\}\_h(\\epsilon)\]\\rangle
 \\Downarrow(q,\\rho\_2)\}
 \{\\langle\\operatorname\{try\}(B\_t,x,h,B\_c),\\rho\\rangle
 \\Downarrow(q,\\operatorname\{restore\}\_x(\\rho\_1,\\rho\_2))\}.
\\]

A catch error propagates outward; the same catch does not handle it again.
The runtime performs this lexical cleanup for catch bindings, including when
the handler returns, breaks, continues, or raises another execution error.

## Implementation and regression cases

`block` and `with_bindings` in the runtime implement completion propagation and
lexical restoration. The checker uses an internal `BlockFlow` summary, with
separate normal, return, break, continue, and failure outcomes. `None` denotes
an unreachable outcome; an empty environment denotes a reachable outcome with
no definitely available names.

The executable regressions in `tests/semantics.rs` and `tests/conformance/` cover:

| Obligation | Corrected behavior |
|---|---|
| Try passes through non-error completions | Return, break, and continue reach their enclosing boundary |
| Temporary bindings restore on every exit | Optional, match, and catch names restore or disappear before propagation |
| Returns are checked independently | Every reachable return has a compatible type and its own source diagnostic |
| Parallel branches share only the entry scope | A sibling's result cannot be referenced before the join |
| Loops preserve back-edge types | Normal/continue paths preserve entry types; zero iterations and breaks determine the exit scope |
| Catch models partial execution | Its input contains only bindings compatible across every failure prefix |

For example, this now returns `1`, rather than falling through to `3`:

```text
language "0.2";
pipeline probe() -> Number {
  try { return 1; } catch err { return 2; }
  return 3;
}
```

This returns `"original"` when supplied `err = "original"`, rather than leaking
the assertion message through the shadowed name:

```text
language "0.2";
pipeline probe(err: String) -> String {
  try { assert false, "failure"; } catch err {}
  return err;
}
```

## Static completion analysis

Expression inference and structural assignability are unchanged. Each statement
produces a completion summary. Environments from different paths are joined
only within the same completion kind, retaining names present with mutually
assignable types on both paths. An unreachable path contributes nothing.
Return outcomes retain their expression type and source span rather than an
outgoing environment, because the returned value has already been evaluated.

- **Sequence:** check the next statement only from the normal outcome. Accumulate
  returns, breaks, continues, and failures separately. Unreachable tails have
  no typing contribution; the separate analyzer still reports warnings.
- **Primitive statements:** successful calls and approvals extend the normal
  environment; assertions preserve it. Return has only a return outcome;
  break and continue have their respective loop-control outcomes. The enclosing
  block conservatively includes a possible failure before each statement,
  matching the runtime's time-budget check and expression/call failures.
- **Branches:** check both alternatives from the same entry environment and
  merge each completion kind separately. Missing else is an empty normal block.
- **Temporary scopes:** restore the entry type or remove the name in every
  outgoing environment before merging. Returned types remain unchanged.
- **Loops:** check the body from the entry environment. Each normal/continue
  back-edge must retain every entry binding at an assignable type. Merge the
  entry environment (zero iterations) with break exits for the normal output.
  Consume break/continue and propagate returns/failures. No constant-condition
  reasoning or termination inference is performed.
- **Catch:** merge the try block's failure environments, add the error binding,
  and check the handler from that environment. Restore the error name afterward.
  Successful try completions pass through; handler errors propagate outward.
  The time-budget check before the try statement itself belongs to the enclosing
  block and is not handled by that try's catch.
- **Parallel:** infer each run from the same snapshot, then publish fresh,
  distinct target types together. Partial results on a failure do not establish
  any additional definitely available names. Ordered map and race retain their
  existing result-type checks and contribute normal or failure outcomes.
- **Pipeline boundary:** require at least one reachable return and check each
  against the declared type. AGL 0.2 permits a normal fallthrough outcome;
  0.3–0.6 reject it. Test blocks have no declared return-type requirement.

This is a conservative implementation contract, not a mechanized typing proof.

## Properties and remaining proof obligations

**Conditional determinism (this core only).** Fix a single-valued expression
evaluator and a single-valued call relation. For every \\(B,\\rho,q\_1,\\rho\_1,q\_2,\\rho\_2\\),
if both \\(\\langle B,\\rho\\rangle\\Downarrow(q\_1,\\rho\_1)\\) and
\\(\\langle B,\\rho\\rangle\\Downarrow(q\_2,\\rho\_2)\\), then
\\(q\_1=q\_2\\) and \\(\\rho\_1=\\rho\_2\\).

Proof by mutual induction on the first statement/block derivation, universally
quantifying the competing derivation. Empty blocks and primitives are unique
by definition and the evaluator/call assumptions. For sequence, the statement
induction hypothesis fixes whether the tail runs and its initial environment;
the block hypothesis fixes the tail result. For conditionals and optional
binding, evaluation fixes the branch, its hypothesis fixes the result, and
restoration is a function. For loops, evaluation fixes exit versus iteration;
the body hypothesis fixes the completion, and the recursive loop hypothesis
applies only in the normal/continue cases, to a strictly smaller derivation.
For try, the body hypothesis fixes whether catch runs; the catch hypothesis
and functional payload/restoration fix its result. These exhaust the syntax.
This is a paper proof for the parameterized rules, not a mechanized proof or a
proof about the implementation.

**Noninterference of skipped tails.** By inversion of sequencing, a statement
with completion other than \\(N\\) prevents evaluation of its remaining block.

**No termination or full type-soundness claim.** Pure handlers alone do not imply
determinism for the full language: deadlines, shared budgets, and races can
make outcomes depend on scheduling. A `while true { continue; }` also refutes
unconditional termination even without calls.

The checker now tracks completions separately as described above. Remaining
proof work includes exhaustive expression rules, preservation of the inferred
loop invariants, and correspondence between the checker, this model, and the
runtime. Tagged match cleanup has regression coverage but still needs a formal
rule in this core syntax.

Retry/fallback and resource transitions, structured-concurrency traces, and
durable replay with crash points remain outside this model. In particular,
parallel failure cleanup, replay identity/final-result selection, and approval
policy enforcement remain separate runtime reliability issues. Full-language
progress and preservation remain open.
