# Crate map

Current workspace crates:

| Crate | Responsibility |
|---|---|
| `semasm-core` | Diagnostics, errors, spans, IDs |
| `semasm-contract` | Portable contracts |
| `semasm-asir` | ASIR types |
| `semasm-target` | Target identity and kits |
| `semasm-build` | Tool execution, assembly, linking, execution, and reports |
| `semasm-agent` | Provider-neutral agent task packets, harness, and `VerificationReport` |
| `semasm-cli` | `semasm` binary |
| `semasm-obj` | Structured object-file inspection |
| `semasm-decode` | Normalized physical instruction decoding |
| `semasm-cfg` | Control-flow graph construction |
| `semasm-x86` | x86-64 lowering and ABI analysis |
| `semasm-aarch64` | AArch64 lowering and ABI analysis |
| `semasm-riscv` | RISC-V lowering and ABI analysis |

Boundaries are documented in [ADR 0002](../../../adr/0002-crate-boundaries.md).
Crate count is not itself evidence of independent versioning.
