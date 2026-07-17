//! Target identity and target-kit interfaces.
//!
//! ISA, ABI, platform, object format, dialect, and execution profile are
//! separate concepts. Architecture-specific backends are added only after a
//! vertical slice needs them.

#![forbid(unsafe_code)]

pub mod tools;

use std::fmt;

use semasm_core::{Error, Result};

/// Instruction set architecture family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Isa {
    /// x86-64 (AMD64).
    X86_64,
    /// AArch64 (ARM 64-bit).
    AArch64,
    /// RISC-V 64-bit.
    Riscv64,
    /// RISC-V 32-bit.
    Riscv32,
}

impl fmt::Display for Isa {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::X86_64 => "x86_64",
            Self::AArch64 => "aarch64",
            Self::Riscv64 => "riscv64",
            Self::Riscv32 => "riscv32",
        })
    }
}

/// Calling convention / ABI family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Abi {
    /// System V AMD64 ABI (Linux, BSD, etc.).
    SysVAmd64,
    /// Microsoft x64 calling convention.
    WindowsX64,
    /// AArch64 Procedure Call Standard.
    Aapcs64,
    /// RISC-V psABI (LP64 / ILP32 variants selected elsewhere).
    Riscv,
}

impl fmt::Display for Abi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SysVAmd64 => "sysv-amd64",
            Self::WindowsX64 => "win64",
            Self::Aapcs64 => "aapcs64",
            Self::Riscv => "riscv",
        })
    }
}

/// Object file / executable container format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectFormat {
    /// ELF (typically Linux / bare-metal).
    Elf,
    /// PE/COFF (Windows).
    PeCoff,
}

impl fmt::Display for ObjectFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Elf => "elf",
            Self::PeCoff => "pe-coff",
        })
    }
}

/// Assembly source dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dialect {
    /// NASM Intel syntax.
    NasmIntel,
    /// GNU assembler AT&T syntax.
    GasAtt,
    /// GNU/LLVM unified or architecture-default syntax.
    GasUnified,
}

impl fmt::Display for Dialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NasmIntel => "nasm-intel",
            Self::GasAtt => "gas-att",
            Self::GasUnified => "gas-unified",
        })
    }
}

/// Runtime / environment profile for the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionProfile {
    /// Hosted userspace with a minimal OS surface.
    HostedMinimal,
    /// Freestanding with no OS assumptions.
    Freestanding,
    /// Bare-metal firmware / board image.
    BareMetal,
}

impl fmt::Display for ExecutionProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::HostedMinimal => "hosted-minimal",
            Self::Freestanding => "freestanding",
            Self::BareMetal => "bare-metal",
        })
    }
}

/// Complete target identity (architecture alone is never enough).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TargetIdentity {
    /// Canonical triple-like name (for example `x86_64-unknown-linux-gnu`).
    pub name: String,
    /// ISA family.
    pub isa: Isa,
    /// ABI.
    pub abi: Abi,
    /// Object format.
    pub object_format: ObjectFormat,
    /// Preferred assembly dialect for this kit.
    pub dialect: Dialect,
    /// Execution profile.
    pub profile: ExecutionProfile,
}

impl TargetIdentity {
    /// Well-known first-slice target: x86-64 Linux System V ELF.
    #[must_use]
    pub fn x86_64_linux_gnu() -> Self {
        Self {
            name: "x86_64-unknown-linux-gnu".to_string(),
            isa: Isa::X86_64,
            abi: Abi::SysVAmd64,
            object_format: ObjectFormat::Elf,
            dialect: Dialect::NasmIntel,
            profile: ExecutionProfile::HostedMinimal,
        }
    }

    /// Parse a known target name. Unknown names return [`Error::NotFound`].
    ///
    /// Full target registry and kits arrive in later slices.
    pub fn parse_known(name: &str) -> Result<Self> {
        match name {
            "x86_64-unknown-linux-gnu" | "x86_64-linux-gnu" => Ok(Self::x86_64_linux_gnu()),
            other => Err(Error::not_found(format!(
                "unknown or unsupported target `{other}` (planned targets are not yet registered)"
            ))),
        }
    }
}

impl fmt::Display for TargetIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

/// Placeholder for a full target kit (tools, ABI tables, fragments).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetKit {
    /// Identity of this kit.
    pub identity: TargetIdentity,
}

impl TargetKit {
    /// Build a kit shell around an identity.
    #[must_use]
    pub fn new(identity: TargetIdentity) -> Self {
        Self { identity }
    }

    /// Target display name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.identity.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_linux_x86_64() {
        let t = TargetIdentity::parse_known("x86_64-unknown-linux-gnu").unwrap();
        assert_eq!(t.isa, Isa::X86_64);
        assert_eq!(t.abi, Abi::SysVAmd64);
        assert_eq!(t.object_format, ObjectFormat::Elf);
    }

    #[test]
    fn rejects_unknown_target() {
        let err = TargetIdentity::parse_known("avr-unknown-none").unwrap_err();
        assert!(matches!(err, Error::NotFound(_)));
    }

    #[test]
    fn kit_exposes_name() {
        let kit = TargetKit::new(TargetIdentity::x86_64_linux_gnu());
        assert_eq!(kit.name(), "x86_64-unknown-linux-gnu");
    }
}
