# Changelog

All notable changes are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and version numbers
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Pure-int oracle distinguishes `min_usize` vs `max_usize` (claim + vectors);
  ambiguous contracts stay fail-closed. Contract fixture `max_usize.sem.toml`.
- `max_usize` Gate-ready pack: SysV/Win64 asm (correct/wrong/write/callee-saved),
  e2e verify tests, consumer VerificationReport goldens, and capabilities
  evidence fixtures.
- Buffer find-first oracle `builtin.buffer.find_first_u8` (`find_first_byte`):
  first index of needle, or `length` when absent; name-ambiguous buffer scans
  stay fail-closed.

### Changed

- Capability manifest documents Tranche O honesty: x86 assemble/link/execute
  remain `experimental` despite `agent_verify = verified_in_ci`; no pipeline
  level bump without dedicated owner evidence.
- Tranche O1 adversarial depth: `sum_i64` callee-saved twins (SysV+Win64) and
  Win64 decode/lower-gap parity fixtures (`unknown_insn`, `trailing_bytes`).

## [0.1.0] - 2026-07-23

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
- Gas buffer-scan behavioral harnesses for AArch64 (`svc` write/exit) and
  RV64 (`ecall` write/exit); `semasm agent verify --allow-execution` works
  under qemu in cross-target CI.
- Multi-target agent semantic gates: Win64 PE (`abi_win64`), AArch64 Linux
  (gas + `decode_aarch64`), and RV64 Linux (`decode_riscv64` + gas) in addition
  to existing SysV ELF. Buffer-scan behavioral harness for Win64 (`main` +
  kernel32 I/O). Fixtures `count_byte_win64.asm`, `count_byte_aarch64.S`,
  `count_byte_riscv64.S`.
- `Pipeline::assemble_for_target` / `link_for_target` (NASM vs GNU `as`, PE vs
  static ELF). Doctor tool slots for AArch64/RV64 cross binutils + qemu.
- RISC-V `abi_register_map` registration and Capstone `decode_riscv64`.
- Immutable agent `VerificationReport` in `semasm-agent` (`verify` module):
  semantic gates, executable gate, and optional harness behavior composed once
  via `VerificationReport::from_parts` (no pending mutation).
- `semasm agent verify` always emits the structured report on semantic failure,
  executable-container failure, execution denial (without `--allow-execution`),
  and completed behavioral runs. Terminal and JSON share the same model.
- Ignored end-to-end test that asserts `"status": "execution_denied"` JSON when
  the Linux verification toolchain is present; CI `decode` installs nasm,
  binutils, and qemu-user for that path.
- [ADR 0002](adr/0002-crate-boundaries.md): crate-boundary audit keeps the
  thirteen-crate workspace without merges (PR-16).
- Shared vertical slices for `count_byte`, `sum_i64`, and `min_usize` (SysV +
  Win64 e2e, consumer goldens, adversarial write/callee-saved twins).
- Oracle v2 splits weak contract `ensures` from `proof_basis: oracle_and_vectors`;
  evidence card, candidate compare, and controller handshake for VAA adapters.

### Changed

- Agent verify writes harness sources as `.S` for gas dialects and `.asm` for
  NASM.
- Agent verify assemble/link steps dispatch by target dialect and object format
  instead of always using NASM `elf64` + ELF `ld` flags.
- `generate_harness` takes an `Abi` and returns `Result` covering SysV, Win64,
  AAPCS64, and RISC-V buffer-scan harnesses.
- Capability manifest documents that `verify = verified_in_ci` is **pipeline**
  evidence, not agent semantic-gate completeness.
- Agent verify assemble steps and context acceptance commands use
  `TargetIdentity::nasm_format()` instead of a hardcoded `elf64`.
- Decode/lowering coverage fields in verification reports are instruction
  counts only; undecoded-byte detail stays in error messages.
- Buffer-scan harness synthesis (`AGENT-004`) derives max fixture length from
  `requires` (`length <= N` / `length < N`), needle from `needle == K` when
  present (else synthesizer default `0x41`), and the null-when-empty vector
  only when a `memory_read` effect names `{ptr}[0..{len}]`. It no longer uses
  `bounded_stack_bytes` as a buffer-length bound.

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
- `semasm agent verify --format json` now serializes `VerificationReport`
  (not bare `HarnessReport`). Nested `behavior` remains a `HarnessReport` when
  execution was allowed; otherwise `behavior` is `null`.
- Status strings are snake_case: `verified`, `semantic_failed`,
  `executable_failed`, `behavior_failed`, `execution_denied`.
- Experimental `VerificationReport` JSON Schema `0.1` is published at
  `crates/semasm-agent/schemas/verification-report.json` with policy in
  `docs/AGENT_SCHEMA_POLICY.md` (includes root `schema_version`).
- Harness API: `generate_harness(symbol, vectors, abi) -> Result<String, String>`
  (SysV, Win64, AAPCS64, and RISC-V buffer-scan generators).
- Partial architecture coverage: x86-64 agent/pipeline maturity is
  experimental on assemble/link/execute; AArch64/RV64 pipeline evidence is
  stronger than x86 decode/lower completeness. See `semasm status`.

[Unreleased]: https://github.com/megaalive/semasm/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/megaalive/semasm/releases/tag/v0.1.0
