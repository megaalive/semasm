# Dependencies

Every third-party crate must have a written reason. Prefer the Rust standard library. Disable default features that pull unused capabilities.

## Policy summary

| Rule | Detail |
|---|---|
| No async runtime in core/CLI | Unless a demonstrated product need exists |
| No embedded database | Initial product |
| No dynamic Rust plugin ABI | Initial product |
| No direct LLVM library link | Initial product; CLI adapters later if needed |
| No AI provider SDK in core | Agent protocol stays model-agnostic |
| Licenses | See `deny.toml` allow list |
| Lockfile | `Cargo.lock` committed for reproducible CLI builds |

## Current dependencies

| Crate | Used by | Reason |
|---|---|---|
| `thiserror` | `semasm-core` | Explicit, ergonomic error enums without large ecosystem surface |
| `clap` (derive) | `semasm-cli` only | Standard CLI parsing for the tool binary |
| `serde` | `semasm-contract` | Schema derive for contracts and JSON diagnostics |
| `serde_json` | `semasm-contract`, `semasm-cli` | JSON diagnostic / check reports |
| `toml` | `semasm-contract` | Contract file format (`*.sem.toml`) |
| `win32job` | `semasm-build` on Windows only | Safe Job Object ownership so timeouts terminate descendants even after a launcher exits; `std::process` has no process-tree primitive. Version 2.0.3 has no feature flags, async runtime, networking, or native C dependency; it is MIT OR Apache-2.0 and maintained in its public repository. The Windows API bindings increase only Windows compile time and binary size. |

Workspace path crates (`semasm-core`, `semasm-contract`, `semasm-asir`, `semasm-target`) are first-party.

## Planned categories (not yet added)

| Category | Likely crates | When |
|---|---|---|
| Object files | `object` | object inspection slices |
| Test temp dirs | test-only helper | integration tests |

## Review checklist for new dependencies

1. Is the standard library enough?
2. License compatible with MIT OR Apache-2.0 distribution?
3. Maintained and free of known critical advisories?
4. Default features minimized?
5. Does it pull async, networking, or native C libs into core by accident?
6. Entry added to this file in the same PR?
