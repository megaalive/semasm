# Verify an existing routine

Start with an assembly routine and its semantic contract. This example uses
the checked-in `count_byte` pair:

```bash
cargo run -q -p semasm-cli -- agent verify \
  fixtures/asm/count_byte.asm \
  fixtures/contracts/count_byte.sem.toml \
  --target x86_64-unknown-linux-gnu \
  --format json
```

Without `--allow-execution`, a valid static path returns
`execution_denied`. This is intentional: the report proves static gates ran,
not that behavioral vectors executed.

After `target doctor` reports every role resolved, opt into execution:

```bash
cargo run -q -p semasm-cli -- agent verify \
  fixtures/asm/count_byte.asm \
  fixtures/contracts/count_byte.sem.toml \
  --target x86_64-unknown-linux-gnu \
  --allow-execution --format json \
  --card target/count-byte-evidence.md
```

Accept only `status: verified`. `verified_under_preconditions` carries caller
obligations and must not be promoted to unconditional verification.
