# ADR 0005: Multi-ISA MemCmp and write-shape harness honesty

## Status

Accepted

## Context

Buffer-scan, pure-int, and `sum_i64` harnesses already generate for AAPCS64 and
RISC-V. Before Horizon H3, **MemCmp** and write-shape leaves (`replace_byte`,
`memset`, `memcpy`) were **x86-only**: `generate_harness` returned a clear
`Err` on `Abi::Aapcs64` / `Abi::Riscv` for those shapes. Capabilities comments
and X4/X5 docs treated that fail-closed posture as intentional — not partial
multi-ISA support.

Thin closed HlaX64 Win64 emit bridges for those leaves. HlaX64 emit is **not**
a SemASM A64/RV harness. Opening A64/RV without an ADR risked reading a single
green fixture as “MemCmp verified on all ISAs,” or co-shipping write-shape
generators before MemCmp was green.

## Decision

### Current (after H3 + write-shape follow-on)

- **MemCmp** agent harness: **x86 SysV + Win64 + AArch64 + RISC-V** (H3).
- **Write-shape** (`replace_byte` / `memset` / `memcpy`) agent harness:
  **x86 SysV + Win64 + AArch64 + RISC-V** (follow-on after H3; sample-based
  guard bytes on all those ISAs per ADR 0004 / H2).
- HlaX64 bridges for these leaves: Win64 shared-library emit only; not SemASM
  A64/RV verification.

### Implementation order (Horizon Closeout)

1. **MemCmp on AArch64, then RISC-V** (Horizon H3) — **landed**.
2. **Write-shape on A64/RV** (`replace_byte` / `memset` / `memcpy`) —
  **landed** as a separate tranche after MemCmp.
3. Overlap / alias of distinct buffer args for `memcpy` stays **fail-closed**
  (ADR 0003); synthesis never claims defined overlapping behavior.
4. Region-precise / guard-byte evidence remains governed by ADR 0004 (sample-
  based dynamic checks ≠ formal store-region proof).

### Caps honesty

`capabilities.toml` must say MemCmp **and** write-shape agent harnesses are
**x86 + A64 + RV**. Do not conflate that with formal `ensures` or symbolic
alias proof.

## Consequences

- Horizon H3 removed MemCmp fail-closed arms for A64/RV.
- Write-shape A64/RV fail-closed arms are removed in the follow-on tranche.
- Formal `ensures`, symbolic alias, and decode/lower `verified_in_ci` remain
  out of scope (Horizon-locked deferred; see STABILIZATION_PROGRESS Horizon
  map).

## Non-goals

- No CryptOpt, HSM, live Gate CI, or formal theorem prover.
- No claim that HlaX64 `-Wverify` equals SemASM Verified on any ISA.
