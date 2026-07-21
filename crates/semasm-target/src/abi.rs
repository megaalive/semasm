//! Target ABI register assignment.
//!
//! Maps parameters and return values to hardware registers according to
//! each target's calling convention.  This is the authoritative source for
//! code generation and context bundles.

use serde::{Deserialize, Serialize};

use crate::{Abi, TargetIdentity};

/// Register assignment for a target ABI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ABIRegisterMap {
    /// Parameter-passing registers in declaration order
    /// (e.g. `rdi`, `rsi`, `rdx`, `rcx`, `r8`, `r9` for SysV).
    pub parameter_registers: Vec<String>,
    /// Return-value register (e.g. `rax`).
    pub return_register: String,
    /// Callee-saved registers the implementation must preserve.
    pub preserved_registers: Vec<String>,
    /// Caller-saved (volatile) registers the implementation may freely clobber.
    pub volatile_registers: Vec<String>,
}

impl ABIRegisterMap {
    /// Map the n-th integer-class parameter to its register name.
    ///
    /// Returns `None` when the parameter would be passed on the stack.
    #[must_use]
    pub fn param_register(&self, index: usize) -> Option<&str> {
        self.parameter_registers.get(index).map(String::as_str)
    }
}

// ---------------------------------------------------------------------------
// Known ABI definitions
// ---------------------------------------------------------------------------

/// System V AMD64 ABI (Linux, BSD, Solaris).
///
/// - Integer args: %rdi, %rsi, %rdx, %rcx, %r8, %r9, then stack.
/// - Return:       %rax.
/// - Callee-saved: %rbx, %rbp, %r12–%r15.
/// - Volatile:     %rax, %rcx, %rdx, %rsi, %rdi, %r8–%r11.
fn sysv_amd64() -> ABIRegisterMap {
    ABIRegisterMap {
        parameter_registers: vec![
            "rdi".into(),
            "rsi".into(),
            "rdx".into(),
            "rcx".into(),
            "r8".into(),
            "r9".into(),
        ],
        return_register: "rax".into(),
        preserved_registers: vec![
            "rbx".into(),
            "rbp".into(),
            "r12".into(),
            "r13".into(),
            "r14".into(),
            "r15".into(),
        ],
        volatile_registers: vec![
            "rax".into(),
            "rcx".into(),
            "rdx".into(),
            "rsi".into(),
            "rdi".into(),
            "r8".into(),
            "r9".into(),
            "r10".into(),
            "r11".into(),
        ],
    }
}

/// Microsoft x64 calling convention (Windows).
///
/// - Integer args: %rcx, %rdx, %r8, %r9, then stack.
/// - Return:       %rax.
/// - Callee-saved: %rbx, %rbp, %rdi, %rsi, %r12–%r15.
/// - Volatile:     %rax, %rcx, %rdx, %r8–%r11, %xmm0–%xmm5.
fn win64() -> ABIRegisterMap {
    ABIRegisterMap {
        parameter_registers: vec!["rcx".into(), "rdx".into(), "r8".into(), "r9".into()],
        return_register: "rax".into(),
        preserved_registers: vec![
            "rbx".into(),
            "rbp".into(),
            "rdi".into(),
            "rsi".into(),
            "r12".into(),
            "r13".into(),
            "r14".into(),
            "r15".into(),
        ],
        volatile_registers: vec![
            "rax".into(),
            "rcx".into(),
            "rdx".into(),
            "r8".into(),
            "r9".into(),
            "r10".into(),
            "r11".into(),
        ],
    }
}

/// AArch64 Procedure Call Standard (Linux, BSD).
///
/// - Args: %x0–%x7, then stack.
/// - Return: %x0.
/// - Callee-saved: %x19–%x28.
/// - Volatile: %x0–%x17, %x30 (link).
fn aapcs64() -> ABIRegisterMap {
    ABIRegisterMap {
        parameter_registers: (0..8).map(|i| format!("x{i}")).collect(),
        return_register: "x0".into(),
        preserved_registers: (19..=28).map(|i| format!("x{i}")).collect(),
        volatile_registers: (0..=17).map(|i| format!("x{i}")).collect(),
    }
}

