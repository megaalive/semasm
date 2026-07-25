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
| **Fb6** | Range-guard index (`cmp`+`jae`/`jge` fall-through) → `proven_inside` | **done** |
| **Fb7** | Post-test counted-loop induction (`access; inc; cmp; jb`) → `proven_inside` | **done** |
| **Fb8** | Countdown induction (`mov N; dec; access; jnz`) → `proven_inside` | **done** |
| **Fb9a** | CFG-confirmed structured pre-test induction | **done** |
| **Fb9b** | Arbitrary loop invariant inference | **locked** |

## Non-goals

- Promoting symbolic-length `verified_under_preconditions` → `verified`
- Arbitrary loop invariant inference (Fb9b)
- Changing VAA profile names for affine leaves
- Claiming symbolic / unguarded indexed accesses are statically inside

## Concrete cell path (Fb3 honesty)

Leaves with **literal** region length (e.g. `load_byte0` / `store_byte0`,
`length = "1"`) can reach overall SemASM `verified` with
`region_access.status = passed` when affine accesses are `proven_inside`.
This does **not** promote symbolic-length Phase C/D leaves
(`length = "length"` + `length <= N` obligations). VAA profile
`memory-leaf-concrete-v1` rejects caller-obligation demotion.

## Indexed addresses (Fb4–Fb8 honesty)

`AccessAddr::Indexed { base_param, scale, displacement, index_const,
index_max_exclusive }` models `[base + index*scale + disp]` when the base
register has parameter affinity. Collectors no longer collapse these to
`Unknown`.

- **Without** `index_const` / `index_max_exclusive`: `region_access` records
  `may_escape` + `declared_precondition` → aggregate
  `passed_under_preconditions` (≠ unconditional `passed` / `verified`).
- **With** `index_const` (Fb5): x86 collectors track GP constants
  (`mov reg, imm`, `xor reg,reg`) and fold like affine
  `base+(index_const*scale+displacement)`.
- **With** `index_max_exclusive` (Fb6): x86 collectors arm a fall-through
  upper bound after `cmp reg, imm` + `jae`/`jnb`/`jge`. Bounds require both
  ends of `[disp, (max-1)*scale+disp]` inside a **literal** region. Writes
  that change the index (e.g. `inc`) clear the live Fb6 bound.
- **With** `index_max_exclusive` (Fb7): post-test count-up loops
  `xor idx,idx; … access; inc; cmp idx,N; jb …`.
- **With** `index_max_exclusive` (Fb8): countdown loops
  `mov idx,N; …; dec idx; access; …; jnz/jns …` so after `dec`,
  `idx ∈ [0,N)`. Linear pattern match — **not** CFG-sound arbitrary
  induction.
- **With** `index_max_exclusive` (Fb9a): structured pre-test loops
  `xor idx,idx; header: cmp idx,N; jae exit; access; inc idx; jmp header`.
  Physical branch destinations must resolve to the header/exit instruction
  addresses and no other body instruction may write `idx`. This is
  CFG-confirmed for that narrow shape; arbitrary invariant inference remains
  Fb9b locked.
