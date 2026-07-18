# SemASM 0.1 release checklist

Run from a clean checkout of the candidate tag. A failed or skipped required
gate blocks release.

## Source and compatibility

- [ ] `CHANGELOG.md` contains the release version and date.
- [ ] Workspace and crate versions match the `vMAJOR.MINOR.PATCH` tag.
- [ ] Capability documentation is regenerated from `capabilities.toml`.
- [ ] Public JSON compatibility status and CLI exit codes are documented.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
      passes.
- [ ] `cargo test --workspace --all-features` passes.
- [ ] `cargo doc --workspace --no-deps` passes.
- [ ] MSRV and dependency-audit CI jobs pass.

## Evidence

- [ ] Linux and Windows x86-64 end-to-end jobs pass.
- [ ] AArch64 and RV64 structural/QEMU evidence jobs pass.
- [ ] Canonical evidence hashes match across independent output roots.
- [ ] Negative corpus tests pass.
- [ ] Every fuzz target compiles; the latest bounded fuzz workflow is green.

## Packaging

- [ ] `scripts/verify-release.ps1` or `scripts/verify-release.sh` passes.
- [ ] `cargo package --workspace --allow-dirty` succeeds before tagging (omit
      `--allow-dirty` on the clean release checkout).
- [ ] Linux and Windows CLI archives are produced from the tag.
- [ ] `SHA256SUMS` contains every archive and verifies successfully.
- [ ] Release notes state partial architecture coverage and pre-1.0 API status.
- [ ] The signed/annotated tag and GitHub release point to the same commit.
