# Stabilization Progress

This checklist tracks the recommended pull-request sequence from the early
technical review. A checked item means its acceptance scope is implemented,
tested across the workspace, committed, and pushed. CI evidence must be green
before work advances past a failed item.

- [x] PR-01 — Baseline and scope freeze
- [x] PR-02 — Fix false-clean ABI paths
- [x] PR-03 — Fix x86 analysis soundness
- [x] PR-04 — Strict pipeline command outcomes
- [x] PR-05 — Explicit execution state
- [x] PR-06 — Runner stdin and environment hardening
- [x] PR-07 — Bounded output capture
- [x] PR-08 — Process-tree termination
- [x] PR-09 — Dedicated Linux and Windows end-to-end CI
- [x] PR-10 — Capability manifest and generated status
- [x] PR-11 — Documentation synchronization
- [x] PR-12 — Structured object verification
- [x] PR-13 — Cross-target executable evidence
- [x] PR-14 — CLI modularization
- [x] PR-15 — Schema and deterministic report versioning
- [x] PR-16 — Crate-boundary ADR and targeted consolidation
- [x] PR-17 — Negative corpus and fuzz entry points
- [x] PR-18 — 0.1 release preparation

## Current focus

Stabilization PR-01…18 is complete. The **SemASM Bulletproof Roadmap**
phases P0–P5 are implemented:

| Phase | Status |
|---|---|
| P0 | CI owner jobs set `SEMASM_REQUIRE_TOOLCHAIN=1`; soft-skip local-only |
| P1 | Additive `pipeline_verify` / `agent_verify` in capabilities + status/README |
| P2 | Adversarial agent fixtures + ignored e2e on owner jobs |
| P3 | `isolation` on verify/build reports; SECURITY.md honesty; prefer qemu_user |
| P4 | High-value mnemonic batch (A64/RV/x86) + natural count_byte fixtures |
| P5 | Pure-integer `(usize, usize) → usize` harness shape + wrong-fixture |

The 0.1 release workflow remains prepared and must only be triggered from a
reviewed `v0.1.0` tag after checklist gates stay green. See
`docs/CLI_COMPATIBILITY.md`, `docs/AGENT_SCHEMA_POLICY.md`, and
`ARCHITECTURE.md`.
