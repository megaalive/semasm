# Controller protocol (SemASM → VAA)

Canonical handshake for an external controller (such as VAA) that drives
`semasm agent verify` and consumes the JSON report.

## Canonical command

```text
semasm agent verify <source.asm> <contract.sem.toml> --format json \
  [--target <identity>] [--allow-execution] [--card <path.md>] [--card-json <path.json>]
```

Exit `0` only when `status` is `verified`. Otherwise exit non-zero; a structured
report is still emitted on stdout when the verify pipeline reached gate
evaluation (see `CLI_COMPATIBILITY.md`).

Controllers may also probe identity with:

```text
semasm version --format json
semasm status --format json
```

See `CLI_COMPATIBILITY.md` for field lists. These probes describe SemASM’s
embedded `capabilities.toml` maturity and are **not** a substitute for VAA’s
agent-verify snapshot.

## Streams

| Stream | Content |
|---|---|
| **stdout** | Exactly one JSON [`VerificationReport`](../crates/semasm-agent/schemas/verification-report.json) document (pretty-printed). Controllers **must** parse stdout alone. |
| **stderr** | Human-readable progress and errors. Never concatenate with stdout before JSON parse. |

## Report provenance (schema `0.4`)

Additive controller fields on every emitted report:

| Field | Meaning |
|---|---|
| `tool_version` | Stable string `semasm {SEMASM_VERSION}` |
| `contract_digest` | `sha256:` + full lowercase hex of contract file bytes |
| `source_digest` | `sha256:` + full lowercase hex of candidate source bytes |

`behavior_oracle` (when present) names the builtin profile and
`proof_basis: oracle_and_vectors`. Controllers must not claim that weak
contract `ensures` alone proved equality.

## Status map to VAA-style 4-outcome vocabulary

| SemASM `status` | VAA-ish outcome |
|---|---|
| `verified` | `verified` |
| `behavior_failed` / `semantic_failed` / `executable_failed` | `violated` |
| `execution_denied` | `incomplete` (static OK; execution not opted in) |
| toolchain / I/O early exit (no report on stdout) | `failed` |

## Follow-up in the VAA repo (not SemASM)

1. Parse `VerificationReport` schema `0.4` from **stdout only**.
2. Map `status` with the table above.
3. Smoke: `vaa verify` against SemASM `count_byte` (or an equivalent task that
   points at a SemASM contract).

Until that adapter lands, concatenating stdout+stderr or expecting fictional
fields such as top-level `diagnostics` will fail closed.
