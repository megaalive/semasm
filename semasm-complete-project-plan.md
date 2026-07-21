# SemASM: Complete Project Plan and Agent-Executable Technical Specification

> **Working title:** SemASM  
> **Core intermediate representation:** ASIR — Assembly Semantic Intermediate Representation  
> **Document status:** Initial execution baseline  
> **Prepared:** 2026-07-17  
> **Implementation language:** Rust  
> **Repository language:** English for source code, comments, documentation, issues, and contribution material

---

## 1. Executive Summary

SemASM is a multi-architecture semantic infrastructure for software written directly in assembly language.

The long-term objective is not merely to explain or lint existing assembly. The primary objective is to make it practical for an AI coding agent to produce complete programs directly in target assembly without requiring a high-level implementation language or shipping a high-level language runtime.

The project introduces a semantic layer that preserves information normally missing from assembly source:

- program intent;
- function contracts;
- parameter and return-value meaning;
- semantic types;
- ABI bindings;
- memory effects;
- preconditions and postconditions;
- control-flow invariants;
- register ownership, liveness, and clobber rules;
- target capabilities and environmental assumptions;
- measurable size, memory, startup, and performance constraints.

The semantic layer does **not** replace the assembler and must not silently become another general-purpose high-level language. Assembly remains the delivered implementation. SemASM provides the contracts, target knowledge, analysis, agent context, validation, and test pipeline around it.

The central engineering distinction is:

```text
Rich build-time tooling is acceptable.
Rich shipped runtime is not.
```

Rust, Capstone, LLVM tools, QEMU, debuggers, and optional analyzers may be used in the development factory. They are not automatically linked into the generated program. A generated executable or firmware image should contain only the instructions, data, startup code, platform interfaces, and optional runtime fragments explicitly selected for that target.

The first production-quality scope should support:

1. x86-64 Linux, System V ABI, ELF;
2. x86-64 Windows, Windows x64 ABI, PE/COFF;
3. AArch64 Linux, AAPCS64, ELF;
4. RISC-V 64 Linux, RISC-V psABI, ELF;
5. RISC-V 32 bare-metal on a QEMU-supported reference machine as the first IoT/freestanding profile.

The project must be built through small vertical slices. Every slice must produce a demonstrable executable artifact, a test, diagnostics, documentation, and a clear extension point. Architecture diagrams and abstract interfaces alone do not count as progress.

---

## 2. Project Thesis

### 2.1 Problem

AI coding models generally perform better when source code exposes high semantic density through names, types, structured control flow, APIs, module boundaries, and explicit contracts. Raw assembly exposes exact machine operations but often hides intent.

As a result, an agent that writes assembly directly must simultaneously:

- infer or remember the program's intended behavior;
- obey an ISA;
- obey a platform ABI;
- manage registers and stack state;
- preserve calling-convention rules;
- handle object format and relocation rules;
- select platform services;
- reason about memory safety;
- maintain control-flow correctness;
- satisfy size and performance constraints.

This creates a large semantic gap between a human request and valid target code.

### 2.2 Proposed solution

SemASM closes that gap with three cooperating layers:

1. **Portable semantic contracts** describe what code must do without becoming an implementation language.
2. **Target kits** describe how a target architecture, ABI, operating environment, assembler, object format, and execution profile work.
3. **Verification pipelines** prove as much as practical about the generated assembly and empirically test the remainder.

### 2.3 Core proposition

```text
Human intent
    + portable semantic contract
    + target kit
    + agent-generated assembly
    + deterministic verification
    = minimal target-specific program
```

### 2.4 Honest limitation

Direct assembly does not guarantee smaller or faster software. Modern optimizing compilers can outperform hand-written or agent-written assembly. SemASM must therefore reject ideology-driven claims and measure actual results.

The project succeeds only when it can demonstrate improvements relevant to a declared purpose, such as:

- smaller deployed binary or firmware;
- no language runtime;
- lower peak memory;
- predictable startup;
- deterministic control flow;
- precise hardware use;
- reduced dependency surface;
- target-specific behavior unavailable through a generic runtime.

---

## 3. Product Boundary

### 3.1 SemASM is

- a semantic contract system for assembly programs;
- a multi-ISA and multi-ABI target model;
- an Assembly Semantic IR for analysis;
- a static checker for selected instruction semantics;
- an object-file inspection and conformance tool;
- an agent context generator;
- an agent patch validation pipeline;
- a cross-target behavioral conformance harness;
- a size, memory, startup, and performance measurement framework;
- a contributor-friendly SDK for adding architectures, ABIs, and execution profiles.

### 3.2 SemASM is not

- a new assembler;
- a replacement for NASM, GAS, LLVM MC, linkers, or debuggers;
- LLVM IR with different syntax;
- a general-purpose high-level language;
- a transpiler that hides the final assembly implementation;
- a claim that all software should be written in assembly;
- an automatic guarantee of safety or optimal performance;
- a bundled AI model provider;
- a mandatory runtime linked into generated programs;
- a reverse-engineering framework first, although object analysis is required.

### 3.3 Explicit anti-goal

SemASM must not evolve into a language where users write loops, classes, collections, exceptions, async tasks, or business logic in a new syntax and then call the output "assembly." The portable layer specifies contracts and constraints; the implementation remains target assembly.

---

## 4. Design Principles

### P1. Shipped artifacts remain minimal

The analyzer may be large. The generated target program must not inherit analyzer dependencies.

### P2. No hidden runtime

Every startup routine, allocator, syscall wrapper, panic path, formatter, or device driver fragment included in a target must be visible in the build manifest and size report.

### P3. ISA, ABI, platform, object format, and dialect are separate concepts

Do not encode `x86-64` as if it automatically means Linux, ELF, NASM, or System V.

### P4. Assembly source remains inspectable

Generated assembly is a first-class artifact committed, diffed, reviewed, assembled, and debugged normally.

### P5. Agent output is untrusted until verified

A plausible patch is not a valid patch. Validation is mandatory.

### P6. Vertical slices before framework expansion

A new abstraction is accepted only after at least one executable use case requires it.

### P7. Core remains architecture-neutral

Architecture-specific instruction behavior belongs in an architecture backend.

### P8. Heavy integrations remain optional

Capstone, LLVM, QEMU, Unicorn, SMT solvers, and vendor SDKs must be isolated behind features or external adapters.

### P9. Diagnostics must teach

An error should report the violated semantic or ABI rule, not merely say that validation failed.

### P10. Evidence over claims

Binary size, section size, relocation count, startup time, memory use, and performance must be measured and reproducible.

### P11. English repository language

Source identifiers, comments, diagnostics, documentation, issue templates, contribution guides, and examples use clear technical English.

### P12. Humble public communication

The README should explain the problem, current capability, limitations, and a working demo without exaggerated claims about replacing compilers.

---

## 5. Primary Users and Use Cases

### 5.1 Assembly application author

Wants an agent to implement a small native utility directly in assembly with explicit ABI, memory, and artifact constraints.

### 5.2 Embedded and IoT developer

Wants a freestanding firmware component with no language runtime, fixed memory regions, explicit MMIO access, and a reproducible image.

### 5.3 Low-level library maintainer

Wants equivalent implementations of a routine for x86-64, AArch64, and RISC-V and needs behavioral conformance tests.

### 5.4 Security reviewer

Wants to inspect stack discipline, register preservation, memory effects, indirect branches, syscalls, and unexplained runtime fragments.

### 5.5 Architecture contributor

Wants to add a target backend using a documented interface and a conformance test suite without understanding the entire repository.

### 5.6 AI agent operator

Wants deterministic task packets, validation commands, failure diagnostics, and bounded edit scope so an agent can execute tasks safely.

---

## 6. Terminology

| Term | Meaning |
|---|---|
| **Semantic contract** | Portable declaration of behavior, inputs, outputs, effects, invariants, and constraints. |
| **ASIR** | Architecture-neutral analysis representation derived from assembly and metadata. |
| **Target kit** | A complete description of ISA, ABI, platform, object format, assembler/linker, execution profile, and validation tools. |
| **Execution profile** | Hosted, freestanding, bare-metal, kernel, boot, or another environment defining available services. |
| **Target triple** | Architecture-vendor/system-environment identity, extended by SemASM where necessary. |
| **Dialect** | Assembly source syntax such as NASM Intel, GAS Intel, GAS AT&T, or LLVM integrated assembler syntax. |
| **Physical instruction** | Parsed or decoded target instruction. |
| **Semantic operation** | Normalized ASIR operation such as load, store, compare, branch, call, or return. |
| **Runtime fragment** | Explicit reusable assembly component selected into a program, such as `_start`, syscall wrapper, or UART driver. |
| **Task packet** | Bounded machine-readable package given to an AI agent. |
| **Conformance fixture** | Small source/object/executable case with known expected semantic behavior. |

---

## 7. Target Identity Model

A target must never be represented by architecture alone.

```text
TargetIdentity =
    ISA
  + ISA extensions
  + endianness
  + word size
  + ABI
  + platform
  + object format
  + assembly dialect
  + assembler
  + linker
  + execution profile
  + hardware or machine model
```

Example identities:

```text
x86_64-unknown-linux-gnu
  ISA: x86-64 baseline
  ABI: System V AMD64
  Object: ELF64
  Dialect: NASM Intel
  Assembler: NASM
  Linker: LLD or GNU ld
  Profile: hosted-minimal

x86_64-pc-windows-msvc
  ISA: x86-64 baseline
  ABI: Windows x64
  Object: COFF / PE32+
  Dialect: NASM Intel
  Assembler: NASM
  Linker: lld-link or link.exe
  Profile: hosted-minimal

aarch64-unknown-linux-gnu
  ISA: AArch64
  ABI: AAPCS64
  Object: ELF64
  Dialect: GNU/LLVM unified syntax
  Assembler: llvm-mc or clang integrated assembler
  Linker: LLD
  Profile: hosted-minimal

riscv64gc-unknown-linux-gnu
  ISA: RV64GC
  ABI: LP64D or selected RISC-V psABI
  Object: ELF64
  Dialect: GNU/LLVM RISC-V syntax
  Assembler: llvm-mc or GNU assembler
  Linker: LLD
  Profile: hosted-minimal

riscv32imac-unknown-none-elf-qemu-virt
  ISA: RV32IMAC
  ABI: ILP32
  Object: ELF32 plus raw image as needed
  Dialect: GNU/LLVM RISC-V syntax
  Assembler: llvm-mc or GNU assembler
  Linker: LLD
  Profile: bare-metal
  Machine: QEMU virt
```

