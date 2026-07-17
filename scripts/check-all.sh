#!/usr/bin/env bash
# Local acceptance checks for SemASM (Unix).
set -euo pipefail
cd "$(dirname "$0")/.."

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
cargo run -p semasm-cli -- --version

echo "All checks passed."
