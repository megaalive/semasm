# Stabilization Baseline — July 2026

This document records the repository state before stabilization work changes
production behavior. It is a snapshot, not a capability promise. The raw local
transcript was produced with `scripts/baseline.ps1`; it is intentionally written
under `target/` and is not committed because it contains host-specific paths.

## Snapshot

- Commit: `e394d8f08f6d1453bdac368b1068ff805312ea7b`
- Host: Windows, `x86_64-pc-windows-msvc`
- Rust: `rustc 1.97.1 (8bab26f4f 2026-07-14)`, LLVM 22.1.6
- Cargo: `cargo 1.97.1 (c980f4866 2026-06-30)`
- Declared MSRV: Rust 1.85
- Baseline command: `./scripts/baseline.ps1`
- Deterministic transcript path: `target/stabilization-baseline.txt`

The working tree contained an unrelated, untracked `.commandcode/` directory.
It was not inspected as project evidence and is not part of this baseline.

## Command results

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Passed |
| `cargo test --workspace` | Passed; five tool-dependent tests remained ignored |
| `cargo test --workspace --all-features` | Passed; five tool-dependent tests remained ignored |
| `cargo doc --workspace --no-deps` | Passed |
| `cargo deny check --all-features` | Not run: `cargo-deny` was not installed (exit 101) |
| `cargo check --workspace --no-default-features` | Passed |
| `cargo test --workspace --no-default-features` | Passed; five tool-dependent tests remained ignored |
| `cargo +1.85 check --workspace --all-targets` | Passed with Rust 1.85.1 |
| `cargo +1.85 test --workspace --no-default-features` | Passed with Rust 1.85.1; five tests remained ignored |
| `cargo run -q -p semasm-cli -- --version` | Passed |
| `cargo run -q -p semasm-cli -- status` | Passed; output is known to omit AArch64 and RISC-V crates |
| `cargo run -q -p semasm-cli -- target doctor x86_64-unknown-linux-gnu` | Incomplete (exit 1): the requested Linux linker and runner were not found on this Windows host |
| `cargo build -p semasm-cli` | Passed |
| `cargo build -p semasm-cli --no-default-features --release` | Passed |
| `git diff --check` | Passed at collection time |

No ignored end-to-end test was executed by the standard workspace test
commands. A passing row above therefore does not prove assembly, link, native
execution, or cross-target execution.

## External tools

| Tool | Observed state |
|---|---|
| NASM | 3.01 |
| GNU objdump | Binutils 2.45 |
| GNU ld | Binutils 2.45 |
| MinGW `cc` | GCC 15.2.0 |
| Clang | 22.1.6 |
| `lld-link` | 22.1.6 |
| MSVC `link` | Not found |
| `qemu-aarch64` | Not found |
| `qemu-riscv64` | Not found |
| `cargo-deny` | Not found |

Versions and paths can differ on another host. Run the baseline script on each
CI or development environment instead of copying these observations.

## Ignored tests

| Test location | Requirement declared by the test |
|---|---|
| `crates/semasm-build/src/pipeline.rs:496` | NASM on `PATH` |
| `crates/semasm-build/src/pipeline.rs:521` | NASM and linker on `PATH` |
| `crates/semasm-build/src/pipeline.rs:573` | NASM on `PATH` |
| `crates/semasm-build/src/pipeline.rs:592` | NASM and `lld-link` on a Windows host |
| `crates/semasm-build/src/report.rs:808` | NASM and linker on `PATH` |

## Fixture inventory

- `fixtures/asm/count_byte.asm`
- `fixtures/asm/count_byte_wrong.asm`
- `fixtures/asm/exit.asm`
- `fixtures/asm/exit.sem.toml`
- `fixtures/asm/hello_win64.asm`
- `fixtures/asm/win64_broken.asm`
- `fixtures/asm/win64_clean.asm`
- `fixtures/contracts/count_byte.sem.toml`
- `fixtures/contracts/write_all.sem.toml`
- `fixtures/diagnostics/ctr003_bad_type.sem.toml`

## Capability evidence

The levels below describe evidence observed in this baseline. “Unit” means the
workspace tests exercised in-process behavior. “Ignored E2E” means a scenario
exists but was not executed by the acceptance commands. “Not exercised” does
not claim that the implementation is absent.

| Target area | Identity | Decode/lower/ABI evidence | Build evidence | Execution evidence |
|---|---|---|---|---|
| x86-64 Linux SysV | Unit-tested | Unit-tested, partial semantics | Ignored E2E only | Ignored E2E only |
| x86-64 Windows MSVC | Present in source and unit tests | Unit-tested, partial semantics | Ignored E2E only | Ignored E2E only |
| AArch64/AAPCS64 | Present in source and unit tests | Unit-tested, partial semantics | Not exercised | Not exercised; QEMU unavailable |
| RISC-V | Present in source and unit tests | Unit-tested, partial semantics | Not exercised | Not exercised; QEMU unavailable |

This matrix deliberately does not use `verified` or `clean`. The review found
known false-clean paths around unsupported and unknown instructions; those are
scheduled for later stabilization changes.

## Binary sizes

The sizes below are Windows PE files built on the snapshot host. They are not
cross-platform comparisons.

- Debug CLI (`target/debug/semasm.exe`): 5,037,568 bytes
- Release CLI, no default features (`target/release/semasm.exe`): 2,011,136 bytes

## Known failures and uncertainty

- Dependency-policy validation is unavailable until `cargo-deny` is installed.
- The Linux target-doctor smoke check is incomplete on this Windows host.
- Five external-tool tests are ignored by the ordinary test commands.
- Linux native, Windows PE end-to-end, AArch64 QEMU, and RISC-V QEMU evidence was
  not produced by this baseline run.
- The CLI status text is hand-maintained and already lags the workspace crate
  list. It must not be treated as the source of truth.
- Analyzer soundness findings in the review remain unfixed in this baseline;
  passing unit tests do not negate those findings.
- No claim about reproducibility across hosts is made from a single host run.

## Scope freeze and next change

Until the stabilization sequence is complete, do not add an ISA, analyzer,
crate, dependency, provider integration, or high-level architecture. The next
ordered work package is STAB-002: introduce the machine-readable capability
manifest. Production behavior is unchanged by this baseline commit.