---

## 8. High-Level Architecture

SemASM is divided into three planes.

### 8.1 Authoring plane

Responsible for human intent, semantic contracts, target selection, agent task preparation, and generated source management.

```text
Request
  -> project manifest
  -> semantic contract
  -> target kit
  -> agent task packet
  -> assembly source patch
```

### 8.2 Verification plane

Responsible for syntax, encoding, ABI, semantics, object format, behavior, and measurable constraints.

```text
assembly source
  -> assembler
  -> object file
  -> object inspector
  -> disassembler
  -> ASIR lowering
  -> static checks
  -> linker
  -> sandbox/emulator
  -> behavioral tests
  -> size/performance report
```

### 8.3 Delivery plane

Contains only the selected program source, object files, linked image, explicit runtime fragments, symbols or debug data selected by policy, and a manifest of what was shipped.

```text
program.asm
startup.asm
selected runtime fragments
linker script or response file
final executable / firmware image
artifact report
```

No SemASM Rust crate is linked into the output by default.

---

## 9. End-to-End User Workflow

### 9.1 Initialize

```bash
semasm new tiny-echo --target x86_64-unknown-linux-gnu
```

Creates:

```text
tiny-echo/
├── semasm.toml
├── contracts/
│   └── main.sem.toml
├── src/
│   └── x86_64-linux/
│       └── main.asm
├── tests/
│   ├── behavior/
│   └── fixtures/
└── reports/
```

### 9.2 Describe behavior

The author or planning agent creates a semantic contract that describes inputs, outputs, effects, and constraints, not an implementation algorithm disguised as a language.

### 9.3 Prepare an agent task

```bash
semasm task create implement-main \
  --contract contracts/main.sem.toml \
  --target x86_64-unknown-linux-gnu \
  --output .semasm/tasks/implement-main/
```

### 9.4 Agent writes or patches assembly

The agent receives:

- contract;
- relevant target rules;
- allowed files;
- instruction and platform references;
- existing symbols;
- validation commands;
- explicit forbidden actions;
- expected tests.

### 9.5 Validate

```bash
semasm verify --target x86_64-unknown-linux-gnu
```

### 9.6 Inspect evidence

```bash
semasm report --format markdown
```

The report includes:

- source files included;
- runtime fragments included;
- sections and sizes;
- imports or syscalls;
- ABI validation;
- semantic warnings;
- test results;
- execution environment;
- reproducibility information.

---

## 10. Repository Structure

Use a Cargo workspace, but do not begin with too many crates. Start with a small coherent workspace and split only when boundaries become stable.

Recommended mature layout:

```text
semasm/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── LICENSE-APACHE
├── LICENSE-MIT
├── README.md
├── CONTRIBUTING.md
├── CODE_OF_CONDUCT.md
├── SECURITY.md
├── GOVERNANCE.md
├── ROADMAP.md
├── CHANGELOG.md
├── ARCHITECTURE.md
├── deny.toml
├── rustfmt.toml
├── clippy.toml
├── book.toml
│
├── crates/
│   ├── semasm-core/
│   ├── semasm-contract/
│   ├── semasm-asir/
│   ├── semasm-target/
│   ├── semasm-analysis/
│   ├── semasm-object/
│   ├── semasm-agent-protocol/
│   ├── semasm-runner/
│   ├── semasm-report/
│   ├── semasm-cli/
│   │
│   ├── semasm-arch-x86/
│   ├── semasm-arch-aarch64/
│   ├── semasm-arch-riscv/
│   │
│   ├── semasm-abi-win64/
│   ├── semasm-abi-sysv-amd64/
│   ├── semasm-abi-aapcs64/
│   ├── semasm-abi-riscv/
│   │
│   ├── semasm-format-elf/
│   ├── semasm-format-coff/
│   │
│   ├── semasm-adapter-capstone/
│   ├── semasm-adapter-llvm-cli/
│   ├── semasm-adapter-qemu/
│   └── semasm-adapter-unicorn/
│
├── targets/
│   ├── x86_64-linux-sysv/
│   ├── x86_64-windows-win64/
│   ├── aarch64-linux-aapcs64/
│   ├── riscv64-linux-lp64/
│   └── riscv32-qemu-virt-baremetal/
│
├── runtime-fragments/
│   ├── linux/
│   ├── windows/
│   ├── baremetal/
│   └── devices/
│
├── examples/
│   ├── exit-code/
│   ├── echo/
│   ├── byte-counter/
│   ├── checksum/
│   └── uart-hello-riscv32/
│
├── fixtures/
│   ├── contracts/
│   ├── objects/
│   ├── diagnostics/
│   └── target-conformance/
│
├── docs/
│   └── src/
│       ├── SUMMARY.md
│       ├── introduction.md
│       ├── quickstart.md
│       ├── concepts/
│       ├── targets/
│       ├── contributing/
│       └── internals/
│
├── rfcs/
├── adr/
├── scripts/
├── xtask/
└── .github/
    ├── workflows/
    ├── ISSUE_TEMPLATE/
    ├── pull_request_template.md
    └── dependabot.yml
```

### 10.1 Initial workspace constraint

> **Implementation status (July 2026):** This section records the bootstrap
> constraint, not the current tree. The workspace now has thirteen crates after
> VS-02 through VS-08 implementation work. Their current capability evidence is
> defined in `capabilities.toml`; their boundary value remains subject to the
> stabilization crate-boundary audit.

At repository bootstrap, implement only:

```text
semasm-core
semasm-contract
semasm-asir
semasm-target
semasm-cli
```

Create additional crates only when a completed vertical slice demonstrates a stable boundary.

This avoids producing an impressive directory tree with no working program.

---

## 11. Crate Responsibilities

### `semasm-core`

Shared IDs, source spans, diagnostics, deterministic collections, error model, path policy, and common utilities.

Must not depend on architecture backends, Capstone, LLVM, QEMU, an async runtime, or an AI provider.

### `semasm-contract`

Parses and validates portable semantic contracts and inline annotation metadata.

### `semasm-asir`

Defines ASIR types, operations, basic blocks, values, memory regions, control-flow representation, and serialization format.

### `semasm-target`

Defines target identity, target-kit interfaces, capability lookup, ABI bindings, tool discovery, and target manifests.

### `semasm-analysis`

Implements data-flow, liveness, stack-state tracking, type propagation, memory-effect checking, signedness checking, and contract conformance.

### `semasm-object`

Reads symbols, sections, relocations, entry point, imports, exports, and executable metadata through the Rust `object` crate or format-specific adapters.

### `semasm-agent-protocol`

Creates deterministic task packets and validates agent responses. It contains no model-specific SDK in the core protocol.

### `semasm-runner`

Defines execution sandbox interfaces and test result normalization.

### `semasm-report`

Produces JSON, Markdown, SARIF where appropriate, and concise terminal diagnostics.

### Architecture crates

Translate physical instructions into ASIR operations and expose register aliasing, flag behavior, addressing modes, and target-specific validation.

### ABI crates

Define argument binding, return locations, volatile and nonvolatile registers, stack rules, unwind expectations, aggregate passing, and call-site requirements.

### Adapter crates

Optional integrations only. Their dependencies must not leak into core crates.

---

## 12. Dependency Policy

### 12.1 Core dependency rules

- Prefer the Rust standard library for foundational data structures.
- No async runtime in the core or CLI unless a demonstrated product requirement exists.
- No embedded database in the initial product.
- No dynamic Rust plugin ABI.
- No direct LLVM library dependency in the initial product.
- No AI provider SDK in core crates.
- Every dependency must have a written reason in `DEPENDENCIES.md`.
- Disable default features when they pull unnecessary capabilities.
- Pin a stable Rust toolchain in `rust-toolchain.toml`.
- Commit `Cargo.lock` for reproducible CLI builds.

### 12.2 Recommended initial dependencies

Use exact selections only during repository bootstrap after checking current maintenance and licenses.

Likely categories:

- serialization: `serde`, `serde_json`, TOML parser;
- errors: a small explicit error enum, optionally `thiserror` outside the smallest core;
- CLI: `clap` in `semasm-cli` only, or a smaller argument parser if measured binary size matters for the tool itself;
- object files: `object`;
- temporary test directories: a maintained test-only crate;
- snapshot tests: optional and restricted to diagnostics;
- Capstone binding: optional feature or adapter crate.

### 12.3 Heavy integration policy

| Integration | Mode | Reason |
|---|---|---|
| Capstone | Optional Rust/C adapter | Multi-architecture decoding and register detail. |
| `llvm-mc` / `llvm-objdump` | External process first | Avoid embedding LLVM into the core. |
| LLVM C++ libraries | Future thin C ABI bridge | Only if CLI tools are insufficient. |
| QEMU | External runner | System and user-mode emulation across targets. |
| Unicorn | Optional external or adapter | Fine-grained emulation; GPL implications must remain isolated and reviewed. |
| Z3 or another SMT solver | Future optional adapter | Advanced proofs only after concrete checks exist. |

---

## 13. Semantic Contract Format

### 13.1 Format decision

Use TOML for project and contract manifests in the first version.

Reasons:

- familiar in Rust projects;
- readable by humans and agents;
- deterministic enough for configuration;
- easier to validate than free-form comments;
- supports comments;
- avoids introducing a custom parser before the semantic model is stable.

Inline assembly annotations remain supported for local facts, but portable behavior belongs in sidecar contract files.

### 13.2 Example project manifest

