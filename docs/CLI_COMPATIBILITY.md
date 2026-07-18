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
