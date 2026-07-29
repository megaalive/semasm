# Catch a deliberate assembly bug

Run the known-wrong implementation against the same contract and oracle:

```bash
cargo run -q -p semasm-cli -- agent verify \
  fixtures/asm/count_byte_wrong.asm \
  fixtures/contracts/count_byte.sem.toml \
  --target x86_64-unknown-linux-gnu \
  --allow-execution --format json
```

The command exits non-zero and reports `behavior_failed`. Inspect
`behavior.cases[]` for expected and observed values. This demonstrates a
behavioral mismatch, not merely a syntax or linker failure.

For side-by-side evidence:

```bash
cargo run -q -p semasm-cli -- agent compare \
  fixtures/asm/count_byte.asm \
  fixtures/asm/count_byte_wrong.asm \
  fixtures/contracts/count_byte.sem.toml \
  --target x86_64-unknown-linux-gnu \
  --allow-execution --format json
```

Do not turn a toolchain error, unsupported shape, or `execution_denied` into a
passing result. Those outcomes mean the requested evidence was not produced.
