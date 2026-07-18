# Contributing to SemASM

Thank you for helping build SemASM. This project values small vertical slices, measurable evidence, and clear English documentation over large unfinished frameworks.

## Language

Repository language is **English** for source identifiers, comments, diagnostics, documentation, issues, pull requests, and commit messages.

## Development setup

1. Install a stable Rust toolchain with `rustfmt` and `clippy` (see `rust-toolchain.toml`).
2. Clone the repository and run the acceptance commands below from a clean tree.

### Windows and WSL

Windows contributors may use WSL2 for Linux target evidence. Install Rust
inside the WSL distribution as well as NASM and the required ELF linker; the
Windows Rust installation is not automatically available inside Linux. From
WSL, open the repository under `/mnt/<drive>/...`, then run `semasm target
doctor x86_64-unknown-linux-gnu` before the Linux end-to-end command in the
README. Record WSL evidence as local Linux evidence, never as CI evidence.

## Required local checks

Run these before opening a pull request:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo test --workspace --all-features
cargo doc --workspace --no-deps
cargo run -p semasm-cli -- --version
cargo run -p semasm-cli -- status
```

Optional (when installed):

```bash
cargo deny check
mdbook build docs
```

## Design rules (short)

1. **Shipped artifacts stay minimal.** Generated programs must not link SemASM crates by default.
2. **No hidden runtime.** Any startup, syscall wrapper, or fragment must be explicit in manifests and size reports.
3. **ISA, ABI, platform, object format, and dialect are separate.**
4. **Agent output is untrusted until verified.**
5. **Vertical slices before abstractions.** New crates or layers need at least one executable use case.
6. **Heavy integrations are optional** (Capstone, LLVM, QEMU, SMT, AI SDKs) and must not leak into core crates.

## Pull requests

- Keep changes focused and reviewable.
- Add or update tests for behavioral changes.
- Update docs when user-visible behavior or policy changes.
- Do not commit generated binaries or secrets.
- Reference the vertical slice or issue when applicable.

## Adding a dependency

1. Prefer the standard library.
2. Document the reason in `DEPENDENCIES.md`.
3. Disable unused default features when possible.
4. Ensure the license is acceptable under `deny.toml`.
5. Do not add AI provider SDKs, async runtimes, or LLVM library links to core crates without an approved design change.

## Architecture contributions

Architecture and ABI backends require conformance fixtures and documentation. Prefer following an existing target kit once the first hosted slice exists.

## Code of conduct

Participation is governed by [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Security

See [SECURITY.md](SECURITY.md) for reporting unsafe code generation, sandbox escapes, and other security issues.
