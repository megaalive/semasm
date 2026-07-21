# CLI and JSON compatibility

SemASM 0.1 is a pre-1.0 developer release. Command names and documented exit
classes are supported for the 0.1 line, while JSON shapes evolve only through
their declared schema policies.

## Exit codes

- `0`: the requested operation completed successfully.
- `1`: execution, validation, analysis, tool invocation, or I/O failed.
- `2`: command-line usage or target identity was invalid.

Commands that require execution return non-zero when a runner is unavailable or
fails. Partial or unsupported semantic coverage is never promoted to complete
verification; commands either report incomplete evidence or require an explicit
opt-in where that behavior is supported.

## Agent verify (`semasm agent verify`)

Exit `0` only when overall status is `verified` (static gates passed and every
harness vector passed after `--allow-execution`).

Otherwise exit `1` and still emit a structured report when gates were reached:

| Status | When |
|---|---|
| `semantic_failed` | Object/decode/lowering/ABI/capability gate failed |
| `executable_failed` | Linked image failed the executable-container policy |
| `execution_denied` | Static gates passed; `--allow-execution` was not set |
| `behavior_failed` | Execution ran; one or more vectors failed |

JSON document type is `VerificationReport` from `semasm-agent::verify`:

- `schema_version` — experimental agent schema (`0.2`); see
  `AGENT_SCHEMA_POLICY.md` and
  `crates/semasm-agent/schemas/verification-report.json`
- `status`, `target`, `routine_symbol`, `isolation`
- `semantic` — object policy, instruction-oriented `decode` / `lowering`
  coverage (`total` / `modeled` / `unknown`), ABI, capability, and control
  statuses
- `executable` — post-link container gate (`passed` / `failed` / `skipped`)
- `behavior` — `HarnessReport` when execution ran; otherwise `null`.
  Case count for the buffer-scan shape is 6 or 7 depending on whether a
  `memory_read` region `{ptr}[0..{len}]` proves null-when-empty policy.
- `behavior_oracle` — named versioned profile when the contract matches a
  builtin shape (e.g. `builtin.buffer.count_equal_u8` v1). Equality for
  golden shapes is proven by this oracle + vectors, not by weak contract
  `ensures` alone (`count <= length` is not `count == occurrences`).

Coverage units are instructions, never raw bytes. Byte decode gaps appear only
in stderr / error messages. Agent JSON remains experimental in 0.x: tolerate
additive fields; do not treat unknown coverage as verified. Versioning rules
are in `AGENT_SCHEMA_POLICY.md` (not the artifact-report evidence-hash policy).

Semantic gate runners (with the `capstone` feature) complete static
object/decode/lowering/ABI/capability checks for:

- `x86_64-unknown-linux-gnu` (System V + ELF)
- `x86_64-pc-windows-msvc` (Microsoft x64 + PE)
- `aarch64-unknown-linux-gnu` (AAPCS64 + ELF, GNU `as` assemble path)
- `riscv64gc-unknown-linux-gnu` (RISC-V LP64 + ELF, GNU `as` assemble path)

Behavioral harness execution (`--allow-execution`) is implemented for:

- `x86_64-unknown-linux-gnu` (SysV Linux syscalls)
- `x86_64-pc-windows-msvc` (Win64 `WriteFile` / `ExitProcess`)
- `aarch64-unknown-linux-gnu` (AAPCS64 Linux `svc` write/exit via GNU as)
- `riscv64gc-unknown-linux-gnu` (LP64 Linux `ecall` write/exit via GNU as)

Other targets fail closed before claiming verification.

### Capability manifest vs agent gates

`capabilities.toml` field `verify` (and assemble/link/execute at
`verified_in_ci`) records **pipeline** evidence — typically an exit-fixture
assemble/link/run job — not agent semantic-gate completeness. See the header
comment in `capabilities.toml` and target descriptions for AArch64/RV64.

## JSON status

- Artifact reports are experimental but versioned. Schema compatibility and
  canonical hashing follow `REPORT_SCHEMA_POLICY.md`.
- Contract documents use contract schema `0.1`; unknown versions and fields are
  rejected according to `crates/semasm-contract/COMPATIBILITY.md`.
- Capability/status JSON is generated from `capabilities.toml`. Its
  `schema_version` must be checked by consumers.
- Analysis, ABI, object-inspection, doctor, and agent JSON are experimental in
  0.1. Consumers must tolerate additive fields and must not infer complete
  verification when coverage fields report unsupported or unknown input.

Removing a field, changing its meaning, or changing an exit-code class requires
a changelog entry. Stable JSON compatibility is not promised until a future 1.0
release unless a document-specific schema policy states otherwise.
