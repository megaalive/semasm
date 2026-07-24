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
| **Da0** | ADR 0009 + this plan + progress unlock | **done** |
| **Da1** | Inventory gaps vs Dx families (A64 + RV) | **done** |
| **Da2** | Grow adversarial corpus (± twins; fail-closed) | **done** |
| **Da3** | CI filters / owner-job assert coverage | **done** |
| **Da4** | Readiness table green + **owner sign-off** | **done** (sign-off: “kerjakan eksekusi”) |
| **Da5** | Caps TOML flip + honesty comments (same commit) | **done** |

### Da1 — Gap inventory (vs Dx) — recorded

| Family (Dx) | A64/RV | Status |
|---|---|---|
| Syscall / privilege | `svc` / `ecall` (+ hvc/smc / CSR capability widen) | **met** |
| Wrong-behavior | `count_byte_wrong_*` | **met** |
| Unknown / unmodelled | `fmov`/`mrs` · `fence`/`mulh` | **met** |
| Trailing / undecodable | `count_byte_trailing_bytes_*` | **met** |
| W+X | `count_byte_wx_*` ELF `.semasm_wx,"awx"` (not `.text`; gas strips `w` there) | **met** |
| Indirect CFG leaf | x86-only (`control` skipped on A64/RV) | **N/A documented** |

### Da4 — Sign-off

Owner authorized Da5 caps flip via chat “kerjakan eksekusi” on the Da plan
(2026-07-24). Claim remains CI-verified **sample coverage**, not full-ISA
formal decode proof.

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
