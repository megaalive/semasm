# Security Policy

## Supported versions

SemASM is in early development. Security fixes are applied on a best-effort basis to the default branch. There is no long-term support release yet.

## What to report

Please report vulnerabilities that could reasonably affect users, including:

- unsafe or incorrect **code generation guidance** that could cause agents or users to produce exploitable assembly when following SemASM contracts or validators;
- **sandbox / runner escapes** once execution harnesses exist;
- path traversal or arbitrary file write in the CLI;
- dependency supply-chain issues in published crates;
- cryptographic or integrity failures in reports or task packets (when those features exist).

Ordinary correctness bugs in analysis that do not have a security impact may be filed as normal issues.

## What not to open as public issues

Do not file public GitHub issues for active exploits against:

- sandbox bypass;
- remote code execution in tooling;
- intentional generation of malware.

## How to report

Prefer private disclosure:

1. Email the maintainers using a contact listed in the GitHub repository security advisories / contact metadata when available.
2. If that channel is not yet configured, open a **draft** GitHub Security Advisory on the repository, or contact the repository owners directly.

Include:

- affected commit or version;
- reproduction steps;
- impact assessment;
- whether a public PoC already exists.

## Policy notes specific to SemASM

- SemASM validates assembly; it does **not** claim memory safety for arbitrary agent-written code.
- Generated programs intentionally contain no SemASM runtime; security of the shipped binary depends on the assembly and platform interfaces chosen by the author.
- Verification and build reports record an `isolation` field: `static_only`, `qemu_user`, or `native_host`. That field describes how (or whether) a process was started — not an OS sandbox guarantee.

### Execution isolation: guaranteed today vs not guaranteed

| Guaranteed today (`crates/semasm-build/src/exec.rs`) | Not guaranteed by default |
|---|---|
| Wall-clock timeout with process-tree kill | seccomp / Landlock |
| Sanitized / allowlisted child environment | Windows job-object network deny |
| Bounded stdout/stderr capture | Container / gVisor / Firecracker |
| Null stdin by default | Full network isolation |
| Prefer `qemu_user` over native for Linux ELF when both exist | Memory-safety of agent asm |

Agent verify prefers **qemu-user** for Linux guest targets when QEMU is on `PATH`; Win64 uses **native_host** only. `static_only` means no candidate process ran (execution denied or static gates only).

## Response expectations

During bootstrap, response times are not guaranteed. Maintainers will acknowledge serious reports when possible and coordinate a fix before public detail is required.
