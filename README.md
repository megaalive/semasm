# SemASM

**SemASM** is multi-architecture semantic infrastructure for software written
directly in assembly. It supplies portable semantic contracts, target kits,
analysis (ASIR), and verification so humans and AI agents can produce and check
minimal target programs **without shipping a high-level language runtime**.

> **Status:** pre-1.0 developer tooling (0.1 line). Thirteen workspace crates,
> multi-target agent verify (SysV / Win64 / AArch64 / RV64), structured
> `VerificationReport` evidence (including optional Region/Alias Evidence v1
> `region-affine-v1`, Region Access Evidence v1 `region-access-affine-v1`, and
> Contract Expression Semantics v1 `contract-expr-v1` —
> selected affine relations + documented expression subset, **not** general
> alias analysis or full contract verification), and named CI owner jobs are in tree.
> Capability maturity is defined only by `capabilities.toml` — code present ≠
> CI-proven support. Agent output remains **untrusted until verified**.

## Architecture (build-time only)

```text
Authoring plane          Verification plane           Delivery plane
-----------------        --------------------         ----------------
intent + contracts  -->  assemble / ASIR / checks -->  .asm + objects
target kits              object inspect / tests        linked image
agent task packets       size / performance report     no SemASM runtime
```

Rich tools (Rust analyzers, optional Capstone/QEMU adapters) live in the
**factory**. Generated executables contain only instructions, data, startup,
platform interfaces, and **explicitly selected** fragments — never a SemASM
runtime.

## Golden demo: `count_byte` in five minutes

The shortest path that shows why SemASM exists: a real routine, a semantic
contract, static gates, then behavioral vectors under `--allow-execution`.

Requirements: Rust stable, plus `nasm`, an ELF linker, `objdump`/`llvm-objdump`,
and `qemu-x86_64` (see `semasm target doctor x86_64-unknown-linux-gnu`). On
Windows PE hosts use `--target x86_64-pc-windows-msvc` with NASM + `lld-link`.

```bash
cargo build -p semasm-cli --features capstone

# Static gates only → execution_denied (structured JSON, exit 1)
cargo run -p semasm-cli --features capstone -- agent verify \
  fixtures/asm/count_byte.asm \
  fixtures/contracts/count_byte.sem.toml \
  --format json

# Full verify → verified when every harness vector passes (exit 0)
# Also write a one-page evidence card for PRs / agents
cargo run -p semasm-cli --features capstone -- agent verify \
  fixtures/asm/count_byte.asm \
  fixtures/contracts/count_byte.sem.toml \
  --allow-execution \
  --format json \
  --card /tmp/count_byte-card.md

# Deliberate wrong implementation → behavior_failed (never silent success)
cargo run -p semasm-cli --features capstone -- agent verify \
  fixtures/asm/count_byte_wrong.asm \
  fixtures/contracts/count_byte.sem.toml \
  --allow-execution \
  --format json

# Compare two candidates against one contract
cargo run -p semasm-cli --features capstone -- agent compare \
  fixtures/asm/count_byte.asm \
  fixtures/asm/count_byte_wrong.asm \
  fixtures/contracts/count_byte.sem.toml \
  --allow-execution \
  --format json
```

Also available: Win64 (`count_byte_win64.asm`), AArch64 / RV64 gas fixtures,
and a second harness shape `min_usize` (`fixtures/contracts/min_usize.sem.toml`).
Report field `isolation` is `static_only`, `qemu_user`, or `native_host` —
honesty about how (or whether) a process ran, not an OS sandbox claim.
x86 golden-path leaves also fail closed on indirect control flow (`jmp rax` /
`call rax`) and on stores into a read-only buffer leaf.
Equality for `count_byte` is proven by `behavior_oracle`
(`builtin.buffer.count_equal_u8` + vectors), not by the weak contract
`ensures count <= length` alone.

One-shot scripts (print `status` / `isolation` / vector count for correct +
wrong):

```bash
# Linux SysV (needs nasm + linker + qemu-x86_64)
bash scripts/golden-demo.sh
```

```powershell
# Windows PE (default) or pass -SysV for Linux tools
powershell -ExecutionPolicy Bypass -File scripts/golden-demo.ps1
```

## Quick start (tooling)

```bash
cargo build -p semasm-cli
cargo run -p semasm-cli -- --version
cargo run -p semasm-cli -- status
cargo run -p semasm-cli -- contract check fixtures/contracts/count_byte.sem.toml
cargo run -p semasm-cli -- target doctor x86_64-unknown-linux-gnu
```

Pipeline assemble / link / run (exit fixture, not agent gates):

```bash
cargo run -p semasm-cli -- build fixtures/asm/exit.asm \
  --target x86_64-unknown-linux-gnu \
  --out-dir target/e2e-linux
```

