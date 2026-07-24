# ADR 0006: Region and Alias Evidence v1

## Status

Accepted

## Context

Write-shape and buffer leaves (`memcpy`, `memset`, `replace_byte`, scans,
`memcmp`) already depend on relationships between memory regions. Today SemASM
has:

- **ADR 0003** — synthesis-side fail-closed for overlapping `memcpy` vectors
  (not a leaf analysis).
- **ADR 0004** — sample-based guard bytes for x86 write-shape (not a proof).
- Static `memory` gate — read-only buffer leaves only; write-shape skips.

Without a narrow region/alias slice, the verifier must over-accept, over-reject,
or lean only on behavioral oracle vectors. This ADR opens **Region/Alias
Evidence v1** as the first vertical slice toward formal contract semantics —
not a general alias analyzer, SMT backend, or theorem prover.

## Decision

### Model

- Regions are closed intervals `[base + offset, base + offset + length)`.
- `base` is a **named pointer parameter** only.
- `offset` is a constant or simple affine form; `length` is a constant or
  integer parameter.
- Relation statuses:
  `proven_disjoint` | `proven_equal` | `proven_contains` |
  `proven_partial_overlap` | `may_overlap` | `invalid_region` |
  `not_evaluated`.
- Honesty: `unknown ≠ disjoint`; `may_overlap ≠ safe`;
  `not_evaluated ≠ passed`.

### Contract surface (v1)

```toml
[[function.memory.regions]]
name = "src"
base = "src"
length = "length"
access = "read"

[[function.memory.relations]]
left = "src"
right = "dst"
require = "disjoint"   # disjoint | equal | contains
```

### Evidence

`VerificationReport.alias_analysis` with `model = "region-affine-v1"`, per-
relation `required`/`observed`/`basis`, `unknown_memory_accesses`, and
`assumptions`. Aggregate: conflict → failed; unproven required + unknowns →
incomplete (reported under fail-closed semantic failure); all required proven →
passed. No `passed with warning`.

### Scope / non-goals

**In:** x86-64 first; identity-based disjoint/equal for distinct/same params;
affine constant overlap when obvious; mark unmodeled memory ops unknown;
`memmove` out.

**Out:** pointer provenance, heap identity, pointer-from-arbitrary-int,
nonlinear arithmetic, linked structures, general points-to, SMT/theorem
prover, “formal memory safety”, “complete alias analysis”.

### Claim wording

Allowed: *SemASM can prove selected affine memory-region relations for
supported leaf-routine patterns.*

Forbidden: *SemASM formally proves memory safety* / *complete alias analysis*.

### Roadmap after v1 (locked deferred)

1. Contract expression semantics v1 — **landed** (ADR 0007; Ce0–Ce5).
2. A64/RV memory-effect parity (decode enough for Region/Alias facts) —
   **unlocked** as ADR 0008 + `docs/A64_RV_MEMORY_EFFECT_PARITY_PLAN.md`
   (Me0 docs; Me1–Me5 pending). **Not** a decode/lower maturity flip.
3. Isolation ops proof (VAA; escalate if public untrusted execution).
4. Trust root nyata (last; authenticity ≠ semantic truth).

## Consequences

- Caps/README must name **region-affine-v1**, not “general alias analysis”.
- ADR 0003/0004 remain; synthesis disjoint ≠ Region/Alias proof.
- Full symbolic alias stays Horizon-locked deferred.