```toml
[project]
name = "tiny-echo"
contract_version = "0.1"

[build]
default_target = "x86_64-unknown-linux-gnu"
deterministic = true

[policy]
allow_network_during_build = false
allow_host_execution = false
require_object_inspection = true
require_behavior_tests = true
forbid_hidden_runtime = true

[budgets]
max_text_bytes = 4096
max_rodata_bytes = 1024
max_writable_data_bytes = 512
max_runtime_fragments = 3

[[targets]]
id = "x86_64-unknown-linux-gnu"
source_dir = "src/x86_64-linux"
contract = "contracts/main.sem.toml"
```

### 13.3 Example function contract

```toml
[function]
name = "write_all"
summary = "Write all requested bytes or return an explicit failure status."
visibility = "internal"

[[function.parameters]]
name = "buffer"
type = "ptr<const u8>"
role = "input"

[[function.parameters]]
name = "length"
type = "usize"
role = "input"

[[function.returns]]
name = "written"
type = "usize"

[[function.returns]]
name = "status"
type = "status"

[[function.requires]]
expression = "buffer.valid_for_read(length)"
reason = "The function reads exactly length bytes from buffer."

[[function.ensures]]
expression = "status.ok implies written == length"

[[function.effects]]
kind = "memory_read"
region = "buffer[0..length]"

[[function.effects]]
kind = "platform_io"
resource = "stdout"

[function.constraints]
no_heap = true
no_recursion = true
bounded_stack_bytes = 128
```

### 13.4 Contract rules

A contract may express:

- semantic parameters and returns;
- target-independent types;
- permitted memory regions;
- memory read/write effects;
- platform I/O effects;
- allowed syscalls or imports;
- preconditions and postconditions;
- no-heap, no-recursion, or bounded-stack constraints;
- deterministic behavior requirements;
- error model;
- size and dependency budgets;
- target-specific overrides.

A contract must not express a full imperative implementation.

### 13.5 Expression language scope

The initial expression language must be deliberately small:

- identifiers;
- integer and boolean literals;
- equality and ordering;
- boolean operators;
- arithmetic required for ranges;
- range expressions;
- `valid_for_read`, `valid_for_write`, `aligned`, and similar approved predicates;
- implication;
- named result states.

No loops, function definitions, allocation, arbitrary code execution, or embedded scripting.

---

## 14. Inline Assembly Annotations

Inline annotations describe local facts that are difficult to derive and belong near the implementation.

Example:

```asm
;@sem.function write_all
;@sem.bind buffer = abi.arg0
;@sem.bind length = abi.arg1
;@sem.bind written = abi.return0
;@sem.clobber abi.volatile
;@sem.stack.max_local 64

write_all:
    ;@sem.state remaining = length
    ;@sem.invariant 0 <= written <= length
.loop:
    ; implementation
```

Rules:

- use the comment syntax of the source dialect;
- annotations begin with `@sem.`;
- do not restate obvious instruction behavior;
- portable meaning belongs in contract files;
- target binding and local invariants may remain inline;
- stale or contradictory annotations are errors, not documentation trivia;
- every annotation must have a source span for diagnostics.

---

## 15. ASIR v0 Design

### 15.1 Purpose

ASIR is an analysis representation, not a user-facing implementation language and not initially a code-generation IR.

Its first responsibilities are:

- normalize instructions across architectures;
- represent control flow;
- propagate semantic roles and types;
- track register and memory effects;
- check ABI and contract constraints;
- generate precise diagnostics and agent context.

### 15.2 Core entities

```rust
pub struct Module {
    pub target: TargetId,
    pub functions: Vec<Function>,
    pub symbols: Vec<Symbol>,
    pub memory_regions: Vec<MemoryRegion>,
}

pub struct Function {
    pub id: FunctionId,
    pub name: SymbolId,
    pub contract: Option<ContractId>,
    pub blocks: Vec<BlockId>,
    pub entry: BlockId,
    pub frame: FrameModel,
}

pub struct BasicBlock {
    pub id: BlockId,
    pub operations: Vec<Operation>,
    pub terminator: Terminator,
}
```

### 15.3 Initial operation set

```text
Copy
Load
Store
Address
Add
Subtract
Multiply
UnsignedDivide
SignedDivide
Remainder
BitAnd
BitOr
BitXor
ShiftLeft
LogicalShiftRight
ArithmeticShiftRight
Compare
Extend
Truncate
Select
Call
SystemCall
Barrier
Fence
Intrinsic
Unknown
```

Control flow:

```text
Jump
ConditionalJump
IndirectJump
Return
Trap
Unreachable
```

### 15.4 Explicit widths and signedness

Every numeric operation must state width. Signedness is attached where it changes semantics.

```text
compare.u64 greater_than
compare.i32 less_than
extend.zero u8 -> u64
extend.sign i32 -> i64
```

### 15.5 Register alias model

A backend must describe partial-register relationships.

Examples:

```text
x86:
  RAX contains EAX, AX, AL, AH
  write EAX clears RAX[63:32]

AArch64:
  X0 contains W0
  write W0 clears X0[63:32]

RISC-V RV64:
  ADDW computes 32-bit result then sign-extends to XLEN
```

### 15.6 Unknown semantics

Unsupported instructions must lower to an explicit `Unknown` operation containing:

- original mnemonic;
- operands;
- known reads and writes;
- known memory effects;
- reason semantic lowering is unavailable.

The analyzer must never silently pretend full understanding.

### 15.7 Serialization

ASIR JSON is intended for debugging, fixtures, and agent context. The internal Rust API is authoritative.

Every serialized ASIR document includes:

- schema version;
- SemASM version;
- target identity;
- source hash or object hash;
- backend version;
- unresolved semantics count.

---

## 16. Architecture Backend Contract

A backend must provide:

```rust
pub trait ArchitectureBackend {
    fn architecture(&self) -> ArchitectureId;
    fn parse_register(&self, name: &str) -> Option<RegisterId>;
    fn register_info(&self, register: RegisterId) -> &RegisterInfo;
    fn lower_instruction(
        &self,
        instruction: &PhysicalInstruction,
        context: &LoweringContext,
    ) -> LoweringResult;
    fn branch_info(&self, instruction: &PhysicalInstruction) -> BranchInfo;
    fn memory_accesses(&self, instruction: &PhysicalInstruction) -> Vec<MemoryAccess>;
    fn explicit_reads(&self, instruction: &PhysicalInstruction) -> RegisterSet;
    fn explicit_writes(&self, instruction: &PhysicalInstruction) -> RegisterSet;
    fn implicit_reads(&self, instruction: &PhysicalInstruction) -> RegisterSet;
    fn implicit_writes(&self, instruction: &PhysicalInstruction) -> RegisterSet;
}
```

### 16.1 Initial backend subset

Do not implement entire ISAs first.

Initial common subset:

- register-to-register move;
- immediate materialization;
- load and store;
- integer arithmetic;
- integer comparison;
- direct and conditional branches;
- direct calls and returns;
- stack pointer adjustment;
- selected system-call instruction;
- architecture-specific no-op and trap.

SIMD, atomics, privilege instructions, floating point, vector extensions, and complex string instructions are separate milestones.

### 16.2 Backend conformance requirements

Each backend contribution must include:

- register table tests;
- aliasing tests;
- instruction decode fixtures;
- ASIR lowering snapshots;
- control-flow tests;
- known unsupported instruction tests;
- at least one executable example;
- target documentation;
- official specification references.

---

## 17. ABI Backend Contract

An ABI backend defines:

- integer and floating argument locations;
- return locations;
- volatile and nonvolatile registers;
- stack alignment;
- call-site stack requirements;
- red zone or shadow space;
- aggregate passing rules;
- unwind metadata expectations;
- variadic call rules;
- platform-specific register use;
- entry-point contract.

Example interface:

```rust
pub trait AbiBackend {
    fn abi(&self) -> AbiId;
    fn bind_parameter(&self, index: usize, ty: &SemanticType) -> AbiLocation;
    fn bind_return(&self, index: usize, ty: &SemanticType) -> AbiLocation;
    fn volatile_registers(&self) -> RegisterSet;
    fn preserved_registers(&self) -> RegisterSet;
    fn required_stack_alignment(&self) -> u32;
    fn call_site_rules(&self) -> &[AbiRule];
    fn validate_function(&self, function: &FunctionAnalysis) -> Vec<Diagnostic>;
}
```

ABI rules must cite their source specification in backend documentation and tests.

---

## 18. Target Kit

A target kit is a directory plus a Rust registration entry.

```text
targets/x86_64-linux-sysv/
├── target.toml
├── toolchain.toml
├── platform.toml
├── allowed-services.toml
├── linker/
│   └── minimal.ld
├── runtime/
│   └── start.asm
├── tests/
└── README.md
```

Example:

```toml
[target]
id = "x86_64-unknown-linux-gnu"
architecture = "x86_64"
abi = "sysv-amd64"
platform = "linux"
object_format = "elf64"
dialect = "nasm-intel"
profile = "hosted-minimal"

[tools.assembler]
program = "nasm"
args = ["-f", "elf64"]

[tools.linker]
program = "ld.lld"

[tools.disassembler]
program = "llvm-objdump"

[tools.runner]
kind = "native-or-qemu-user"

[policy]
default_allow_host_execution = false
```

### 18.1 Target kit quality levels

- **Experimental:** builds one fixture; incomplete semantics.
- **Preview:** multiple examples, ABI checks, CI, documented limitations.
- **Supported:** stable target manifest, conformance suite, release artifacts, maintained documentation.

No target is called supported merely because the assembler accepts its code.

---

## 19. Agent Integration Design

### 19.1 Model-provider neutrality

The core project must not depend on a specific model or vendor.

Initial integration works through files and commands:

```text
semasm creates task packet
external agent edits allowed files
semasm validates result
```

A future adapter may invoke an OpenAI-compatible, Anthropic-compatible, local, or custom provider, but this remains outside the semantic core.

### 19.2 Task packet contents

```text
.semasm/tasks/TASK-001/
├── task.json
├── instructions.md
├── contract.sem.toml
├── target-context.json
├── relevant-source.txt
├── allowed-files.json
├── forbidden-actions.json
├── validation.json
├── expected-diagnostics.json
└── evidence/
```

### 19.3 Required task fields

