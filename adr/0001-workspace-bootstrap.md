# ADR 0001: Workspace bootstrap with five crates

## Status

Accepted

## Context

SemASM must avoid an impressive empty monorepo. The project plan constrains the initial workspace to five crates.

## Decision

Start with:

- `semasm-core`
- `semasm-contract`
- `semasm-asir`
- `semasm-target`
- `semasm-cli`

Do not add analysis, object, agent, arch, ABI, or adapter crates until a completed vertical slice needs a stable boundary.

## Consequences

- Faster bootstrap and clearer ownership.
- Some types will move when real boundaries emerge; that is acceptable before 1.0.
