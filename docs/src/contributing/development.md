# Development

Follow the root `CONTRIBUTING.md`. Required checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo test --workspace --all-features
cargo doc --workspace --no-deps
cargo run -q -p semasm-cli -- status
```

On Windows, WSL2 can provide local Linux evidence, but it needs its own Rust,
NASM, and linker installation. Run `target doctor` inside WSL before attempting
the Linux end-to-end example in the quickstart.