```json
{
  "task_id": "TASK-001",
  "objective": "Implement the write_all routine for x86-64 Linux.",
  "target": "x86_64-unknown-linux-gnu",
  "allowed_files": [
    "src/x86_64-linux/write_all.asm",
    "tests/behavior/write_all.toml"
  ],
  "forbidden": [
    "modify contracts",
    "add external dependencies",
    "execute generated code on the host",
    "disable validation"
  ],
  "required_commands": [
    "cargo test -p semasm-contract",
    "semasm verify --target x86_64-unknown-linux-gnu"
  ]
}
```

### 19.4 Agent response contract

The agent must return:

- summary of changes;
- files changed;
- assumptions;
- unresolved issues;
- exact validation commands run;
- test and analyzer results;
- size change;
- no claims not supported by command output.

### 19.5 Repair loop

```text
Generate patch
  -> parse and assemble
  -> inspect object
  -> static analysis
  -> link
  -> execute in sandbox/emulator
  -> compare behavior
  -> report failure packet
  -> agent repairs only allowed files
```

The repair loop has a configured maximum iteration count and preserves all diagnostics for review.

### 19.6 Agent context minimization

Do not give the agent the entire repository by default.

Context selection should include:

- contract;
- target rules;
- called and calling symbols;
- relevant struct layouts;
- current function source;
- nearby tests;
- exact diagnostic history.

This improves precision and reduces accidental edits.

---

## 20. Verification Ladder

Verification occurs in ordered levels. A later success does not erase an earlier failure.

### Level 0: Manifest and contract validation

- schema version recognized;
- target exists;
- contract references valid names;
- effect and constraint expressions parse;
- no contradictory target settings.

### Level 1: Source validation

- source files exist;
- annotation syntax valid;
- declared symbols exist;
- no forbidden includes or macros under policy;
- no source outside allowed roots.

### Level 2: Assembly

- selected assembler succeeds;
- warnings are captured;
- command line is recorded;
- output hash is recorded.

### Level 3: Object inspection

- correct architecture and format;
- expected symbols and sections;
- no unexpected imports;
- relocations within policy;
- entry point or exported symbols present;
- section permissions sane;
- runtime fragments accounted for.

### Level 4: Disassembly and ASIR lowering

- instructions decoded;
- control-flow graph built;
- unsupported instructions counted;
- register reads/writes known where possible;
- semantic operations produced.

### Level 5: ABI validation

- preserved registers restored;
- stack aligned at calls;
- shadow space or red zone handled correctly;
- arguments and returns follow ABI;
- stack delta balanced on returns;
- indirect calls and returns match policy.

### Level 6: Contract and static semantic validation

- memory effects fit declared regions;
- signedness and width consistent with semantic types;
- forbidden heap or recursion absent where detectable;
- stack budget respected;
- declared outputs assigned on all normal exits;
- allowed services only.

### Level 7: Link validation

- linker succeeds;
- map file generated;
- final sections inspected;
- symbol resolution complete;
- no hidden runtime libraries.

### Level 8: Sandboxed execution

- host execution disabled by default;
- use QEMU, container, VM, restricted process, or target-specific sandbox;
- timeout and output limits enforced;
- exit status, stdout, stderr, signals, and faults captured.

### Level 9: Behavioral conformance

- deterministic fixtures;
- boundary inputs;
- error cases;
- cross-target output equality where applicable;
- property-based tests for pure routines where practical.

### Level 10: Non-functional budgets

- `.text`, read-only data, writable data, BSS;
- total file size;
- imported symbol count;
- runtime fragment count;
- peak memory where measurable;
- startup time where meaningful;
- instruction count or cycles under a declared measurement method.

---

## 21. CLI Design

The CLI should be predictable, scriptable, and useful to humans and agents.

### Core commands

```text
semasm new
semasm init
semasm target list
semasm target doctor
semasm contract check
semasm assemble
semasm inspect
semasm lower
semasm analyze
semasm link
semasm run
semasm test
semasm verify
semasm report
semasm task create
semasm task validate
semasm context
```

### Command behavior rules

- default output is concise terminal text;
- `--json` is available for agents;
- nonzero exit codes are stable and documented;
- diagnostics contain codes such as `ABI001` or `MEM004`;
- commands do not access the network unless explicitly requested;
- commands support `--explain <diagnostic-code>`;
- tool invocations are shown with `--verbose`;
- no spinner or interactive prompt in CI mode;
- paths in JSON are normalized and deterministic.

Example diagnostic:

```text
error[ABI003]: stack is not 16-byte aligned at call site
  --> src/x86_64-windows/main.asm:42:5
   |
42 |     call WriteFile
   |     ^^^^^^^^^^^^^^ RSP mod 16 is 8 at this instruction
   |
   = target: x86_64-pc-windows-msvc
   = rule: Windows x64 call-site stack alignment
   = state derived from: entry RSP, push rbx, sub rsp, 32
   = help: reserve an additional 8 bytes or change the frame layout
```

---

## 22. Initial Target Matrix

| Order | Target | Purpose | Initial dialect/toolchain | Execution |
|---:|---|---|---|---|
| 1 | x86-64 Linux SysV ELF | Simplest hosted minimal end-to-end path | NASM + LLD | QEMU user or restricted native Linux |
| 2 | x86-64 Windows Win64 PE | User-relevant desktop target and ABI contrast | NASM + lld-link | Windows sandbox/VM |
| 3 | AArch64 Linux AAPCS64 ELF | First non-x86 architecture | LLVM integrated assembler + LLD | QEMU user/system |
| 4 | RISC-V 64 Linux ELF | Open ISA and IoT-adjacent target | LLVM/GNU tools + LLD | QEMU user/system |
| 5 | RISC-V 32 bare-metal QEMU virt | First true freestanding/IoT vertical slice | LLVM/GNU tools + linker script | QEMU system |

### Deferred targets

- x86 32-bit;
- ARMv7/AArch32;
- Windows ARM64;
- UEFI;
- WebAssembly;
- AVR;
- ESP32 variants;
- RP2040/ARM Cortex-M;
- Linux kernel modules;
- eBPF;
- GPU ISAs;
- SIMD and vector extensions.

They remain deferred until the target-kit contract and conformance suite are proven by the first five targets.

---

# 23. Vertical Slice Execution Plan

Each vertical slice below has an observable artifact. Tasks are intentionally ordered. An agent must not skip acceptance criteria or replace a requested implementation with placeholders.

---

## VS-00 — Repository Bootstrap and Governance

### Objective

Create a public repository that builds cleanly, communicates a realistic purpose, and is ready for small contributions before complex functionality begins.

### Deliverables

- Cargo workspace with initial five crates;
- pinned stable Rust toolchain;
- license files;
- English README with project status and limitations;
- contribution, security, governance, and code-of-conduct files;
- CI for format, lint, test, documentation, and dependency audit;
- issue and pull-request templates;
- ADR and RFC directories;
- first mdBook skeleton;
- no architecture implementation yet.

### Tasks

#### `BOOT-001` Create repository skeleton

Actions:

1. Create workspace root.
2. Add `semasm-core`, `semasm-contract`, `semasm-asir`, `semasm-target`, and `semasm-cli`.
3. Configure workspace package metadata and shared lints.
4. Add `rust-toolchain.toml` with stable toolchain, `rustfmt`, and `clippy` components.
5. Add a minimal `semasm --version` command.

