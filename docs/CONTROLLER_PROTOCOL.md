# Controller protocol (SemASM → VAA)

Canonical handshake for an external controller (such as VAA) that drives
`semasm agent verify` and consumes the JSON report.

## Canonical command

```text
semasm agent verify <source.asm> <contract.sem.toml> --format json \
  [--target <identity>] [--allow-execution] [--card <path.md>] [--card-json <path.json>]
```

Exit `0` when `status` is `verified` or `verified_under_preconditions`.
Otherwise exit non-zero; a structured report is still emitted on stdout when
the verify pipeline reached gate evaluation (see `CLI_COMPATIBILITY.md`).

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
| **stdout** | Exactly one JSON document (pretty-printed). Controllers **must** parse stdout alone. Discriminate by fields: a [`VerificationReport`](../crates/semasm-agent/schemas/verification-report.json) has `status` + `schema_version` (≥0.4,<0.6); an early [`AgentFailureEnvelope`](../crates/semasm-agent/schemas/agent-failure.json) has `kind: "agent_failure"` + `code` / `stage` / `retryability`. |
| **stderr** | Human-readable progress and errors. Never concatenate with stdout before JSON parse. |

## Report provenance (schema `0.5`)

Additive controller fields on every emitted report:

| Field | Meaning |
|---|---|
| `tool_version` | Stable string `semasm {SEMASM_VERSION}` |
| `contract_digest` | `sha256:` + full lowercase hex of contract file bytes |
| `source_digest` | `sha256:` + full lowercase hex of candidate source bytes |

`behavior_oracle` (when present) names the builtin profile and
`proof_basis: oracle_and_vectors`. Controllers must not claim that weak
contract `ensures` alone proved equality.

Alias / contract-expression honesty (ADR 0010): `alias_analysis.evidence_basis`,
`true_under_precondition`, and `verified_under_preconditions` mean callee
analysis under declared caller obligations — **not** unconditional proof.

## Status map to VAA-style vocabulary

| SemASM `status` | VAA-ish outcome |
|---|---|
| `verified` | `verified` |
| `verified_under_preconditions` | `verified_under_preconditions` (do not promote to `verified`) |
| `behavior_failed` / `semantic_failed` / `executable_failed` | `violated` |
| `execution_denied` | `incomplete` (static OK; execution not opted in) |
| `agent_failure` envelope (`kind=agent_failure`) | `failed` (use `code` / `retryability` for harness class) |

### Early failure codes (stable)

| `code` | Typical stage | `retryability` |
|---|---|---|
| `INVALID_TARGET` | usage | never |
| `TOOLCHAIN_INCOMPLETE` | toolchain | tooling |
| `SOURCE_IO` / `CONTRACT_IO` / `SCRATCH_IO` / `HARNESS_IO` | io / pipeline | tooling |
| `CONTRACT_INVALID` / `CONTRACT_ENCODING` | contract | never |
| `UNSUPPORTED_SHAPE` / `HARNESS_MISMATCH` | unsupported_shape | never |
| `ASSEMBLE_FAILED` / `ASSEMBLE_ERROR` / `ASSEMBLE_HARNESS_*` | assemble | never / tooling |
| `LINK_FAILED` / `LINK_ERROR` | link | never / tooling |

## Follow-up in the VAA repo (not SemASM)

1. Parse stdout only: `VerificationReport` (≥0.4,<0.6) **or** `agent_failure` 0.1.
2. Map `status` / failure `code` with the tables above into VAA harness classes
   (`accepted`, `violated_repairable`, `incomplete_coverage`,
   `toolchain_retryable`, `policy_blocked`, `failed`).
3. Drive loops via `vaa harness prepare|submit|resume|status` (see VAA
   `docs/agent-harness.md`).

Never concatenate stdout+stderr. Never promote `incomplete` or
`verified_under_preconditions` to `verified` unless the task explicitly allows
under-preconditions acceptance.
