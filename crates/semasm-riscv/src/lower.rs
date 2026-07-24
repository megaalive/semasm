#![allow(
    clippy::too_many_lines,
    clippy::redundant_closure,
    clippy::match_same_arms
)]
//! Lowering of decoded RISC-V instructions into the architecture-neutral
//! [`semasm_asir::OpKind`] shape used by the analysis and ABI passes.
//!
//! The decoder (Capstone, in `semasm-decode`) emits
//! [`semasm_decode::PhysicalInstruction`] values whose operands are strings in Capstone's
//! `op_str` style (`a0, a1`, `8(sp)`, `0x1000`, ...). This module parses that
//! surface for the fixture instruction subset of RV64-002 and classifies each
//! instruction into an [`OpKind`].
//!
//! Instructions outside the modelled subset return [`Lowering::Unsupported`]
//! so downstream passes can decide how to treat them (e.g. skip in an ABI
//! walk, or surface an explicit "not modelled" note).

use crate::{Gpr, Register, Width};
use semasm_asir::OpKind;
use semasm_decode::PhysicalInstruction;

/// A lowered RISC-V instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredInstr {
    /// Canonical mnemonic (condition suffixes and width letters stripped where
    /// they are encoded in the operand/width instead).
    pub mnemonic: String,
    /// Architecture-neutral operation kind.
    pub kind: OpKind,
    /// View width of the primary register operand (B32 for `W`-family,
    /// B64 otherwise).
    pub width: Width,
    /// Whether the primary operation is signed (used by analysis for
    /// `cmp`/`cmn` signedness; `None` when not applicable).
    pub signed: Option<bool>,
    /// Lowered operands in program order.
    pub operands: Vec<Operand>,
}

/// A lowered operand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    /// A register view.
    Reg(Register),
    /// A memory reference.
    Mem(MemOperand),
    /// An immediate value (already sign/zero resolved by the parser).
    Imm(i64),
}

/// A memory operand: `base` plus optional scaled `index` and byte `disp`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemOperand {
    /// Base register (almost always a GPR or `SP`).
    pub base: Option<Register>,
    /// Scaled index register, if present.
    pub index: Option<Register>,
    /// Index scale (1, 2, 4, 8, ...).
    pub scale: i64,
    /// Signed byte displacement.
    pub disp: i64,
    /// Access width (B32/B64) inferred from the mnemonic size letter.
    pub width: Width,
}

/// Outcome of lowering one decoded instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lowering {
    /// Successfully lowered.
    Lowered(LoweredInstr),
    /// Not part of the modelled subset; carries the original mnemonic so
    /// downstream can report what was skipped.
    Unsupported {
        /// The decoded mnemonic.
        mnemonic: String,
    },
}

/// Lower a single decoded instruction.
///
/// Returns [`Lowering::Unsupported`] for instructions outside the RV64-002
/// fixture subset.
#[must_use]
pub fn lower(p: &PhysicalInstruction) -> Lowering {
    let m = p.mnemonic.as_str();
    let (base, suffix) = split_mnemonic(m);
    // RISC-V 32-bit ops end in 'w' (addw, lw, sw, etc.) or base starts with 'w' (w0)
    let width = if base.ends_with('w') || base.starts_with('w') {
        Width::B32
    } else {
        Width::B64
    };
    let signed = suffix_is_signed(suffix);

    let raw_ops = &p.operands;
    let kind = classify(base);

    match kind {
        None => Lowering::Unsupported {
            mnemonic: p.mnemonic.clone(),
        },
        Some(kind) => {
            let operands = parse_operands(raw_ops, width);
            // RISC-V memory operands are like "8(sp)" not "[...]"
            let has_mem = raw_ops.iter().any(|o| o.contains('(') && o.ends_with(')'));
            let (mnemonic, kind) = if matches!(kind, OpKind::Load | OpKind::Store) && !has_mem {
                ("mv".to_string(), OpKind::Store)
            } else if matches!(kind, OpKind::Return | OpKind::Call) {
                (default_mnemonic(kind), kind)
            } else {
                (base.to_string(), kind)
            };
            Lowering::Lowered(LoweredInstr {
                mnemonic,
                kind,
                width,
                signed,
                operands,
            })
        }
    }
}

