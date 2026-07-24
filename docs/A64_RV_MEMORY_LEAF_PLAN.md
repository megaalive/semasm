# A64 / RV `memory` leaf — plan (Mm)

**Status:** **done** (Mm0–Mm5; CI green at `e991182`)  
**Honesty locks:** Incomplete ≠ Verified · SoftHSM ≠ HSM · CI sample ≠ full-ISA · this leaf is **read-only buffer scan** only (same contract as x86), not full memory-safety proof.

## Goal

Port the x86 `read_only_buffer` semantic leaf to AArch64 and RISC-V so `agent verify` can set `semantic.memory = passed|failed` (not `skipped`) when the contract requests a read-only buffer scan.

## x86 reference

- Gate: `harness::is_read_only_buffer_scan` → `check_x86_read_only_buffer_leaf`
- Write detect: first operand `Mem`, mnemonic mov/add/…; carve-out `[rbp±disp]` spills
- Adversarial: `fixtures/asm/count_byte_write.asm` → `memory: failed`

## A64 / RV differences

Capstone store order is **src, Mem** (not Mem-first like x86):

| Arch | Example write | Operand shape |
|------|---------------|---------------|
| A64 | `strb wzr, [x0]` | Reg, Mem |
| RV | `sb zero, 0(a0)` | Reg, Mem |

Detection = `OpKind::Store` + any non-stack `Mem` operand (reuse carve-outs from `memory_effects_{aarch64,riscv}.rs`: SP/FP without index).

## Steps

| ID | Work | Done when |
|----|------|-----------|
| Mm0 | This plan | **done** |
| Mm1 | `is_*_explicit_memory_write` + `check_*_read_only_buffer_leaf`; wire A64/RV arms; `memory: Passed` on success | **done** |
| Mm2 | Fixtures `count_byte_write_{aarch64,riscv64}.S` + adversarial tests | **done** |
| Mm3 | Flip unit expects `Skipped` → `Passed` for golden memory; CLI_COMPAT + capabilities + CHANGELOG | **done** |
| Mm4 | Push; wait `e2e-aarch64` / `e2e-riscv64` green | **done** (`e991182`) |
| Mm5 | ROADMAP: mark Me-mem done | **done** |

## Out of scope

- Full alias / region / W^X interaction changes
- Modelling every store mnemonic (`stp`, `stur`, atomics, …) — unmodelled → Fail (conservative)
- VAA controller depth beyond existing Vd pin