Acceptance criteria:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
cargo run -p semasm-cli -- --version
```

All commands pass from a clean clone.

#### `BOOT-002` Establish repository policy

Create:

- `CONTRIBUTING.md`;
- `SECURITY.md`;
- `GOVERNANCE.md`;
- `CODE_OF_CONDUCT.md`;
- `DEPENDENCIES.md`;
- `ARCHITECTURE.md`;
- `ROADMAP.md`;
- `CHANGELOG.md`.

Acceptance criteria:

- contribution guide contains exact build/test commands;
- security policy explains how to report unsafe code-generation or sandbox bypass issues;
- architecture document states that generated programs do not link SemASM by default;
- roadmap labels current targets as planned, not supported.

#### `BOOT-003` Establish CI

Minimum jobs:

- Linux stable Rust;
- Windows stable Rust;
- formatting;
- Clippy with warnings denied;
- unit and documentation tests;
- dependency advisory scan;
- license and source-policy scan;
- mdBook build;
- artifact upload for CLI binaries only after release workflow exists.

Acceptance criteria:

- CI runs on pull requests;
- required checks are documented;
- no generated binary is committed;
- caches cannot hide a clean-build failure.

#### `BOOT-004` Create public-facing README

README sections:

1. one-sentence description;
2. current status warning;
3. small architecture diagram;
4. five-minute example placeholder linked to VS-02 once available;
5. supported and planned targets;
6. why semantic metadata is needed;
7. what SemASM is not;
8. contribution entry points;
9. license.

Acceptance criteria:

- no unsupported performance claims;
- no claim that SemASM replaces compilers;
- no giant feature checklist;
- first page is understandable without compiler research experience.

---

## VS-01 — Contract Parser and Deterministic Diagnostics

### Objective

Parse a useful semantic contract and reject invalid contracts with source-aware diagnostics.

### Demonstration

```bash
semasm contract check contracts/write_all.sem.toml
```

### Tasks

#### `CONTRACT-001` Define schema types

Implement Rust types for:

- contract version;
- function;
- parameters and returns;
- semantic type strings;
- requirements and guarantees;
- effects;
- constraints;
- target overrides.

Acceptance criteria:

- all public types documented;
- serialization round-trip tests;
- unknown required fields rejected or preserved according to a written compatibility policy.

#### `CONTRACT-002` Implement semantic type parser

Initial grammar:

```text
bool
status
u8/u16/u32/u64/u128
i8/i16/i32/i64/i128
usize/isize
ptr<T>
ptr<const T>
slice<T>
array<T, N>
opaque<Name>
```

Acceptance criteria:

- parser returns source spans;
- nested pointer types work;
- invalid widths fail clearly;
- no arbitrary Rust, C, or LLVM type syntax is accepted accidentally.

#### `CONTRACT-003` Implement expression subset

Implement the bounded expression grammar described earlier.

Acceptance criteria:

- precedence tests;
- malformed range diagnostics;
- unknown identifiers reported;
- expression evaluation is not yet required for machine state.

#### `CONTRACT-004` Add diagnostic codes

Initial codes:

```text
CTR001 unsupported contract version
CTR002 duplicate parameter
CTR003 unknown semantic type
CTR004 invalid expression
CTR005 unknown identifier
CTR006 contradictory memory effect
CTR007 invalid target override
```

Acceptance criteria:

- terminal and JSON representations;
- stable test fixtures;
- `semasm --explain CTR003` works.

---

## VS-02 — First Executable: x86-64 Linux Exit Code

### Objective

Build, inspect, link, and run a tiny x86-64 Linux assembly program with no C runtime.

### Demonstration

```bash
semasm verify examples/exit-code --target x86_64-unknown-linux-gnu
```

Expected result:

- an ELF executable;
- exits with declared code;
- no dynamic dependencies if the target profile requests static minimal output;
- artifact report lists `_start`, `.text`, and selected linker inputs.

### Tasks

#### `TARGET-001` Implement target identity types

Acceptance criteria:

- parse target ID;
- print canonical target ID;
- reject incompatible combinations such as Win64 ABI with ELF Linux profile;
- deterministic JSON representation.

#### `TARGET-002` Implement tool discovery

Discover:

- NASM;
- `ld.lld` with documented fallback policy;
- `llvm-objdump` or configured disassembler;
- QEMU user-mode runner where available.

Acceptance criteria:

```bash
semasm target doctor x86_64-unknown-linux-gnu
```

Reports found versions, missing tools, and exact corrective action without installing anything automatically.

#### `BUILD-001` Implement process execution wrapper

Requirements:

- explicit argument arrays, never shell-concatenated commands;
- timeout;
- stdout/stderr capture;
- working-directory control;
- environment allowlist;
- command record for reports;
- no network action.

#### `BUILD-002` Assemble and link first fixture

Source contains direct Linux syscall exit path.

Acceptance criteria:

- source assembles and links;
- final executable architecture is verified;
- test runs under a configured safe runner;
- exit code matches contract;
- build is reproducible under the same tool versions.

#### `REPORT-001` Generate artifact report

Include:

- source hash;
- assembler and linker versions;
- command lines;
- object and executable hashes;
- section sizes;
- symbols;
- dynamic dependency status;
- execution result.

---

## VS-03 — First Agent-Generated Routine

### Objective

Use a task packet to let an external coding agent implement a bounded x86-64 Linux routine, then validate it without trusting the agent's prose.

### Fixture

A pure byte-counting function:

```text
count_byte(buffer, length, needle) -> count
```

No syscalls, no heap, no recursion.

### Tasks

#### `AGENT-001` Define task packet schema

Acceptance criteria:

- JSON schema file committed;
- deterministic task packet generation;
- allowed files and commands required;
- contract and target hashes included.

#### `AGENT-002` Generate context bundle

Bundle includes:

- function contract;
- ABI parameter mapping;
- preserved and volatile registers;
- allowed instruction subset;
- existing source;
- test vectors;
- acceptance commands.

Acceptance criteria:

- no unrelated repository files included;
- context output stable under unchanged inputs;
- context can be rendered as Markdown and JSON.

#### `AGENT-003` Validate patch scope

Acceptance criteria:

- modifications outside allowed files fail;
- contract modification fails unless explicitly allowed;
- deleted tests fail validation;
- generated files cannot replace source-of-truth inputs.

#### `AGENT-004` Behavioral test harness for pure functions

Create a small target harness that invokes the routine with test vectors.

Required cases:

- empty input;
- one byte;
- no match;
- all match;
- embedded zero bytes;
- maximum configured fixture length;
- null pointer only when length is zero, according to declared policy.

Acceptance criteria:

- agent-generated routine passes all cases;
- failure output includes input and observed result;
- implementation is not accepted merely because it assembles;
- `semasm agent verify` emits `VerificationReport` for gate failure, execution
  denial, and behavioral outcomes (`docs/CLI_COMPATIBILITY.md`); harness shape
  expansion must not weaken that evidence contract.

---

## VS-04 — Object Inspection and Capstone Decoding

### Objective

Inspect generated object files and decode instructions into a normalized physical instruction stream.

### Tasks

#### `OBJECT-001` Integrate Rust object reader

Read:

- format;
- architecture;
- sections;
- symbols;
- relocations;
- entry point where available;
- imports/exports where applicable.

Acceptance criteria:

- ELF fixture coverage;
- malformed object input does not panic;
- output is deterministic JSON;
- object architecture mismatch is a hard error.

#### `DECODE-001` Add optional Capstone adapter

Requirements:

- isolated crate and feature;
- architecture selection explicit;
- detailed mode enabled where needed;
- capture explicit/implicit registers when available;
- adapter output normalized into SemASM physical instruction types.

Acceptance criteria:

- x86-64 fixture decoded;
- address, bytes, mnemonic, operands, and known register effects captured;
- unsupported Capstone detail is represented honestly;
- core builds without Capstone feature.

#### `CFG-001` Build basic control-flow graph

Initial scope:

- direct unconditional branch;
- direct conditional branch;
- fallthrough;
- direct call;
- return;
- unknown indirect transfer.

Acceptance criteria:

- graph fixture for loops and branches;
- unreachable blocks reported;
- indirect targets remain unknown rather than guessed.

---

## VS-05 — x86-64 ASIR and System V ABI Checks

### Objective

Lower the instruction subset used by existing examples into ASIR and validate stack/register behavior.

### Tasks

#### `X86-001` Register and alias model

Acceptance criteria:

- RAX/EAX/AX/AL/AH relationships tested;
- EAX zero-extension behavior tested;
- stack and instruction pointers modeled;
- register classes documented.

#### `X86-002` Lower common instruction subset

Start with actual fixture instructions only. Likely:

```text
mov
lea
xor
add
sub
inc
dec
cmp
test
jmp
je/jne
ja/jae/jb/jbe
jg/jge/jl/jle
call
ret
syscall
push
pop
```

Acceptance criteria:

- every instruction in current examples either lowers or emits explicit unsupported semantics;
- width and signedness included;
- memory operands normalized.

#### `ABI-SYSV-001` Implement System V AMD64 core rules

Initial checks:

- argument register binding;
- integer return binding;
- preserved registers;
- call-site stack alignment;
- stack balance;
- red-zone policy for leaf functions;
- syscall clobbers separately modeled from function ABI.

Acceptance criteria:

- intentionally broken fixtures produce expected codes;
- correct byte-count routine passes;
- diagnostic includes derived stack state.

#### `ANALYSIS-001` Implement forward state propagation

Track:

- register semantic role;
- width;
- known constants where cheap;
- stack pointer delta;
- basic memory-region provenance;
- comparison signedness context.

Acceptance criteria:

- converges on loops;
- conflicting incoming state becomes explicit unknown/joined state;
- no exponential path exploration.

---

## VS-06 — Windows x64 Target

### Objective

Produce a minimal Windows PE executable and demonstrate the ABI differences from System V.

### Demonstration

A console program that writes a fixed string and exits, using an explicitly selected Windows platform interface.

### Tasks

#### `WIN64-001` Add target kit and tool doctor

Support NASM plus `lld-link` first. `link.exe` may be an optional discovered alternative.

#### `WIN64-002` Implement Windows x64 ABI rules

Initial checks:

- RCX, RDX, R8, R9 argument locations;
- RAX return;
- nonvolatile registers;
- 16-byte stack alignment;
- 32-byte shadow store at call sites;
- stack balance;
- selected unwind limitations documented.

#### `COFF-001` Inspect COFF/PE output

Check:

- machine type;
- sections;
- entry point;
- imports;
- relocations;
- subsystem;
- unexpected runtime libraries.

#### `WIN64-003` Behavior and artifact report

Acceptance criteria:

- executable runs in Windows CI or a controlled Windows runner;
- output matches fixture;
- report lists every imported DLL and symbol;
- no C runtime unless explicitly selected;
- broken shadow-space fixture is detected before execution.

---

## VS-07 — AArch64 Linux Target

### Objective

Prove that ASIR and contracts are not x86-shaped.

### Tasks

#### `A64-001` Implement AArch64 register model

Include X/W aliases, stack pointer treatment, link register, zero register, and condition flags.

#### `A64-002` Lower fixture instruction subset

Likely:

```text
mov aliases
ldr/str
add/sub
cmp aliases
b
b.cond
bl
ret
svc
cbz/cbnz
```

#### `AAPCS64-001` Implement core AAPCS64 rules

- argument and return binding;
- preserved registers;
- stack alignment;
- link register handling;
- call-site state.

#### `A64-003` Port byte-count routine

Use the same portable contract and behavior tests as x86-64.

Acceptance criteria:

- contract file reused without cloning target-independent content;
- output matches x86-64 fixtures;
- QEMU runner records environment;
- unsupported instructions explicitly counted.

---

## VS-08 — RISC-V 64 Linux Target

### Objective

Add an open ISA and prove target-kit extensibility.

### Tasks

#### `RV64-001` Implement RV64 register and extension model

- integer registers and ABI names;
- XLEN;
- zero register;
- selected extension set;
- W-instruction sign-extension behavior;
- compressed-instruction metadata where decoded.

#### `RV64-002` Lower common instruction subset

Include actual fixture instructions and pseudo-instruction normalization.

#### `RVABI-001` Implement LP64 core calling convention

- argument/return registers;
- preserved registers;
- stack alignment;
- return address;
- syscall distinction.

#### `RV64-003` Port examples and conformance tests

Acceptance criteria:

- same byte-count contract;
- same behavior vectors;
- QEMU execution;
- ELF inspection;
- target report includes exact ISA extensions.

---

## VS-09 — RISC-V 32 Bare-Metal IoT Profile

### Objective

Demonstrate the real project ambition: an agent-generated freestanding program with no hosted OS runtime.

### Initial machine

Use a documented QEMU RISC-V `virt` machine configuration. Do not begin with a commercial board requiring proprietary tooling.

### Demonstration

A firmware image that:

1. starts from reset/entry code;
2. initializes a stack from a linker-defined memory region;
3. writes a message through a documented UART MMIO interface or approved semihosting path;
4. exits or signals completion through a documented QEMU mechanism;
5. stays within declared text, data, and stack budgets.

### Tasks

#### `BARE-001` Add freestanding execution profile

Model:

- no OS;
- no process ABI assumptions beyond selected platform calling convention;
- physical memory map;
- MMIO regions;
- entry state;
- interrupt state assumptions;
- stack region;
- image format.

#### `BARE-002` Linker script ownership

Create explicit linker script with symbols for:

- text start/end;
- read-only data;
- data;
- BSS;
- stack bottom/top;
- image end.

Acceptance criteria:

- map file inspected;
- overlaps rejected;
- stack and MMIO regions do not overlap image sections;
- section sizes reported.

#### `BARE-003` Startup runtime fragment

Implement a visible assembly startup fragment:

- set stack pointer;
- optionally clear BSS;
- call semantic `main` function;
- signal completion.

Acceptance criteria:

- fragment appears in artifact manifest;
- exact byte contribution reported;
- no hidden runtime archive.

#### `BARE-004` MMIO semantic effects

Add contract support for named MMIO regions:

```toml
[[memory.regions]]
name = "uart"
kind = "mmio"
base = "target.uart.base"
size = 256
permissions = ["read", "write"]
```

Static analysis must distinguish normal RAM from MMIO effects.

#### `BARE-005` QEMU system runner

Requirements:

- fixed machine and CPU arguments;
- no network device unless explicitly required;
- output capture;
- execution timeout;
- deterministic completion signal;
- command and QEMU version recorded.

Acceptance criteria:

- firmware test passes in CI where QEMU is available;
- host is not modified;
- output exact match;
- image and memory budgets pass.

---

## VS-10 — Cross-Target Semantic Conformance

### Objective

Verify that multiple assembly implementations satisfy one portable contract.

### Tasks

#### `CONF-001` Define shared behavior suite format

Example:

```toml
[suite]
function = "count_byte"

