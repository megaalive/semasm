# A64/RV Memory-Effect Parity — execution plan (Gelombang 3)

Prerequisite: Region/Alias v1 (ADR 0006) + ContractExpr v1 (ADR 0007) **done**.
Scope lock: [ADR 0008](../adr/0008-a64-rv-memory-effect-parity.md).

**Not** a full-ISA decode/lower bump. Target: enough lowered memory effects
for `region-affine-v1` (and thus living `contract-expr-v1` region atoms) on
AArch64 + RISC-V. `decode`/`lower` stay `partial` until a Dx-style checklist
is met in a **later** wave.

## Claim

Allowed: Region/Alias v1 memory-effect facts on A64/RV for supported leaf
patterns (same honesty as x86).

Forbidden: “A64/RV decode verified in CI” / complete alias analysis.

Honesty: `unknown ≠ disjoint`; missing effects → incomplete; no silent pass.

## Steps (Me0–Me5)

| Step | Focus | Status |
|---|---|---|
| **Me0** | ADR 0008 Accepted + this plan + progress pointers | **done** |
| **Me1** | AArch64 memory-effect collector (affinity + unknown) | **done** |
| **Me2** | RISC-V memory-effect collector (affinity + unknown) | **done** |
| **Me3** | Wire `agent verify` alias (+ expr) on A64/RV paths | **done** |
| **Me4** | ± fixtures + CI filters (memcpy/memset-style; no memmove) | **done** |
| **Me5** | Caps / README / CHANGELOG honesty; G4 stays locked | **done** (G4 later landed on VAA) |

### Me1 — AArch64 effects

Mirror x86 `memory_effects` for AAPCS64:

- Seed pointer params: `x0`… (AAPCS64 integer/pointer args).
- Model `ldr`/`str`/`ldrb`/`strb`/… with `[base, #imm]` as affine when base
  carries param affinity; `sp`/`x29` frame → stack; else unknown.
- Pointer arithmetic (`add`/`sub` imm on affinity regs) preserves identity.
- Unsupported / unparsed mem → `AccessAddr::Unknown`.

Prefer CLI module(s) next to existing x86 collector (ADR 0002: ISA crates
need not depend on `semasm-contract`).

### Me2 — RISC-V effects

Same honesty for RV ABI (`a0`…):

- `ld`/`sd`/`lb`/`sb`/… with `imm(reg)` forms.
- Stack (`sp`) ignored for region/alias; unknown base → unknown.

### Me3 — Wire verify

In `verify_candidate_semantics` A64/RV arms:

- After successful lower, `collect_*_memory_effects` → `evaluate_alias` when
  `function.memory` present.
- `finalize_report` already attaches `contract_expressions` from alias;
  region atoms should **pass** on A64/RV for golden `memcpy` once effects
  are clean (today they omit when alias is `None`).

### Me4 — Corpus / CI

| Case | Expected |
|---|---|
| A64/RV `memcpy` + living disjoint + clean effects | alias passed (+ expr if present) |
| Unknown-address A64/RV twin | alias incomplete |
| Exact-alias / contradict fixtures (contract-level) | failed / incomplete as today |

Owner: unit tests on collectors + `decode (capstone)` / cross-target e2e
filters (existing A64/RV `agent_verify_memcpy_*` should surface
`alias_analysis` when wired).

### Me5 — Hygiene

- Caps: region-affine-v1 on A64/RV for supported leaves; **do not** flip
  `decode`/`lower` maturity.
- Progress: mark Me0–Me5 done; G4 (isolation ops) landed separately on VAA;
  G5 trust root remains locked.

## Non-goals

- No CryptOpt; no theorem prover; no `memmove`.
- No A64/RV `verified_in_ci` decode/lower in this wave.
- No G4 isolation / G5 trust root.

## Push order (when implementing)

1. Me1 A64 collector + tests → commit.
2. Me2 RV collector + tests → commit.
3. Me3–Me4 wire + corpus/CI → commit.
4. Me5 caps/docs → commit; tip pin on VAA.
