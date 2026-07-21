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

Assemble → object inspect → disassemble → ASIR lower → static checks → link →
sandbox/emulator → behavioral tests → size/performance reports.

Agent candidate verification (`semasm agent verify`) follows the same plane and
emits an immutable `VerificationReport` from `semasm-agent::verify`:

1. Assemble the candidate with the target's `nasm_format()`.
2. Static semantic gates (object policy, decode, lowering, ABI, capability).
3. Assemble harness, link, executable-container gate.
4. Behavioral harness only after `--allow-execution`.

Stage results are composed once; failures still emit the report (JSON or
terminal) so agents can audit evidence. See `docs/CLI_COMPATIBILITY.md`.

### Delivery plane

Only selected `.asm` sources, objects, linked images, explicit runtime fragments, optional debug data, and artifact reports.

## Current crate map

```text
semasm-cli
    |
    +-- semasm-contract  --> semasm-core
    +-- semasm-agent     --> semasm-contract, semasm-target
    +-- semasm-build     --> semasm-target, semasm-obj
    +-- semasm-decode    --> normalized physical instructions
    +-- semasm-cfg       --> semasm-decode
    +-- semasm-x86       --> x86 lowering and ABI analysis
    +-- semasm-aarch64   --> AArch64 lowering and ABI analysis
    +-- semasm-riscv     --> RISC-V lowering and ABI analysis
    +-- semasm-obj       --> structured object inspection
    +-- semasm-asir      --> semasm-core
    +-- semasm-target    --> semasm-core, capability manifest
```

The workspace contains thirteen crates. Their boundaries remain subject to the
stabilization crate-boundary audit; this map records current implementation and
does not assert that every split requires independent versioning.

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
