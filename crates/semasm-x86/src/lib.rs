//! x86-64 register and alias model for SemASM.
//!
//! This crate captures the parts of the x86-64 architecture that later slices
//! (ASIR lowering, ABI checks) need to reason about:
//!
//! * the general-purpose register file and its overlapping views
//!   (`RAX`/`EAX`/`AX`/`AL`/`AH`),
//! * the x86 **zero-extension** semantics of 32-bit writes (writing `EAX`
//!   clears the upper 32 bits of `RAX`),
//! * the stack pointer (`RSP`), instruction pointer (`RIP`) and flags
//!   (`RFLAGS`),
//! * the System V AMD64 register classes (volatile / nonvolatile).
//!
//! It is intentionally a pure data model with no disassembly dependency; the
//! decoder (in `semasm-decode`) and the ABI checks (later slices) build on
//! top of it.

use serde::{Deserialize, Serialize};

pub mod abi;
pub mod lower;

/// Bit width of a register view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Width {
    /// 8-bit view (low byte `AL` or high byte `AH`).
    B8,
    /// 16-bit view (`AX`, `CX`, ...).
    B16,
    /// 32-bit view (`EAX`, `ECX`, ...). A write here zero-extends the
    /// containing 64-bit register.
    B32,
    /// Full 64-bit register (`RAX`, `RCX`, ...).
    B64,
}

impl Width {
    /// Number of bits in this view.
    #[must_use]
    pub const fn bits(self) -> u32 {
        match self {
            Width::B8 => 8,
            Width::B16 => 16,
            Width::B32 => 32,
            Width::B64 => 64,
        }
    }
}

/// A general-purpose register (the 64-bit storage cell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Gp {
    /// Accumulator.
    Rax,
    /// Base / callee-saved.
    Rbx,
    /// Counter / argument / volatile.
    Rcx,
    /// Data / argument / volatile.
    Rdx,
    /// Source index / argument / volatile.
    Rsi,
    /// Destination index / argument / volatile.
    Rdi,
    /// Stack pointer (must be preserved by the callee).
    Rsp,
    /// Base pointer / callee-saved.
    Rbp,
    /// Argument / volatile.
    R8,
    /// Argument / volatile.
    R9,
    /// Argument / volatile.
    R10,
    /// Argument / volatile.
    R11,
    /// Callee-saved.
    R12,
    /// Callee-saved.
    R13,
    /// Callee-saved.
    R14,
    /// Callee-saved.
    R15,
}

impl Gp {
    /// The canonical 64-bit view of this register.
    #[must_use]
    pub const fn full(self) -> Register {
        Register::gp(self, Width::B64, false)
    }

    /// The 32-bit view (`EAX`, `ECX`, ...) of this register.
    #[must_use]
    pub const fn low32(self) -> Register {
        Register::gp(self, Width::B32, false)
    }

    /// The 16-bit view (`AX`, `CX`, ...) of this register.
    #[must_use]
    pub const fn low16(self) -> Register {
        Register::gp(self, Width::B16, false)
    }

    /// The low 8-bit view (`AL`) of this register.
    #[must_use]
    pub const fn low8(self) -> Register {
        Register::gp(self, Width::B8, false)
    }

    /// The high 8-bit view (`AH`) of this register.
    #[must_use]
    pub const fn high8(self) -> Register {
        Register::gp(self, Width::B8, true)
    }
}

/// The storage backing a [`Register`] view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Storage {
    /// A general-purpose 64-bit register and its sub-views.
    Gp(Gp),
    /// Instruction pointer.
    Rip,
    /// Flags register.
    Rflags,
    /// Segment register by index (`0` = `ES` ... `5` = `GS`).
    Seg(u8),
    /// XMM vector register by index.
    Xmm(u8),
}

