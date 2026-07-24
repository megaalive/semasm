//! Lowering of decoded AArch64 instructions into the architecture-neutral
//! [`semasm_asir::OpKind`] shape used by the analysis and ABI passes.
//!
//! The decoder (Capstone, in `semasm-decode`) emits
//! [`semasm_decode::PhysicalInstruction`] values whose operands are strings in Capstone's
//! `op_str` style (`x0, [x1, #8]`, `b.eq #imm`, ...). This module
//! parses that surface for the fixture instruction subset of A64-002 and
//! classifies each instruction into an [`OpKind`].
//!
//! Instructions outside the modelled subset return [`Lowering::Unsupported`]
//! so downstream passes can decide how to treat them (e.g. skip in an ABI
//! walk, or surface an explicit "not modelled" note).

use crate::{Gp, Register, Width};
use semasm_asir::OpKind;

/// A lowered AArch64 instruction.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemOperand {
    /// Base register (almost always a GP register or `SP`).
    pub base: Option<Register>,
    /// Scaled index register, if present.
    pub index: Option<Register>,
    /// Index scale (1, 2, 4, 8, ...).
    pub scale: i64,
    /// Signed byte displacement.
    pub disp: i64,
    /// Access width (B8/B32/B64) inferred from the mnemonic size letter.
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