/// Lower every decoded instruction, keeping a 1:1 mapping with the decoded
/// list (unsupported ones become [`LoweredInstr`] with `OpKind::Unknown`
/// so CFG instruction indices stay aligned). Mirrors `semasm-x86`'s
/// `lower_keep_all`.
#[must_use]
pub fn lower_keep_all(instrs: &[PhysicalInstruction]) -> Vec<LoweredInstr> {
    instrs
        .iter()
        .map(|p| match lower(p) {
            Lowering::Lowered(l) => l,
            Lowering::Unsupported { mnemonic } => LoweredInstr {
                mnemonic,
                kind: OpKind::Unknown,
                width: Width::B64,
                signed: None,
                operands: Vec::new(),
            },
        })
        .collect()
}

/// Split `b.eq` → (`b`, `eq`); `ldr` → (`ldr`, ``); `movz` → (`movz`, ``).
fn split_mnemonic(m: &str) -> (&str, &str) {
    if let Some(dot) = m.find('.') {
        (&m[..dot], &m[dot + 1..])
    } else {
        (m, "")
    }
}

/// Whether a condition/operation suffix implies signed arithmetic.
fn suffix_is_signed(suffix: &str) -> Option<bool> {
    if suffix.starts_with('s') {
        Some(true)
    } else if suffix.starts_with('u') {
        Some(false)
    } else {
        None
    }
}

/// Map a base mnemonic to an [`OpKind`], or `None` if unmodelled.
fn classify(base: &str) -> Option<OpKind> {
    match base {
        "add" | "sub" | "mul" | "and" | "or" | "xor" | "sll" | "srl" | "sra" => {
            Some(OpKind::Binary)
        }
        "ld" | "lb" | "lh" | "lw" | "ldu" | "lbu" | "lhu" | "lwu" => Some(OpKind::Load),
        "sd" | "sb" | "sh" | "sw" | "mv" => Some(OpKind::Store),
        "addi" | "addiw" | "subw" | "addw" | "andi" | "ori" | "slli" => Some(OpKind::Binary),
        "li" => Some(OpKind::Store),
        "slt" | "sltu" | "slti" | "sltiu" => Some(OpKind::Compare),
        "beq" | "bne" | "blt" | "bge" | "bltu" | "bgeu" | "beqz" | "bnez" | "j" | "jr" => {
            Some(OpKind::Branch)
        }
        "jalr" | "jal" => Some(OpKind::Call), // jalr also used for returns; lower() disambiguates
        "ret" => Some(OpKind::Return),
        "ecall" | "ebreak" => Some(OpKind::Unknown),
        "csrr" | "csrw" | "csrrw" | "csrrs" | "csrrc" | "csrrwi" | "csrrsi" | "csrrci" => {
            Some(OpKind::Unknown)
        }
        _ => None,
    }
}

/// Parse Capstone operand strings into [`Operand`] values.
fn parse_operands(raw: &[String], default_width: Width) -> Vec<Operand> {
    raw.iter()
        .filter_map(|o| parse_one_operand(o, default_width))
        .collect()
}

fn parse_one_operand(o: &str, default_width: Width) -> Option<Operand> {
    let t = o.trim();
    if t.is_empty() {
        return None;
    }
    // Immediate?
    if let Some(imm) = parse_immediate(t) {
        return Some(Operand::Imm(imm));
    }
    // Memory reference `[...]` or RISC-V `offset(base)`?
    if t.starts_with('[') && t.ends_with(']') {
        return parse_memory(t, default_width).map(Operand::Mem);
    }
    // RISC-V style: `8(sp)` or `0x1000(x5)`
    if t.contains('(') && t.ends_with(')') {
        return parse_memory_riscv(t, default_width).map(Operand::Mem);
    }
    // Register?
    parse_register(t, default_width).map(Operand::Reg)
}

