# ADR 0008: A64/RV Memory-Effect Parity for Region/Alias v1

## Status

Accepted (scope lock; implementation follows
`docs/A64_RV_MEMORY_EFFECT_PARITY_PLAN.md`)

## Context

Region/Alias Evidence v1 (ADR 0006) and Contract Expression Semantics v1
(ADR 0007) land on **x86 first**: `semasm agent verify` collects memory
effects and fills `alias_analysis` / `contract_expressions` only on the
x86-64 SysV/Win64 paths. AArch64 and RISC-V already decode/lower enough for
ABI gates and behavioral harnesses (MemCmp + write-shape), but they return
**no** alias slice today (`alias_analysis` absent → region atoms in
`contract-expr-v1` become `not_evaluated` and the expression block is
omitted).

The roadmap’s Gelombang 3 target is **memory-effect parity for the facts
Region/Alias v1 needs** — not a full-ISA decode/lower maturity bump, and not
flipping A64/RV `decode`/`lower` to `verified_in_ci` (that stays a separate
Dx-style checklist).

## Decision

### In scope

- Collect observed memory accesses from lowered AArch64 and RISC-V
  instructions with the same honesty model as x86: affine
  `base_param + const` when param affinity is known; stack/frame ignored;
  unmodeled → `AccessAddr::Unknown` (never silent pass).
- Seed affinities from AAPCS64 / RV ABI argument registers for pointer
  parameters.
- Wire `evaluate_alias` (+ subsequent `contract_expressions`) on A64/RV
  agent-verify paths when `[function.memory]` is present.
- ± fixtures / CI filters for supported leaf patterns (`memcpy` /
  `memset`-style), documenting expected passed / incomplete / failed.
- Caps/docs: claim **region-affine-v1 memory effects on A64/RV for supported
  leaves**, not “A64/RV decode verified” or complete alias analysis.

### Out of scope

- Bumping AArch64/RV64 `decode` / `lower` maturity to `verified_in_ci`.
- Full points-to, provenance, heap identity, `memmove`.
- SMT / theorem prover; expanding contract-expr beyond ADR 0007 subset.
- Isolation ops (G4) or trust root (G5).

### Claim wording

Allowed: *SemASM can produce Region/Alias v1 memory-effect facts on AArch64
and RISC-V for supported leaf-routine patterns (same honesty as x86).*

Forbidden: *A64/RV decode/lower are CI-verified full ISA* / *formal memory
safety on all ISAs*.

### Execution

Ordered steps **Me0–Me5** in `docs/A64_RV_MEMORY_EFFECT_PARITY_PLAN.md`.

## Consequences

- ADR 0005/0006/0007 remain; harness multi-ISA and alias-slice multi-ISA for
  Region/Alias v1 supported leaves (Me5 done).
- G4 isolation ops proof and G5 trust **ops** proof landed on VAA; production
  trust root remains Horizon-locked.
- A separate future ADR is required before any A64/RV `decode`/`lower`
  maturity flip.
