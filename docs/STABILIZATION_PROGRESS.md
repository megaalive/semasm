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

Stabilization PR-01…18 and the **Bulletproof Roadmap (P0–P5)** are complete.
Active work is the **X86 Golden Path Depth** vertical slice (SysV + Win64):

| Step | Focus | Status |
|---|---|---|
| S0 | E2E symmetry (`count_byte` SysV verified; `min_usize` Win64) | done |
| S1 | ABI adversarial (stack, callee-saved, red-zone, Win64 shadow) | done |
| S2 | Decode/lower adversarial (unknown insn, trailing bytes) | done |
| S3 | Object policy rejects W+X sections | done |
| S4 | Demo scripts + docs / deferred list | done |

### Deferred (explicitly out of this slice)

- Wiring CFG / indirect-branch policy into `agent verify`
- Memory alias analysis on the buffer-scan shape
- C compiler `-O2` / `-Os` binary-size bake-off in CI
- New ISAs or broad mnemonic expansion
- GitHub Release `v0.1.0` (checklist-gated separately)

Demo: `scripts/golden-demo.sh` (Linux SysV) or `scripts/golden-demo.ps1`
(Windows PE by default; `-SysV` for Linux tools).

The 0.1 release workflow remains prepared and must only be triggered from a
reviewed `v0.1.0` tag after checklist gates stay green. See
`docs/CLI_COMPATIBILITY.md`, `docs/AGENT_SCHEMA_POLICY.md`, and
`ARCHITECTURE.md`.
