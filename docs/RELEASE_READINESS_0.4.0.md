# SemASM 0.4.0 release readiness

Candidate date: 2026-07-29

The 0.4.0 candidate closes P1 reliability and P2 product-surface work after
`v0.3.0`.

## Compatibility audit

- Product version and all internal workspace requirements are `0.4.0`.
- Verification Report remains schema `0.5`; Artifact Report remains `0.4`;
  Agent Failure, Capability, and Contract remain `0.1`.
- New public verification-schema compatibility readers implement the existing
  older/current/forward-opt-in policy.
- Existing CLI commands and exit classes are unchanged.
- Additive consumer surface: reusable x86-64 Linux verification workflow.
- Consumer workflow and published CLI are both pinned to `v0.4.0`.

## Required evidence

- Official local release verification script.
- Linux and Windows unit/integration owners.
- x86-64 Linux/Windows, AArch64, and RV64 end-to-end owners.
- MSRV, cargo-deny, reliability stress, and performance baseline.
- Seven bounded fuzz campaigns with retained artifacts.
- Linux/Windows release archives and verified `SHA256SUMS`.

Tagging is blocked until every pre-tag gate is green on the exact candidate
commit.
