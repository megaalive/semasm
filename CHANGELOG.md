# Changelog

All notable changes are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and version numbers
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Tw A64/RV adversarial twins** — `*_wrong` / `memcmp_*_write` fixtures for
  write-shape, MemCmp, and `min_usize` on AArch64/RV64 (behavior/memory
  fail-closed). Sample coverage ≠ formal proof.

Patch past `v0.2.0`: A64/RV leaf depth for `control` and `memory`. Incomplete ≠
Verified; sample CI coverage ≠ full CFG/CFI or region-precise store proof; no
crates.io.

### Added

- **Mm A64/RV `memory` leaf** — read-only buffer scan fail-closed on
  AArch64/RV64 (`strb`/`sb` adversarial fixtures); golden paths report
  `memory: passed`. Sample coverage ≠ region-precise store proof (ADR 0004).
  See `docs/A64_RV_MEMORY_LEAF_PLAN.md`.
- **Co A64/RV `control` gate** — leaf indirect CFG fail-closed (`br`/`blr`,
  `jr`/`jalr`) with adversarial fixtures; golden paths report `control:
  passed`. Sample coverage ≠ full CFG proof. See
  `docs/A64_RV_CONTROL_GATE_PLAN.md`.

### Changed

- Dependabot: `actions/checkout@v7` on CI/fuzz/release workflows.

## [0.2.0] - 2026-07-24

Pre-1.0 developer release past `v0.1.0`. Incomplete ≠ Verified; oracle vectors ≠
formal `ensures`; SoftHSM/search/HlaX64 non-claims live on the VAA side of the
stack.

### Stack role

SemASM is the semantic verifier: object policy, decode/lower, ABI/CFG,
capabilities, behavioral oracles, and `VerificationReport` evidence. It is
**not** an agent controller (that is VAA).

### Added

- **Write-shape leaves** — `replace_byte` / `memset` / `memcpy` contracts,
  oracles (`builtin.buffer.*`), x86 SysV+Win64 harnesses with sample-based
  guard bytes (H2 / ADR 0004), and AArch64+RISC-V harnesses (post-Horizon
  follow-on). Overlap for `memcpy` stays fail-closed (ADR 0003).
- **MemCmp multi-ISA** — AArch64 + RISC-V dual-buffer harnesses + fixtures
  (H3 / ADR 0005); x86 MemCmp already present.
- **Pure-int / buffer leaves** — `min_usize` / `max_usize`, `find_first_byte`,
  `find_last_byte`, `sum_i64` Gate-ready packs (SysV/Win64) with adversarial
  twins.
- **ADRs** — 0003 write-shape; 0004 region-precise memory honesty; 0005
  multi-ISA MemCmp/write-shape honesty; **0006 Region/Alias Evidence v1**
  (`region-affine-v1`: selected affine relations; not general alias analysis);
  **0007 Contract expression semantics v1** (`contract-expr-v1` subset eval +
  `VerificationReport.contract_expressions`; see
  `docs/CONTRACT_EXPR_V1_SUBSET.md`); **0008 A64/RV memory-effect parity**
  (Me0–Me5: A64/RV collectors + `agent verify` alias/expr wire + ± fixtures);
  **0009 A64/RV decode/lower bump** (Dx-parity; Da0–Da5 done —
  `verified_in_ci` sample coverage; as-of-0.2.0 the indirect CFG leaf was still
  x86-only — **superseded in 0.2.1** by Co).
- **G1–G5 program** — Region/Alias → ContractExpr → memory-effect parity →
  VAA isolation ops → VAA trust ops (**ops proof Done**; production trust root
  / hardware HSM / operated remote log remain locked).
- **Da A64/RV decode/lower** — adversarial corpus (unknown insn, trailing,
  W+X via `.semasm_wx`, privilege) + caps flip; CI-verified ≠ formal full-ISA
  proof.
- **Region/Alias Evidence v1** — `function.memory.regions` /
  `relations` schema (CTR008), x86 + AArch64 + RISC-V effect collection,
  fail-closed relation engine, `VerificationReport.alias_analysis`, ± corpus
  fixtures (no `memmove`).
- **Contract Expression Semantics v1** — fail-closed subset evaluator for
  `regions.disjoint|equal|contains` (and closed comparisons with bindings);
  unknown predicates → incomplete; contradicting atoms → failed.
- **Dx adversarial deepen** — unknown-insn classes (`vzeroupper` / `cpuid` /
  `rdtsc`), trailing-bytes on multiple leaves, W+X (incl. patched Win64
  COFF), indirect branch; `agent verify` can stage prebuilt `.obj`/`.o`.
- **Horizon Closeout docs** — landable vs locked-deferred map; formal
  ensures / symbolic alias / CryptOpt / HSM / live Gate remain locked.

### Changed

- **Dx owner sign-off** — x86-64 Linux/Windows `decode` / `lower` →
  `verified_in_ci` (adversarial CI corpus; **≠** full-ISA formal proof).
- **Da owner sign-off** — AArch64/RV64 Linux `decode` / `lower` →
  `verified_in_ci` (ADR 0009 adversarial corpus; **≠** full-ISA formal proof;
  as-of-0.2.0 indirect CFG leaf policy was still x86-only — **superseded in
  0.2.1** by Co).
- x86 assemble/link/execute/`pipeline_verify` already `verified_in_ci` (M1);
  `agent_verify` remains a separate claim from pipeline evidence.
- Caps / README / STABILIZATION honesty synced with multi-ISA write-shape
  and Dx/Da bumps.

### Honesty / non-goals (unchanged)

- No formal theorem prover / `ensures` proof; no full symbolic / general alias
  analysis (Region/Alias v1 is selected affine relations only);
  no CryptOpt embed; sample-based guards ≠ store-region proof.

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
- Partial architecture coverage: x86-64 `decode`/`lower` are `verified_in_ci`
  after Dx owner sign-off (adversarial corpus ≠ full-ISA formal proof);
  AArch64/RV64 `decode`/`lower` remain `partial`. See `semasm status` and
  `capabilities.toml`.

[Unreleased]: https://github.com/megaalive/semasm/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/megaalive/semasm/releases/tag/v0.2.1
[0.2.0]: https://github.com/megaalive/semasm/releases/tag/v0.2.0
[0.1.0]: https://github.com/megaalive/semasm/releases/tag/v0.1.0
