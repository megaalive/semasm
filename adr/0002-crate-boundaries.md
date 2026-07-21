# ADR 0002: Crate boundaries after stabilization audit

## Status

Accepted

## Context

The workspace grew from the five bootstrap crates (ADR 0001) to thirteen crates
as vertical slices landed. Stabilization PR-16 required documenting which
boundaries still earn their keep (ownership, dependency isolation, or feature
isolation) and consolidating only splits that no longer provide a clear
benefit. Thirteen crates must not be read as thirteen independently versioned
products.

## Decision

**Do not merge any crates in this decision.** Every current split retains a
clear ownership or isolation benefit. Targeted consolidation is deferred until
a future slice removes that benefit.

| Boundary | Decision |
|---|---|
| ISA crates (`semasm-x86`, `semasm-aarch64`, `semasm-riscv`) | **Keep** — backends own ISA semantics and mature independently |
| `semasm-agent` vs `semasm-cli` | **Keep** — provider-neutral packets/harness/`VerificationReport` stay out of the binary orchestrator |
| `semasm-build` | **Keep** — process execution, pipeline, artifact reports |
| `semasm-obj` | **Keep** — container inspection ≠ decode/CFG |
| `semasm-contract` / `semasm-target` / `semasm-core` | **Keep** — bootstrap ownership unchanged |
| Capstone behind `semasm-decode` feature | **Keep** — optional FFI isolation |
| `semasm-asir` | **Keep** — architecture-neutral IR; small size is not a merge criterion |
| `semasm-decode` + `semasm-cfg` | **Keep** — Capstone physical decode vs CFG construction; CLI may bind both under one feature without merging crates |
| `semasm-riscv` vs CLI `capstone` feature | **Keep library-only** until a CLI inspect/verify surface for RISC-V exists; intentional temporary omission from CLI features, not a merge signal |

Do **not** further fragment toward the long-term plan layout (separate
runner/report/abi/format/adapter crates). Those roles already live in
`semasm-build`, `semasm-agent`, and `semasm-obj`.

## Consequences

- Future merges require a new ADR that names the removed ownership benefit.
- Wiring `semasm-riscv` into CLI features is a product decision when RISC-V
  inspect/verify commands land; it is not required by this ADR.
- Documentation (`ARCHITECTURE.md`, crate map) cites this ADR as the boundary
  source of truth for the current thirteen-crate workspace.
