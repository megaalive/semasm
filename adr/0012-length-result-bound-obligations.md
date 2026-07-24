# ADR 0012: Length / result bound obligations (formal bounds chip)

## Status

Accepted (Fb0 docs; Fb1 evaluator extension planned in same wave)

## Context

Contract Expression Semantics v1 (ADR 0007) evaluates region atoms and
**closed** integer comparisons (literals or concrete `ExprBindings`). Leaf
contracts routinely declare `requires` such as `length <= 4096` and `ensures`
such as `result <= length`. With empty bindings those expressions are
`not_evaluated` and do not contribute to the living
`contract_expressions` slice.

Sei program plan §5 (P2) lists length/result bounds and caller obligations as
the next narrow expansion — **not** SMT, loop invariants, or general memory
safety. Region Access Evidence (ADR 0011) already demotes symbolic-length
affine accesses to `passed_under_preconditions`; contract-expr should record
matching **bound obligations** honestly rather than omitting them.

## Decision

### Scope (Fb1)

- For **`requires` only**: comparisons of the form
  `integer_param OP integer_literal` or `integer_literal OP integer_param`,
  where `integer_param` is a declared parameter of integer/`usize`/`isize`
  type, evaluate to **`true_under_precondition`** with basis
  `caller_bound_obligation`.
- Meaning: the bound is a **declared caller obligation**, not a static proof
  that every possible concrete value satisfies it.
- Do **not** promote to `proven_true`.
- **`ensures`** involving unbound result/length names remain
  `not_evaluated` until concrete/post-state bindings exist (oracle ≠ proof).

### Non-goals

- Proving loop indices stay inside regions (`base[index]` with induction).
- Evaluating `result <= length` without bindings.
- SMT / arithmetic beyond simple comparisons.
- Claiming formal memory safety or unconditional `verified`.

### Model id

Remain `contract-expr-v1`. Document the extension in
`docs/CONTRACT_EXPR_V1_SUBSET.md` and
`docs/FORMAL_BOUNDS_CHIP_PLAN.md`. Do not invent a silent v2 model until a
broader Ce v2 surface lands.

### Claim wording

Allowed: *SemASM records declared integer bound requires as caller
obligations in contract-expr evidence.*

Forbidden: *SemASM proves all callers respect length bounds* /
*formal verification of ensures*.

## Consequences

- Fixtures that only had `length <= N` (no region atoms) will emit a living
  `contract_expressions` slice with `passed_under_preconditions`.
- VAA `memory-leaf-affine-v1` already allows caller obligations on
  contract-expr; no profile change required for Fb1.
- Index-bounded affine access (collector + `AccessAddr` extension) stays a
  later Fb step after this obligation surface is green.
