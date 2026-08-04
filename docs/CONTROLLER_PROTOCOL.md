# Controller protocol (SemASM → VAA)

Canonical handshake for an external controller (such as VAA) that drives
`semasm agent verify` and consumes the JSON report.

## Agent claim boundary

SemASM **verifies**; agents (and their controllers) **propose**. An agent never
decides acceptance — SemASM's `status` does. Fixed boundary for any agent-facing
surface built on SemASM:

- **`incomplete` ≠ `verified`** and **`verified_under_preconditions` (VUP) ≠
  unconditional `verified`.** Report the status verbatim; never round VUP or
  incomplete up to "verified".
- **Unmodeled mnemonic / shape → fail-closed.** SemASM emits an
  `agent_failure` envelope or a non-`verified` status; controllers must stop, not
  invent a result from stderr.
- **Parse stdout JSON only** (see Streams below); stderr is human noise.

VAA is the reference controller. Its consumer-facing charter and the list of
shapes an agent is allowed to attempt live in VAA's repo:
`docs/HONESTY.md` and `schemas/agent-leaf-allowlist.json`. Keep those in sync
with this protocol; do not market SemASM as a general-purpose assembly prover.

## Canonical command

```text
semasm agent verify <source.asm> <contract.sem.toml> --format json \
  [--target <identity>] [--allow-execution] [--vectors-file <vectors.json>] \
  [--card <path.md>] [--card-json <path.json>]
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
| **stdout** | Exactly one JSON document (pretty-printed). Controllers **must** parse stdout alone. Discriminate by fields: a [`VerificationReport`](../crates/semasm-agent/schemas/verification-report.json) has `status` + `schema_version` (≥0.4,<0.7); an early [`AgentFailureEnvelope`](../crates/semasm-agent/schemas/agent-failure.json) has `kind: "agent_failure"` + `code` / `stage` / `retryability`. |
| **stderr** | Human-readable progress and errors. Never concatenate with stdout before JSON parse. |

## Report provenance (schema `0.6`)

Additive controller fields on every emitted report:

| Field | Meaning |
|---|---|
| `tool_version` | Stable string `semasm {SEMASM_VERSION}` |
| `contract_digest` | `sha256:` + full lowercase hex of contract file bytes |
| `source_digest` | `sha256:` + full lowercase hex of candidate source bytes |
| `vector_set` | Ordered origin binding for every case plus the canonical external document digest |

## External vectors

`--vectors-file` accepts schema `0.1` JSON with `contract_digest`, `target`,
`routine_symbol`, and `cases[]` containing only `id` plus named `inputs`.
Unknown fields (including `expected`) are rejected. SemASM retains all builtin
vectors, validates the binding and scalar input types, then computes expected
outputs with the recognized builtin oracle. Unsupported pointer/region shapes
fail closed with an `agent_failure` document.

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

### Lint / ABI finding codes (stable; not seals)

These appear in `VerificationReport.findings[]` and/or `semasm abi` /
`win64-abi` JSON. They are **heuristic or ABI-gate diagnostics**. They must
**never** be promoted to `verified` / hosted seal claims.

| `code` | Typical meaning | Severity |
|---|---|---|
| `STACK_ALIGN_CALL` | RSP not 16-byte aligned at `call` | error (ABI gate) |
| `STACK_BALANCE_RET` | RSP delta ≠ 0 at `ret` | error (ABI gate) |
| `SHADOW_SPACE_MISSING` | Win64 call without ≥32-byte home space | error (ABI gate) |
| `CALLEE_SAVED_*` | nonvolatile not preserved/restored | error (ABI gate) |
| `RIP_INDEX` | memory op uses RIP base with index/scale (NASM AV class) | error (lint) |
| `CALLER_SAVED` | SysV volatile (e.g. RSI/RDI) read after `call`/`syscall` clobber without redefine | warning (lint) |

### Hosted / VAA-only codes (not SemASM seals)

Emitted by VAA hosted tooling (`hosted-check`, build). Always `seal_claim: false`.

| `code` | Typical meaning | Agent action |
|---|---|---|
| `HOSTED_SMOKE_FAILED` | session stdin→stdout check failed | fix I/O / exit / expectations |
| `OUTPUT_LOCKED` | PE/output path locked on Windows | use stamped `run_path` / close process |
| `DISPATCH_FALLTHROUGH` | (reserved) command dispatch miss | fix exact-match chain; smoke each cmd |
| `MULTI_LINE_READ` | (reserved) pipe multi-line stdin bug class | line-split buffer |

Policy: **do not admit REPL / `mainCRTStartup`** as a leaf. `UNSUPPORTED_SHAPE`
for unrecognized hosted entry points remains correct.

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
