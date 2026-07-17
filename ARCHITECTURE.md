# Architecture

## Thesis

SemASM closes the semantic gap between human or agent intent and valid multi-target assembly by combining:

1. **Portable semantic contracts** — what code must do, without becoming an implementation language.
2. **Target kits** — ISA, ABI, platform, object format, dialect, tools, and execution profile.
3. **Verification pipelines** — static checks where practical, empirical tests for the rest.

## Critical invariant

```text
Rich build-time tooling is acceptable.
Rich shipped runtime is not.
```

**Generated programs do not link SemASM by default.** No SemASM Rust crate is part of the delivery plane unless an author deliberately vendors a runtime *fragment* written in assembly and selected in a manifest.

## Three planes

### Authoring plane

Intent, project manifests, contracts, target selection, agent task packets, and assembly source management.

### Verification plane

Assemble → object inspect → disassemble → ASIR lower → static checks → link → sandbox/emulator → behavioral tests → size/performance reports.

### Delivery plane

Only selected `.asm` sources, objects, linked images, explicit runtime fragments, optional debug data, and artifact reports.

## Bootstrap crate map

```text
semasm-cli
    |
    +-- semasm-contract  --> semasm-core
    +-- semasm-asir      --> semasm-core
    +-- semasm-target    --> semasm-core
    +-- semasm-core
```

Later crates (`semasm-analysis`, `semasm-object`, arch/ABI/format/adapters) appear only when a vertical slice demonstrates a stable boundary.

## Target identity

A target is never “just” an architecture:

```text
TargetIdentity = ISA + extensions + endianness + word size
               + ABI + platform + object format + dialect
               + assembler + linker + execution profile
               + hardware/machine model
```

## Principles (abbreviated)

See the full project plan for P1–P12. Highlights:

- Assembly source remains inspectable and reviewable.
- Agent output is untrusted until verified.
- Diagnostics teach the violated rule.
- Evidence (size, tests) over marketing claims.
- Core stays architecture-neutral; backends own instruction semantics.

## Out of scope for architecture docs

Detailed ASIR operation catalogs, ABI tables, and agent protocol schemas live in subsequent ADRs and the complete project plan as they are implemented.
