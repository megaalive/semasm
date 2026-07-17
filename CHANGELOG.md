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
- **VS-01:** portable contract schema, semantic type parser, expression subset, codes `CTR001`–`CTR007`.
- `semasm contract check <path>` with terminal and JSON output.
- `semasm --explain CTR003` / `semasm explain CTR003`.
- Fixture `fixtures/contracts/write_all.sem.toml` and compatibility policy in `crates/semasm-contract/COMPATIBILITY.md`.
- **TARGET-002:** `semasm target doctor <target>` command probes host PATH for required tooling (assembler, linker, disassembler, runner) and reports found versions or install hints. Terminal and JSON output supported.
- `semasm-target::tools` module with `ToolKind`, `ToolProbe`, `ToolSlot`, `DoctorReport`, and fallback chains (`ld.lld`→`ld.bfd`, `llvm-objdump`→`objdump`).

### Notes

- No architecture backends or assembly demos yet (VS-02+).
