# Roadmap

This roadmap describes implementation order. Target maturity is defined only by
`capabilities.toml`; a completed vertical slice does not by itself mean
CI-proven support.

## Vertical slices

| Slice | Title | Status |
|---|---|---|
| VS-00 | Repository bootstrap and governance | **done** |
| VS-01 | Contract parser and deterministic diagnostics | **done** |
| VS-02–VS-08 | Build/report, agents, object/decode/CFG, and architecture experiments | **implemented** |
| Stabilization | Fail-closed execution, honest claims, adversarial corpus, isolation truth | **bulletproof P0–P5 done** |
| X86 Golden Depth | SysV+Win64 e2e symmetry, ABI/decode/W+X adversarial CI, golden demo | **done** |

## Declared target identities

1. x86-64 Linux, System V, ELF
2. x86-64 Windows, Windows x64 ABI, PE/COFF
3. AArch64 Linux, AAPCS64, ELF
4. RISC-V 64 Linux, psABI, ELF
5. RISC-V 32 bare-metal on a QEMU-supported machine

The exact maturity of decoding, lowering, ABI analysis, assembly, linking,
execution, and verification for each identity is generated from
`capabilities.toml` and shown by `semasm status`.

## Deferred examples

x86-32, ARMv7, Windows ARM64, UEFI, WebAssembly, AVR, many MCU boards,
kernel modules, eBPF, and GPU ISAs are deferred until target-kit contracts and
conformance evidence are proven.

## Near-term criteria (after X86 Golden Depth + T0–T6)

- Keep CI owner jobs green with `SEMASM_REQUIRE_TOOLCHAIN=1`.
- Prefer fail-closed adversarial fixtures over broader mnemonic coverage.
- Do **not** add new ISAs until the x86 golden path stays deep and honest.
- Done in W/T waves (not queued): CFG/indirect leaf in `agent verify`;
  read-only buffer memory leaf; contract/harness mismatch fail-closed;
  A64/RV `control`/`memory` reported as `skipped` until ported.
- Still deferred: full memory alias / symbolic proof; formal `ensures`;
  optional offline C size comparison (not a CI gate).
- **Done:** GitHub Release tag `v0.1.0` (checklist-gated; CLI archives only).

Consumer pin / framed smoke / capability claim bind: see VAA `docs/progress.md`
(waves through Tranche N / `max_usize` Gate Done).

See `semasm-complete-project-plan.md` for the original ordered vertical slices
and `docs/status/BASELINE-2026-07.md` for the stabilization baseline.
