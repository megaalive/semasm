//! RISC-V 64-bit (RV64G) register and alias model for SemASM.
//!
//! This crate captures the parts of the RISC-V architecture that later slices
//! (ASIR lowering, RISC-V psABI checks) need to reason about:
//!
//! * the 32 general-purpose registers `x0`–`x31` (where `x0` is hardwired zero),
//! * the ABI names: `zero`, `ra`, `sp`, `gp`, `tp`, `t0`–`t6`, `s0`/`fp`, `s1`–`s11`,
//!   `a0`–`a7`,
//! * the zero register (`x0`) — reads as 0, writes discarded,
//! * the stack pointer `sp` (`x2`),
//! * the return address `ra` (`x1`),
//! * the frame pointer `fp`/`s0` (`x8`),
//! * RV32 vs RV64 width distinction (`W`-instructions sign-extend),
//! * the standard extension set `G` = `I` `M` `A` `F` `D` (base + mul/div + atomics + FP).
//!
//! It is intentionally a pure data model with no disassembly dependency; the
//! decoder (in `semasm-decode`) and the ABI checks (later slices) build on top of it.

use serde::{Deserialize, Serialize};

/// Bit width of a register view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Width {
    /// 32-bit view (RV32 native, or RV64 `W`-suffix instructions which sign-extend).
    B32,
    /// Full 64-bit register (RV64 native).
    B64,
}

impl Width {
    /// Number of bits in this view.
    #[must_use]
    pub fn bits(self) -> u32 {
        match self {
            Self::B32 => 32,
            Self::B64 => 64,
        }
    }
}

/// A general-purpose register index (one of the 32 architectural registers).
///
/// The RISC-V ABI assigns semantic roles to specific registers:
/// - `x0`  — `zero` (hardwired zero)
/// - `x1`  — `ra`  (return address)
/// - `x2`  — `sp`  (stack pointer)
/// - `x3`  — `gp`  (global pointer)
/// - `x4`  — `tp`  (thread pointer)
/// - `x5`–`x7`  — `t0`–`t2` (temporaries, caller-saved)
/// - `x8`  — `s0`/`fp` (saved register / frame pointer)
/// - `x9`  — `s1` (saved register)
/// - `x10`–`x11` — `a0`–`a1` (arguments/return values)
/// - `x12`–`x17` — `a2`–`a7` (arguments)
/// - `x18`–`x27` — `s2`–`s11` (saved registers, callee-saved)
/// - `x28`–`x31` — `t3`–`t6` (temporaries, caller-saved)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Gpr {
    /// `x0` — hardwired zero register.
    Zero = 0,
    /// `x1` — return address (`ra`).
    Ra = 1,
    /// `x2` — stack pointer (`sp`).
    Sp = 2,
    /// `x3` — global pointer (`gp`).
    Gp = 3,
    /// `x4` — thread pointer (`tp`).
    Tp = 4,
    /// `x5` — temporary `t0`.
    T0 = 5,
    /// `x6` — temporary `t1`.
    T1 = 6,
    /// `x7` — temporary `t2`.
    T2 = 7,
    /// `x8` — saved register `s0` / frame pointer `fp`.
    S0 = 8,
    /// `x9` — saved register `s1`.
    S1 = 9,
    /// `x10` — argument/return `a0`.
    A0 = 10,
    /// `x11` — argument/return `a1`.
    A1 = 11,
    /// `x12` — argument `a2`.
    A2 = 12,
    /// `x13` — argument `a3`.
    A3 = 13,
    /// `x14` — argument `a4`.
    A4 = 14,
    /// `x15` — argument `a5`.
    A5 = 15,
    /// `x16` — argument `a6`.
    A6 = 16,
    /// `x17` — argument `a7`.
    A7 = 17,
    /// `x18` — saved register `s2`.
    S2 = 18,
    /// `x19` — saved register `s3`.
    S3 = 19,
    /// `x20` — saved register `s4`.
    S4 = 20,
    /// `x21` — saved register `s5`.
    S5 = 21,
    /// `x22` — saved register `s6`.
    S6 = 22,
    /// `x23` — saved register `s7`.
    S7 = 23,
    /// `x24` — saved register `s8`.
    S8 = 24,
    /// `x25` — saved register `s9`.
    S9 = 25,
    /// `x26` — saved register `s10`.
    S10 = 26,
    /// `x27` — saved register `s11`.
    S11 = 27,
    /// `x28` — temporary `t3`.
    T3 = 28,
    /// `x29` — temporary `t4`.
    T4 = 29,
    /// `x30` — temporary `t5`.
    T5 = 30,
    /// `x31` — temporary `t6`.
    T6 = 31,
}

impl Gpr {
    /// The canonical 64-bit view of this register.
    #[must_use]
    pub const fn full(self) -> Register {
        Register::gpr(self, Width::B64, false)
    }

