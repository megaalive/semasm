# ABI analysis maturity bump — plan (Ab)

**Status:** in progress (corpus + caps flip)  
**Honesty:** CI-verified sample coverage ≠ formal ABI / calling-convention proof.

## Claim

Allowed: flip `abi_analysis` to `verified_in_ci` on x86-64 Linux/Windows,
AArch64 Linux, and RV64 Linux after owner e2e asserts `semantic.abi = failed`
on callee-saved + stack-imbalance twins.

Forbidden: claiming complete AAPCS64/SysV/Win64/LP64 conformance; RV32 bump.

## Corpus

| Twin | x86 | A64 | RV64 |
|---|---|---|---|
| callee-saved clobber | CI | `count_byte_callee_saved_aarch64.S` | `count_byte_callee_saved_riscv64.S` |
| stack imbalance | CI | `count_byte_stack_imbalance_aarch64.S` | deferred (Capstone addi/sd operand shape; unit tests cover walker) |

Also: fix A64 ABI walker for gas/Capstone `sub sp, sp, #imm` (3-op form);
recognize RV `c.addi16sp` in the walker (unit-tested).

## Out of scope

- Horizon locks; new ISAs; formal ABI proofs
