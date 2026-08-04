# SemASM 0.5.0 release readiness

The 0.5.0 release closes the external-vector evidence gap and the Win64 SDK
linker regression reported by a contributor.

Tagging is allowed only when all of these hold on the exact candidate:

- Workspace crates and internal requirements are version `0.5.0`.
- Formatting, clippy, workspace tests, docs, and source packages pass.
- Verification Report schema 0.6 and compatibility fixtures are synchronized.
- Win64 `sum_range` passes with builtin plus external oracle-derived vectors;
  the deliberately wrong candidate reports `behavior_failed`.
- The release workflow extracts each archive. Its Windows smoke runs a real
  Win64 oracle verification using the packaged `semasm.exe`.
- `SHA256SUMS` verifies before the GitHub Release is created.

Any skipped or failed required gate blocks the tag.
