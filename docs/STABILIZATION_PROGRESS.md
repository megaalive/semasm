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
W1–W5, controller handshake, shared `count_byte` / `sum_i64` / `min_usize` slices
(VAA Gate-1/2), hardening T0–T6, runner JSON R0–R2, and Tranche M are complete on
`main`. GitHub Release **`v0.1.0`**, Tranche N–Q, X0/X1 object-policy depth, and
Tranche R (search→ingest Gate loop) are complete. **X2 + S + T** closed
(`find_last_byte` Gate + `vaa search --ingest`). **X3 + U + V** closed
(`count_byte` Win64 callee_saved depth, `memcmp` Gate pack, VAA pin +
search `--ingest --allow-execution` smoke).

### Next waves (X4 + H4 + Y)

| Wave | Focus | Owner | Status |
|---|---|---|---|
| **X4** | MemCmp harness fail-closed on AArch64/RISC-V + caps honesty | SemASM | **done** |
| **H4** | HlaX64 → VAA bridge for `find_last_byte` | HlaX64+VAA | pending |
| **Y0–Y2** | Pin tips + `memcmp` search `--ingest` Gate parity | VAA | pending |

MemCmp agent harness remains **x86-only** (SysV + Win64). AArch64/RISC-V
generation fails closed with a clear error — not partial multi-ISA support.

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

### SemASM hardening (T0–T6) — closed

| Wave | Focus | Status |
|---|---|---|
| T0 | Sync this progress doc (S2–S4 honesty + T* table) | done |
| T1 | Lock framed Win64 ABI + rbp-spill exemption with tests | done |
| T2 | `sum_i64` consumer goldens + oracle v2 | done |
| T3 | `sum_i64` adversarial memory-write twins | done |
| T4 | Contract/harness mismatch fail-closed | done |
| T5 | A64/RV `control`/`memory` → `skipped` when unimplemented | done |
| T6 | Pure-int oracle claim names `min` | done |

Tranche SemASM hardening is closed on `main`. VAA pin / framed smoke waves
**N0–N4** and stack integrity **P0–P2** are done (see VAA `docs/progress.md`).

### Runner + SemASM JSON (R0–R2) — closed

| Wave | Focus | Owner | Status |
|---|---|---|---|
| R0 | Honesty docs: P* closed; next = R* | both | done |
| R1 | VAA ProcessRunner streaming cap + Win stdin EOF | VAA | done |
| R2 | SemASM `version`/`status --format json` | SemASM | done |

VAA post-alpha trust depth (**P7** / **P8**) is Done on the consumer side
(practice seals, SoftHSM smoke, Fulcio opt-in ≠ SemASM Verified).

### Tranche M (M0–M4) — closed

| Wave | Focus | Owner | Status |
|---|---|---|---|
| **M0** | Tip honesty: ROADMAP + this file point to Tranche M | SemASM | **done** |
| **M1** | `capabilities.toml` evidence fixtures include `sum_i64` corpus | SemASM | **done** |
| **M2** | `min_usize` Gate-ready pack (goldens / twins / honesty parity) | SemASM | **done** |
| **M3** | One x86 adversarial twin wave around golden path | SemASM | **done** |
| **M4** | VAA pin tip + `min_usize` Gate-1/2 fixtures/smoke | VAA | **done** |

### Release tip `v0.1.0` — done

Annotated tag + GitHub Release archives (`SHA256SUMS`) after
`docs/RELEASE_CHECKLIST.md` gates. No crates.io publish in this ceremony.

### Next waves (N0–N2 — Tranche N, post-`v0.1.0`)

| Wave | Focus | Owner | Status |
|---|---|---|---|
| **N0** | `max_usize` oracle/claim distinction + contract (min regression) | SemASM | **done** |
| **N1** | `max_usize` asm/e2e/goldens/adversarial + capabilities evidence | SemASM | **done** |
| **N2** | VAA pin tip + `max_usize` Gate-1/2 fixtures/smoke | VAA | **done** |

**Honesty:** Gate-1 Incomplete ≠ Verified. SoftHSM / Fulcio / practice seals ≠
SemASM Verified. Pipeline assemble/link/execute on x86 remains `experimental`.

Demo: `scripts/golden-demo.sh` (Linux SysV) or `scripts/golden-demo.ps1`
(Windows PE by default; `-SysV` for Linux tools).

Tranche N is closed on tip `623d22c` (SemASM) with VAA handoff `5a5c6d9`.

### Next waves (O0–O1 — Tranche O, x86 depth)

| Wave | Focus | Owner | Status |
|---|---|---|---|
| **O0** | Caps/docs honesty: x86 pipeline stays experimental; next = O→P | SemASM | **done** |
| **O1** | One adversarial family around `sum_i64` / Win64 decode-gap parity | SemASM | **done** |

### After O — Tranche P (`find_first_byte` Gate)

