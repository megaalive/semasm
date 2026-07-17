# Contract schema compatibility policy

## Current schema

- Version string accepted by the loader: `"0.1"` only (`CTR001` otherwise).
- Document format: TOML sidecar files (typically `*.sem.toml`).

## Unknown fields

**Rejected.** Core contract tables use strict deserialization (`deny_unknown_fields`).

Rationale: silent acceptance would let agents invent imperative-looking fields and treat them as real contracts. Unknown keys are schema errors with a clear diagnostic, not soft warnings.

## Unknown required fields

Missing required keys (for example `function.name`) fail deserialization with a parse diagnostic. There is no silent default for function names or parameter types.

## Forward compatibility

- Adding optional fields in a future `0.2` schema will require bumping `contract_version`.
- Parsers for `0.1` must not invent defaults that change semantic meaning.
- Expression and semantic-type grammars are closed: unknown syntax fails (`CTR003` / `CTR004`).

## Preservation

Raw TOML text is not rewritten by `semasm contract check`. Validation is read-only.
