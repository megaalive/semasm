# Fluent Assembly Agent Surface (SemASM side)

SemASM owns **technical capability maturity**, **diagnostic codes**, and
**target authoring profiles**. VAA owns admission policy, work packets, seals,
and agent session fluency.

Canonical product roadmap (releases A–D, non-goals, honesty boundary):

> See VAA [`docs/fluent-agent-surface.md`](https://github.com/megaalive/vaa/blob/main/docs/fluent-agent-surface.md)
> and VAA [`docs/HONESTY.md`](https://github.com/megaalive/vaa/blob/main/docs/HONESTY.md).

## SemASM deliverables for Release A (**delivered**)

VAA Releases A–D (capability admission, fluent repair session, authoring
cases, correctness-preserving optimize) are delivered on VAA `v0.2.0`.
SemASM still owns the technical surfaces below; VAA owns admission policy,
seals, and session fluency.

1. **Capability admission export** — `capabilities.toml` carries optional
   `[[admission]]` rows (contract shape / oracle / authoring_level /
   acceptance_level / required_gates / optional `leaf_names`). Controllers
   obtain a versioned, digestable JSON snapshot via
   `semasm capabilities --format json`.
2. **Stable CLI JSON** — `semasm capabilities --format json` for VAA to pin
   and freeze into task identity (`digest` field is `sha256:` + hex).
3. **Target authoring profile** — `semasm target profile <target> --format json`
   generated from target kit + assembler dialect + ABI facts; consumed by VAA
   as `target-profile.json`.
4. **Stable diagnostic codes** in verification reports so VAA can map failures
   without parsing stderr.

## Fail-closed reminders

- Capability maturity is defined only by `capabilities.toml` (+ its JSON export).
- Code present ≠ CI-proven support.
- RISC-V agent-verify remains unavailable until a dedicated gate exists.
- Agents remain untrusted proposers; SemASM remains the verifier.
- Pure `i64 → i64` **is** supported when the routine name matches a recognized
  op (`abs`, `inc`/`increment`, `max`/`min`, …). `UNSUPPORTED_SHAPE` usually
  means the name was not recognized — not that scalars are unsupported. Hosted
  REPL/I-O programs are outside the leaf harness; see VAA
  `docs/leaf-vs-hosted.md`.
- Behavioral admission is seeded only for x86_64 NASM leaves that match the VAA
  allowlist; do not read authoring profiles as sealed acceptance.
