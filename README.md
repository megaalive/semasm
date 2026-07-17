# SemASM

**SemASM** is multi-architecture semantic infrastructure for software written directly in assembly language. It supplies portable semantic contracts, target kits, analysis (ASIR), and verification so humans and AI agents can produce and check minimal target programs without shipping a high-level language runtime.

> **Status:** early development. VS-00 bootstrap, VS-01 contract checking, and TARGET-002 tool discovery (`target doctor`) are in tree. There are **no** production architecture backends, assembler integration, or end-to-end assembly demos yet. Do not treat planned targets as supported.

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

Requirements: a recent stable Rust toolchain (`rustfmt`, `clippy`).

```bash
cargo build -p semasm-cli
cargo run -p semasm-cli -- --version
cargo run -p semasm-cli -- status
cargo run -p semasm-cli -- contract check fixtures/contracts/write_all.sem.toml
cargo run -p semasm-cli -- --explain CTR003
cargo run -p semasm-cli -- target doctor x86_64-unknown-linux-gnu
cargo run -p semasm-cli -- target doctor x86_64-unknown-linux-gnu --format json
```

Quality gates used in CI:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
```

A five-minute assembly demo will land with later vertical slices (see the project plan / ROADMAP). Until then, this repository is scaffolding only.

## Planned targets (not supported yet)

| Identity | Notes |
|---|---|
| `x86_64-unknown-linux-gnu` | System V, ELF — first hosted slice |
| `x86_64-pc-windows-msvc` | Windows x64, PE/COFF |
| `aarch64-unknown-linux-gnu` | AAPCS64, ELF |
| `riscv64gc-unknown-linux-gnu` | RISC-V psABI, ELF |
| `riscv32imac-unknown-none-elf` (QEMU virt) | bare-metal IoT profile |

## Why semantic metadata?

Raw assembly states machine operations precisely but often hides intent: contracts, ABI bindings, memory effects, register ownership, and measurable constraints. SemASM records that information at build time so agents and checkers can validate patches without inventing another general-purpose language.

## What SemASM is not

- Not a new assembler (uses NASM, GAS, LLVM MC, system linkers).
- Not LLVM IR with different syntax.
- Not a high-level language that “compiles down to” assembly while hiding the implementation.
- Not a guarantee that hand-written or agent-written assembly beats optimizing compilers.
- Not a bundled AI model provider.
- Not a mandatory runtime linked into generated programs.

## Workspace crates (bootstrap)

| Crate | Role |
|---|---|
| `semasm-core` | IDs, spans, diagnostics, errors |
| `semasm-contract` | Portable semantic contracts (types only for now) |
| `semasm-asir` | ASIR graph types |
| `semasm-target` | Target identity, kit shells, tool discovery (`target doctor`) |
| `semasm-cli` | `semasm` binary |

Further crates (analysis, object, agent protocol, arch/ABI backends) appear only when a vertical slice needs a stable boundary.

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