    /// The 32-bit view (`W`-register) of this register.
    #[must_use]
    pub const fn low32(self) -> Register {
        Register::gpr(self, Width::B32, false)
    }

    /// The architectural index `0..=31`.
    #[must_use]
    pub const fn index(self) -> u8 {
        self as u8
    }

    /// ABI name (e.g. `ra`, `sp`, `a0`, `s0`, `t0`).
    #[must_use]
    pub const fn abi_name(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::Ra => "ra",
            Self::Sp => "sp",
            Self::Gp => "gp",
            Self::Tp => "tp",
            Self::T0 => "t0",
            Self::T1 => "t1",
            Self::T2 => "t2",
            Self::S0 => "s0",
            Self::S1 => "s1",
            Self::A0 => "a0",
            Self::A1 => "a1",
            Self::A2 => "a2",
            Self::A3 => "a3",
            Self::A4 => "a4",
            Self::A5 => "a5",
            Self::A6 => "a6",
            Self::A7 => "a7",
            Self::S2 => "s2",
            Self::S3 => "s3",
            Self::S4 => "s4",
            Self::S5 => "s5",
            Self::S6 => "s6",
            Self::S7 => "s7",
            Self::S8 => "s8",
            Self::S9 => "s9",
            Self::S10 => "s10",
            Self::S11 => "s11",
            Self::T3 => "t3",
            Self::T4 => "t4",
            Self::T5 => "t5",
            Self::T6 => "t6",
        }
    }
}

/// The storage backing a [`Register`] view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Storage {
    /// A general-purpose register and its sub-views.
    Gpr(Gpr),
}

/// A register view: a storage cell plus a width.
///
/// `x0` is `Storage::Gpr(Zero)` at `B64`; `w0` is `B32`; `sp` is `Storage::Gpr(Sp)` at `B64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Register {
    /// Backing storage.
    pub storage: Storage,
    /// View width.
    pub width: Width,
    /// Reserved for symmetry with the x86 model; always `false` for RISC-V
    /// (no partial-byte views like `AH`).
    pub high: bool,
}

impl Register {
    /// Construct a GPR view.
    #[must_use]
    pub const fn gpr(reg: Gpr, width: Width, high: bool) -> Self {
        Self {
            storage: Storage::Gpr(reg),
            width,
            high,
        }
    }

    /// The zero register (`x0` / `w0`).
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            storage: Storage::Gpr(Gpr::Zero),
            width: Width::B64,
            high: false,
        }
    }

    /// The stack pointer (`x2` / `sp`).
    #[must_use]
    pub const fn sp() -> Self {
        Self {
            storage: Storage::Gpr(Gpr::Sp),
            width: Width::B64,
            high: false,
        }
    }

    /// The return address register (`x1` / `ra`).
    #[must_use]
    pub const fn ra() -> Self {
        Self {
            storage: Storage::Gpr(Gpr::Ra),
            width: Width::B64,
            high: false,
        }
    }

    /// The frame pointer (`x8` / `s0` / `fp`).
    #[must_use]
    pub const fn fp() -> Self {
        Self {
            storage: Storage::Gpr(Gpr::S0),
            width: Width::B64,
            high: false,
        }
    }

    /// The full 64-bit register that contains this view, if it is a GPR view.
    #[must_use]
    pub fn canonical(self) -> Option<Register> {
        match self.storage {
            Storage::Gpr(gpr) => Some(gpr.full()),
        }
    }
}

/// RISC-V **volatile** (caller-saved) GPRs under the LP64 ABI.
///
/// A caller that needs these across a `jalr`/`call` must preserve them itself.
pub const VOLATILE_GPR: &[Gpr] = &[
    Gpr::Ra, // x1
    Gpr::T0, // x5
    Gpr::T1, // x6
    Gpr::T2, // x7
    Gpr::A0, // x10
    Gpr::A1, // x11
    Gpr::A2, // x12
    Gpr::A3, // x13
    Gpr::A4, // x14
    Gpr::A5, // x15
    Gpr::A6, // x16
    Gpr::A7, // x17
    Gpr::T3, // x28
    Gpr::T4, // x29
    Gpr::T5, // x30
    Gpr::T6, // x31
];

/// RISC-V **nonvolatile** (callee-saved) GPRs under the LP64 ABI.
///
/// A callee that uses one must restore it before returning.
/// Includes `sp` (x2), `fp`/`s0` (x8), `s1`–`s11` (x9, x18–x27).
/// Note: `gp` (x3) and `tp` (x4) are treated as reserved/constant across calls
/// in most ABIs; they are not listed here as general callee-saved.
pub const NONVOLATILE_GPR: &[Gpr] = &[
    Gpr::Sp,  // x2
    Gpr::S0,  // x8 (fp)
    Gpr::S1,  // x9
    Gpr::S2,  // x18
    Gpr::S3,  // x19
    Gpr::S4,  // x20
    Gpr::S5,  // x21
    Gpr::S6,  // x22
    Gpr::S7,  // x23
    Gpr::S8,  // x24
    Gpr::S9,  // x25
    Gpr::S10, // x26
    Gpr::S11, // x27
];

