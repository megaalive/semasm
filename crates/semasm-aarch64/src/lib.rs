//! AArch64 register and alias model for SemASM.
//!
//! This crate captures the parts of the AArch64 (ARMv8-A) architecture that
//! later slices (ASIR lowering, AAPCS64 checks) need to reason about:
//!
//! * the 31 general-purpose registers `X0`–`X30` and their 32-bit
//!   views `W0`–`W30` (a write to `Wn` zero-extends the upper 32 bits
//!   of `Xn`, mirroring x86-64 `EAX` semantics),
//! * the stack pointer `SP` (a distinct 64-bit cell, not `X31`),
//! * the zero register `XZR`/`WZR` (`X31` in encoding — reads as 0,
//!   writes are discarded),
//! * the condition flags `NZCV`.
//!
//! `X30` is the link register (`LR`); `X29` is the frame record pointer
//! (`FP`). The preserve/volatile split follows AAPCS64.
//!
//! It is intentionally a pure data model with no disassembly dependency; the
//! decoder (in `semasm-decode`) and the ABI checks (later slices) build on
//! top of it.

use serde::{Deserialize, Serialize};

/// Bit width of a register view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Width {
    /// 8-bit view (no architecturally named AArch64 byte regisers; kept
    /// for compatibility with the shared `Storage` model).
    B8,
    /// 32-bit view (`W0`–`W30`, `WZR`).
    B32,
    /// Full 64-bit register (`X0`–`X30`, `SP`, `XZR`).
    B64,
}

impl Width {
    /// Number of bits in this view.
    #[must_use]
    pub fn bits(self) -> u32 {
        match self {
            Self::B8 => 8,
            Self::B32 => 32,
            Self::B64 => 64,
        }
    }
}

/// A general-purpose register (one of the 31 allocatable cells, or the
/// zero register). `X30` is `Lr` and `X29` is `Fp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Gp {
    /// `X0` — 1st integer argument / result.
    X0,
    /// `X1` — 2nd integer argument / result.
    X1,
    /// `X2` — 3rd integer argument / result.
    X2,
    /// `X3` — 4th integer argument / result.
    X3,
    /// `X4` — 5th integer argument.
    X4,
    /// `X5` — 6th integer argument.
    X5,
    /// `X6` — 7th integer argument.
    X6,
    /// `X7` — 8th integer argument.
    X7,
    /// `X8` — indirect result location / 9th argument (caller-saved).
    X8,
    /// `X9` — caller-saved scratch.
    X9,
    /// `X10` — caller-saved scratch.
    X10,
    /// `X11` — caller-saved scratch.
    X11,
    /// `X12` — caller-saved scratch.
    X12,
    /// `X13` — caller-saved scratch.
    X13,
    /// `X14` — caller-saved scratch.
    X14,
    /// `X15` — caller-saved scratch (IP0).
    X15,
    /// `X16` — caller-saved scratch (IP1).
    X16,
    /// `X17` — caller-saved scratch (IP2).
    X17,
    /// `X18` — platform register (treated caller-saved here).
    X18,
    /// `X19` — callee-saved.
    X19,
    /// `X20` — callee-saved.
    X20,
    /// `X21` — callee-saved.
    X21,
    /// `X22` — callee-saved.
    X22,
    /// `X23` — callee-saved.
    X23,
    /// `X24` — callee-saved.
    X24,
    /// `X25` — callee-saved.
    X25,
    /// `X26` — callee-saved.
    X26,
    /// `X27` — callee-saved.
    X27,
    /// `X28` — callee-saved.
    X28,
    /// `X29` — frame record pointer (`FP`).
    Fp,
    /// `X30` — link register (`LR`).
    Lr,
    /// `X31` — the zero register (`XZR`/`WZR`). Reads as 0; writes
    /// are discarded.
    Zr,
}

impl Gp {
    /// The canonical 64-bit view of this register.
    #[must_use]
    pub const fn full(self) -> Register {
        Register::gp(self, Width::B64, false)
    }

    /// The 32-bit view (`W0`–`W30`, `WZR`) of this register.
    #[must_use]
    pub const fn low32(self) -> Register {
        Register::gp(self, Width::B32, false)
    }

    /// The numeric index `0..=31` this register maps to in the AArch64
    /// encoding (`X0`→0 … `X30`→30, `Zr`→31, `Fp`→29, `Lr`→30).
    #[must_use]
    pub const fn index(self) -> u8 {
        match self {
            Self::X0 => 0,
            Self::X1 => 1,
            Self::X2 => 2,
            Self::X3 => 3,
            Self::X4 => 4,
            Self::X5 => 5,
            Self::X6 => 6,
            Self::X7 => 7,
            Self::X8 => 8,
            Self::X9 => 9,
            Self::X10 => 10,
            Self::X11 => 11,
            Self::X12 => 12,
            Self::X13 => 13,
            Self::X14 => 14,
            Self::X15 => 15,
            Self::X16 => 16,
            Self::X17 => 17,
            Self::X18 => 18,
            Self::X19 => 19,
            Self::X20 => 20,
            Self::X21 => 21,
            Self::X22 => 22,
            Self::X23 => 23,
            Self::X24 => 24,
            Self::X25 => 25,
            Self::X26 => 26,
            Self::X27 => 27,
            Self::X28 => 28,
            Self::Fp => 29,
            Self::Lr => 30,
            Self::Zr => 31,
        }
    }
}

