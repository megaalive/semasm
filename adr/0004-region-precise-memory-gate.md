# ADR 0004: Region-precise memory gate honesty

## Status

Accepted

## Context

ADR 0003 shipped write-shape buffer leaves (`replace_byte`, `memset`,
`memcpy`) and stated that the memory gate, for those shapes, "allows stores
**only** into the declared writable buffer region(s)". That sentence is
aspirational, not a description of an implemented analysis. As of this ADR:

- The static `memory` semantic gate
  (`check_x86_read_only_buffer_leaf` / `is_x86_explicit_memory_write` in
  `semasm-cli`) runs **only** when `is_read_only_buffer_scan` is true. For any
  contract that declares `memory_write` — every write-shape leaf — the
  function returns `Ok(())` immediately, before looking at a single decoded
  instruction. There is no static, instruction-level, or region-aware check
  of write-shape leaves today.
- The only evidence that a write-shape leaf touched the *right* bytes is
  **dynamic**: the harness (`evaluate_count_and_buffer` and friends in
  `semasm-agent`) executes the leaf against a small, hand-curated set of
  synthesized oracle vectors and compares the post-call buffer to the
  oracle-predicted bytes for exactly `dst[0..length]` (or the scan buffer,
  for `replace_byte`). Sentinel prefill (e.g. `MEMCPY_DST_SENTINEL = 0xEE`)
  catches a leaf that under-writes and leaves stale bytes in place; it does
  not catch a leaf that writes correct bytes into the region **and also**
  writes somewhere else.
- Nothing in the current harness allocates or checks guard/canary bytes
  immediately before or after the declared region, so an out-of-declared-
  region write that lands inside the same fixture allocation (or on an
  adjacent page the test happens not to probe) is not detected. Overlap/alias
  avoidance is a **synthesis-side** guarantee (SemASM never generates
  aliasing `dst`/`src` vectors) — it is not an analysis that rejects a leaf
  submitted with genuinely aliasing behavior.
- Coverage is bounded to a handful of lengths/values per leaf, x86-only
  (AArch64/RISC-V `memory` gate is `Skipped`, not evaluated), and re-derived
  by hand per contract — there is no general points-to, alias, or symbolic
  memory model behind any of this.

Shipping a second write-shape leaf (`memset`) and a third (`memcpy`) without
correcting the ADR 0003 wording risks the same "Incomplete ≠ Verified"
honesty failure this project has repeatedly locked against (D0, X4, X5).
This ADR corrects the record before Rmem work (if any) begins, and gives CI a
concrete bar for when the ADR 0003 sentence would actually be true.

## Decision

### What memory gate v2 may claim for write-shape leaves today

For a write-shape leaf (`replace_byte`, `memset`, `memcpy`) that passes
`agent verify`:

- The leaf's compiled x86 object, run against every synthesized oracle
  vector for that contract, produced a destination buffer whose
  `[0..length)` bytes matched the oracle-predicted bytes exactly, and (where
  applicable) returned the oracle-predicted status/count.
- Sentinel-prefilled fixtures give some confidence the match is not
  coincidental (the leaf actually wrote the bytes rather than the buffer
  already containing them).
- This is **heuristic, dynamic, sample-based evidence**, bounded to the
  finite vector set SemASM synthesizes, on x86 only. It is not a proof that
  the leaf writes *only* into the declared region for all inputs, and it is
  not alias analysis.

Docs, reports, and future ADRs **must** describe this as "harness-checked
against synthesized vectors" or "dynamically checked, region-scoped to the
declared length" — never as "proven", "guaranteed", or "only into declared
region" without the CI criteria in the next section being met and cited.

### What stays deferred

Unchanged from ADR 0003, restated for clarity now that two more write-shape
leaves have landed:

