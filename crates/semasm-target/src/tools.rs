//! Tool discovery for target kits.
//!
//! Probes the system `PATH` for required tooling (assembler, linker,
//! disassembler, runner) and reports found versions or actionable install
//! instructions.  No tool is ever installed automatically.

use std::fmt;
use std::process::Command;

use crate::{Abi, Isa, ObjectFormat, TargetIdentity};

// ---------------------------------------------------------------------------
// Tool kinds
// ---------------------------------------------------------------------------

/// A tool or tool category required by a target kit.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ToolKind {
    /// NASM assembler.
    Nasm,
    /// LLD linker (ELF / COFF).
    Lld,
    /// GNU ld linker fallback.
    LdBfd,
    /// LLVM object dumper.
    LlvmObjdump,
    /// GNU objdump fallback.
    Objdump,
    /// QEMU user-mode runner for a specific ISA.
    Qemu(&'static str),
}

impl ToolKind {
    /// Display label shown in human-readable output.
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Nasm => "nasm",
            Self::Lld => "ld.lld",
            Self::LdBfd => "ld.bfd",
            Self::LlvmObjdump => "llvm-objdump",
            Self::Objdump => "objdump",
            Self::Qemu(cpu) => cpu,
        }
    }

    /// Binary name to probe on `PATH`.
    #[must_use]
    pub fn binary(&self) -> &str {
        match self {
            Self::Nasm => "nasm",
            Self::Lld => "ld.lld",
            Self::LdBfd => "ld.bfd",
            Self::LlvmObjdump => "llvm-objdump",
            Self::Objdump => "objdump",
            Self::Qemu(_) => self.label(),
        }
    }

    /// Rough category for grouping in reports.
    #[must_use]
    pub fn category(&self) -> &str {
        match self {
            Self::Nasm => "assembler",
            Self::Lld | Self::LdBfd => "linker",
            Self::LlvmObjdump | Self::Objdump => "disassembler",
            Self::Qemu(_) => "runner",
        }
    }

    /// Platform-agnostic install hint.
    #[must_use]
    pub fn install_hint(&self) -> Vec<&'static str> {
        match self {
            Self::Nasm => vec![
                "apt install nasm",
                "brew install nasm",
                "choco install nasm",
            ],
            Self::Lld => vec![
                "apt install lld",
                "brew install lld",
                "Ensure LLD is available from an LLVM installation",
            ],
            Self::LdBfd | Self::Objdump => vec!["apt install binutils", "brew install binutils"],
            Self::LlvmObjdump => vec![
                "apt install llvm",
                "brew install llvm",
                "Ensure llvm-objdump (or objdump) is on PATH",
            ],
            Self::Qemu(_) => vec!["apt install qemu-user", "brew install qemu"],
        }
    }
}

impl fmt::Display for ToolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

// ---------------------------------------------------------------------------
// Probe result
// ---------------------------------------------------------------------------

/// Outcome of probing a single tool.
#[derive(Debug, Clone)]
pub struct ToolProbe {
    /// Which tool was probed.
    pub kind: ToolKind,
    /// Whether the binary was found and executed successfully.
    pub found: bool,
    /// First line of `--version` output (if available).
    pub version: Option<String>,
    /// `OsString`-to-string error detail.
    pub detail: Option<String>,
}

impl ToolProbe {
    fn probe(kind: ToolKind) -> Self {
        let binary = kind.binary();
        match Command::new(binary).arg("--version").output() {
            Ok(output) => {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .next()
                        .map(|s| s.trim().to_string())
                        .or_else(|| {
                            String::from_utf8_lossy(&output.stderr)
                                .lines()
                                .next()
                                .map(|s| s.trim().to_string())
                        });
                    Self {
                        kind,
                        found: true,
                        version,
                        detail: None,
                    }
                } else {
                    Self {
                        kind,
                        found: false,
                        version: None,
                        detail: Some(format!("exit code {}", output.status)),
                    }
                }
            }
            Err(e) => Self {
                kind,
                found: false,
                version: None,
                detail: Some(e.to_string()),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Tool-candidate chain (preferred + fallbacks)
// ---------------------------------------------------------------------------

/// Ordered list of candidate binaries for a single tool role.
/// The first candidate that is found on `PATH` is the effective tool.
#[derive(Debug, Clone)]
pub struct ToolSlot {
    /// Human-readable role name (e.g. `"linker"`, `"disassembler"`).
    pub role: &'static str,
    /// Candidates in preference order.
    pub candidates: Vec<ToolKind>,
    /// Index of the candidate that was found, if any.
    pub resolved: Option<usize>,
    /// Probe results for all checked candidates.
    pub probes: Vec<ToolProbe>,
}

impl ToolSlot {
    fn probe(role: &'static str, candidates: Vec<ToolKind>) -> Self {
        let mut probes = Vec::with_capacity(candidates.len());
        let mut resolved = None;
        for (i, kind) in candidates.iter().enumerate() {
            let probe = ToolProbe::probe(kind.clone());
            if probe.found && resolved.is_none() {
                resolved = Some(i);
            }
            probes.push(probe);
        }
        Self {
            role,
            candidates,
            resolved,
            probes,
        }
    }

    /// The effective tool that will be used (first found candidate).
    #[must_use]
    pub fn effective(&self) -> Option<&ToolProbe> {
        self.resolved.map(|i| &self.probes[i])
    }
}

// ---------------------------------------------------------------------------
// Doctor report
// ---------------------------------------------------------------------------

/// Full tool-chain report for a target.
#[derive(Debug, Clone)]
pub struct DoctorReport {
    /// Canonical target name.
    pub target: String,
    /// Tool slots (each role with preferred + fallback candidates).
    pub slots: Vec<ToolSlot>,
}

impl DoctorReport {
    /// True when every role has at least one resolved candidate.
    #[must_use]
    pub fn all_found(&self) -> bool {
        self.slots.iter().all(|s| s.resolved.is_some())
    }

    /// Count of resolved (found) tool roles.
    #[must_use]
    pub fn found_count(&self) -> usize {
        self.slots.iter().filter(|s| s.resolved.is_some()).count()
    }

    /// Total tool roles.
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.slots.len()
    }
}

// ---------------------------------------------------------------------------
// Target → required tools mapping
// ---------------------------------------------------------------------------

/// Return the tool slots required for a given target.
#[must_use]
pub fn required_tools(target: &TargetIdentity) -> Vec<ToolSlot> {
    match (target.isa, target.abi, target.object_format) {
        (Isa::X86_64, Abi::SysVAmd64, ObjectFormat::Elf) => vec![
            ToolSlot::probe("assembler", vec![ToolKind::Nasm]),
            ToolSlot::probe("linker", vec![ToolKind::Lld, ToolKind::LdBfd]),
            ToolSlot::probe(
                "disassembler",
                vec![ToolKind::LlvmObjdump, ToolKind::Objdump],
            ),
            ToolSlot::probe("runner", vec![ToolKind::Qemu("qemu-x86_64")]),
        ],
        _ => vec![],
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Doctor the tool-chain for a target identity.
///
/// Every slot is probed in preference order.  The report carries the full
/// probe results so callers can render human text or JSON.
#[must_use]
pub fn doctor(target: &TargetIdentity) -> DoctorReport {
    DoctorReport {
        target: target.name.clone(),
        slots: required_tools(target),
    }
}
