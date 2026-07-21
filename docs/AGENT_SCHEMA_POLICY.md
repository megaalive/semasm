# Agent JSON Schema Policy

SemASM agent JSON documents (task packets and verification reports) use an
independent `MAJOR.MINOR` `schema_version`. That version is **not** derived from
the SemASM crate version and is **not** governed by the artifact-report policy
in `REPORT_SCHEMA_POLICY.md` (no deterministic-evidence hash / volatile split).

## Documents

| Document | Constant / field | Checked-in schema |
|---|---|---|
| Task packet | packet `version` (today `"0.1.0"`) | `crates/semasm-agent/schemas/task-packet.json` |
| Verification report | `VERIFICATION_REPORT_SCHEMA_VERSION` / `schema_version` | `crates/semasm-agent/schemas/verification-report.json` |

CLI field meanings and exit codes for verify live in `CLI_COMPATIBILITY.md`.

## Compatibility rules

- While the document remains experimental (0.x), consumers must tolerate
  **additive** fields.
- Removing a field, changing its meaning, or changing an enum string requires a
  `MAJOR` bump and a changelog Compatibility entry.
- Adding optional fields or new enum variants that do not redefine existing
  meanings may bump `MINOR`.
- Readers should reject a different major; older minors of the same major may
  be accepted with a compatibility warning.

## Regenerating schemas

Schemas are produced with the `schema` feature of `semasm-agent` (schemars):

```text
# Rewrite checked-in schema files after intentional shape changes:
SEMASM_WRITE_SCHEMAS=1 cargo test -p semasm-agent --features schema schema_json_matches_checked_in -- --nocapture

# Default CI / local check: fail if generated schema drifts from checked-in files
cargo test -p semasm-agent --features schema schema_json_matches_checked_in
```

Do not silently rewrite schemas in ordinary test runs.
