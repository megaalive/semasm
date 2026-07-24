# Contract Expression Semantics v1 — allowed subset

Model id: `contract-expr-v1` (ADR 0007).

This document is the Ce1 surface lock. The evaluator must reject anything
outside this list with **incomplete** (never silent true). Expressions that
are simply out of scope for static evaluation (need concrete bindings or
oracle-only claims) are reported as **not_evaluated** and do **not** fail the
slice by themselves.

## In (evaluable)

### Literals and connectives

- Boolean literals: `true`, `false`
- Integer literals (decimal)
- Unary: `not` / `!` (boolean only in v1)
- Binary boolean: `and` / `&&`, `or` / `||`, `implies`
- Comparisons on **closed** integer forms only (both sides literals, or a
  bound name from the optional binding map): `==`, `!=`, `<`, `<=`, `>`, `>=`
- Parentheses for grouping

### Region / relation atoms (ADR 0006)

Method-style calls on the reserved receiver `regions`:

| Atom | Meaning | Alias evidence needed |
|---|---|---|
| `regions.disjoint(A, B)` | Regions `A` and `B` are disjoint | `proven_disjoint` for `(A,B)` or `(B,A)` |
| `regions.equal(A, B)` | Regions identical | `proven_equal` |
| `regions.contains(A, B)` | `A` contains `B` | `proven_contains` with left=`A`, right=`B` |

`A` / `B` must be identifiers naming declared `function.memory.regions`.

These atoms compose with boolean connectives, e.g.:

```text
regions.disjoint(src, dst) and true
regions.disjoint(src, dst) implies regions.disjoint(src, dst)
```

## Out (incomplete if attempted as the sole meaning)

- `valid_for_read` / other heap or pointer-validity predicates
- Member projection used as proof (`status.ok`) without a binding policy
- Indexing / ranges as semantic claims (`buf[0..n]`)
- Arithmetic beyond comparing closed integers (`*`, `/`, unary `-` on non-literals)
- Quantifiers, loops, definitions, SMT-style formulas
- Any call other than the three `regions.*` atoms above

## Evaluation policy (static `agent verify`)

1. Walk each `requires` / `ensures` expression.
2. If the expression is **wholly** built from the In-list (region atoms +
   closed comparisons + connectives), evaluate it.
3. If it needs unbound parameters/returns (e.g. `length <= 4096` with no
   binding, `status == 0` post-state), mark **not_evaluated**.
4. If it contains an Out construct that is not covered by (3), mark
   **incomplete** when the expression was intended as a subset claim
   (contains a `regions.*` call or is otherwise classified as attempted);
   pure Out-only expressions with no region atoms → **not_evaluated**.
5. Aggregate over judgements other than `not_evaluated`:
   - any `proven_false` → slice **failed**
   - else any `incomplete` → slice **incomplete**
   - else all `proven_true` → slice **passed**
6. If every expression is `not_evaluated` (no region atoms / no closed
   forms), omit `contract_expressions` from the report (`null` / absent).

## Honesty

- Parse success ≠ evaluated.
- Oracle vectors ≠ expression proof.
- `not_evaluated` ≠ passed.
- `incomplete` ≠ passed.
- Alias `may_overlap` does not satisfy `regions.disjoint`.
