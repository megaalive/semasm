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
| Stabilization | Fail-closed execution, honest claims, adversarial corpus, isolation truth | **bulletproof P0–P5 done; deepen x86 golden path next** |

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

## Near-term stabilization criteria

- Clean workspace checks on Linux and Windows.
- Named Linux, Windows, and cross-target end-to-end CI jobs (owner jobs set
  `SEMASM_REQUIRE_TOOLCHAIN=1`; soft-skip is local-only).
- Unsupported instructions produce an incomplete result, never a clean result.
- Capability documentation stays synchronized with `capabilities.toml`
  (Pipeline vs Agent columns must not be conflated).
- Agent verify emits structured `VerificationReport` evidence on every gate
  outcome; deepen soundness on the x86-64 golden path (`count_byte` /
  `min_usize`) before adding new ISAs.
- Prefer fail-closed adversarial fixtures over broader mnemonic coverage.

See `semasm-complete-project-plan.md` for the original ordered vertical slices
and `docs/status/BASELINE-2026-07.md` for the stabilization baseline.
