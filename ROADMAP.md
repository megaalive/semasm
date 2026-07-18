# Roadmap

This roadmap describes implementation order. Target maturity is defined only by
`capabilities.toml`; a completed vertical slice does not by itself mean
CI-proven support.

## Vertical slices

| Slice | Title | Status |
|---|---|---|
| VS-00 | Repository bootstrap and governance | **done** |
| VS-01 | Contract parser and deterministic diagnostics | **done** |
| VS-02–VS-08 | Build/report, agents, object/decode/CFG, and architecture experiments | **implemented; stabilization evidence incomplete** |
| Stabilization | Fail-closed execution, honest analysis, and target CI evidence | **in progress** |

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
- Named Linux and Windows end-to-end CI jobs.
- Unsupported instructions produce an incomplete result, never a clean result.
- AArch64 and RISC-V claims remain below CI-verified until cross-target jobs pass.
- Capability documentation remains synchronized with the manifest.

See `semasm-complete-project-plan.md` for the original ordered vertical slices
and `docs/status/BASELINE-2026-07.md` for the stabilization baseline.
