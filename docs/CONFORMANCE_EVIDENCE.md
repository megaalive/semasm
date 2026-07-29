# Conformance evidence

SemASM CI emits one machine-readable record for each target-owner job. Records
use [`CONFORMANCE_RECORD_SCHEMA.json`](CONFORMANCE_RECORD_SCHEMA.json) and are
retained as GitHub Actions artifacts.

Outcome vocabulary is intentionally explicit:

- `passed`: every named evidence command ran and passed.
- `failed`: at least one required evidence command ran and failed.
- `skipped`: the command was deliberately not run by policy.
- `unavailable`: execution was requested but its toolchain or runner was absent.
- `not_applicable`: the evidence does not apply to that target or host.

Only `passed` supports a CI-verified claim. A green ordinary workspace test
does not replace target-owner evidence, and an omitted record is not equivalent
to `passed`.

Current owner records:

| Target | Owner evidence |
|---|---|
| `x86_64-unknown-linux-gnu` | build/object ignored suites plus canonical evidence hash |
| `x86_64-pc-windows-msvc` | PE build/run plus Win64 semantic and behavioral corpus |
| `aarch64-unknown-linux-gnu` | assemble/link/QEMU plus AArch64 agent corpus |
| `riscv64gc-unknown-linux-gnu` | assemble/link/QEMU plus RV64 agent corpus |

The reliability job separately exercises timeout, child-tree cleanup, bounded
dual-stream capture, and invalid UTF-8. The performance job records release
binary size and warmed `semasm status` latency, with deliberately generous
regression ceilings rather than performance claims.
