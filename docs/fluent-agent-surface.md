# Fluent Assembly Agent Surface (SemASM side)

SemASM owns **technical capability maturity**, **diagnostic codes**, and
**target authoring profiles**. VAA owns admission policy, work packets, seals,
and agent session fluency.

Canonical product roadmap (releases A–D, non-goals, honesty boundary):

> See VAA [`docs/fluent-agent-surface.md`](https://github.com/megaalive/vaa/blob/main/docs/fluent-agent-surface.md)
> and VAA [`docs/HONESTY.md`](https://github.com/megaalive/vaa/blob/main/docs/HONESTY.md).

## SemASM deliverables for Release A

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
- Behavioral admission is seeded only for x86_64 NASM leaves that match the VAA
  allowlist; do not read authoring profiles as sealed acceptance.
