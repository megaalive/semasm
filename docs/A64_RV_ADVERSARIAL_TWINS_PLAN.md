# A64 / RV adversarial twins — plan (Tw)

**Status:** **done** (`c0655f1`)  
**Honesty:** fail-closed sample twins ≠ formal ensures / store-region proof.

## Goal

Port x86 `*_wrong` / read-only `*_write` adversarial fixtures for leaves that
already have A64/RV goldens, so owner e2e filters (`_aarch64_` / `_riscv64_`)
assert `behavior_failed` / `semantic_failed` beyond `count_byte`.

## Corpus

| Leaf | Twin | Status |
|---|---|---|
| `replace_byte` | wrong (no mutate) | **done** |
| `memset` | wrong (no store) | **done** |
| `memcpy` | wrong (no copy) | **done** |
| `memcmp` | wrong (always 0) + write (store) | **done** |
| `min_usize` | wrong (always a) | **done** |

## Out of scope

- New leaf families (`find_*` = separate wave)
- `abi_analysis` maturity bump
- Horizon locks