[[case]]
name = "empty"
input.buffer = ""
input.needle = 0
expect.count = 0

[[case]]
name = "binary-data"
input.buffer_hex = "00 ff 00 7f"
input.needle = 0
expect.count = 2
```

#### `CONF-002` Run suite across targets

Output matrix:

```text
                 x86_64-linux  x86_64-win  aarch64-linux  riscv64-linux
empty                 PASS          PASS          PASS          PASS
binary-data           PASS          PASS          PASS          PASS
all-match             PASS          PASS          PASS          PASS
```

#### `CONF-003` Differential evidence

For pure routines, compare:

- return values;
- output buffers;
- declared memory writes;
- error status;
- preserved-register sentinel values where harness supports them.

Acceptance criteria:

- a deliberately incorrect target implementation fails with a useful diff;
- tests do not rely solely on stdout.

---

## VS-11 — Size, Memory, and Performance Evidence

### Objective

Make efficiency claims measurable and prevent accidental bloat.

### Tasks

#### `METRIC-001` Section and artifact budgets

Track per target and example:

- file bytes;
- `.text`;
- read-only data;
- initialized writable data;
- BSS;
- debug data separately;
- imports;
- relocations;
- runtime fragments.

#### `METRIC-002` Baseline comparisons

Comparisons are optional but must be fair and documented.

Possible baselines:

- equivalent C implementation compiled with size optimization and no unnecessary runtime;
- previous SemASM version;
- previous assembly implementation.

Rules:

- same behavior and error handling;
- same target and static/dynamic linking assumptions;
- compiler and flags recorded;
- no marketing conclusion from one microbenchmark.

#### `METRIC-003` Performance harness

Measure only where repeatable:

- pure routine throughput;
- startup cost;
- instruction count via available tools;
- `llvm-mca` static analysis as optional evidence, not real hardware truth;
- QEMU results labeled emulated.

Acceptance criteria:

- report separates measured, estimated, and emulated values;
- no cross-host comparison without normalization.

#### `METRIC-004` Regression gates

Allow budgets in contracts and CI:

```text
maximum .text increase: 64 bytes
maximum stack frame: 128 bytes
maximum imports: 2
no new runtime fragment
```

Any budget exception requires a reason in the pull request.

---

## VS-12 — Contributor Target SDK

### Objective

Make adding a target or ABI understandable to contributors who did not design SemASM.

### Tasks

#### `SDK-001` Target template generator

```bash
semasm target scaffold my-architecture
```

Creates compileable placeholders, tests, and documentation checklist.

#### `SDK-002` Backend conformance harness

Checks:

- register uniqueness and aliases;
- width consistency;
- required operation fixtures;
- ABI register sets do not contradict architecture model;
- target toolchain doctor;
- object format consistency;
- executable smoke fixture.

#### `SDK-003` Contribution tutorial

Create an mdBook chapter that adds one fictional or small target element step by step.

#### `SDK-004` Good-first-issue generator

Maintain small contribution categories:

- add instruction lowering fixture;
- improve a diagnostic;
- add malformed contract test;
- document an ABI rule;
- add object-file fixture;
- port a pure example to an existing target.

Acceptance criteria:

- each issue has exact files, tests, and scope;
- issues do not require contributor knowledge of the whole repository.

---

## 24. Dependency and Execution Graph

```text
VS-00 Bootstrap
  -> VS-01 Contracts
      -> VS-02 First executable
          -> VS-03 Agent task
          -> VS-04 Object decoding
              -> VS-05 x86 ASIR + SysV
                  -> VS-06 Windows x64
                  -> VS-07 AArch64
                      -> VS-08 RISC-V 64
                          -> VS-09 RISC-V 32 bare-metal

VS-03 + VS-05 + VS-07 + VS-08
  -> VS-10 Cross-target conformance

VS-02 onward
  -> VS-11 Metrics

VS-05 + at least two non-x86 targets
  -> VS-12 Contributor SDK