| Wave | Focus | Owner | Status |
|---|---|---|---|
| **P0** | Oracle/contract/vectors (absent → length) | SemASM | **done** |
| **P1** | Asm/e2e/goldens/adversarial + capabilities | SemASM | **done** |
| **P2** | VAA pin tip + Gate-1/2 smoke | VAA | **done** |

Buffer index-of shape (not another pure-int leaf). Pattern N0→N2 / M2→M4.

Tranche P is closed on tip `511bb45` (SemASM) with VAA handoff `5961c1b`.

### Next waves (Q0… + X0 — VAA loop + further x86 depth)

| Wave | Focus | Owner | Status |
|---|---|---|---|
| **Q0** | Caps/docs honesty: next = VAA repair/search loop + x86 depth | SemASM+VAA | **done** |
| **Q1** | `find_first_byte` multi-candidate `vaa run` wrong→repair Gate smoke | VAA | **done** |
| **Q2** | `vaa search` nop-slide staging Gate smoke (offline; ≠ CryptOpt/Verified) | VAA | **done** |
| **X0** | Win64 W+X object-policy (patched COFF; NASM cannot emit W+X code) | SemASM | **done** |

Tranche Q + X0 closed on tip `7fa6e18` (SemASM) with VAA handoff `80f848b`.

### Next waves (R0–R1 + X1 — search→ingest + object-policy depth)

| Wave | Focus | Owner | Status |
|---|---|---|---|
| **R0** | Caps/docs honesty: next = search→ingest + Win64 import/noexport | SemASM+VAA | **done** |
| **X1** | Win64 import + noexport object-policy twins (parity SysV) | SemASM | **done** |
| **R1** | `vaa search` staging → `vaa ingest` Gate smoke + verify-chain | VAA | **done** |

Tranche R + X1 closed on tip `c8f2047` (SemASM) with VAA handoff `171b553`.

### Next waves (X2 + S + T)

| Wave | Focus | Owner | Status |
|---|---|---|---|
| **X2a** | Win64 syscall + stack_imbalance object/capability twins | SemASM | **done** (asm encoding fix) |
| **X2b** | VAA mutator `nop-before-ret` | VAA | **done** (`9a490d3`) |
| **S0** | `find_last_byte` oracle/contract/vectors (absent→length) | SemASM | **done** |
| **S1** | `find_last_byte` asm/e2e/goldens/adversarial + CI | SemASM | **done** (`b6d3395`) |
| **S2** | VAA pin + Gate-1/2 (+ run wrong→repair) | VAA | **done** (`dcbc536`) |
| **T0–T2** | `vaa search --ingest` skip Violated → Incomplete | VAA | **done** (`dcbc536`) |

Tranche X2 + S + T closed: SemASM tip `1d57e8d` / functional S1 `b6d3395`;
VAA handoff `1ad5d0e` (S2+T content `dcbc536`).

### Next waves (X3 + U + V)

| Wave | Focus | Owner | Status |
|---|---|---|---|
| **X3** | Win64 `count_byte` callee_saved twin + caps write/indirect sync | SemASM | **done** (`b9a7079`) |
| **U0** | `memcmp` dual-buffer oracle/contract/vectors | SemASM | **done** (`da8b57a`) |
| **U1** | `memcmp` asm/e2e/goldens/adversarial + CI | SemASM | **done** (`ca959f3`) |
| **V0–V3** | VAA pin + memcmp Gate + search allow-execution smoke | VAA | **done** |

SysV `count_byte_red_zone` pairs with Win64 `count_byte_win64_shadow` as the ABI dual
(not a literal `*_red_zone_win64` name twin).

Tranche X3 + U + V closed: SemASM tip
`b8d24c1` / functional U1 `ca959f39924a34a3bca2a5effe71e96e63238250`;
VAA Gate handoff `a9f926d` / V3 docs `789f7ad` (CI pin remains U1 `ca959f3`).

**Honesty:** Gate-1 Incomplete ≠ Verified. SoftHSM / Fulcio / practice seals ≠
SemASM Verified. Pipeline assemble/link/execute on x86 remains `experimental`.
LLM / search mutator output ≠ Verified. `memcmp` oracle/vectors ≠ formal
`ensures`/alias proof. Gate-2 `search --ingest --allow-execution` Verified ≠
CryptOpt. MemCmp harness is x86-only; AArch64/RISC-V fail closed (X4). NASM
win64 does not emit WRITE on code sections; X0 uses
`fixtures/obj/count_byte_wx_win64.obj` (WRITE|EXECUTE patched).

Demo: `scripts/golden-demo.sh` (Linux SysV) or `scripts/golden-demo.ps1`
(Windows PE by default; `-SysV` for Linux tools).

See `docs/CLI_COMPATIBILITY.md`, `docs/CONTROLLER_PROTOCOL.md`,
`docs/AGENT_SCHEMA_POLICY.md`, and `ARCHITECTURE.md`.
