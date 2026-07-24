# ABI analysis maturity bump — plan (Ab)

**Status:** in progress (corpus + caps flip)  
**Honesty:** CI-verified sample coverage ≠ formal ABI / calling-convention proof.

## Claim

Allowed: flip `abi_analysis` to `verified_in_ci` on x86-64 Linux/Windows,
AArch64 Linux, and RV64 Linux after owner e2e asserts `semantic.abi = failed`
on callee-saved + stack-imbalance twins.

Forbidden: claiming complete AAPCS64/SysV/Win64/LP64 conformance; RV32 bump.

## Corpus

| Twin | x86 | A64/RV |
|---|---|---|
| callee-saved clobber | already CI | `count_byte_callee_saved_{aarch64,riscv64}.S` |
| stack imbalance | already CI | `count_byte_stack_imbalance_{aarch64,riscv64}.S` |

Also: fix A64 ABI walker for gas/Capstone `sub sp, sp, #imm` (3-op form).

## Out of scope

- Horizon locks; new ISAs; formal ABI proofs