/// A register view: a storage cell plus a width and (for 8-bit) a half.
///
/// `RAX` is `Storage::Gp(Rax)` at `B64`; `EAX` is `B32`; `AX` is
/// `B16`; `AL` is `B8` low; `AH` is `B8` high.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Register {
    /// Backing storage.
    pub storage: Storage,
    /// View width.
    pub width: Width,
    /// For `B8` views: `true` selects the high byte (`AH`), `false` the
    /// low byte (`AL`). Ignored for wider views.
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

    /// The `RAX`/`RBX`/... 64-bit accumulator register.
    #[must_use]
    pub const fn rax() -> Self {
        Gp::Rax.full()
    }
    /// `RSP`, the stack pointer.
    #[must_use]
    pub const fn rsp() -> Self {
        Gp::Rsp.full()
    }
    /// `RBP`, the frame base pointer.
    #[must_use]
    pub const fn rbp() -> Self {
        Gp::Rbp.full()
    }
    /// `RIP`, the instruction pointer.
    #[must_use]
    pub const fn rip() -> Self {
        Self {
            storage: Storage::Rip,
            width: Width::B64,
            high: false,
        }
    }
    /// `RFLAGS`.
    #[must_use]
    pub const fn rflags() -> Self {
        Self {
            storage: Storage::Rflags,
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

    /// The overlapping 32-bit view of a GP register (`EAX` for `RAX`).
    #[must_use]
    pub fn low32(self) -> Option<Register> {
        match self.storage {
            Storage::Gp(gp) => Some(gp.low32()),
            _ => None,
        }
    }

    /// The 16-bit view (`AX`) of a GP register.
    #[must_use]
    pub fn low16(self) -> Option<Register> {
        match self.storage {
            Storage::Gp(gp) => Some(Register::gp(gp, Width::B16, false)),
            _ => None,
        }
    }

    /// The low 8-bit view (`AL`) of a GP register.
    #[must_use]
    pub fn low8(self) -> Option<Register> {
        match self.storage {
            Storage::Gp(gp) => Some(Register::gp(gp, Width::B8, false)),
            _ => None,
        }
    }

    /// The high 8-bit view (`AH`) of a GP register.
    #[must_use]
    pub fn high8(self) -> Option<Register> {
        match self.storage {
            Storage::Gp(gp) => Some(Register::gp(gp, Width::B8, true)),
            _ => None,
        }
    }

    /// Bit range `[start, end)` that this view occupies within its 64-bit
    /// backing cell. Returns `None` for non-GP storage.
    #[must_use]
    pub fn bit_range(self) -> Option<(u32, u32)> {
        let base = match self.storage {
            Storage::Gp(_) => 0,
            Storage::Rip | Storage::Rflags | Storage::Seg(_) | Storage::Xmm(_) => return None,
        };
        let (start, end) = match (self.width, self.high) {
            (Width::B64, _) => (0, 64),
            (Width::B32, _) => (0, 32),
            (Width::B16, _) => (0, 16),
            (Width::B8, false) => (0, 8),
            (Width::B8, true) => (8, 16),
        };
        Some((base + start, base + end))
    }

    /// Whether two views overlap (alias each other) in the same backing cell.
    #[must_use]
    pub fn overlaps(self, other: Register) -> bool {
        if self.storage != other.storage {
            return false;
        }
        match (self.bit_range(), other.bit_range()) {
            (Some((s1, e1)), Some((s2, e2))) => s1 < e2 && s2 < e1,
            _ => false,
        }
    }
}

/// A half-open `[start, end)` bit range within a 64-bit cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BitRange {
    /// Inclusive start bit.
    pub start: u32,
    /// Exclusive end bit.
    pub end: u32,
}

/// Effect of writing a register view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteEffect {
    /// Bits written by the store itself.
    pub written: BitRange,
    /// When `true`, the upper bits `[written.end, 64)` of the backing
    /// 64-bit register are zeroed as part of the write. This is the x86-64
    /// **zero-extension** rule that applies to 32-bit writes.
    pub zero_extend_upper: bool,
}

impl Register {
    /// Compute the write effect of storing into this view.
    ///
    /// * a 32-bit (`EAX`) write additionally zero-extends the upper 32 bits
    ///   of `RAX`;
    /// * 8/16-bit and 64-bit writes affect only their own bit range.
    #[must_use]
    pub fn write_effect(self) -> Option<WriteEffect> {
        let (start, end) = self.bit_range()?;
        let zero_extend_upper = self.width == Width::B32;
        Some(WriteEffect {
            written: BitRange { start, end },
            zero_extend_upper,
        })
    }
}

/// High-level register classes for the x86-64 System V AMD64 ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RegClass {
    /// General-purpose integer register.
    GeneralPurpose,
    /// Flags register.
    Flags,
    /// Instruction pointer.
    InstructionPointer,
    /// Segment register.
    Segment,
    /// XMM vector register.
    Vector,
}

impl Register {
    /// The register class this view belongs to.
    #[must_use]
    pub fn class(self) -> RegClass {
        match self.storage {
            Storage::Gp(_) => RegClass::GeneralPurpose,
            Storage::Rflags => RegClass::Flags,
            Storage::Rip => RegClass::InstructionPointer,
            Storage::Seg(_) => RegClass::Segment,
            Storage::Xmm(_) => RegClass::Vector,
        }
    }
}

/// System V AMD64 **volatile** (caller-saved) GP registers.
///
/// The caller must preserve these across a call; a callee may clobber them.
pub const VOLATILE_GP: &[Gp] = &[
    Gp::Rax,
    Gp::Rcx,
    Gp::Rdx,
    Gp::Rsi,
    Gp::Rdi,
    Gp::R8,
    Gp::R9,
    Gp::R10,
    Gp::R11,
];

/// System V AMD64 **nonvolatile** (callee-saved) GP registers.
///
/// A callee that uses one must restore it before returning. `RSP` is listed
/// for completeness: it must be restored by any function, and is the stack
/// pointer.
pub const NONVOLATILE_GP: &[Gp] = &[
    Gp::Rbx,
    Gp::Rbp,
    Gp::R12,
    Gp::R13,
    Gp::R14,
    Gp::R15,
    Gp::Rsp,
];

/// Whether `reg` is a volatile (caller-saved) GP register under System V.
#[must_use]
pub fn is_volatile(reg: Gp) -> bool {
    VOLATILE_GP.contains(&reg)
}

