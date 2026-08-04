# Use SemASM as a pull-request gate

The reusable workflow downloads a published Linux archive, verifies its entry
against `SHA256SUMS`, installs the external x86-64 toolchain, runs behavioral
verification, and retains the JSON report plus evidence card.

Create `.github/workflows/semasm.yml` in the consumer repository:

```yaml
name: SemASM

on:
  pull_request:
  push:
    branches: [main]

jobs:
  verify-count-byte:
    uses: megaalive/semasm/.github/workflows/consumer-verify.yml@v0.5.0
    with:
      source: asm/count_byte.asm
      contract: contracts/count_byte.sem.toml
      symbol: count_byte
      semasm-version: 0.5.0
```

Pin both the reusable workflow ref and `semasm-version`. Updating either is an
explicit compatibility decision. The initial reusable surface supports
`x86_64-unknown-linux-gnu`; other target owners remain in SemASM's own CI until
their consumer packaging path is separately proven.
