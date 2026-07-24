# ADR 0011: Region Access Evidence v1

## Status

Accepted (Sei — planned implementation; Ra0 docs)

## Context

ADR 0006 / 0010 deliver region **relations** (disjoint/equal/…) with honest
proof vs precondition. They do **not** bind each known memory access to a
declared contract region with bounds/permission evidence.

ADR 0004 already locks that write-shape “region-precise” claims are not
proven by the static memory gate. Sei plan
(`docs/SEMANTIC_EVIDENCE_INTEGRITY_PLAN.md` §4) requires a narrow **Region
Access Evidence** slice before broader memory-safety wording.

## Decision

### Model id

`region-access-affine-v1`

### In scope (v1)

- Affine addresses only: `base`, `base+const`, `base+index`,
  `base+index*scale`, simple affine offsets when index bounds come from
  recognized contract facts.
- Per access: instruction offset, load/store, width, address expression,
  matched region (optional), `bounds_status`, `permission_status`.
- Bounds: `proven_inside` | `proven_outside` | `may_escape` | `unknown`.
- Permission: `allowed` | `denied` | `unknown`.
- Fail-closed: store→read-only / proven outside → violated (slice failed);
  may escape / unmodeled / unknown effect → incomplete.
- Target rollout: engine target-neutral; **x86-64 acceptance first**;
  AArch64/RV64 observational until separate parity bumps.

### Out of scope (v1)

Arbitrary pointer arithmetic, pointer-from-load, heap provenance, nonlinear
math, general interprocedural alias, self-modifying code, “formal memory
safety”, global “multi-ISA supported” claim.

### Report

`VerificationReport.region_access` (optional) with model, aggregate status,
counts (`accesses_total`, `accesses_proven_inside`, `accesses_unknown`), and
per-access rows. Bound to contract + source digests via the parent report.

### Claim wording

Allowed: *SemASM reports bounded affine region and memory-access evidence for
supported leaf-routine patterns.*

Forbidden: *SemASM proves general memory safety.*

### Relation to other ADRs

- **0004** — honesty cliff remains; this ADR is the first *narrow* evidence
  slice toward region access, not a full gate replacement.
- **0006 / 0010** — relations/obligations stay separate; access evidence does
  not reintroduce name-based disjoint proofs.
- **0007** — contract-expr may later cite access facts; not required for Ra1.

## Consequences

- Caps must name `region-access-affine-v1` and which targets/corpus are gated.
- VAA may require `unknown_accesses == 0` only after the x86 corpus is green.
- Implementation steps: `docs/REGION_ACCESS_EVIDENCE_V1_PLAN.md` (Ra0–RaN).
