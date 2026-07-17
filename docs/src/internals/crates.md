# Crate map

Bootstrap crates:

| Crate | Responsibility |
|---|---|
| `semasm-core` | Diagnostics, errors, spans, IDs |
| `semasm-contract` | Portable contracts |
| `semasm-asir` | ASIR types |
| `semasm-target` | Target identity and kits |
| `semasm-cli` | `semasm` binary |

Additional crates are added only when a vertical slice needs them.
