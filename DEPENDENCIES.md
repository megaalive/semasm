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

Workspace path crates (`semasm-core`, `semasm-contract`, `semasm-asir`, `semasm-target`) are first-party.

## Planned categories (not yet added)

| Category | Likely crates | When |
|---|---|---|
| Serialization | `serde`, `serde_json`, TOML parser | VS-01 contract parsing |
| Object files | `object` | object inspection slices |
| Test temp dirs | test-only helper | integration tests |

## Review checklist for new dependencies

1. Is the standard library enough?
2. License compatible with MIT OR Apache-2.0 distribution?
3. Maintained and free of known critical advisories?
4. Default features minimized?
5. Does it pull async, networking, or native C libs into core by accident?
6. Entry added to this file in the same PR?