/// Integer argument registers in LP64 order (a0–a7).
pub const ARG_REGS: &[Gpr] = &[
    Gpr::A0,
    Gpr::A1,
    Gpr::A2,
    Gpr::A3,
    Gpr::A4,
    Gpr::A5,
    Gpr::A6,
    Gpr::A7,
];

/// Integer/address return register (first result slot).
pub const RETURN_REG: Gpr = Gpr::A0;

/// Required stack alignment in bytes (RISC-V LP64: 16-byte at all times).
pub const STACK_ALIGN: i64 = 16;

/// Whether `reg` is a volatile (caller-saved) GPR under LP64.
#[must_use]
pub fn is_volatile(reg: Gpr) -> bool {
    VOLATILE_GPR.contains(&reg)
}

/// Whether `reg` is a nonvolatile (callee-saved) GPR under LP64.
#[must_use]
pub fn is_nonvolatile(reg: Gpr) -> bool {
    NONVOLATILE_GPR.contains(&reg)
}

pub mod abi;
pub mod lower;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpr_index_matches_encoding() {
        assert_eq!(Gpr::Zero.index(), 0);
        assert_eq!(Gpr::Ra.index(), 1);
        assert_eq!(Gpr::Sp.index(), 2);
        assert_eq!(Gpr::S0.index(), 8);
        assert_eq!(Gpr::A0.index(), 10);
        assert_eq!(Gpr::A7.index(), 17);
        assert_eq!(Gpr::S11.index(), 27);
        assert_eq!(Gpr::T6.index(), 31);
    }

    #[test]
    fn full_and_low32_views() {
        assert_eq!(Gpr::A0.full(), Register::gpr(Gpr::A0, Width::B64, false));
        assert_eq!(Gpr::A0.low32(), Register::gpr(Gpr::A0, Width::B32, false));
    }

    #[test]
    fn abi_names_match_rv64_lp64() {
        assert_eq!(Gpr::Zero.abi_name(), "zero");
        assert_eq!(Gpr::Ra.abi_name(), "ra");
        assert_eq!(Gpr::Sp.abi_name(), "sp");
        assert_eq!(Gpr::Gp.abi_name(), "gp");
        assert_eq!(Gpr::Tp.abi_name(), "tp");
        assert_eq!(Gpr::T0.abi_name(), "t0");
        assert_eq!(Gpr::S0.abi_name(), "s0");
        assert_eq!(Gpr::A0.abi_name(), "a0");
        assert_eq!(Gpr::S2.abi_name(), "s2");
        assert_eq!(Gpr::T3.abi_name(), "t3");
    }

    #[test]
    fn zero_and_sp_are_distinct() {
        assert_ne!(Register::zero(), Register::sp());
        assert_eq!(Register::zero().storage, Storage::Gpr(Gpr::Zero));
        assert_eq!(Register::sp().storage, Storage::Gpr(Gpr::Sp));
    }

    #[test]
    fn volatile_and_nonvolatile_partition() {
        // a0-a7, ra, t0-t6 are volatile
        assert!(is_volatile(Gpr::A0));
        assert!(is_volatile(Gpr::Ra));
        assert!(is_volatile(Gpr::T0));
        assert!(is_volatile(Gpr::T6));
        // sp, fp/s0, s1-s11 are nonvolatile
        assert!(is_nonvolatile(Gpr::Sp));
        assert!(is_nonvolatile(Gpr::S0));
        assert!(is_nonvolatile(Gpr::S11));
        // No register is in both sets.
        for v in VOLATILE_GPR {
            assert!(!is_nonvolatile(*v), "{v:?} must not be nonvolatile");
        }
        for n in NONVOLATILE_GPR {
            assert!(!is_volatile(*n), "{n:?} must not be volatile");
        }
    }

    #[test]
    fn arg_regs_order_matches_lp64() {
        let expected = [
            Gpr::A0,
            Gpr::A1,
            Gpr::A2,
            Gpr::A3,
            Gpr::A4,
            Gpr::A5,
            Gpr::A6,
            Gpr::A7,
        ];
        for (i, g) in expected.iter().enumerate() {
            assert_eq!(argument_register(i), Some(g.full()));
        }
        assert_eq!(argument_register(8), None);
    }

    #[test]
    fn return_reg_is_a0() {
        assert_eq!(return_register(), Gpr::A0.full());
    }
}

/// The integer argument register for the Nth integer argument (0-based).
///
/// Returns `None` once the eight GPR argument registers are exhausted —
/// further arguments are passed on the stack.
#[must_use]
pub fn argument_register(index: usize) -> Option<Register> {
    ARG_REGS.get(index).map(|g| g.full())
}

/// The integer/address return register (`a0` / `x10`).
#[must_use]
pub fn return_register() -> Register {
    RETURN_REG.full()
}
