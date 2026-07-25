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
| **Fb4** | Index-bounded `AccessAddr` (base+index) modeled | **done** |
| **Fb5** | Static constant-index → `proven_inside` | **done** |
| **Fb6** | Loop-carried / range index proof | **locked** |

## Non-goals

- Promoting symbolic-length `verified_under_preconditions` → `verified`
- Loop invariant inference (Fb6)
- Changing VAA profile names for affine leaves
- Claiming symbolic / loop-carried indexed accesses are statically inside

## Concrete cell path (Fb3 honesty)

Leaves with **literal** region length (e.g. `load_byte0` / `store_byte0`,
`length = "1"`) can reach overall SemASM `verified` with
`region_access.status = passed` when affine accesses are `proven_inside`.
This does **not** promote symbolic-length Phase C/D leaves
(`length = "length"` + `length <= N` obligations). VAA profile
`memory-leaf-concrete-v1` rejects caller-obligation demotion.

## Indexed addresses (Fb4 / Fb5 honesty)

`AccessAddr::Indexed { base_param, scale, displacement, index_const }` models
`[base + index*scale + disp]` when the base register has parameter
affinity. Collectors no longer collapse these to `Unknown`.

- **Without** `index_const`: `region_access` records `may_escape` +
  `declared_precondition` → aggregate `passed_under_preconditions`
  (≠ unconditional `passed` / `verified`).
- **With** `index_const` (Fb5): x86 collectors track GP constants
  (`mov reg, imm`, `xor reg,reg`) and fold the access like affine
  `base+(index_const*scale+displacement)`. On a **literal-length** region
  that offset can be `proven_inside`. This is **not** loop-index induction
  (Fb6 locked).