fn parse_immediate(t: &str) -> Option<i64> {
    let s = t.trim_start_matches('#').trim();
    let s = s
        .trim_start_matches(':')
        .trim_start_matches("lo12")
        .trim_start_matches("hi21")
        .trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        i64::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<i64>().ok()
    }
}

/// Parse a register name into a [`Register`].
#[allow(clippy::too_many_lines)]
fn parse_register(name: &str, default_width: Width) -> Option<Register> {
    let n = name.trim();
    // Special names.
    match n {
        "zero" | "x0" => return Some(Register::zero()),
        "ra" | "x1" => return Some(Register::ra()),
        "sp" | "x2" => return Some(Register::sp()),
        "gp" | "x3" => return Some(Register::gpr(Gpr::Gp, Width::B64, false)),
        "tp" | "x4" => return Some(Register::gpr(Gpr::Tp, Width::B64, false)),
        "fp" | "s0" | "x8" => return Some(Register::fp()),
        _ => {}
    }
    // `xN` / `wN` (N in 0..=31) and ABI aliases.
    let bytes = n.as_bytes();
    let letter = (*bytes.first()?) as char;
    let letter = letter.to_ascii_lowercase();
    let width = if letter == 'w' {
        Width::B32
    } else {
        default_width
    };
    let idx: u8 = if matches!(letter, 'x' | 'w') {
        n[1..].parse().ok()?
    } else if n == "s1" || n == "x9" {
        9
    } else if n == "a0" || n == "x10" {
        10
    } else if n == "a1" || n == "x11" {
        11
    } else if n == "a2" || n == "x12" {
        12
    } else if n == "a3" || n == "x13" {
        13
    } else if n == "a4" || n == "x14" {
        14
    } else if n == "a5" || n == "x15" {
        15
    } else if n == "a6" || n == "x16" {
        16
    } else if n == "a7" || n == "x17" {
        17
    } else if n == "s2" || n == "x18" {
        18
    } else if n == "s3" || n == "x19" {
        19
    } else if n == "s4" || n == "x20" {
        20
    } else if n == "s5" || n == "x21" {
        21
    } else if n == "s6" || n == "x22" {
        22
    } else if n == "s7" || n == "x23" {
        23
    } else if n == "s8" || n == "x24" {
        24
    } else if n == "s9" || n == "x25" {
        25
    } else if n == "s10" || n == "x26" {
        26
    } else if n == "s11" || n == "x27" {
        27
    } else if n == "t3" || n == "x28" {
        28
    } else if n == "t4" || n == "x29" {
        29
    } else if n == "t5" || n == "x30" {
        30
    } else if n == "t6" || n == "x31" {
        31
    } else if n == "t0" || n == "x5" {
        5
    } else if n == "t1" || n == "x6" {
        6
    } else if n == "t2" || n == "x7" {
        7
    } else {
        return None;
    };
    let gpr = match idx {
        0 => Gpr::Zero,
        1 => Gpr::Ra,
        2 => Gpr::Sp,
        3 => Gpr::Gp,
        4 => Gpr::Tp,
        5 => Gpr::T0,
        6 => Gpr::T1,
        7 => Gpr::T2,
        8 => Gpr::S0,
        9 => Gpr::S1,
        10 => Gpr::A0,
        11 => Gpr::A1,
        12 => Gpr::A2,
        13 => Gpr::A3,
        14 => Gpr::A4,
        15 => Gpr::A5,
        16 => Gpr::A6,
        17 => Gpr::A7,
        18 => Gpr::S2,
        19 => Gpr::S3,
        20 => Gpr::S4,
        21 => Gpr::S5,
        22 => Gpr::S6,
        23 => Gpr::S7,
        24 => Gpr::S8,
        25 => Gpr::S9,
        26 => Gpr::S10,
        27 => Gpr::S11,
        28 => Gpr::T3,
        29 => Gpr::T4,
        30 => Gpr::T5,
        31 => Gpr::T6,
        _ => return None,
    };
    Some(Register::gpr(gpr, width, false))
}