```

Agents must execute prerequisites first unless a task explicitly states that it can proceed independently.

---

## 25. Testing Strategy

### 25.1 Unit tests

Use for:

- type parsing;
- expression parsing;
- target identity;
- register aliasing;
- instruction lowering;
- stack-state transfer;
- diagnostic formatting.

### 25.2 Golden fixtures

Use for:

- diagnostics;
- ASIR JSON;
- object reports;
- target context bundles.

Golden files must be reviewable and updated only with an explicit command.

### 25.3 Integration tests

Use actual assemblers and linkers only in integration tests marked by required tool capability.

A missing optional tool should produce a skipped capability result, not a false pass.

### 25.4 Malformed input tests

Every parser and object reader must test:

- truncated input;
- unsupported version;
- invalid UTF-8 where relevant;
- path traversal attempts;
- oversized counts;
- duplicate identifiers;
- contradictory metadata.

### 25.5 Fuzzing

Add fuzzing after parsers stabilize:

- semantic type parser;
- contract expression parser;
- annotation parser;
- physical instruction normalization;
- object metadata adapter boundaries.

Fuzz targets must not execute generated binaries.

### 25.6 Property tests

Good candidates:

- contract serialization round-trip;
- target ID parse/print round-trip;
- register alias set consistency;
- stack-delta join behavior;
- pure assembly routine behavior across random bounded inputs.

### 25.7 Differential tests

For a pure routine:

- compare target implementations with a tiny reference oracle used only in tests;
- the oracle may be Rust because it is not shipped;
- behavior, not generated instruction sequence, is compared.

### 25.8 Test naming

```text
<component>__<scenario>__<expected_result>
```

Example:

```text
win64_stack__missing_shadow_space__reports_abi004
```

---

## 26. Continuous Integration Matrix

### Required on every pull request

| Job | Linux | Windows | Notes |
|---|---:|---:|---|
| `cargo fmt` | Yes | No | One canonical formatting job is enough. |
| Clippy all targets/features | Yes | Yes | Warnings denied. |
| Core unit tests | Yes | Yes | No external assembler required. |
| Documentation tests | Yes | Yes | Public examples compile. |
| Contract fixtures | Yes | Yes | Deterministic output. |
| ELF target smoke test | Yes | Optional | Linux job installs required tools. |
| PE/COFF target smoke test | Optional | Yes | Windows tools documented. |
| AArch64 QEMU | Yes | No | Capability-gated. |
| RISC-V QEMU | Yes | No | Capability-gated. |
| mdBook build | Yes | No | Broken internal links fail where supported. |
| Dependency security/license | Yes | No | Policy exceptions documented. |

### Nightly or scheduled jobs

- fuzz smoke runs;
- extended QEMU target suite;
- reproducible clean builds;
- minimum supported Rust version if declared;
- performance trend capture without blocking normal pull requests until stable;
- dependency update compatibility.

### Release jobs

- build CLI for selected host platforms;
- generate checksums;
- generate software bill of materials where practical;
- publish mdBook;
- attach changelog;
- verify signed tag according to governance policy;
- do not publish target-generated example binaries as if they were trusted production artifacts.

---

## 27. Security and Trust Model

### 27.1 Untrusted inputs

Treat as untrusted:

- semantic contract files;
- assembly source;
- object files;
- agent responses;
- tool stdout/stderr;
- external tool paths;
- target kits from third parties;
- generated executables and firmware.

### 27.2 Core protections

- no shell command concatenation;
- canonicalize and constrain writable paths;
- explicit process timeouts;
- output-size limits;
- environment allowlists;
- no network by default;
- no automatic tool installation;
- no host execution by default;
- temporary directories with controlled permissions;
- do not follow symlinks outside project root during agent patch validation;
- parse malformed files without panics.

### 27.3 Execution isolation

Preferred order:

1. pure static validation;
2. emulator or VM;
3. OS sandbox/restricted process;
4. native execution only when explicitly allowed.

A target runner declares its isolation level in reports.

### 27.4 Agent safety rules

An agent must not:

- disable tests or validation to make a task pass;
- edit contracts unless allowed;
- add network access;
- download or execute unknown binaries;
- run generated code on the host without permission;
- alter budget thresholds without task authorization;
- claim a target is supported based on assembly success alone.

### 27.5 Unsafe Rust policy

- deny unsafe code in core crates by default;
- isolate unavoidable FFI in adapter crates;
- document every unsafe block with invariants;
- add targeted tests around FFI boundaries;
- report unsafe line count in release metrics if practical.

---

## 28. Performance and Bloat Budgets

### 28.1 Separate tool budgets from output budgets

Tool budgets:

- CLI startup and binary size may be monitored but are secondary;
- optional features must not become default accidentally;
- no background service required for basic use.

Output budgets:

- mandatory per project or example;
- section-level visibility;
- runtime fragment accounting;
- no hidden dependency closure.

### 28.2 Default example budgets

These are starting policies, not universal claims:

| Example | Text | Writable data | Heap | Runtime |
|---|---:|---:|---:|---|
| exit-code | <= 256 B target code where practical | 0 B | none | startup only |
| byte-count function | <= 512 B | 0 B | none | none |
| tiny echo | <= 4 KiB | <= 512 B | none | explicit platform I/O fragment |
| RISC-V UART hello | <= 4 KiB | <= 512 B | none | explicit startup + UART |

Budgets may vary by object format and alignment. Reports must distinguish payload code from file-format overhead.

### 28.3 Efficiency dimensions

Never reduce efficiency to executable file size alone. Track:

- code bytes;
- data bytes;
- runtime dependencies;
- peak stack;
- heap use;
- startup work;
- system calls or device accesses;
- steady-state instruction behavior;
- maintainability and verification cost.

---

## 29. Coding Standards

### 29.1 Language

All code identifiers, comments, diagnostics, documentation, commit messages, and public issue text use English.

### 29.2 Rust style

- stable Rust;
- `rustfmt` canonical formatting;
- Clippy warnings denied in CI, with narrow documented exceptions;
- typed IDs instead of raw integer mixing;
- exhaustive `match` for semantic variants;
- explicit error types at crate boundaries;
- avoid `unwrap` and `expect` outside tests and provable initialization paths;
- no global mutable state;
- deterministic iteration where output is serialized;
- no hidden filesystem or network side effects;
- public APIs documented with examples where useful;
- documentation examples tested.

### 29.3 Assembly style

Each target kit documents its dialect-specific style. Common requirements:

- function header annotation;
- ABI and parameter binding visible;
- preserved-register strategy visible;
- stack-frame layout documented when nontrivial;
- local labels consistent;
- macros kept small and inspectable;
- no unexplained magic addresses or constants;
- MMIO and syscall numbers named;
- comments explain intent or invariant, not instruction translation;
- generated assembly includes a header naming generator/task and contract hash, but not secrets or model credentials.

### 29.4 Comments

Good:

```asm
; Preserve the caller's buffer pointer across the syscall retry loop.
```

Bad:

```asm
; Move RCX to RBX.
mov rbx, rcx
```

### 29.5 Commit policy

Prefer one task per commit where practical.

Commit format:

```text
<area>: <imperative summary>
```

Examples:

```text
contract: reject duplicate return names
win64: diagnose missing shadow space
riscv: lower unsigned branch instructions
```

---

## 30. Documentation Strategy

### 30.1 Documentation layers

1. **README:** immediate understanding and first successful command.
2. **mdBook:** conceptual guide, tutorials, target guides, contributor guide.
3. **Rustdoc:** crate and API reference with tested examples.
4. **ADR:** accepted architectural decisions.
5. **RFC:** proposed major changes.
6. **Target README:** exact toolchain, ABI references, limitations, and fixtures.

### 30.2 Required early mdBook chapters

```text
Introduction
Why Assembly Needs Semantic Leverage
What SemASM Is Not
Quickstart
Semantic Contracts
Target Identity
Agent Task Packets
Verification Ladder
Artifact Reports
Adding a Target
Security Model
Current Limitations
```

### 30.3 Documentation quality gates

- code examples compile or are explicitly marked non-compiling;
- commands are run in CI when practical;
- every supported target has a doctor command example;
- stale screenshots are avoided;
- version-specific external behavior cites official documentation;
- generated reference pages do not replace explanatory tutorials.

---

## 31. Contributor Attraction Strategy

The repository will not gain contributors merely because the idea is technically interesting. It needs low-friction evidence that contributions can succeed.

### 31.1 Provide a five-minute success

A contributor should be able to:

```bash
git clone ...
cargo test --workspace
cargo run -p semasm-cli -- contract check examples/exit-code/contracts/main.sem.toml
```

without installing every target tool.

### 31.2 Separate contributor lanes

Document independent contribution paths:

- Rust core;
- contract fixtures;
- diagnostics;
- x86 instruction semantics;
- AArch64 semantics;
- RISC-V semantics;
- ABI documentation;
- assembly examples;
- QEMU target support;
- documentation and tutorials;
- security review.

### 31.3 Make small issues truly small

A `good first issue` should normally affect one concept and include:

- exact background;
- expected files;
- test command;
- expected diagnostic or output;
- links to the relevant architecture section;
- explicit non-goals.

### 31.4 Credit contributors visibly

- maintain contributors section or automated credit system;
- mention target contributors in release notes;
- include authors in target documentation where appropriate;
- avoid making every architectural decision dependent on one maintainer's private preference.

### 31.5 Use public design records

Substantial design decisions should be discussed in issues, RFCs, or ADRs so new contributors can understand why the code is structured a certain way.

### 31.6 Avoid the empty-framework problem

Public interest is easier when the repository contains:

- a working program;
- an intentionally broken example;
- a useful diagnostic;
- a cross-target comparison;
- a clear target contribution tutorial.

Build these before promoting a grand roadmap.

---

## 32. Governance and Licensing

### 32.1 Suggested project license

Use dual licensing:

```text
Apache License 2.0 OR MIT License
```

This is common in the Rust ecosystem and contributor-friendly, but the final choice should be reviewed against all bundled files and target fragments.

### 32.2 Third-party boundaries

- Capstone has its own license and remains an optional dependency.
- LLVM tools and libraries have their own licensing terms.
- QEMU is used as an external tool.
- Unicorn has GPL licensing implications and must remain isolated and optional unless legal review approves another distribution approach.
- Architecture specifications may have documentation licenses that do not permit copying large sections into the repository.

Store links and concise derived rules, not copied specification text.

### 32.3 Governance phases

Initial:

- benevolent maintainer with documented decision process;
- pull request review required;
- target ownership listed.

Later:

- maintainers by subsystem;
- RFC voting or consensus policy;
- release manager rotation;
- security response team.

---

## 33. Versioning and Compatibility

Version separately:

- SemASM CLI/crates;
- contract schema;
- ASIR serialization schema;
- target-kit schema;
- agent task-packet schema.

A CLI release may evolve without forcing an ASIR schema change.

### Compatibility rules

- schema documents include explicit version;
- readers reject unsupported major versions clearly;
- minor additions should be backward compatible where possible;
- unknown semantic operations are preserved or rejected according to schema policy, never silently dropped;
- target kits declare minimum SemASM version;
- diagnostics codes remain stable within a major release.

---

## 34. Release Milestones

Do not attach calendar promises. Release by demonstrated capability.

### `0.1.0` — Contract and first executable

Required:

- VS-00 through VS-02;
- x86-64 Linux exit-code example;
- deterministic artifact report;
- public documentation.

### `0.2.0` — Agent task and x86 semantic checks

Required:

- VS-03 through VS-05;
- pure function generated through task packet;
- System V ABI diagnostics;
- Capstone optional adapter.

### `0.3.0` — Windows and second ISA

Required:

- Windows x64 target;
- AArch64 Linux target;
- shared contract reused;
- target support levels documented.

### `0.4.0` — RISC-V and bare-metal

Required:

- RISC-V 64 Linux;
- RISC-V 32 bare-metal QEMU fixture;
- MMIO and memory map semantics;
- no hidden runtime evidence.

### `0.5.0` — Cross-target conformance and metrics

Required:

- shared behavioral suites;
- size and runtime-fragment budgets;
- contributor target SDK preview.

### `1.0.0` criteria

- contract and task schemas stable;
- at least three supported targets, not merely preview;
- ABI conformance fixtures mature;
- malformed input and security testing established;
- clear compatibility and governance policies;
- documented real-world program beyond toy examples;
- honest known limitations.

---

## 35. Risk Register

### R1. Scope becomes a new compiler

Mitigation:

- preserve assembly as implementation;
- keep contract language declarative and bounded;
- reject imperative features;
- require RFC for any contract-language expansion.

### R2. Attempting full ISA semantics too early

Mitigation:

- lower only instructions used by vertical slices;
- explicit `Unknown` semantics;
- backend coverage metrics;
- contributors add fixtures incrementally.

### R3. Agent-generated assembly appears correct but is unsafe

Mitigation:

- mandatory verification ladder;
- no host execution by default;
- behavior and ABI tests;
- patch-scope enforcement;
- explicit unsupported-semantics count.

### R4. Tooling becomes bloated

Mitigation:

- optional adapters;
- no LLVM library embedding initially;
- core dependency policy;
- feature-matrix CI;
- separate tool size from shipped artifact size.

### R5. Generated assembly is worse than compiler output

Mitigation:

- benchmark fairly;
- permit compiler baselines;
- optimize for declared purpose, not pride;
- report regressions honestly.

### R6. Multi-target abstraction becomes x86-centric

Mitigation:

- add AArch64 before advanced x86 work;
- require RISC-V before stabilizing target APIs;
- prohibit x86 register names in core semantic contracts.

### R7. Metadata becomes stale

Mitigation:

- validate annotations against source/object behavior;
- contract hashes in generated source;
- contradictions are errors;
- generated reports bind metadata to source and object hashes.

### R8. Lack of contributors

Mitigation:

- working demos before promotion;
- small contribution lanes;
- clear target SDK;
- fast core test path without external tools;
- public ADR/RFC history;
- realistic README.

### R9. Licensing conflict

Mitigation:

- isolate optional GPL tools;
- dependency license checks;
- do not copy architecture specifications;
- review runtime fragment provenance.

### R10. IoT target explosion

Mitigation:

- distinguish ISA backend from board/machine profile;
- first bare-metal target is QEMU `virt`;
- hardware boards added only through documented machine kits;
- do not promise every microcontroller family.

### R11. Formal verification ambition blocks useful delivery

Mitigation:

- deterministic checks and behavioral tests first;
- optional SMT later;
- no claim of proof when only analysis or tests exist.

---

## 36. Definition of Done

A normal implementation task is done only when:

- requested behavior is implemented;
- source is formatted according to project rules;
- unit tests pass;
- integration tests relevant to the target pass;
- diagnostics are added for new failure modes;
- documentation is updated;
- no unrelated files changed;
- artifact or output budgets pass;
- security constraints remain enabled;
- no hidden runtime or dependency was introduced;
- the agent records commands and results;
- limitations are stated honestly.

A target is done at **Preview** level only when:

- tool doctor works;
- contract binds to ABI;
- source assembles;
- object inspection works;
- at least one routine lowers into ASIR;
- ABI checks exist;
- executable test runs in CI or a documented controlled environment;
- target guide exists;
- unsupported areas are listed.

---

## 37. Instructions for Coding Agents Executing This Plan

### 37.1 General execution rules

1. Read `ARCHITECTURE.md`, relevant ADRs, and the assigned task before editing.
2. Do not implement later-slice abstractions preemptively.
3. Preserve English naming and comments.
4. Keep core dependencies minimal.
5. Never add a dependency without updating `DEPENDENCIES.md`.
6. Never execute generated assembly directly on the host unless the task explicitly authorizes it.
7. Do not modify contracts, budgets, or tests merely to make an implementation pass.
8. Use official ISA and ABI references.
9. Add an intentionally failing fixture for every new diagnostic.
10. Report unsupported semantics explicitly.
11. Run the exact required validation commands.
12. Do not claim success when an external tool or target test was skipped.

### 37.2 Required agent completion report

```markdown
## Task Completion

