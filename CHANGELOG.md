# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
once the first release is tagged. Until then, the API is unstable.

## [Unreleased]

### Added

- Cargo workspace bootstrap with crates: `semasm-core`, `semasm-contract`, `semasm-asir`, `semasm-target`, `semasm-cli`.
- `semasm --version` / `semasm version` and `semasm status` commands.
- Repository governance documents, dual licensing, CI skeleton, and mdBook stub.
- Core types: diagnostics, errors, spans, IDs; ASIR and target identity shells.

### Notes

- No architecture backends or assembly demos yet (VS-00 only).
