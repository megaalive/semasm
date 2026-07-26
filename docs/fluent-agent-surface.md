# Fluent Assembly Agent Surface (SemASM side)

SemASM owns **technical capability maturity**, **diagnostic codes**, and
**target authoring profiles**. VAA owns admission policy, work packets, seals,
and agent session fluency.

Canonical product roadmap (releases A–D, non-goals, honesty boundary):

> See VAA [`docs/fluent-agent-surface.md`](https://github.com/megaalive/vaa/blob/main/docs/fluent-agent-surface.md)
> and VAA [`docs/HONESTY.md`](https://github.com/megaalive/vaa/blob/main/docs/HONESTY.md).

## SemASM deliverables for Release A

1. **Capability admission export** — evolve `capabilities.toml` so controllers
   can obtain a versioned, digestable JSON snapshot that answers not only
   “what pipeline maturity does this target have?” but also “what contract
   shapes / oracles / acceptance levels are admitted?”
2. **Stable CLI JSON** — `semasm capabilities --format json` (or equivalent
   versioned status payload) for VAA to pin and freeze into task identity.
3. **Target authoring profile** — generated from target kit + assembler dialect
   + verification profile; consumed by VAA as `target-profile.json`.
4. **Stable diagnostic codes** in verification reports so VAA can map failures
   without parsing stderr.

## Fail-closed reminders

- Capability maturity is defined only by `capabilities.toml` (+ its JSON export).
- Code present ≠ CI-proven support.
- RISC-V agent-verify remains unavailable until a dedicated gate exists.
- Agents remain untrusted proposers; SemASM remains the verifier.