/// Lower a single decoded AArch64 instruction.
///
/// Returns [`Lowering::Unsupported`] for instructions outside the A64-002
/// fixture subset.
#[must_use]
pub fn lower(p: &semasm_decode::PhysicalInstruction) -> Lowering {
    let m = p.mnemonic.as_str();
    // Split a mnemonic like `b.eq` or `ldrb` into (base, suffix).
    let (base, suffix) = split_mnemonic(m);
    let width = if base.starts_with('w') || base == "movz" && m.starts_with("movz w") {
        Width::B32
    } else {
        Width::B64
    };
    let signed = suffix_is_signed(suffix);

    // Operands (already comma-split by the decoder).
    let raw_ops = &p.operands;

    let kind = classify(base);
    match kind {
        None => Lowering::Unsupported {
            mnemonic: p.mnemonic.clone(),
        },
        Some(kind) => {
            // Parse operands first so we can refine the operation width from
            // the actual register views (e.g. `w0` ⇒ B32).
            let operands = parse_operands(raw_ops, width);
            let width = operand_width(&operands).unwrap_or(width);
            // `mov`/equivalents without a memory operand are data moves.
            let (mnemonic, kind) = if matches!(kind, OpKind::Load | OpKind::Store)
                && !raw_ops.iter().any(|o| o.starts_with('['))
            {
                ("mov".to_string(), OpKind::Store)
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

/// Refine the operation width from the first register-typed operand, so a
/// 32-bit form (`w0`) yields `B32` while `x0`/`sp`/`xzr` yield `B64`.
fn operand_width(ops: &[Operand]) -> Option<Width> {
    ops.iter().find_map(|o| match o {
        Operand::Reg(r) => Some(r.width),
        Operand::Mem(m) => Some(m.width),
        Operand::Imm(_) => None,
    })
}

/// Lower every decoded instruction, keeping a 1:1 mapping with the decoded
/// list (unsupported ones become [`LoweredInstr`] with `OpKind::Unknown`
/// so CFG instruction indices stay aligned). Mirrors `semasm-x86`'s
/// `lower_keep_all`.
#[must_use]
pub fn lower_keep_all(instrs: &[semasm_decode::PhysicalInstruction]) -> Vec<LoweredInstr> {
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

/// Split `b.eq` → (`b`, `eq`); `ldrb` → (`ldrb`, ``); `movz` → (`movz`, ``).
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

/// Canonical mnemonic surfaced for `ret`/`bl` (whose decoded form is the
/// condition-free base).
fn default_mnemonic(kind: OpKind) -> String {
    match kind {
        OpKind::Return => "ret".to_string(),
        OpKind::Call => "bl".to_string(),
        _ => String::new(),
    }
}

/// Map a base mnemonic to an [`OpKind`], or `None` if unmodelled.
fn classify(base: &str) -> Option<OpKind> {
    match base {
        "mov" | "movz" | "movk" | "movn" | "str" | "strb" | "strh" => Some(OpKind::Store),
        "ldr" | "ldrb" | "ldrh" | "ldrsb" | "ldrsh" | "ldrsw" => Some(OpKind::Load),
        "add" | "adds" | "sub" | "subs" | "mul" | "and" | "orr" | "eor" | "lsl" | "lsr" => {
            Some(OpKind::Binary)
        }
        "cmp" | "cmn" | "tst" => Some(OpKind::Compare),
        "b" | "br" | "cbz" | "cbnz" | "tbz" | "tbnz" => Some(OpKind::Branch),
        "bl" | "blr" => Some(OpKind::Call),
        "ret" => Some(OpKind::Return),
        "svc" | "hvc" | "smc" | "brk" | "hlt" => Some(OpKind::Unknown),
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
    if t.starts_with('[') {
        return parse_memory(t, default_width).map(Operand::Mem);
    }
    if let Some(imm) = parse_immediate(t) {
        return Some(Operand::Imm(imm));
    }
    // Otherwise a register name.
    parse_register(t, default_width).map(Operand::Reg)
}

/// Parse an immediate: `#123`, `#0x1f`, `#imm`, or a bare `123`/`0x1f`.
fn parse_immediate(t: &str) -> Option<i64> {
    let s = t.trim_start_matches('#').trim();
    let s = s.trim_start_matches(':'); // `:lo12:` style relocs
    let s = s
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
fn parse_register(name: &str, default_width: Width) -> Option<Register> {
    let n = name.trim();
    // Special names.
    match n {
        "sp" | "SP" => return Some(Register::sp()),
        "xzr" | "XZR" | "wzr" | "WZR" => return Some(Register::zr()),
        _ => {}
    }
    // `xN` / `wN` (N in 0..=30) and `fp`/`lr`.
    let bytes = n.as_bytes();
    let letter = (*bytes.first()?) as char;
    let letter = letter.to_ascii_lowercase();
    let width = if letter == 'w' {
        Width::B32
    } else {
        default_width
    };
    let idx: u8 = if matches!(letter, 'x' | 'w') {
        let num: u8 = n[1..].parse().ok()?;
        num
    } else if n == "fp" || n == "FP" {
        29
    } else if n == "lr" || n == "LR" {
        30
    } else {
        return None;
    };
    let gp = match idx {
        0 => Gp::X0,
        1 => Gp::X1,
        2 => Gp::X2,
        3 => Gp::X3,
        4 => Gp::X4,
        5 => Gp::X5,
        6 => Gp::X6,
        7 => Gp::X7,
        8 => Gp::X8,
        9 => Gp::X9,
        10 => Gp::X10,
        11 => Gp::X11,
        12 => Gp::X12,
        13 => Gp::X13,
        14 => Gp::X14,
        15 => Gp::X15,
        16 => Gp::X16,
        17 => Gp::X17,
        18 => Gp::X18,
        19 => Gp::X19,
        20 => Gp::X20,
        21 => Gp::X21,
        22 => Gp::X22,
        23 => Gp::X23,
        24 => Gp::X24,
        25 => Gp::X25,
        26 => Gp::X26,
        27 => Gp::X27,
        28 => Gp::X28,
        29 => Gp::Fp,
        30 => Gp::Lr,
        31 => Gp::Zr,
        _ => return None,
    };
    Some(Register::gp(gp, width, false))
}

/// Parse a memory operand `[base]`, `[base, #off]`, `[base, idx]`,
/// `[base, idx, lsl #3]`, or `[base, #off]!` (writeback ignored).
fn parse_memory(t: &str, default_width: Width) -> Option<MemOperand> {
    let inner = t
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('!')
        .trim();
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if parts.is_empty() {
        return None;
    }
    let base = parse_register(parts[0], default_width);
    let mut index = None;
    let mut scale = 1i64;
    let mut disp = 0i64;
    for p in &parts[1..] {
        if let Some(imm) = parse_immediate(p) {
            disp = imm;
        } else if let Some(reg) = parse_register(p, default_width) {
            // `lsl #3` style scale is folded into a later part.
            index = Some(reg);
        } else if let Some(rest) = p.strip_prefix("lsl") {
            if let Some(s) = parse_immediate(rest) {
                scale = s.max(1);
            }
        }
    }
    let width = match t {
        s if s.contains("b]") && !s.contains("sb]") && !s.contains("sh]") => Width::B8,
        s if s.contains("h]") => Width::B32,
        _ => default_width,
    };
    Some(MemOperand {
        base,
        index,
        scale,
        disp,
        width,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(mnemonic: &str, operands: &[&str]) -> semasm_decode::PhysicalInstruction {
        semasm_decode::PhysicalInstruction {
            address: 0,
            bytes: Vec::new(),
            mnemonic: mnemonic.to_string(),
            operands: operands.iter().map(|s| (*s).to_string()).collect(),
            read_regs: Vec::new(),
            write_regs: Vec::new(),
            groups: Vec::new(),
            detail_available: false,
        }
    }

    #[test]
    fn mov_reg_is_store() {
        let l = lower(&dec("mov", &["x0", "x1"])).expect_lowered();
        assert_eq!(l.kind, OpKind::Store);
        assert_eq!(l.operands.len(), 2);
    }

    #[test]
    fn ldr_is_load_with_memory() {
        let l = lower(&dec("ldr", &["x0", "[x1, #8]"])).expect_lowered();
        assert_eq!(l.kind, OpKind::Load);
        match &l.operands[1] {
            Operand::Mem(m) => {
                assert_eq!(m.base, Some(Gp::X1.full()));
                assert_eq!(m.disp, 8);
            }
            other => panic!("expected mem, got {other:?}"),
        }
    }

    #[test]
    fn add_is_binary() {
        let l = lower(&dec("add", &["x0", "x1", "x2"])).expect_lowered();
        assert_eq!(l.kind, OpKind::Binary);
    }

    #[test]
    fn cmp_is_compare() {
        let l = lower(&dec("cmp", &["x0", "x1"])).expect_lowered();
        assert_eq!(l.kind, OpKind::Compare);
    }

    #[test]
    fn b_cond_is_branch() {
        let l = lower(&dec("b.eq", &["#0x10"])).expect_lowered();
        assert_eq!(l.kind, OpKind::Branch);
        assert_eq!(l.mnemonic, "b");
    }

    #[test]
    fn bl_is_call() {
        let l = lower(&dec("bl", &["#0x1000"])).expect_lowered();
        assert_eq!(l.kind, OpKind::Call);
    }

    #[test]
    fn ret_is_return() {
        let l = lower(&dec("ret", &[])).expect_lowered();
        assert_eq!(l.kind, OpKind::Return);
    }

    #[test]
    fn svc_is_unknown() {
        let l = lower(&dec("svc", &["#0"])).expect_lowered();
        assert_eq!(l.kind, OpKind::Unknown);
    }

    #[test]
    fn cbz_cbnz_are_branches() {
        let l = lower(&dec("cbz", &["x0", "#0x10"])).expect_lowered();
        assert_eq!(l.kind, OpKind::Branch);
        assert_eq!(l.mnemonic, "cbz");
        let l = lower(&dec("cbnz", &["w1", "#0x20"])).expect_lowered();
        assert_eq!(l.kind, OpKind::Branch);
        assert_eq!(l.mnemonic, "cbnz");
        assert_eq!(l.width, Width::B32);
    }

    #[test]
    fn adds_subs_are_binary() {
        let l = lower(&dec("adds", &["x0", "x1", "x2"])).expect_lowered();
        assert_eq!(l.kind, OpKind::Binary);
        assert_eq!(l.mnemonic, "adds");
        let l = lower(&dec("subs", &["x1", "x1", "#1"])).expect_lowered();
        assert_eq!(l.kind, OpKind::Binary);
        assert_eq!(l.mnemonic, "subs");
    }

    #[test]
    fn sp_and_zr_parse() {
        let l = lower(&dec("add", &["sp", "sp", "#16"])).expect_lowered();
        assert_eq!(l.operands[0], Operand::Reg(Register::sp()));
        let l2 = lower(&dec("mov", &["x0", "xzr"])).expect_lowered();
        assert_eq!(l2.operands[1], Operand::Reg(Register::zr()));
    }

    #[test]
    fn w_register_is_32bit_view() {
        let l = lower(&dec("mov", &["w0", "w1"])).expect_lowered();
        assert_eq!(l.width, Width::B32);
        assert_eq!(l.operands[0], Operand::Reg(Gp::X0.low32()));
    }

    #[test]
    fn fmov_and_mrs_are_unsupported() {
        assert!(matches!(
            lower(&dec("fmov", &["d0", "d0"])),
            Lowering::Unsupported { .. }
        ));
        assert!(matches!(
            lower(&dec("mrs", &["x0", "nzcv"])),
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
                Lowering::Unsupported { mnemonic } => panic!("unexpected unsupported: {mnemonic}"),
            }
        }
    }
}
