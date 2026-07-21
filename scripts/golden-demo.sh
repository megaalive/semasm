#!/usr/bin/env bash
# Golden-path demo: count_byte correct + wrong via agent verify (SysV Linux).
set -euo pipefail
cd "$(dirname "$0")/.."

CONTRACT=fixtures/contracts/count_byte.sem.toml
CARD_DIR="${TMPDIR:-/tmp}/semasm-golden-demo-$$"
mkdir -p "$CARD_DIR"

run_case() {
  local label=$1
  local source=$2
  local allow=${3:-}
  local card="$CARD_DIR/$(basename "$source" .asm).md"
  local args=(agent verify "$source" "$CONTRACT" --format json --card "$card")
  if [[ -n "$allow" ]]; then
    args+=(--allow-execution)
  fi
  echo "=== $label ==="
  set +e
  out="$(cargo run -q -p semasm-cli --features capstone -- "${args[@]}" 2>/tmp/semasm-demo.err)"
  status=$?
  set -e
  if grep -q "toolchain incomplete" /tmp/semasm-demo.err 2>/dev/null; then
    echo "toolchain incomplete — run: cargo run -p semasm-cli -- target doctor x86_64-unknown-linux-gnu"
    cat /tmp/semasm-demo.err >&2
    exit 1
  fi
  printf '%s\n' "$out" | python3 -c "
import json, sys
r = json.load(sys.stdin)
behavior = r.get('behavior') or {}
cases = behavior.get('cases') or []
print(f\"status={r.get('status')} isolation={r.get('isolation')} vectors={len(cases)} exit=${status}\")
"
  if [[ -f "$card" ]]; then
    echo "--- evidence card: $card ---"
    head -n 20 "$card"
  fi
}

run_case "static gates only (expect execution_denied)" fixtures/asm/count_byte.asm
run_case "allow-execution correct (expect verified)" fixtures/asm/count_byte.asm allow
run_case "allow-execution wrong (expect behavior_failed)" fixtures/asm/count_byte_wrong.asm allow

echo "=== compare correct vs wrong ==="
cargo run -q -p semasm-cli --features capstone -- \
  agent compare \
  fixtures/asm/count_byte.asm \
  fixtures/asm/count_byte_wrong.asm \
  "$CONTRACT" \
  --allow-execution \
  --format json | python3 -c "
import json, sys
r = json.load(sys.stdin)
print(f\"status_a={r.get('status_a')} status_b={r.get('status_b')} preferred={r.get('preferred')}\")
"

echo "Golden demo finished."
