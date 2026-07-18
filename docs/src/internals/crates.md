# Crate map

Current workspace crates:

| Crate | Responsibility |
|---|---|
| `semasm-core` | Diagnostics, errors, spans, IDs |
| `semasm-contract` | Portable contracts |
| `semasm-asir` | ASIR types |
| `semasm-target` | Target identity and kits |
| `semasm-build` | Tool execution, assembly, linking, execution, and reports |
| `semasm-agent` | Provider-neutral agent task and result packets |
| `semasm-cli` | `semasm` binary |
| `semasm-obj` | Structured object-file inspection |
| `semasm-decode` | Normalized physical instruction decoding |
| `semasm-cfg` | Control-flow graph construction |
| `semasm-x86` | x86-64 lowering and ABI analysis |
| `semasm-aarch64` | AArch64 lowering and ABI analysis |
| `semasm-riscv` | RISC-V lowering and ABI analysis |

The crate-boundary stabilization audit will document which of these boundaries
have lasting dependency or feature-isolation value.
