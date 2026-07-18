# Artifact Report Schema Policy

SemASM artifact reports use an independent `MAJOR.MINOR` schema version. The
current artifact-report schema is `0.4`; it is not derived from the SemASM
package version.

## Document shape

Every serialized artifact report contains:

- `schema_version` at the document root;
- `deterministic_evidence`, the canonical evidence covered by the evidence
  hash;
- `deterministic_evidence_sha256`, the lowercase SHA-256 digest of the compact
  canonical JSON bytes;
- `volatile_metadata`, which retains diagnostic data that is not reproducible
  across hosts or runs.

Deterministic evidence includes the normalized target, source and artifact
content hashes, sorted tool identities, structured command arguments,
normalized sections and symbols, and verification outcomes. Source, object,
and executable arguments use the logical labels `$SOURCE`, `$OBJECT`, and
`$EXECUTABLE`. A discovered Windows SDK import-library path uses
`$KERNEL32_IMPORT_LIBRARY`.

Host paths, formatted command lines, durations, raw tool output, runtime
stdout/stderr, capture diagnostics, and termination diagnostics remain in
`volatile_metadata` and are excluded from the evidence hash.

## Version changes

- Increment `MINOR` when fields are added, deterministic normalization changes,
  or the serialized layout changes without redefining the document's purpose.
- Increment `MAJOR` when existing meanings become incompatible or the document
  no longer represents the same artifact-report contract.
- A canonicalization change must increment the schema version because it can
  change the evidence hash without changing artifact contents.

Field order in canonical JSON is defined by the serializer structs. Collections
whose semantic order is irrelevant are sorted before serialization. Command
order is retained because it represents pipeline order.

## Reader compatibility

The default reader policy is strict:

- the current version is accepted as `Current`;
- an older minor from the same major line is accepted as `CompatibleOlder`;
- a newer minor is rejected unless `ReportReadOptions::allow_newer_minor` is
  explicitly enabled, in which case it is reported as `ForwardOptIn`;
- a different major, missing version, malformed version, or non-string version
  is always rejected.

Consumers should surface a compatibility warning for `CompatibleOlder` and
`ForwardOptIn`. Forward opt-in does not assert that unknown fields or semantics
are understood; it only permits callers designed to preserve or inspect them
to proceed deliberately.

## Change checklist

Any artifact-report schema change must:

1. update `ARTIFACT_REPORT_SCHEMA_VERSION`;
2. update compatibility tests for the previous and next versions;
3. add tests proving which changed inputs affect the canonical hash;
4. verify that volatile host/run data does not affect the canonical hash;
5. run workspace formatting, clippy, tests, and the relevant CLI smoke test;
6. update this policy when compatibility or canonicalization rules change.
