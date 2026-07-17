# Governance

## Status

SemASM is in bootstrap. Governance is intentionally lightweight.

## Maintainers

Maintainers are listed in the repository’s GitHub settings and/or `CODEOWNERS` when present. Until a formal team is published, the repository owner(s) act as maintainers.

## Decision process

1. **Small changes** (bug fixes, docs, tests, dependency patches) merge via ordinary pull request review.
2. **Interface or crate boundary changes** should reference an ADR under `adr/` or an RFC under `rfcs/` when they affect multiple crates or public CLI stability.
3. **New architecture or ABI backends** require conformance fixtures and documentation before they are marked supported.
4. **Heavy optional integrations** (Capstone, LLVM libraries, QEMU, Unicorn, AI SDKs) require an explicit design note and feature flags.

## Scope control

The product boundary in the project plan applies:

- SemASM is semantic infrastructure around assembly, not a new general-purpose language.
- Reject features that hide assembly behind a high-level implementation language while claiming “assembly delivery.”

## Releases

Release process, versioning, and crate publication policy will be documented when the first usable vertical slice ships. Until then, crate versions remain `0.1.0` and APIs are unstable.

## Code of conduct

Community behavior is governed by [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
