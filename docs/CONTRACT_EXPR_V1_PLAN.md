# Contract Expression Semantics v1 — execution plan (Gelombang 2)

Prerequisite: Region/Alias Evidence v1 (ADR 0006 / Ra0–Ra6) **done**.
Scope lock: [ADR 0007](../adr/0007-contract-expression-semantics-v1.md).

**Not** SMT, theorem prover, loop invariants, or full `requires`/`ensures`
verification. One wave at a time; G3–G5 remain deferred.

## Claim

Allowed: evaluate a **documented subset** of contract expressions against
living region/relation evidence (and trivial closed atoms).

Forbidden: “formal verification of all contracts” / complete expression
coverage.

Honesty: `unknown ≠ true`; parse-only ≠ evaluated; oracle vectors ≠ expression
proof.

## Steps (Ce0–Ce5)

| Step | Focus | Status |
|---|---|---|
| **Ce0** | ADR 0007 Accepted + this plan + progress pointers | **done** |
| **Ce1** | Document allowed AST subset + region/relation atoms | **done** |
| **Ce2** | Fail-closed evaluator (unit tests; unknown ops → incomplete) | **done** |
| **Ce3** | Wire `agent verify` report field + terminal summary | **done** |
| **Ce4** | ± fixtures + CI filters (reuse memcpy/memset memory blocks) | **done** |
| **Ce5** | Caps / README / CHANGELOG honesty; unlock G3 criteria check | **done** |

### Ce1 — Subset document

Author `docs/CONTRACT_EXPR_V1_SUBSET.md` (or section in this file) listing:

- **In:** literals; param/return idents when bound; comparisons; boolean
  connectives; `implies`; atoms that name ADR 0006 regions/relations
  (concrete surface TBD in Ce1 — e.g. predicate forms tied to
  `require=disjoint|equal|contains` evidence).
- **Out:** unbounded arithmetic, heap predicates without living regions,
  `valid_for_read` as full memory safety, quantifiers, loops.

### Ce2 — Evaluator

Module under `semasm-contract` (preferred; next to `expr` / `alias`):

- Input: checked contract + alias evidence (when present) + optional concrete
  binding for harness/static checks.
- Output per expression: `proven_true` | `proven_false` | `incomplete` |
  `not_evaluated` + short `basis`.
- Aggregate: any `proven_false` on required `ensures`/`requires` policy →
  **failed**; required-but-incomplete → **incomplete**; all required proven →
  **passed**. No “passed with warning”.

### Ce3 — Report

Extend `VerificationReport` with an optional block, e.g.:

```json
"contract_expressions": {
  "model": "contract-expr-v1",
  "status": "passed|incomplete|failed",
  "expressions": [...],
  "assumptions": [...]
}
```

Fail-closed: non-passed contributes to `semantic_failed` (same honesty as
`alias_analysis`).

### Ce4 — Corpus / CI

Minimum:

| Case | Expected |
|---|---|
| `memcpy` + living `disjoint` + expr citing that relation | passed (when subset atom exists) |
| Expr with unknown predicate / op | incomplete |
| Expr contradicting proven relation | failed |
| Contract without memory / without expr atoms in subset | skipped or not_evaluated (document) |

Owner CI: unit tests in `semasm-contract` + `semasm-cli` filters on
`decode (capstone)` / workspace test (mirror Ra5).

### Ce5 — Hygiene + unlock

- Caps: name **contract-expr-v1** subset, not “full ensures proof”.
- Update ADR 0006 “roadmap after v1” row 1 → landed when Ce5 done.
- **Unlock G3** only after Ce5: A64/RV memory-effect parity plan/ADR separate.

Subset surface: [CONTRACT_EXPR_V1_SUBSET.md](CONTRACT_EXPR_V1_SUBSET.md).

## Non-goals (carry from ADR 0007)

No CryptOpt; no general prover; no A64/RV decode maturity flip in this wave;
no claim that oracle `contract_ensures` strings are evaluated proofs.

## Push order (when implementing)

1. Ce1 subset doc → commit.
2. Ce2 evaluator + tests → commit; CI green.
3. Ce3–Ce4 report + corpus → commit.
4. Ce5 caps/docs → commit; then open G3 plan if desired.
