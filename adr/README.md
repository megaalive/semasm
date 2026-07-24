# Architecture Decision Records

Place ADRs here as `NNNN-title.md` (for example `0001-dual-license.md`).

ADRs record durable design decisions (crate boundaries, target model, dependency bans). Prefer short context, decision, and consequences.

## Index

| ADR | Title | Status |
|---|---|---|
| [0001](0001-workspace-bootstrap.md) | Workspace bootstrap with five crates | Accepted |
| [0002](0002-crate-boundaries.md) | Crate boundaries after stabilization audit | Accepted |
| [0003](0003-write-shape-buffer-leaves.md) | Write-shape buffer leaves (`replace_byte` v1) | Accepted |
| [0004](0004-region-precise-memory-gate.md) | Region-precise memory gate honesty (Rmem) | Accepted |
| [0005](0005-multi-isa-memcmp-write-shape.md) | Multi-ISA MemCmp / write-shape harness honesty | Accepted |
| [0006](0006-region-alias-evidence-v1.md) | Region and Alias Evidence v1 (affine slice) | Accepted |
| [0007](0007-contract-expression-semantics-v1.md) | Contract expression semantics v1 (subset eval) | Accepted |
| [0008](0008-a64-rv-memory-effect-parity.md) | A64/RV memory-effect parity for Region/Alias v1 | Accepted |
| [0009](0009-a64-rv-decode-lower-bump.md) | A64/RV decode/lower maturity bump (Dx-parity) | Accepted |
| [0010](0010-alias-proof-assumption-obligation.md) | Alias proof vs assumption vs caller obligation (Sei P0) | Accepted |
| [0011](0011-region-access-evidence-v1.md) | Region Access Evidence v1 (affine access slice) | Accepted |
| [0012](0012-length-result-bound-obligations.md) | Length / result bound obligations (formal bounds chip) | Accepted |
