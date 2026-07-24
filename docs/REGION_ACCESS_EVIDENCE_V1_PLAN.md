# Region Access Evidence v1 — plan (Sei Ra)

Prerequisite: Sei P0 / ADR 0010 (alias proof vs caller obligation). ADR:
[`adr/0011-region-access-evidence-v1.md`](../adr/0011-region-access-evidence-v1.md).
Parent program: [`SEMANTIC_EVIDENCE_INTEGRITY_PLAN.md`](SEMANTIC_EVIDENCE_INTEGRITY_PLAN.md) §4.

Honesty: Incomplete ≠ Verified; unknown access ≠ inside; sample corpus ≠
general memory safety.

## Claim

Allowed: bounded affine region/access evidence for supported leaf patterns on
targets that have an acceptance corpus.

Forbidden: general memory safety / complete alias / multi-ISA “supported”
without per-target corpus.

## Steps

| Step | Focus | Status |
|---|---|---|
| **Ra0** | ADR 0011 + this plan + progress pointers | **done** |
| **Ra1** | Target-neutral access evidence types + engine stub (report shape) | **planned** |
| **Ra2** | Match affine accesses to contract regions (x86 effects in) | **planned** |
| **Ra3** | Bounds + permission status; fail-closed aggregate | **planned** |
| **Ra4** | Wire `VerificationReport.region_access`; schema bump if needed | **planned** |
| **Ra5** | x86-64 acceptance corpus (minimum fixtures in Sei §4) | **planned** |
| **Ra6** | Caps/docs honesty; A64/RV observational only | **planned** |
| **Ra7** | AArch64 parity corpus (separate) | **locked** |
| **Ra8** | RV64 parity corpus (separate) | **locked** |

## Minimum corpus (x86, Ra5)

```text
load_inside_read_region
store_inside_write_region
store_to_read_only_region
load_before_region
store_after_region
unknown_base_register
known_base_unknown_offset
multi_byte_access_crosses_end
same_region_read_write
memcpy_disjoint_regions
memcpy_possible_overlap
```

## Non-goals

- SMT / symbolic execution
- Promoting unknown → warning when task requires complete access evidence
- Flipping A64/RV to acceptance before Ra7/Ra8
