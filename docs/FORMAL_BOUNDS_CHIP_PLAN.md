# Formal Bounds Chip — plan (Fb)

Prerequisite: ADR 0012; Contract Expr v1 (ADR 0007); Region Access v1
(ADR 0011). Parent: [`SEMANTIC_EVIDENCE_INTEGRITY_PLAN.md`](SEMANTIC_EVIDENCE_INTEGRITY_PLAN.md) §5.

Honesty: `true_under_precondition` ≠ `proven_true`; Incomplete ≠ Verified;
sample leaf ≠ formal memory safety / loop-invariant proof.

## Claim

Allowed: record declared integer `requires` bounds as caller obligations in
`contract-expr-v1`; later (Fb2+) narrow index-bounded affine access evidence.

Forbidden: SMT; proving arbitrary `ensures`; claiming general memory safety.

## Steps

| Step | Focus | Status |
|---|---|---|
| **Fb0** | ADR 0012 + this plan + subset/progress pointers | **done** |
| **Fb1** | Evaluator: `requires` param↔literal int cmp → `true_under_precondition` | **done** |
| **Fb2** | Corpus ± fixtures + CLI report honesty | **done** |
| **Fb3** | Caps/docs; unlock concrete-length cell path + index-bounded access spike | **done** |
| **Fb4** | Index-bounded `AccessAddr` (base+index) — separate chip | **locked** |

## Non-goals

- Promoting symbolic-length `verified_under_preconditions` → `verified`
- Loop invariant inference
- Changing VAA profile names for affine leaves

## Concrete cell path (Fb3 honesty)

Leaves with **literal** region length (e.g. `load_byte0` / `store_byte0`,
`length = "1"`) can reach overall SemASM `verified` with
`region_access.status = passed` when affine accesses are `proven_inside`.
This does **not** promote symbolic-length Phase C/D leaves
(`length = "length"` + `length <= N` obligations). VAA profile
`memory-leaf-concrete-v1` rejects caller-obligation demotion.