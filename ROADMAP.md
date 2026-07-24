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
| G1–G5 + Da | Region/Alias, ContractExpr, Me parity, VAA Io/Tr ops, A64/RV decode bump | **done** |
| Rel-0.2 | Annotated tag + GitHub Release `v0.2.0` | **done** |

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

## Near-term criteria (post-`v0.2.0`)

- Keep CI owner jobs green with `SEMASM_REQUIRE_TOOLCHAIN=1`.
- Prefer fail-closed adversarial fixtures over broader mnemonic coverage.
- Do **not** add new ISAs until landed paths stay deep and honest.
- **Next depth (queued):** VAA controller/Gate smoke on tip `v0.2.0` (+ Co
  control tip when landed). A64/RV `control` leaf ported (sample coverage;
  `memory` still skipped).
- Still deferred (Horizon-locked): full memory alias / symbolic proof; formal
  `ensures`; CryptOpt; hardware HSM; live-model Gate; optional offline C size
  comparison (not a CI gate).
- **Done:** GitHub Release tags `v0.1.0` and `v0.2.0` (checklist-gated; CLI
  archives only; no crates.io).
- **Done (Co):** A64/RV `control` gate (indirect CFG leaf; see
  `docs/A64_RV_CONTROL_GATE_PLAN.md`).

Consumer pin / Gate smoke: see VAA `docs/progress.md`. Shared progress:
`docs/STABILIZATION_PROGRESS.md`.

See `semasm-complete-project-plan.md` for the original ordered vertical slices
and `docs/status/BASELINE-2026-07.md` for the stabilization baseline.
