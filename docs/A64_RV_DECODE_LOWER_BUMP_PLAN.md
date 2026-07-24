# A64/RV Decode/Lower Maturity Bump — execution plan

Prerequisite: G1–G5 done; x86 Dx bump landed. Scope lock:
[ADR 0009](../adr/0009-a64-rv-decode-lower-bump.md).

**Not** a full-ISA formal proof. Target: enough adversarial CI coverage that
AArch64 + RV64 `decode` / `lower` may move `partial` → `verified_in_ci` under
the same discipline as Dx.

## Claim

Allowed: CI-verified sample coverage of named adversarial families on A64/RV.

Forbidden: “complete A64/RV ISA decode” / silent pass / bump without sign-off.

Honesty: Incomplete ≠ Verified; agent ≠ pipeline; x86 Dx ≠ A64/RV bump.

## Steps (Da0–Da5)

| Step | Focus | Status |
|---|---|---|
| **Da0** | ADR 0009 + this plan + progress unlock | **landed** (this commit) |
| **Da1** | Inventory gaps vs Dx families (A64 + RV) | pending |
| **Da2** | Grow adversarial corpus (± twins; fail-closed) | pending |
| **Da3** | CI filters / owner-job assert coverage | pending |
| **Da4** | Readiness table green + **owner sign-off** | pending |
| **Da5** | Caps TOML flip + honesty comments (same commit) | pending |

### Da1 — Gap inventory (vs Dx)

| Family (Dx) | A64/RV today | Gap |
|---|---|---|
| Syscall / privilege class | `svc` / `ecall` twins | likely **met** |
| Wrong-behavior (oracle) | `count_byte_wrong_*` | met (behavior_failed) |
| Unknown / unmodelled mnemonic | thin / absent | **need** ISA-appropriate unknowns |
| Trailing / undecodable after ret | thin / absent | **need** |
| W+X / indirect leaf policy | x86-heavy | port or document N/A |

### Da2 — Corpus

Add fixtures + `agent_verify_adversarial` filters asserting `semantic_failed`
(non-zero + JSON). `#[ignore]` only for missing cross toolchain.

### Da3 — CI

`cross-target-e2e` already runs `_aarch64_` / `_riscv64_`. Ensure new filters
are included; keep `decode (capstone)` unit coverage if applicable.

### Da4 — Sign-off

Do **not** flip caps until user/CI owner explicitly signs off (same bar as Dx).

### Da5 — Caps

Update `capabilities.toml` A64/RV `decode`/`lower` + comment block in one
commit with the readiness table marked **signed**.

## Non-goals

- Formal ensures / full symbolic alias / CryptOpt / hardware HSM.
- Claiming Region/Alias completeness beyond ADR 0006/0008.

## Push order

1. Da0 docs (this wave) → commit.
2. Da1–Da3 corpus + CI → commit(s).
3. Da4 sign-off (human) → Da5 caps flip → commit; VAA tip pin.
