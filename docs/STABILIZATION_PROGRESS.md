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

Stabilization PR-01…18, Bulletproof P0–P5, X86 Golden Path Depth, Evidence
W1–W5, controller handshake, and the shared `count_byte` / `sum_i64` slices
(with VAA Gate-1/2) are complete on `main`.

| Step | Focus | Status |
|---|---|---|
| H0 | Sync this progress doc | done |
| W4a | Oracle honesty (`contract_ensures` / `proof_basis`, schema 0.3) | done |
| W4b | Read-only buffer leaf gate (`semantic.memory`) | done |
| W4c | Golden demo / README oracle-vs-ensures clarity | done |
| W5a | Report provenance (`tool_version`, digests, schema 0.4) | done |
| W5b | `CONTROLLER_PROTOCOL.md` + status map for VAA | done |
| W5c | Golden `VerificationReport` fixture for consumers | done |

### Completed recently (not deferred)

- CFG / indirect-branch leaf policy wired into `agent verify` (`control` gate)
- Evidence card (`--card`), candidate compare, named versioned behavior oracles
- Oracle v2 splits weak contract `ensures` from `proof_basis: oracle_and_vectors`
- Read-only buffer leaf rejects explicit memory stores (`memory` gate)
- Controller handshake fields + stdout-only protocol for VAA adapters
- `sum_i64` shape `builtin.buffer.wrapping_sum_i64` (SysV + Win64 e2e)
- Win64 framed ABI: `mov rsp,rbp` restore + `[rbp±disp]` spill carve-out for
  compiler-produced leaves (needs dedicated regression lock — T1)

### Deferred (explicitly out of current waves)

- Formal `ensures result == count(...)` / general theorem proving
- Full memory alias / symbolic proof beyond the read-only leaf gate
- C compiler `-O2` / `-Os` binary-size bake-off in CI
- New ISAs or broad mnemonic expansion
- GitHub Release `v0.1.0` (checklist-gated separately)
- VAA / HlaX64 product work (sibling repos; see `CONTROLLER_PROTOCOL.md`)

### Shared vertical slice (SemASM + VAA) — done

| Wave | Focus | Status |
|---|---|---|
| S0 | Lock honesty: next shared slice is `count_byte`, not `sum_i64` | done |
| S1 | Consumer golden `verified` JSON for count_byte | done |
| S2 | VAA CI Gate-1: live Incomplete (`execution_denied`) + seal chain | done (VAA) |
| S3 | VAA `--allow-execution` + Gate-2 Verified | done (VAA) |
| S4 | SemASM `sum_i64` contract/oracle/harness + VAA fixtures | done |

**Honesty:** Gate-1 (`execution_denied` → VAA Incomplete) is **not** a verified
vertical slice. Gate-2 requires opt-in execution.

### Next waves (SemASM hardening T0–T6)

| Wave | Focus | Status |
|---|---|---|
| T0 | Sync this progress doc (S2–S4 honesty + T* table) | in progress |
| T1 | Lock framed Win64 ABI + rbp-spill exemption with tests | planned |
| T2 | `sum_i64` consumer goldens + oracle v2 | planned |
| T3 | `sum_i64` adversarial memory-write twins | planned |
| T4 | Contract/harness mismatch fail-closed | planned |
| T5 | A64/RV `control`/`memory` → `skipped` when unimplemented | planned |
| T6 | Pure-int oracle claim names `min` | planned |

Demo: `scripts/golden-demo.sh` (Linux SysV) or `scripts/golden-demo.ps1`
(Windows PE by default; `-SysV` for Linux tools).

The 0.1 release workflow remains prepared and must only be triggered from a
reviewed `v0.1.0` tag after checklist gates stay green. See
`docs/CLI_COMPATIBILITY.md`, `docs/CONTROLLER_PROTOCOL.md`,
`docs/AGENT_SCHEMA_POLICY.md`, and `ARCHITECTURE.md`.