Quality gates used in CI:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
```

## What SemASM does not yet prove

- Support maturity is per capability; a target is not globally "supported"
  merely because some rows are `CI-verified`.
- External-tool scenarios run in dedicated CI owner jobs
  (`SEMASM_REQUIRE_TOOLCHAIN=1`); local soft-skip is not allowed there.
- A decoder or lowering implementation may cover only part of an ISA.
  On x86-64 Linux/Windows and AArch64/RV64 Linux, `decode` / `lower` are
  `CI-verified` against the Dx/Da adversarial corpora (not a full-ISA formal
  proof). RV32 decode remains `declared` / lower `partial`.
- Canonical report reproducibility is checked across independent output roots;
  this does not promise byte-identical artifacts from every toolchain.
- Manifest **Pipeline** vs **Agent** columns are separate: pipeline
  `CI-verified` means fixture assemble/link/run; **Agent** means
  `semasm agent verify` gates (+ harness when claimed). See
  `docs/CLI_COMPATIBILITY.md`.
- Incomplete ABI analysis is never promoted to a clean full verification.

## Target capability evidence

The table is generated from `capabilities.toml`. Levels describe implementation
and evidence maturity; they are not a blanket support promise. **Pipeline** =
build e2e; **Agent** = `semasm agent verify`.

<!-- capabilities:start -->
| Identity | Decode | Lower | ABI | Assemble | Link | Execute | Pipeline | Agent |
|---|---|---|---|---|---|---|---|---|
| `x86_64-unknown-linux-gnu` | CI-verified | CI-verified | CI-verified | CI-verified | CI-verified | CI-verified | CI-verified | CI-verified |
| `x86_64-pc-windows-msvc` | CI-verified | CI-verified | CI-verified | CI-verified | CI-verified | CI-verified | CI-verified | CI-verified |
| `aarch64-unknown-linux-gnu` | CI-verified | CI-verified | CI-verified | CI-verified | CI-verified | CI-verified | CI-verified | CI-verified |
| `riscv64gc-unknown-linux-gnu` | CI-verified | CI-verified | CI-verified | CI-verified | CI-verified | CI-verified | CI-verified | CI-verified |
| `riscv32imac-unknown-none-elf` | declared | partial | unit-tested | unavailable | unavailable | unavailable | unavailable | declared |
<!-- capabilities:end -->

## Why semantic metadata?

Raw assembly states machine operations precisely but often hides intent:
contracts, ABI bindings, memory effects, register ownership, and measurable
constraints. SemASM records that at build time so agents and checkers can
validate patches without inventing another general-purpose language.

## What SemASM is not

- Not a new assembler (uses NASM, GAS, system linkers).
- Not LLVM IR with different syntax.
- Not a high-level language that “compiles down to” assembly while hiding the implementation.
- Not a guarantee that hand-written or agent-written assembly beats optimizing compilers.
- Not a bundled AI model provider.
- Not a mandatory runtime linked into generated programs.
- Not a memory-safety proof for arbitrary agent asm.

## Workspace crates

| Crate | Role |
|---|---|
| `semasm-core` | IDs, spans, diagnostics, errors |
| `semasm-contract` | Portable semantic contracts and validation |
| `semasm-asir` | ASIR graph types |
| `semasm-target` | Target identity, capability registry, and tool discovery |
| `semasm-build` | Safe process execution, build pipeline, artifact reports |
| `semasm-agent` | Provider-neutral agent packets, harness, and verification reports |
| `semasm-cli` | `semasm` binary |
| `semasm-obj` | Structured object-file inspection |
| `semasm-decode` | Physical instruction decoding |
| `semasm-cfg` | Control-flow graph construction |
| `semasm-x86` | x86-64 lowering and ABI analysis |
| `semasm-aarch64` | AArch64 lowering and ABI analysis |
| `semasm-riscv` | RISC-V lowering and ABI analysis |

Crate count is not itself evidence of capability maturity; see the table above.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). English is required for source, comments,
docs, and issues. Prefer small vertical slices that produce executable evidence
over large abstract frameworks. Near-term priority is deepening soundness and
the golden demo, not adding new ISAs.

Release compatibility and gates are documented in
[docs/CLI_COMPATIBILITY.md](docs/CLI_COMPATIBILITY.md) and
[docs/RELEASE_CHECKLIST.md](docs/RELEASE_CHECKLIST.md). The `v0.1.0` GitHub
Release distributes CLI archives + `SHA256SUMS` (no crates.io publish yet).
Pre-1.0 APIs may still evolve.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Related documents

- [ARCHITECTURE.md](ARCHITECTURE.md) — planes, principles, crate boundaries  
- [ROADMAP.md](ROADMAP.md) — vertical slices  
- [DEPENDENCIES.md](DEPENDENCIES.md) — dependency policy  
- [SECURITY.md](SECURITY.md) — isolation honesty vs sandbox claims  
- [docs/STABILIZATION_PROGRESS.md](docs/STABILIZATION_PROGRESS.md) — PR checklist + bulletproof phases  
- [semasm-complete-project-plan.md](semasm-complete-project-plan.md) — full technical specification  