/// RISC-V LP64 integer ABI (psABI).
///
/// - Integer args: a0–a7, then stack.
/// - Return:       a0.
/// - Callee-saved: sp, s0–s11.
/// - Volatile:     ra, t0–t6, a0–a7.
fn riscv_lp64() -> ABIRegisterMap {
    ABIRegisterMap {
        parameter_registers: (0..8).map(|i| format!("a{i}")).collect(),
        return_register: "a0".into(),
        preserved_registers: {
            let mut regs = vec!["sp".into(), "s0".into(), "s1".into()];
            regs.extend((2..=11).map(|i| format!("s{i}")));
            regs
        },
        volatile_registers: {
            let mut regs = vec!["ra".into()];
            regs.extend((0..=6).map(|i| format!("t{i}")));
            regs.extend((0..=7).map(|i| format!("a{i}")));
            regs
        },
    }
}

// ---------------------------------------------------------------------------
// TargetIdentity integration
// ---------------------------------------------------------------------------

impl TargetIdentity {
    /// Return the ABI register map for this target, if one is known.
    ///
    /// Returns `None` for targets whose ABI has not yet been registered.
    #[must_use]
    pub fn abi_register_map(&self) -> Option<ABIRegisterMap> {
        match self.abi {
            Abi::SysVAmd64 => Some(sysv_amd64()),
            Abi::WindowsX64 => Some(win64()),
            Abi::Aapcs64 => Some(aapcs64()),
            Abi::Riscv => Some(riscv_lp64()),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sysv_has_six_parameter_registers() {
        let target = TargetIdentity::x86_64_linux_gnu();
        let map = target.abi_register_map().expect("SysV ABI known");
        assert_eq!(map.parameter_registers.len(), 6);
        assert_eq!(map.param_register(0), Some("rdi"));
        assert_eq!(map.param_register(3), Some("rcx"));
        assert_eq!(map.param_register(5), Some("r9"));
        assert!(map.param_register(6).is_none());
    }

    #[test]
    fn sysv_has_return_register() {
        let target = TargetIdentity::x86_64_linux_gnu();
        let map = target.abi_register_map().unwrap();
        assert_eq!(map.return_register, "rax");
    }

    #[test]
    fn sysv_preserved_does_not_overlap_volatile() {
        let target = TargetIdentity::x86_64_linux_gnu();
        let map = target.abi_register_map().unwrap();
        for p in &map.preserved_registers {
            assert!(
                !map.volatile_registers.contains(p),
                "{p} is both preserved and volatile"
            );
        }
    }

    #[test]
    fn win64_has_four_parameter_registers() {
        let target = TargetIdentity {
            name: "x86_64-unknown-windows".into(),
            isa: crate::Isa::X86_64,
            abi: crate::Abi::WindowsX64,
            object_format: crate::ObjectFormat::PeCoff,
            dialect: crate::Dialect::NasmIntel,
            profile: crate::ExecutionProfile::HostedMinimal,
        };
        let map = target.abi_register_map().expect("Win64 ABI known");
        assert_eq!(map.parameter_registers.len(), 4);
        assert_eq!(map.param_register(0), Some("rcx"));
        assert_eq!(map.param_register(3), Some("r9"));
        assert!(map.param_register(4).is_none());
    }

    #[test]
    fn aapcs64_has_eight_parameter_registers() {
        let target = TargetIdentity {
            name: "aarch64-unknown-linux-gnu".into(),
            isa: crate::Isa::AArch64,
            abi: crate::Abi::Aapcs64,
            object_format: crate::ObjectFormat::Elf,
            dialect: crate::Dialect::GasUnified,
            profile: crate::ExecutionProfile::HostedMinimal,
        };
        let map = target.abi_register_map().expect("AAPCS64 ABI known");
        assert_eq!(map.parameter_registers.len(), 8);
        assert_eq!(map.param_register(0), Some("x0"));
        assert_eq!(map.param_register(7), Some("x7"));
        assert!(map.param_register(8).is_none());
    }

    #[test]
    fn riscv_has_eight_parameter_registers() {
        let target = TargetIdentity {
            name: "riscv64-unknown-linux-gnu".into(),
            isa: crate::Isa::Riscv64,
            abi: crate::Abi::Riscv,
            object_format: crate::ObjectFormat::Elf,
            dialect: crate::Dialect::GasUnified,
            profile: crate::ExecutionProfile::HostedMinimal,
        };
        let map = target.abi_register_map().expect("RISC-V ABI known");
        assert_eq!(map.parameter_registers.len(), 8);
        assert_eq!(map.param_register(0), Some("a0"));
        assert_eq!(map.param_register(7), Some("a7"));
        assert_eq!(map.return_register, "a0");
        assert!(map.param_register(8).is_none());
    }
}