/// Parse a memory operand like `8(sp)`, `0x1000(x5)`, `(sp)`, etc.
fn parse_memory(t: &str, default_width: Width) -> Option<MemOperand> {
    let inner = &t[1..t.len() - 1].trim(); // strip `[` `]`
    if inner.is_empty() {
        return None;
    }
    // Split `disp(base)` or just `base`.
    let (disp_str, base_str): (&str, &str) = if let Some(paren) = inner.find('(') {
        let disp = inner[..paren].trim();
        let base = inner[paren + 1..].strip_suffix(')')?.trim();
        (disp, base)
    } else {
        ("0", inner)
    };
    let disp = parse_immediate(disp_str).unwrap_or(0);
    let base = parse_register(base_str, Width::B64);
    Some(MemOperand {
        base,
        index: None,
        scale: 1,
        disp,
        width: default_width,
    })
}

/// Parse a RISC-V style memory operand like `8(sp)`, `0x1000(x5)`, `(sp)` (no brackets).
fn parse_memory_riscv(t: &str, default_width: Width) -> Option<MemOperand> {
    let inner = t.trim();
    if inner.is_empty() {
        return None;
    }
    // Split `disp(base)` or just `base`.
    let (disp_str, base_str): (&str, &str) = if let Some(paren) = inner.find('(') {
        let disp = inner[..paren].trim();
        let base = inner[paren + 1..].strip_suffix(')')?.trim();
        (disp, base)
    } else {
        ("0", inner)
    };
    let disp = parse_immediate(disp_str).unwrap_or(0);
    let base = parse_register(base_str, Width::B64);
    Some(MemOperand {
        base,
        index: None,
        scale: 1,
        disp,
        width: default_width,
    })
}