/// Whether `reg` is a nonvolatile (callee-saved) GP register under System V.
#[must_use]
pub fn is_nonvolatile(reg: Gp) -> bool {
    NONVOLATILE_GP.contains(&reg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gp_alias_relationships() {
        let rax = Gp::Rax.full(); // RAX
        let eax = Gp::Rax.low32(); // EAX
        let ax = rax.low16().unwrap(); // AX
        let al = rax.low8().unwrap(); // AL
        let ah = rax.high8().unwrap(); // AH

        assert_eq!(rax.width, Width::B64);
        assert_eq!(eax.width, Width::B32);
        assert_eq!(ax.width, Width::B16);
        assert_eq!(al.width, Width::B8);
        assert!(!al.high);
        assert!(ah.high);
        assert_eq!(ah.width, Width::B8);

        // Canonical full register is the same storage for all of them.
        assert_eq!(rax.canonical(), Some(rax));
        assert_eq!(eax.canonical(), Some(rax));
        assert_eq!(ah.canonical(), Some(rax));
    }

    #[test]
    fn alias_overlap_rules() {
        let rax = Gp::Rax.full();
        let eax = Gp::Rax.low32();
        let ax = rax.low16().unwrap();
        let al = rax.low8().unwrap();
        let ah = rax.high8().unwrap();

        // EAX overlaps RAX and AX and AL and AH (all share bits 0..32).
        assert!(rax.overlaps(eax));
        assert!(rax.overlaps(ax));
        assert!(rax.overlaps(al));
        assert!(rax.overlaps(ah));
        // AX overlaps AL and AH (both inside 0..16).
        assert!(ax.overlaps(al));
        assert!(ax.overlaps(ah));
        // AL and AH do NOT overlap (bits 0..8 vs 8..16).
        assert!(!al.overlaps(ah));
        // Different registers do not alias.
        assert!(!Gp::Rax.full().overlaps(Gp::Rbx.full()));
    }

    #[test]
    fn eax_zero_extension_clears_upper() {
        // Writing EAX must zero the upper 32 bits of RAX.
        let eax = Gp::Rax.low32();
        let eff = eax.write_effect().expect("gp view");
        assert_eq!((eff.written.start, eff.written.end), (0, 32));
        assert!(eff.zero_extend_upper, "32-bit write must zero-extend");

        // Writing RAX (64-bit) does not zero-extend anything above itself.
        let rax = Gp::Rax.full();
        let eff = rax.write_effect().unwrap();
        assert_eq!((eff.written.start, eff.written.end), (0, 64));
        assert!(!eff.zero_extend_upper);

        // Writing AX (16-bit) does NOT zero-extend; only bits 0..16 written.
        let ax = rax.low16().unwrap();
        let eff = ax.write_effect().unwrap();
        assert_eq!((eff.written.start, eff.written.end), (0, 16));
        assert!(!eff.zero_extend_upper);

        // Writing AL likewise does not zero-extend.
        let al = rax.low8().unwrap();
        let eff = al.write_effect().unwrap();
        assert_eq!((eff.written.start, eff.written.end), (0, 8));
        assert!(!eff.zero_extend_upper);
    }

    #[test]
    fn stack_and_ip_modeled() {
        assert_eq!(Register::rsp().storage, Storage::Gp(Gp::Rsp));
        assert_eq!(Register::rsp().class(), RegClass::GeneralPurpose);
        assert_eq!(Register::rip().storage, Storage::Rip);
        assert_eq!(Register::rip().class(), RegClass::InstructionPointer);
        assert_eq!(Register::rflags().storage, Storage::Rflags);
        assert_eq!(Register::rflags().class(), RegClass::Flags);
        // RSP is the stack pointer and is nonvolatile (callee-saved) in SysV.
        assert!(is_nonvolatile(Gp::Rsp));
        assert!(!is_volatile(Gp::Rsp));
    }

    #[test]
    fn register_classes_documented() {
        // Volatile set.
        for &r in VOLATILE_GP {
            assert!(is_volatile(r));
            assert!(!is_nonvolatile(r));
            assert_eq!(r.full().class(), RegClass::GeneralPurpose);
        }
        // Nonvolatile set (excluding RSP which is shared with the stack).
        for &r in NONVOLATILE_GP {
            assert!(is_nonvolatile(r));
        }
        // The two sets are disjoint.
        for &v in VOLATILE_GP {
            assert!(!NONVOLATILE_GP.contains(&v));
        }
    }

    #[test]
    fn bit_ranges_are_consistent() {
        let rax = Gp::Rax.full();
        assert_eq!(rax.bit_range(), Some((0, 64)));
        assert_eq!(rax.low32().unwrap().bit_range(), Some((0, 32)));
        assert_eq!(rax.low16().unwrap().bit_range(), Some((0, 16)));
        assert_eq!(rax.low8().unwrap().bit_range(), Some((0, 8)));
        assert_eq!(rax.high8().unwrap().bit_range(), Some((8, 16)));
        // Non-GP storage has no bit range.
        assert_eq!(Register::rip().bit_range(), None);
    }
}