/// The storage backing a [`Register`] view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Storage {
    /// A general-purpose register and its sub-views.
    Gp(Gp),
    /// The stack pointer (`SP`), a distinct 64-bit cell.
    Sp,
    /// The condition flags (`NZCV`).
    Nzcv,
}

/// A register view: a storage cell plus a width.
///
/// `X0` is `Storage::Gp(X0)` at `B64`; `W0` is `B32`; `SP` is
/// `Storage::Sp` at `B64`; `XZR` is `Storage::Gp(Zr)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Register {
    /// Backing storage.
    pub storage: Storage,
    /// View width.
    pub width: Width,
    /// Reserved for symmetry with the x86 model; always `false` for
    /// AArch64 (no partial-byte views).
    pub high: bool,
}

impl Register {
    /// Construct a GP register view.
    #[must_use]
    pub const fn gp(reg: Gp, width: Width, high: bool) -> Self {
        Self {
            storage: Storage::Gp(reg),
            width,
            high,
        }
    }

    /// `SP`, the stack pointer.
    #[must_use]
    pub const fn sp() -> Self {
        Self {
            storage: Storage::Sp,
            width: Width::B64,
            high: false,
        }
    }

    /// `XZR`/`WZR`, the zero register.
    #[must_use]
    pub const fn zr() -> Self {
        Self {
            storage: Storage::Gp(Gp::Zr),
            width: Width::B64,
            high: false,
        }
    }

    /// `NZCV`, the condition flags.
    #[must_use]
    pub const fn nzcv() -> Self {
        Self {
            storage: Storage::Nzcv,
            width: Width::B64,
            high: false,
        }
    }

    /// The full 64-bit register that contains this view, if it is a GP view.
    #[must_use]
    pub fn canonical(self) -> Option<Register> {
        match self.storage {
            Storage::Gp(gp) => Some(gp.full()),
            _ => None,
        }
    }
}

/// AArch64 **volatile** (caller-saved) GP registers under AAPCS64.
///
/// A caller that needs these across a `bl` must preserve them itself.
pub const VOLATILE_GP: &[Gp] = &[
    Gp::X0,
    Gp::X1,
    Gp::X2,
    Gp::X3,
    Gp::X4,
    Gp::X5,
    Gp::X6,
    Gp::X7,
    Gp::X8,
    Gp::X9,
    Gp::X10,
    Gp::X11,
    Gp::X12,
    Gp::X13,
    Gp::X14,
    Gp::X15,
    Gp::X16,
    Gp::X17,
    Gp::X18,
];

/// AArch64 **nonvolatile** (callee-saved) GP registers under AAPCS64.
///
/// A callee that uses one must restore it before returning. `X29` (`FP`)
/// and `X30` (`LR`) are included: the callee must preserve the incoming
/// `LR` (typically by saving it) and may use `FP` as a frame pointer.
pub const NONVOLATILE_GP: &[Gp] = &[
    Gp::X19,
    Gp::X20,
    Gp::X21,
    Gp::X22,
    Gp::X23,
    Gp::X24,
    Gp::X25,
    Gp::X26,
    Gp::X27,
    Gp::X28,
    Gp::Fp,
    Gp::Lr,
];

/// Whether `reg` is a volatile (caller-saved) GP register under AAPCS64.
#[must_use]
pub fn is_volatile(reg: Gp) -> bool {
    VOLATILE_GP.contains(&reg)
}

/// Whether `reg` is a nonvolatile (callee-saved) GP register under
/// AAPCS64 (including `FP`/`LR`).
#[must_use]
pub fn is_nonvolatile(reg: Gp) -> bool {
    NONVOLATILE_GP.contains(&reg)
}

pub mod abi;
pub mod lower;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gp_index_matches_encoding() {
        assert_eq!(Gp::X0.index(), 0);
        assert_eq!(Gp::Lr.index(), 30);
        assert_eq!(Gp::Lr.index(), 30);
        assert_eq!(Gp::Fp.index(), 29);
        assert_eq!(Gp::Zr.index(), 31);
    }

    #[test]
    fn full_and_low32_views() {
        assert_eq!(Gp::X0.full(), Register::gp(Gp::X0, Width::B64, false));
        assert_eq!(Gp::X0.low32(), Register::gp(Gp::X0, Width::B32, false));
    }

    #[test]
    fn sp_and_zr_are_distinct() {
        assert_ne!(Register::sp(), Register::zr());
        assert_eq!(Register::sp().storage, Storage::Sp);
        assert_eq!(Register::zr().storage, Storage::Gp(Gp::Zr));
    }

    #[test]
    fn volatile_and_nonvolatile_partition() {
        // X0-X18 are volatile; X19-X28, FP, LR are nonvolatile.
        assert!(is_volatile(Gp::X0));
        assert!(is_volatile(Gp::X18));
        assert!(!is_volatile(Gp::X19));
        assert!(is_nonvolatile(Gp::X19));
        assert!(is_nonvolatile(Gp::Lr));
        // No register is in both sets.
        for v in VOLATILE_GP {
            assert!(!is_nonvolatile(*v), "{v:?} must not be nonvolatile");
        }
        for n in NONVOLATILE_GP {
            assert!(!is_volatile(*n), "{n:?} must not be volatile");
        }
    }
}
