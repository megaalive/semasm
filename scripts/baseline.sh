#!/usr/bin/env bash
# Collect the SemASM stabilization baseline without changing tracked source files.
set -u

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"
output_path="${1:-target/stabilization-baseline.txt}"
mkdir -p "$(dirname "$output_path")"
printf '# SemASM stabilization baseline\n' >"$output_path"

section() {
    printf '\n## %s\n' "$1" >>"$output_path"
}

record() {
    label="$1"
    shift
    printf '\n### %s\n' "$label" >>"$output_path"
    "$@" >>"$output_path" 2>&1
    status=$?
    printf '[exit code: %s]\n' "$status" >>"$output_path"
}

section 'Repository and host'
record 'Commit' git rev-parse HEAD
record 'Working tree' git status --short
record 'Host' rustc -Vv
record 'Cargo' cargo -V

section 'External tools'
for tool in nasm objdump ld cc clang link lld-link qemu-aarch64 qemu-riscv64 cargo-deny; do
    if command -v "$tool" >/dev/null 2>&1; then
        printf '%s: %s\n' "$tool" "$(command -v "$tool")" >>"$output_path"
        record "$tool version" "$tool" --version
    else
        printf '%s: not found\n' "$tool" >>"$output_path"
    fi
done

section 'Ignored tests'
grep -R -n --include='*.rs' '#\[ignore' crates >>"$output_path" 2>&1

section 'Fixtures'
find fixtures -type f | LC_ALL=C sort >>"$output_path" 2>&1

section 'Acceptance commands'
record 'Formatting' cargo fmt --all -- --check
record 'Clippy all features' cargo clippy --workspace --all-targets --all-features -- -D warnings
record 'Tests default features' cargo test --workspace
record 'Tests all features' cargo test --workspace --all-features
record 'Documentation' cargo doc --workspace --no-deps
record 'Dependency policy' cargo deny check --all-features
record 'Check no default features' cargo check --workspace --no-default-features
record 'Tests no default features' cargo test --workspace --no-default-features
record 'MSRV check' cargo +1.85 check --workspace --all-targets
record 'MSRV tests no default features' cargo +1.85 test --workspace --no-default-features
record 'CLI version' cargo run -q -p semasm-cli -- --version
record 'CLI status' cargo run -q -p semasm-cli -- status
record 'CLI target doctor' cargo run -q -p semasm-cli -- target doctor x86_64-unknown-linux-gnu
record 'Debug CLI build' cargo build -p semasm-cli
record 'Release CLI build no default features' cargo build -p semasm-cli --no-default-features --release
record 'Git diff check' git diff --check

section 'Binary sizes'
for binary in target/debug/semasm target/release/semasm; do
    if [ -f "$binary" ]; then
        size="$(wc -c <"$binary" | tr -d ' ')"
        printf '%s: %s bytes\n' "$binary" "$size" >>"$output_path"
    else
        printf '%s: not produced\n' "$binary" >>"$output_path"
    fi
done

printf 'Baseline written to %s/%s\n' "$repo_root" "$output_path"
