# ADR 0009: AArch64/RV64 Decode/Lower Maturity Bump

## Status

Accepted (scope lock; implementation follows
`docs/A64_RV_DECODE_LOWER_BUMP_PLAN.md`)

## Context

x86-64 Linux/Windows `decode` / `lower` are already `verified_in_ci` after the
**Dx** adversarial checklist and owner sign-off. AArch64 and RISC-V remain
`partial` by design: Gelombang 3 (ADR 0008) delivered Region/Alias memory-effect
parity **without** a decode/lower maturity flip.

Cross-target e2e already runs `_aarch64_` / `_riscv64_` adversarial filters
(svc/ecall + wrong-behavior). That is not yet a signed bump: coverage is
thinner than x86 (unknown-mnemonic classes, trailing-bytes, W+X, indirect).

## Decision

### In scope

- A **Dx-style** checklist for AArch64 + RV64 `decode` / `lower` only.
- Grow the adversarial corpus to the same *families* as Dx where ISA-meaningful
  (unknown/unmodelled mnemonic, trailing/undecodable bytes, privilege/syscall
  class already present via `svc`/`ecall`, object-policy gaps as applicable).
- Named owner CI jobs (`e2e (AArch64 Linux)` / `e2e (RV64 Linux)` +
  `decode (capstone)` filters) must stay green on the adversarial set.
- Caps honesty in the same change as any TOML flip; claim =
  **CI-verified sample coverage**, not full-ISA formal proof.
- Explicit **owner sign-off** required before `partial` → `verified_in_ci`.

### Out of scope

- Formal full-ISA decode proof; theorem prover; CryptOpt; hardware HSM.
- Flipping pipeline `assemble`/`link`/`execute` (already `verified_in_ci` where
  claimed).
- Expanding Region/Alias beyond ADR 0006/0008 honesty.
- Public-untrusted / production trust root (G4/G5 escalate bars).

### Claim wording

Allowed: *AArch64/RV64 decode/lower are CI-verified sample coverage of the
named adversarial families (Dx-parity checklist).*

Forbidden: *Full A64/RV ISA formally decoded* / *every mnemonic modeled*.

## Consequences

- Horizon line “A64/RV decode stay `partial`” becomes **landable under Da\***;
  bump only after readiness table + sign-off.
- Incomplete ≠ Verified; agent ≠ pipeline; x86 Dx sign-off does **not**
  transfer to A64/RV.
