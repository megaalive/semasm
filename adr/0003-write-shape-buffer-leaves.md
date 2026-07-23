# ADR 0003: Write-shape buffer leaves

## Status

Accepted

## Context

Read-only buffer leaves (`count_byte`, `find_*`, `memcmp`, `sum_i64`) and the
`is_read_only_buffer_scan` / `semantic.memory` gate assume contracts may declare
`memory_read` without `memory_write`. Adversarial twins that store are Violated.

The next semantic ceiling is leaves that **must** write memory (`replace_byte`,
`memcpy`, `memset`). That opens a new `HarnessShape`, softens the read-only
assumption, and can make oracle/adversarial policy ambiguous if shipped without
an explicit contract.

## Decision

### v1 shape scope

- **In scope for write-shape v1:** exactly one leaf family to start —
  **`replace_byte`** (scan buffer, replace occurrences of a needle byte with a
  replacement byte, return count or void-as-agreed in the contract).
- **`memcpy` / `memset`:** design-compatible follow-ons after `replace_byte`
  lands; **not** co-shipped in the first W\* wave.
- **Out of v1:** overlapping/aliasing regions (src/dst overlap), symbolic alias
  analysis, formal `ensures`, multi-ISA harness beyond x86.

### Memory gate relation

- Read-only gate (`is_read_only_buffer_scan`) stays for leaves **without**
  `memory_write` in effects.
- Write-shape leaves **must** declare `memory_write` (and usually `memory_read`)
  on the contract. The memory gate then:
  - allows stores **only** into the declared writable buffer region(s) for that
    shape;
  - still fail-closes on stores outside that region, WX violations, and
    undeclared side effects.
- **Adversarial write** (wrong region, write when contract is read-only, or
  write without declared `memory_write`) remains Violated — distinct from a
  correct leaf write that matches the oracle.

### Oracle and vectors

- New named oracle id (e.g. `builtin.buffer.replace_byte`) with versioned
  vectors in the contract pack.
- Vector policy v1:
  - length 0 allowed (no-op / zero count);
  - needle == replacement allowed (idempotent);
  - **overlap / alias of distinct buffer args:** out of scope — fail-closed at
    contract/harness (reject or skip shape), do not claim defined behavior.

### ABI / harness

- **x86-only first** (SysV + Win64), mirroring MemCmp (X4 fail-closed on
  AArch64/RISC-V until a later ADR).
- New `HarnessShape` variant; do not overload BufferScan with silent writes.

### Explicit non-goals (v1)

- Formal theorem proving / full alias
- CryptOpt / search mutators that invent write leaves
- Pipeline maturity bump or Gate-2 sandbox changes in the same wave
- HlaX64 bridge in W0–W2 (optional only after SemASM+VAA Gate green)

## Consequences

- Honesty docs must stop saying “all buffer leaves are read-only” once a
  write-shape leaf ships; gate wording becomes effect-declared.
- Implementation is a **separate W\* plan** after Accept (see
  `docs/STABILIZATION_PROGRESS.md` maturity inflection). Do not mix with
  CryptOpt embed, formal ensures, or x86 `experimental` → `verified_in_ci`.
- Until Accept, leaf/HlaX64 treadmill remains paused (D0).

## Follow-up (W\* outline — not this ADR)

| Wave | Focus |
|---|---|
| W0 | Contract + oracle + vectors |
| W1 | Harness shape + memory-gate honesty |
| W2 | Asm / e2e x86 |
| W3 | VAA Gate |
| W4 | Optional HlaX64 bridge |
