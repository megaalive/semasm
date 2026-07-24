# ADR 0007: Contract Expression Semantics v1

## Status

Accepted (implemented: Ce0–Ce5; see `docs/CONTRACT_EXPR_V1_PLAN.md`)

## Context

Contracts already **parse** a bounded expression language
(`semasm-contract::expr`) for `requires` / `ensures`, but there is **no
machine evaluation** of those ASTs as semantic evidence. Behavioral truth for
recognized leaves comes from named oracles + vectors
(`behavior_oracle` / `proof_basis = oracle_and_vectors`), not from evaluating
`ensures`.

Region/Alias Evidence v1 (ADR 0006) landed living `function.memory.regions` /
`relations` and `VerificationReport.alias_analysis` (`region-affine-v1`). The
next vertical slice toward formal contract semantics is a **narrow evaluator**
for expressions that refer to those living region/relation facts — not a
general logic engine, SMT backend, or loop-invariant prover.

## Decision

### Scope (v1)

- Evaluate a **documented subset** of `requires` / `ensures` expressions whose
  meaning depends on ADR 0006 region/relation evidence (and trivial closed
  forms already checkable without SMT: integer/bool literals, comparisons on
  parameters when a concrete binding exists, `and` / `or` / `implies` /
  `not` over those atoms).
- Unknown operators, predicates, or unbound names → **fail-closed**
  (`incomplete` / unknown), never silent pass.
- Wire a report field (e.g. `contract_expressions` or nested under semantic
  evidence) with model id, per-expression status, and assumptions.
- Honesty: parse success ≠ evaluated; oracle vectors ≠ expression proof;
  unevaluated `ensures` ≠ passed.

### Non-goals (v1)

- General first-order / separation logic; quantifiers over heaps; SMT.
- Loop invariants; interprocedural specs; full `valid_for_read` memory safety.
- Replacing behavioral oracles for leaf shapes.
- A64/RV expression parity beyond what Region/Alias facts already provide.
- Claiming “formal verification of contracts”.

### Claim wording

Allowed: *SemASM can evaluate a documented subset of contract expressions
against living region/relation evidence (and trivial closed atoms).*

Forbidden: *SemASM formally verifies all requires/ensures* / *theorem prover*.

### Execution

Ordered steps **Ce0–Ce5** in `docs/CONTRACT_EXPR_V1_PLAN.md`. One wave at a
time; do not open G3 (A64/RV memory-effect parity) until Ce5 DoD is green,
unless an effects-only exception is explicitly accepted.

### Relation to other ADRs

- **ADR 0006** — prerequisite; expression atoms may cite proven relations.
- **ADR 0003 / 0004** — unchanged; synthesis/harness evidence ≠ expression eval.
- Oracle `contract_ensures` strings remain reporting of source text, not proof.

## Consequences

- Caps/README must name the expression **subset** and model id, not “full
  contract verification”.
- G3–G5 stay locked deferred with unlock criteria already recorded in
  `docs/STABILIZATION_PROGRESS.md`.
