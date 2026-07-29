# SemASM 0.3.0 release readiness

Candidate date: 2026-07-29

This document records the release audit for the 66 commits between `v0.2.1`
and the 0.3.0 candidate. It complements `RELEASE_CHECKLIST.md`; it does not
replace CI evidence from the exact candidate commit.

## Version and compatibility audit

- Workspace package version and all internal dependency requirements are
  `0.3.0`.
- CLI command removals: none found.
- Additive CLI surfaces: `capabilities`, `target profile`, and optional
  `--target` selection for `decode`, `cfg`, and `analyze`.
- Existing default behavior is retained for raw-blob inspection:
  `x86_64-unknown-linux-gnu` remains the default target.
- Exit classes remain `0` success, `1` operation/verification failure, and `2`
  invalid usage or target.
- Early `agent verify` failures now emit `AgentFailureEnvelope` on stdout.
  Controllers must discriminate it from `VerificationReport`.
- `VerificationReport` schema changed from `0.4` to `0.5`. Additions include
  `verified_under_preconditions`, region-access evidence, and structured
  findings. This is an explicitly versioned pre-1.0 schema change.
- Product version and document schemas remain independent:
  Verification Report `0.5`, Agent Failure `0.1`, Artifact Report `0.4`,
  Capability `0.1`, and Contract `0.1`.

## Local gates

Record results from the exact candidate checkout:

| Gate | Candidate result |
|---|---|
| Workspace version and lockfile | passed |
| `cargo fmt --all -- --check` | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed |
| `cargo test --workspace --all-features` | passed; toolchain-dependent tests remain CI-owned |
| `cargo doc --workspace --no-deps` | passed |
| `cargo package --workspace --no-verify --allow-dirty` | passed for all 13 crates |
| CLI version/status/contract smoke | passed (`semasm 0.3.0`) |

## External evidence required before tagging

- Linux x86-64 end-to-end owner job.
- Windows x86-64 end-to-end owner job.
- AArch64 and RV64 structural/QEMU owner jobs.
- MSRV and dependency-audit jobs.
- Bounded fuzz build/run workflow with retained corpus evidence.
- Release archive jobs for Linux and Windows.
- `SHA256SUMS` generation and verification for every archive.

The `v0.3.0` annotated tag and GitHub Release must not be created until these
jobs are green on the exact candidate commit. Sample coverage remains distinct
from full-ISA, formal ABI, formal `ensures`, or general symbolic memory proof.
