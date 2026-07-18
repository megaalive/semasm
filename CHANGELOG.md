# Changelog

All notable changes are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and version numbers
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-07-18

### Added

- Portable semantic-contract parsing, validation, stable diagnostics, and JSON
  reports.
- Target identities, capability manifest, generated status output, and
  toolchain discovery for Linux x86-64, Windows x86-64, AArch64, and RISC-V.
- Hardened subprocess execution with controlled stdin/environment, bounded
  capture, timeouts, and process-tree termination.
- Reproducible assembly/link pipelines, structured object verification, and
  explicit execution evidence.
- Versioned artifact reports with canonical deterministic evidence hashes and
  separated volatile metadata.
- Physical decoding, CFG extraction, partial lowering, and ABI analysis for the
  support levels recorded in `capabilities.toml`.
- Provider-neutral agent task packets and incomplete-analysis propagation.
- Linux, Windows, AArch64, and RV64 end-to-end CI evidence.
- Adversarial parser/object corpus and isolated bounded fuzz entry points.

### Security

- Commands are executed without shell concatenation and secret-like environment
  values are excluded from reports by default.
- Output floods and descendant processes are bounded by explicit policy.

### Compatibility

- All public surfaces remain pre-1.0 and may evolve in later minor releases.
- Artifact-report JSON uses schema `0.4` and the policy in
  `docs/REPORT_SCHEMA_POLICY.md`.
- Contract TOML uses schema `0.1` and rejects unknown fields.
- CLI exit codes and other JSON compatibility commitments are documented in
  `docs/CLI_COMPATIBILITY.md`.

[Unreleased]: https://github.com/megaalive/semasm/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/megaalive/semasm/releases/tag/v0.1.0