- Full alias / points-to analysis over pointer parameters.
- Symbolic memory / symbolic execution of a leaf's write set.
- A formal `ensures`-style store-region proof (e.g. "writes only bytes in
  `[dst, dst+length)`, for all `dst`, `length`").
- Guard/canary-byte detection of out-of-declared-region writes that land
  inside the same fixture allocation or an adjacent page.
- General multi-buffer overlap/aliasing detection (SemASM continues to avoid
  synthesizing aliasing vectors; it does not analyze submitted leaves for
  aliasing behavior).
- AArch64/RISC-V write-shape harness (stays `Skipped`, matching MemCmp/X4).

None of the above is scheduled by this ADR. If Rmem work is scoped, it gets
its own ADR/plan, per ADR 0003's "separate W\* plan after Accept" pattern.

### When ADR 0003's "only into declared region" wording may be considered fulfilled in CI

The following checklist must **all** hold before any doc cites the ADR 0003
sentence as implemented rather than aspirational. Meeting this bar still does
not authorize the word "proof" — see non-goals below.

1. **Owner CI job.** A named, green-on-`main` job (e.g.
   `region-gate (x86-64 Linux)` / `region-gate (x86-64 Windows)`) that runs
   the write-shape harness with guard-byte fixtures, distinct from the
   existing `agent verify` / e2e jobs that only check the declared region's
   own bytes.
2. **Evidence: guard bytes, not just declared-region bytes.** Fixtures place
   sentinel-filled, mapped guard bytes immediately before and after the
   declared buffer region; the harness asserts those guard bytes are
   unmodified after every synthesized write-shape vector runs, on every CI
   run — not a one-off spot check.
3. **Fail-closed, not skip/warn.** A modified guard byte is `Violated`, not
   `Incomplete`. A leaf/contract combination the guard-byte harness cannot
   evaluate (unsupported shape, missing fixture) is `Incomplete`/`Skipped`
   and must not be reported as passed.
4. **ISA scope stated.** The job's ISA coverage (x86-only unless a future ADR
   extends it) is named in `capabilities.toml` and this doc; AArch64/RISC-V
   stay `Skipped` until their own harness exists.
5. **Wording stays capped.** Even when 1–4 hold, docs say "guard-byte checked
   across synthesized vectors in CI" — the declared-region claim remains
   sample-based dynamic testing, not a substitute for alias analysis or a
   formal proof, and must not be worded as either.

Until all five hold, `docs/STABILIZATION_PROGRESS.md` and any ADR must keep
saying region-precise store proof is deferred.

### Explicit non-goals (this ADR)

- No analyzer, guard-byte harness change, or fixture change ships in this
  ADR. This is a docs-only correction; implementation (if pursued) is a
  separate, later wave under its own name, matching how ADR 0003 separated
  Accept from W0–W3 implementation.
- No change to `capabilities.toml`, gate code, or plan files.
- No retroactive downgrade of `replace_byte`/`memset`/`memcpy` gate status —
  they remain exactly as capable as they were before this ADR; only the
  wording of what that capability means is corrected.

## Consequences

- ADR 0003's "allows stores only into the declared writable buffer
  region(s)" must be read alongside this ADR: true only in the
  harness-checked-against-synthesized-vectors sense, not as a static or
  formal guarantee, until the CI criteria above are met and cited.
- Any future PR that adds guard-byte fixtures and a dedicated CI job can cite
  criterion 1–5 directly instead of re-litigating what "region-precise"
  means.
- `docs/STABILIZATION_PROGRESS.md` gets a one-line honesty update (Rmem =
  ADR 0004 landed, docs-only) rather than a new capability claim.

## Follow-up (not this ADR)

| Candidate | Focus | Status |
|---|---|---|
| Guard-byte harness fixtures | Detect out-of-declared-region writes inside the fixture allocation | Not scheduled |
| Full alias / points-to analysis | General may-alias reasoning over pointer parameters | Not scheduled |
| Formal store-region `ensures` | Prove writes confined to `[dst, dst+length)` for all inputs | Not scheduled |
| AArch64/RISC-V write-shape harness | Lift `Skipped` for `replace_byte`/`memset`/`memcpy` | Not scheduled |

Next candidate outside this ADR: **W4 HlaX64 `replace_byte` bridge** (per
ADR 0003's deferred-bridge note), not an Rmem analyzer.
