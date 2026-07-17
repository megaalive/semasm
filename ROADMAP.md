# Roadmap

Status labels: **done**, **in progress**, **planned**, **deferred**.

Targets listed as planned are **not supported** until a vertical slice marks them otherwise.

## Vertical slices

| Slice | Title | Status |
|---|---|---|
| VS-00 | Repository bootstrap and governance | **done** (scaffold) |
| VS-01 | Contract parser and deterministic diagnostics | **done** (check + explain) |
| VS-02 | First executable hosted slice (x86-64 Linux) | **in progress** (TARGET-002, BUILD-001, BUILD-002 done) |
| Later | Windows x64, AArch64 Linux, RISC-V 64 Linux, RISC-V 32 bare-metal | planned |
| Later | Agent task packets, object inspection, measurement framework | planned |

## Planned first production targets

1. x86-64 Linux, System V, ELF  
2. x86-64 Windows, Windows x64 ABI, PE/COFF  
3. AArch64 Linux, AAPCS64, ELF  
4. RISC-V 64 Linux, psABI, ELF  
5. RISC-V 32 bare-metal on a QEMU-supported machine  

## Deferred (examples)

x86-32, ARMv7, Windows ARM64, UEFI, WebAssembly, AVR, many MCU boards, kernel modules, eBPF, GPU ISAs — deferred until the target-kit contract and conformance suite are proven.

## Near-term success criteria

- Clean workspace build with CI on Linux and Windows.
- Contract check CLI for portable contracts (VS-01).
- One end-to-end “exit code / tiny program” demo with reports (VS-02).

See `semasm-complete-project-plan.md` for the full ordered task list and acceptance criteria.
