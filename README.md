# SemASM

**SemASM** is multi-architecture semantic infrastructure for software written directly in assembly language. It supplies portable semantic contracts, target kits, analysis (ASIR), and verification so humans and AI agents can produce and check minimal target programs without shipping a high-level language runtime.

> **Status:** early stabilization. Contract checking, target discovery, build and
> report infrastructure, object inspection, decoding, CFG construction, and
> partial x86-64, AArch64, and RISC-V semantics are implemented. Capability
> maturity comes only from `capabilities.toml`; code in the tree is not the same
> as CI-proven support.

## Architecture (build-time only)

```text
Authoring plane          Verification plane           Delivery plane
-----------------        --------------------         ----------------
intent + contracts  -->  assemble / ASIR / checks -->  .asm + objects
target kits              object inspect / tests        linked image
agent task packets       size / performance report     no SemASM runtime
```

Rich tools (Rust analyzers, optional Capstone/LLVM/QEMU adapters) may exist in the **factory**. Generated executables and firmware images contain only instructions, data, startup code, platform interfaces, and **explicitly selected** runtime fragments.

## Quick start (tooling)

Requirements: a recent stable Rust toolchain with `rustfmt` and `clippy`.

```bash
cargo build -p semasm-cli
cargo run -p semasm-cli -- --version
cargo run -p semasm-cli -- status
cargo run -p semasm-cli -- contract check fixtures/contracts/write_all.sem.toml
cargo run -p semasm-cli -- --explain CTR003
cargo run -p semasm-cli -- target doctor x86_64-unknown-linux-gnu
cargo run -p semasm-cli -- target doctor x86_64-unknown-linux-gnu --format json

```

The contract command is the current five-minute success path. It exits zero and
prints that the `write_all` contract is valid. It does not assemble or execute a
program.

An exact Linux end-to-end command is:

```bash
cargo run -p semasm-cli -- build fixtures/asm/exit.asm \
  --target x86_64-unknown-linux-gnu \
  --out-dir target/e2e-linux
```

Run it only on Linux or WSL with Rust, NASM, a compatible ELF linker, and any
runner reported by `target doctor`. This path is not yet executed by a named CI
job, so it is an exploratory check rather than published support evidence.

Quality gates used in CI:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
```

## What SemASM does not yet prove

- No target is currently marked `CI-verified` or `release-qualified`.
- Ordinary workspace tests do not execute the ignored external-tool scenarios.
- A decoder or lowering implementation may cover only part of an ISA.
- Cross-host reproducibility and AArch64/RISC-V execution are not established.
- Current ABI commands can drop unsupported instructions. Until that is fixed,
  `clean` means no violation was found in the modeled subset; it does **not**
  prove the whole input was modeled. Unsupported instructions must be treated
  as incomplete evidence by users and automation.

## Target capability evidence

The table is generated from `capabilities.toml`. Levels describe implementation
and evidence maturity; they are not a blanket support promise.

<!-- capabilities:start -->
| Identity | Decode | Lower | ABI | Assemble | Link | Execute | Verify |
|---|---|---|---|---|---|---|---|
| `x86_64-unknown-linux-gnu` | partial | partial | unit-tested | experimental | experimental | experimental | experimental |
| `x86_64-pc-windows-msvc` | partial | partial | unit-tested | experimental | experimental | experimental | experimental |
| `aarch64-unknown-linux-gnu` | partial | partial | unit-tested | unavailable | unavailable | unavailable | experimental |
| `riscv64gc-unknown-linux-gnu` | declared | partial | unit-tested | unavailable | unavailable | unavailable | experimental |
| `riscv32imac-unknown-none-elf` | declared | partial | unit-tested | unavailable | unavailable | unavailable | experimental |
<!-- capabilities:end -->

## Why semantic metadata?

Raw assembly states machine operations precisely but often hides intent: contracts, ABI bindings, memory effects, register ownership, and measurable constraints. SemASM records that information at build time so agents and checkers can validate patches without inventing another general-purpose language.

## What SemASM is not

- Not a new assembler (uses NASM, GAS, LLVM MC, system linkers).
- Not LLVM IR with different syntax.
- Not a high-level language that “compiles down to” assembly while hiding the implementation.
- Not a guarantee that hand-written or agent-written assembly beats optimizing compilers.
- Not a bundled AI model provider.
- Not a mandatory runtime linked into generated programs.

## Workspace crates

| Crate | Role |
|---|---|
| `semasm-core` | IDs, spans, diagnostics, errors |
| `semasm-contract` | Portable semantic contracts and validation |
| `semasm-asir` | ASIR graph types |
| `semasm-target` | Target identity, capability registry, and tool discovery |
| `semasm-build` | Safe process execution, build pipeline (assemble, link, verify, run), artifact reports |
| `semasm-agent` | Provider-neutral agent packets and verification |
| `semasm-cli` | `semasm` binary |
| `semasm-obj` | Structured object-file inspection |
| `semasm-decode` | Physical instruction decoding |
| `semasm-cfg` | Control-flow graph construction |
| `semasm-x86` | x86-64 lowering and ABI analysis |
| `semasm-aarch64` | AArch64 lowering and ABI analysis |
| `semasm-riscv` | RISC-V lowering and ABI analysis |

These boundaries are implemented but still await the stabilization boundary
audit; crate count is not itself evidence of capability maturity.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). English is required for source, comments, docs, and issues. Prefer small vertical slices that produce executable evidence over large abstract frameworks.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Related documents

- [ARCHITECTURE.md](ARCHITECTURE.md) — planes, principles, crate boundaries  
- [ROADMAP.md](ROADMAP.md) — vertical slices  
- [DEPENDENCIES.md](DEPENDENCIES.md) — dependency policy  
- [semasm-complete-project-plan.md](semasm-complete-project-plan.md) — full technical specification  
