#!/usr/bin/env bash
# Verify the SemASM 0.2 source tree before tagging a release.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"
expected_version="${1:-0.2.1}"
manifest_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n1)"
test "$manifest_version" = "$expected_version"

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --no-deps
cargo package --workspace --no-verify --allow-dirty
cargo run -q -p semasm-cli -- --version
cargo run -q -p semasm-cli -- status
cargo run -q -p semasm-cli -- contract check fixtures/contracts/write_all.sem.toml

printf 'SemASM %s release verification passed.\n' "$expected_version"
