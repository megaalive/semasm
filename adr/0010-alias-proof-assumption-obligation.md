# ADR 0010: Alias Proof vs Assumption vs Caller Obligation

## Status

Accepted (Sei P0 — Semantic Evidence Integrity)

## Context

ADR 0006 Region/Alias Evidence v1 treated **distinct pointer-parameter
names** as `proven_disjoint` (identity assumption
`param_pointers_are_distinct_identities_when_named_differently`). That is
unsound: callers may pass the same address (or overlapping offsets) for
`src` and `dst`.

Contract Expression Semantics v1 (ADR 0007) then promoted that observation to
`proven_true` for `regions.disjoint(src, dst)`, mixing **assumption** with
**proof**.

Milestone plan: `docs/SEMANTIC_EVIDENCE_INTEGRITY_PLAN.md` (Sei).

## Decision

### Vocabulary

Separate **relation result** (what is claimed about regions) from **evidence
basis** (why we believe it):

| Dimension | Values (v1.1) |
|---|---|
| Relation observation (engine) | Keep ADR 0006 labels for static proofs (`proven_disjoint`, …) and non-proofs (`may_overlap`, …). Distinct parameter names yield **`may_overlap`**, never `proven_disjoint`. |
| Evidence basis | `proven_static` \| `declared_precondition` \| `unknown` (plus reserved: `observed_runtime`, `behavioral_test`, `assumed_environment`) |

### Explicit precondition (not inferred)

Contracts may declare:

```toml
[[function.memory.relations]]
left = "src"
right = "dst"
require = "disjoint"
basis = "precondition"
```

- Do **not** infer precondition from parameter names or signature shape.
- `basis = "precondition"` means: SemASM may analyze the callee **assuming**
  the relation; compliance remains a **caller obligation**.
- Aggregate alias slice may be `passed_under_preconditions` when every
  required relation is either statically proven or an explicit caller
  obligation, and there are no unknown accesses / contradictions.
- Overall report status may be `verified_under_preconditions` (≠ `verified`).

### Contract expressions

`regions.disjoint|equal|contains` must not become `proven_true` solely from a
declared precondition. Use `true_under_precondition`. Aggregate expression
slice: `passed_under_preconditions` when applicable.

### Honesty

- Callee verification under preconditions does **not** prove every caller
  complies.
- `verified_under_preconditions` must not be silently promoted to `verified`.
- Same-base constant affine disjoint/equal/overlap remain valid
  `proven_static` observations.

## Non-goals (this ADR)

- Renaming all `proven_*` observation strings to bare `disjoint`/`equal`
  (deferred; report already carries `evidence_basis`).
- Region Access Evidence v1 (Sei P1 engine).
- VAA Evidence Requirement Profiles (VAA Sei slice).
- SMT / general alias / formal memory safety.

## Consequences

- Caps/docs cite ADR 0010; ADR 0006 identity-disjoint claim is superseded.
- Verification report schema minor bump (`0.4` → `0.5`) for new status /
  evidence_basis / judgement variants.
- memcpy-like fixtures that need disjoint must set `basis = "precondition"`.
- Follow-on: fixtures listed in Sei plan §2; VAA policy for obligations.
