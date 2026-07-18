# Quickstart

Install the Rust toolchain described by `rust-toolchain.toml`, then run this
five-minute path from the repository root:

```bash
cargo run -q -p semasm-cli -- --version
cargo run -q -p semasm-cli -- status
cargo run -q -p semasm-cli -- contract check fixtures/contracts/write_all.sem.toml
```

The final command exits zero and reports a valid contract. It proves contract
parsing and validation on the current host, not assembly execution.

## Linux end-to-end exploration

On Linux or WSL, first confirm the toolchain:

```bash
cargo run -q -p semasm-cli -- target doctor x86_64-unknown-linux-gnu
```

If Rust, NASM, the required ELF linker, and runner are available, run:

```bash
cargo run -p semasm-cli -- build fixtures/asm/exit.asm \
  --target x86_64-unknown-linux-gnu \
  --out-dir target/e2e-linux
```

This is not yet a named CI evidence job. A local success must not be recorded as
`CI-verified` in `capabilities.toml`.

## Reading analysis results

Current architecture coverage is partial. Until incompleteness propagation is
implemented, a `clean` ABI result means only that the modeled instructions
produced no finding. An unsupported instruction can currently be omitted from
the analysis, so `clean` must not be interpreted as full verification.
