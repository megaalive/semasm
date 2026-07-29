#!/usr/bin/env bash
# One-command SemASM onboarding. Core checks are always required; external
# toolchain readiness is reported separately unless --strict-toolchain is used.
set -euo pipefail
cd "$(dirname "$0")/.."

target="${1:-x86_64-unknown-linux-gnu}"
strict="${2:-}"

echo "== SemASM core readiness =="
cargo run -q -p semasm-cli -- --version
cargo run -q -p semasm-cli -- status
cargo run -q -p semasm-cli -- contract check fixtures/contracts/write_all.sem.toml

echo
echo "== Optional target toolchain readiness: $target =="
set +e
cargo run -q -p semasm-cli -- target doctor "$target"
doctor_status=$?
set -e
if [[ $doctor_status -eq 0 ]]; then
  echo "onboarding_result=core_ready,target_ready"
else
  echo "onboarding_result=core_ready,target_unavailable"
  echo "Core onboarding passed. Follow the install hints above for end-to-end execution."
  if [[ "$strict" == "--strict-toolchain" ]]; then
    exit "$doctor_status"
  fi
fi