- Task ID:
- Objective:
- Status: complete | partial | blocked

## Files Changed

- `path`: reason

## Design Decisions

- decision and rationale

## Validation

- `command`
  - result

## Artifact Impact

- text bytes before/after
- data bytes before/after
- imports/runtime fragments before/after

## Remaining Limitations

- explicit limitation
```

### 37.3 Agent stop conditions

The agent must stop and report partial status when:

- official target behavior cannot be verified;
- task requires changing a public schema not authorized by the task;
- validation requires unsafe host execution;
- a license is unclear;
- an assembler/disassembler disagreement cannot be resolved;
- the task would introduce a hidden runtime;
- acceptance criteria conflict.

The agent should still leave useful tests, diagnostics, or documentation rather than replacing uncertainty with invented behavior.

---

## 38. Standard Agent Task Template

```markdown
# TASK-XXX: <Imperative task title>

## Objective

One concrete user-visible or analyzer-visible result.

## Context

Relevant architecture, target, contract, and existing behavior.

## Allowed Files

- exact path or directory

## Forbidden Changes

- contract changes unless authorized
- dependency additions unless authorized
- test deletion
- validation bypass
- host execution

## Implementation Requirements

1. Specific behavior.
2. Required diagnostics.
3. Required documentation.
4. Required artifact constraints.

## Acceptance Criteria

- externally observable result;
- exact command and expected exit status;
- tests;
- size or dependency budget;
- unsupported behavior reported.

## Validation Commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
<target-specific command>
```

## Completion Report

Use the repository agent completion template.
```

---

## 39. Recommended Initial Public Issues

These should be opened only after the repository skeleton exists.

1. `contract: parse primitive unsigned integer types`
2. `contract: report duplicate parameter names with source spans`
3. `diagnostics: add terminal rendering test for CTR003`
4. `docs: explain ISA versus ABI versus assembly dialect`
5. `target: parse canonical x86_64 Linux target identity`
6. `target: add tool doctor JSON output fixture`
7. `example: add intentionally invalid exit-code contract`
8. `x86: document EAX-to-RAX zero-extension rule`
9. `object: report ELF section names and sizes`
10. `contributing: document how to add a diagnostic fixture`

Each issue must include exact acceptance commands before receiving `good first issue` labeling.

---

## 40. First Reference Program Set

The examples should grow in semantic difficulty.

### E1. Exit code

Teaches entry point, syscall/platform exit, ELF/PE structure, artifact reporting.

### E2. Fixed string output

Teaches data sections, pointer/length binding, platform I/O, imports or syscalls.

### E3. Byte counter

Teaches pure function ABI, loops, memory reads, unsigned bounds, cross-target conformance.

### E4. In-place ASCII uppercase

Teaches writable memory effects, branch behavior, exact modified region.

### E5. Checksum

Teaches arithmetic width, overflow policy, larger randomized test set.

### E6. Tiny line protocol parser

Teaches state machine, bounded buffers, error states, and richer invariants without heap allocation.

### E7. RISC-V UART hello

Teaches bare-metal startup, linker script, memory map, MMIO, and QEMU system execution.

### E8. Small IoT command loop

Deferred until E1–E7 are stable. Must use bounded input, fixed memory, explicit device model, and no dynamic allocation.

---

## 41. Long-Term Product Direction

After the foundation is proven, SemASM may support:

- interactive agent repair using debugger state;
- stack and register timeline visualization;
- semantic diff between assembly patches;
- target-to-target implementation comparison;
- register-allocation suggestions without rewriting source automatically;
- verified runtime fragment catalog;
- device and board kits;
- symbolic execution for bounded routines;
- constant-time and side-channel-oriented policies;
- interrupt and concurrency models;
- SIMD/vector semantics;
- linker and relocation diagnostics;
- source-to-object semantic traceability;
- constrained direct machine-code emission as an advanced, separately trusted mode.

Direct raw machine-code generation must not be an early feature. Assembly source plus a mature assembler provides a valuable verification and review boundary.

---

## 42. Success Criteria for the Project Vision

The project is meaningfully advancing its original goal when all of the following are true:

1. A user can describe a small program through a portable contract and target manifest.
2. An external agent can receive a bounded task packet and write target assembly directly.
3. SemASM can prove basic source/object/ABI consistency and identify unsupported semantics.
4. The program can be tested in a controlled target environment.
5. Equivalent implementations can be validated across at least three ISAs.
6. The final executable or firmware contains no undeclared language runtime.
7. Size and memory claims are supported by reproducible evidence.
8. A new contributor can add a small instruction semantic, fixture, diagnostic, or target component without private guidance.
9. The README remains honest about limitations.
10. At least one nontrivial, bounded, practical program is shipped as an example beyond “hello world.”

---

## 43. Immediate Execution Order

The first agent should execute only the following sequence:

```text
BOOT-001
BOOT-002
BOOT-003
BOOT-004
CONTRACT-001
CONTRACT-002
CONTRACT-003
CONTRACT-004
TARGET-001
TARGET-002
BUILD-001
BUILD-002
REPORT-001
```

Do not begin Capstone, LLVM embedding, AArch64, RISC-V, or an integrated AI provider before this sequence produces a clean-clone executable example and report.

The second execution wave is:

```text
AGENT-001
AGENT-002
AGENT-003
AGENT-004
OBJECT-001
DECODE-001
CFG-001
X86-001
X86-002
ABI-SYSV-001
ANALYSIS-001
```

This order deliberately proves the product workflow before expanding architecture coverage.

---

## 44. Final Architectural Position

SemASM should be designed as a semantic and verification environment around direct assembly authorship, not as another layer that hides machine code.

The intended final relationship is:

```text
Human defines purpose and constraints.
Agent writes target assembly.
SemASM supplies semantic leverage and target knowledge.
Assembler and linker produce the artifact.
Static analysis and execution evidence determine whether it is acceptable.
Only explicit target code is shipped.
```

That preserves the reason for the project: bypassing unnecessary high-level runtime layers where direct target-specific implementation is justified, while adding enough semantics and verification that an AI agent can work at machine level without relying on guesswork alone.

---

## 45. Official Technical References

Use current official specifications during implementation and record the exact revision used by each backend.

### Rust and project tooling

- [Cargo Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [Cargo Features](https://doc.rust-lang.org/cargo/reference/features.html)
- [The rustdoc Book](https://doc.rust-lang.org/rustdoc/what-is-rustdoc.html)
- [Rustdoc Documentation Tests](https://doc.rust-lang.org/rustdoc/documentation-tests.html)
- [mdBook Documentation](https://rust-lang.github.io/mdBook/)
- [RustSec Advisory Database](https://rustsec.org/)
- [cargo-audit](https://github.com/rustsec/rustsec/blob/main/cargo-audit/README.md)

### Machine-code and object tooling

- [Capstone](https://www.capstone-engine.org/)
- [Capstone Supported Architectures](https://www.capstone-engine.org/arch)
- [LLVM `llvm-mc`](https://llvm.org/docs/CommandGuide/llvm-mc.html)
- [LLVM Machine Code Analyzer](https://llvm.org/docs/CommandGuide/llvm-mca.html)
- [Rust `object` crate](https://docs.rs/object/)
- [QEMU Emulation Documentation](https://www.qemu.org/docs/master/about/emulation.html)
- [QEMU Arm System Emulator](https://www.qemu.org/docs/master/system/target-arm.html)
- [QEMU RISC-V System Emulator](https://www.qemu.org/docs/master/system/target-riscv.html)
- [Unicorn Engine](https://github.com/unicorn-engine/unicorn)

### ABI references

- [Microsoft x64 Calling Convention](https://learn.microsoft.com/en-us/cpp/build/x64-calling-convention)
- [Microsoft Overview of x64 ABI Conventions](https://learn.microsoft.com/en-us/cpp/build/x64-software-conventions)
- [x86-64 System V psABI Repository](https://gitlab.com/x86-psABIs/x86-64-ABI)
- [Arm ABI Repository](https://github.com/ARM-software/abi-aa)
- [AAPCS64 Source](https://github.com/ARM-software/abi-aa/blob/main/aapcs64/aapcs64.rst)
- [RISC-V ELF psABI Specification](https://riscv-non-isa.github.io/riscv-elf-psabi-doc/)