/// Canonical mnemonic surfaced for `ret`/`jalr` (whose decoded form is the
/// condition-free base).
fn default_mnemonic(kind: OpKind) -> String {
    match kind {
        OpKind::Return => "ret".to_string(),
        OpKind::Call => "call".to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::{LoweredInstr, Operand};
    use crate::{Gpr, Width};
    use semasm_asir::OpKind as Kind;

    fn dec(mnemonic: &str, operands: &[&str]) -> PhysicalInstruction {
        PhysicalInstruction {
            address: 0,
            bytes: Vec::new(),
            mnemonic: mnemonic.to_string(),
            operands: operands.iter().map(ToString::to_string).collect(),
            read_regs: Vec::new(),
            write_regs: Vec::new(),
            groups: Vec::new(),
            detail_available: false,
        }
    }

    #[test]
    fn add_is_binary() {
        let l = lower(&dec("add", &["a0", "a1", "a2"])).expect_lowered();
        assert_eq!(l.kind, Kind::Binary);
        assert_eq!(l.width, Width::B64);
        assert_eq!(l.operands[0], Operand::Reg(Gpr::A0.full()));
    }

    #[test]
    fn addw_is_binary_32bit() {
        let l = lower(&dec("addw", &["a0", "a1", "a2"])).expect_lowered();
        assert_eq!(l.kind, Kind::Binary);
        assert_eq!(l.width, Width::B32);
    }

    #[test]
    fn ld_is_load_with_memory() {
        let l = lower(&dec("ld", &["a0", "8(sp)"])).expect_lowered();
        assert_eq!(l.kind, Kind::Load);
        assert_eq!(l.operands.len(), 2);
        assert!(matches!(l.operands[1], Operand::Mem(_)));
    }

    #[test]
    fn sd_is_store_with_memory() {
        let l = lower(&dec("sd", &["a0", "8(sp)"])).expect_lowered();
        assert_eq!(l.kind, Kind::Store);
    }

    #[test]
    fn malformed_bracketed_memory_does_not_panic() {
        let l = lower(&dec("ld", &["a0", "[((((((((((((((((((((((((((((((]"])).expect_lowered();
        assert_eq!(l.operands, vec![Operand::Reg(Gpr::A0.full())]);
    }

    #[test]
    fn mv_reg_is_store() {
        let l = lower(&dec("mv", &["a0", "a1"])).expect_lowered();
        assert_eq!(l.kind, Kind::Store);
        assert_eq!(l.operands[0], Operand::Reg(Gpr::A0.full()));
    }

    #[test]
    fn beq_is_branch() {
        let l = lower(&dec("beq", &["a0", "a1", "label"])).expect_lowered();
        assert_eq!(l.kind, Kind::Branch);
    }

    #[test]
    fn beqz_bnez_are_branches() {
        // GNU as / Capstone often emit compressed-zero compares as beqz/bnez.
        let l = lower(&dec("beqz", &["a0", "label"])).expect_lowered();
        assert_eq!(l.kind, Kind::Branch);
        let l = lower(&dec("bnez", &["a1", "label"])).expect_lowered();
        assert_eq!(l.kind, Kind::Branch);
    }

    #[test]
    fn jalr_is_call() {
        let l = lower(&dec("jalr", &["ra", "0(t0)"])).expect_lowered();
        assert_eq!(l.kind, Kind::Call);
    }

    #[test]
    fn ret_is_return() {
        let l = lower(&dec("ret", &[])).expect_lowered();
        assert_eq!(l.kind, Kind::Return);
    }

    #[test]
    fn ecall_is_unknown() {
        let l = lower(&dec("ecall", &[])).expect_lowered();
        assert_eq!(l.kind, Kind::Unknown);
    }

    #[test]
    fn andi_ori_slli_are_binary() {
        let l = lower(&dec("andi", &["a0", "a1", "0xff"])).expect_lowered();
        assert_eq!(l.kind, Kind::Binary);
        let l = lower(&dec("ori", &["a0", "a1", "1"])).expect_lowered();
        assert_eq!(l.kind, Kind::Binary);
        let l = lower(&dec("slli", &["a0", "a1", "3"])).expect_lowered();
        assert_eq!(l.kind, Kind::Binary);
    }

    #[test]
    fn li_is_store_pseudo() {
        let l = lower(&dec("li", &["a0", "42"])).expect_lowered();
        assert_eq!(l.kind, Kind::Store);
        assert_eq!(l.mnemonic, "mv");
        assert_eq!(l.operands[0], Operand::Reg(Gpr::A0.full()));
        assert!(matches!(l.operands[1], Operand::Imm(42)));
    }

    #[test]
    fn sp_and_zero_parse() {
        let l = lower(&dec("addi", &["sp", "sp", "-16"])).expect_lowered();
        assert_eq!(l.operands[0], Operand::Reg(Register::sp()));
        let l2 = lower(&dec("mv", &["a0", "zero"])).expect_lowered();
        assert_eq!(l2.operands[1], Operand::Reg(Register::zero()));
    }

    #[test]
    fn w_register_is_32bit_view() {
        // w10 = a0 (x10), w11 = a1 (x11), w12 = a2 (x12)
        let l = lower(&dec("addw", &["w10", "w11", "w12"])).expect_lowered();
        assert_eq!(l.width, Width::B32);
        assert_eq!(l.operands[0], Operand::Reg(Gpr::A0.low32()));
    }

    #[test]
    fn fence_and_mulh_are_unsupported() {
        assert!(matches!(
            lower(&dec("fence", &[])),
            Lowering::Unsupported { .. }
        ));
        assert!(matches!(
            lower(&dec("mulh", &["a0", "a0", "a1"])),
            Lowering::Unsupported { .. }
        ));
    }

    trait ExpectLowered {
        fn expect_lowered(self) -> LoweredInstr;
    }

    impl ExpectLowered for Lowering {
        fn expect_lowered(self) -> LoweredInstr {
            match self {
                Lowering::Lowered(l) => l,
                Lowering::Unsupported { mnemonic } => panic!("unsupported: {mnemonic}"),
            }
        }
    }
}
