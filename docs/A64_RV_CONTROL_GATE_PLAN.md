# A64/RV Control Gate (indirect CFG leaf) — execution plan

Prerequisite: Rel-0.2 / `v0.2.0` landed. Scope: port the x86 **leaf**
`control` gate (reject indirect jmp/call / unknown terminators) to AArch64 +
RV64 Linux. **Not** a full CFG / CFI formal proof. Memory leaf stays
`skipped` on A64/RV.

Honesty: Incomplete ≠ Verified; sample fail-closed ≠ complete ISA control
proof.

## Claim

Allowed: CI-verified fail-closed rejection of named indirect transfers
(`br`/`blr`, `jr`/`jalr`) on A64/RV leaves, with golden paths reporting
`control: passed`.

Forbidden: claiming complete CFG / PAC / landing-pad verification.

## Steps (Co0–Co5)

| Step | Focus | Status |
|---|---|---|
| **Co0** | This plan + progress unlock | **done** |
| **Co1** | CFG classify: A64/RV unconditional mnemonics (`b`/`br`/`j`/…) | **done** |
| **Co2** | Wire `check_cfg_leaf` on A64/RV; `control=Passed`; memory stays Skipped | **done** |
| **Co3** | Fixtures `count_byte_indirect_{aarch64,riscv64}.S` + adversarial e2e | **done** |
| **Co4** | Caps/docs honesty + readiness | **done** |
| **Co5** | CI green on owner A64/RV jobs | queued |

## Non-goals

- A64/RV `memory` gate (read-only buffer leaf) — still skipped
- Formal CFG / PAC / BTI completeness
