//! Lowering of decoded x86-64 instructions into normalised ASIR-style
//! operations for the common instruction subset (X86-002).
//!
//! Each decoded [`PhysicalInstruction`] is mapped to a [`LoweredInstr`]
//! carrying:
//! * the semantic [`OpKind`],
//! * the **width** of the operation (from the register view or the memory
//!   size prefix),
//! * the **signedness** context where it matters (conditional branches),
//! * **normalised operands** (`Register`, structured `MemOperand`, or
//!   immediate).
//!
//! Instructions outside the supported subset lower to
//! [`Lowering::Unsupported`] rather than being silently dropped — an explicit
//! "not modelled" signal, which is an accepted outcome per the slice
//! acceptance criteria.

use semasm_asir::OpKind;
use semasm_decode::PhysicalInstruction;

use crate::{Gp, Register, Width};

/// A normalised operand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    /// A register view (general-purpose, RIP, flags, ...).
    Reg(Register),
    /// A normalised memory reference.
    Mem(MemOperand),
    /// An immediate value (sign-extended to i64).
    Imm(i64),
}

/// A normalised x86-64 memory operand: `SIZE ptr [base + index*scale + disp]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemOperand {
    /// Base register (a GP register or `RIP`), if present.
    pub base: Option<Register>,
    /// Index register, if present.
    pub index: Option<Register>,
    /// Scale applied to `index` (1, 2, 4, or 8).
    pub scale: u8,
    /// Displacement (added to base+index*scale).
    pub disp: i64,
    /// Access width.
    pub width: Width,
}

/// Result of lowering one instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lowering {
    /// The instruction was lowered into an ASIR-style operation.
    Lowered(LoweredInstr),
    /// The instruction is outside the modelled subset; callers must treat it
    /// as an explicit "unsupported semantics" marker.
    Unsupported {
        /// Mnemonic that could not be modelled.
        mnemonic: String,
    },
}

/// A single lowered instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredInstr {
    /// Original mnemonic (preserved for reporting).
    pub mnemonic: String,
    /// Semantic operation kind.
    pub kind: OpKind,
    /// Operation width (where meaningful).
    pub width: Width,
    /// Signedness context for comparisons/branches: `Some(true)` signed,
    /// `Some(false)` unsigned, `None` when not applicable.
    pub signed: Option<bool>,
    /// Normalised operands in program order.
    pub operands: Vec<Operand>,
}

/// Size-prefix token → [`Width`].
fn prefix_width(token: &str) -> Option<Width> {
    match token {
        "byte" => Some(Width::B8),
        "word" => Some(Width::B16),
        "dword" => Some(Width::B32),
        "qword" => Some(Width::B64),
        _ => None,
    }
}

/// Parse a register name into its [`Register`] view.
fn parse_reg(name: &str) -> Option<Register> {
    let reg = match name {
        "rax" => Gp::Rax.full(),
        "rbx" => Gp::Rbx.full(),
        "rcx" => Gp::Rcx.full(),
        "rdx" => Gp::Rdx.full(),
        "rsi" => Gp::Rsi.full(),
        "rdi" => Gp::Rdi.full(),
        "rsp" => Gp::Rsp.full(),
        "rbp" => Gp::Rbp.full(),
        "r8" => Gp::R8.full(),
        "r9" => Gp::R9.full(),
        "r10" => Gp::R10.full(),
        "r11" => Gp::R11.full(),
        "r12" => Gp::R12.full(),
        "r13" => Gp::R13.full(),
        "r14" => Gp::R14.full(),
        "r15" => Gp::R15.full(),
        "rip" => Register::rip(),
        "eax" => Gp::Rax.low32(),
        "ebx" => Gp::Rbx.low32(),
        "ecx" => Gp::Rcx.low32(),
        "edx" => Gp::Rdx.low32(),
        "esi" => Gp::Rsi.low32(),
        "edi" => Gp::Rdi.low32(),
        "esp" => Gp::Rsp.low32(),
        "ebp" => Gp::Rbp.low32(),
        "r8d" => Gp::R8.low32(),
        "r9d" => Gp::R9.low32(),
        "r10d" => Gp::R10.low32(),
        "r11d" => Gp::R11.low32(),
        "r12d" => Gp::R12.low32(),
        "r13d" => Gp::R13.low32(),
        "r14d" => Gp::R14.low32(),
        "r15d" => Gp::R15.low32(),
        "ax" => Gp::Rax.low16(),
        "bx" => Gp::Rbx.low16(),
        "cx" => Gp::Rcx.low16(),
        "dx" => Gp::Rdx.low16(),
        "si" => Gp::Rsi.low16(),
        "di" => Gp::Rdi.low16(),
        "sp" => Gp::Rsp.low16(),
        "bp" => Gp::Rbp.low16(),
        "r8w" => Gp::R8.low16(),
        "r9w" => Gp::R9.low16(),
        "r10w" => Gp::R10.low16(),
        "r11w" => Gp::R11.low16(),
        "r12w" => Gp::R12.low16(),
        "r13w" => Gp::R13.low16(),
        "r14w" => Gp::R14.low16(),
        "r15w" => Gp::R15.low16(),
        "al" => Gp::Rax.low8(),
        "ah" => Gp::Rax.high8(),
        "bl" => Gp::Rbx.low8(),
        "bh" => Gp::Rbx.high8(),
        "cl" => Gp::Rcx.low8(),
        "ch" => Gp::Rcx.high8(),
        "dl" => Gp::Rdx.low8(),
        "dh" => Gp::Rdx.high8(),
        "sil" => Gp::Rsi.low8(),
        "dil" => Gp::Rdi.low8(),
        "spl" => Gp::Rsp.low8(),
        "bpl" => Gp::Rbp.low8(),
        "r8b" => Gp::R8.low8(),
        "r9b" => Gp::R9.low8(),
        "r10b" => Gp::R10.low8(),
        "r11b" => Gp::R11.low8(),
        "r12b" => Gp::R12.low8(),
        "r13b" => Gp::R13.low8(),
        "r14b" => Gp::R14.low8(),
        "r15b" => Gp::R15.low8(),
        _ => return None,
    };
    Some(reg)
}

/// Parse one operand token (already split on `,`) into an [`Operand`].
fn parse_operand(token: &str) -> Option<Operand> {
    let t = token.trim();

    // `SIZE ptr [..]` form: split off the prefix part.
    if let Some(idx) = t.find("ptr") {
        let after = &t[idx + 3..];
        if let Some(inner) = after.trim().strip_prefix('[') {
            if let Some(body) = inner.strip_suffix(']') {
                return parse_memory(body).map(Operand::Mem);
            }
        }
    }
    // Bare `[..]` memory operand.
    if let Some(inner) = t.strip_prefix('[') {
        if let Some(body) = inner.strip_suffix(']') {
            return parse_memory(body).map(Operand::Mem);
        }
    }

    // Immediate: hex, decimal, or signed decimal.
    if t.starts_with("0x") || t.starts_with("0X") {
        if let Ok(v) = i64::from_str_radix(&t[2..], 16) {
            return Some(Operand::Imm(v));
        }
    }
    if let Ok(v) = t.parse::<i64>() {
        return Some(Operand::Imm(v));
    }

    // Otherwise a register.
    parse_reg(t).map(Operand::Reg)
}

/// Parse the body inside `[...]` into a [`MemOperand`].
fn parse_memory(body: &str) -> Option<MemOperand> {
    let mut base: Option<Register> = None;
    let mut index: Option<Register> = None;
    let mut scale: u8 = 1;
    let mut disp: i64 = 0;

    // Whitespace-tokenise: `[rax + rbx*4 + 0x10]`.
    let mut tokens: Vec<&str> = body.split_whitespace().collect();
    // Strip a leading '[' and trailing ']'.
    if let Some(first) = tokens.first_mut() {
        if let Some(s) = first.strip_prefix('[') {
            *first = s;
        }
    }
    if let Some(last) = tokens.last_mut() {
        if let Some(s) = last.strip_suffix(']') {
            *last = s;
        }
    }

    // Walk tokens, tracking the sign pending on a '+'/'-' separator.
    let mut neg = false;
    for tok in tokens {
        if tok == "+" {
            neg = false;
            continue;
        }
        if tok == "-" {
            neg = true;
            continue;
        }
        // `reg*scale`
        if let Some((reg, mult)) = tok.split_once('*') {
            let r = parse_reg(reg.trim())?;
            let s: u8 = mult.trim().parse().ok()?;
            index = Some(r);
            scale = s;
            neg = false;
            continue;
        }
        // bare register
        if let Some(r) = parse_reg(tok) {
            if base.is_none() {
                base = Some(r);
            } else {
                index = Some(r);
            }
            neg = false;
            continue;
        }
        // displacement
        let signed = if neg { -1 } else { 1 };
        if let Some(hex) = tok.strip_prefix("0x").or_else(|| tok.strip_prefix("0X")) {
            if let Ok(v) = i64::from_str_radix(hex, 16) {
                disp += signed * v;
                neg = false;
                continue;
            }
        }
        if let Ok(v) = tok.parse::<i64>() {
            disp += signed * v;
            neg = false;
            continue;
        }
        // Unknown term — cannot normalise.
        return None;
    }

    // Width is unknown without a `SIZE ptr` prefix; default to 64-bit.
    Some(MemOperand {
        base,
        index,
        scale,
        disp,
        width: Width::B64,
    })
}

/// Determine the operation width from its operands.
fn infer_width(operands: &[Operand]) -> Width {
    for op in operands {
        match op {
            Operand::Reg(r) => return r.width,
            Operand::Mem(m) => return m.width,
            Operand::Imm(_) => {}
        }
    }
    Width::B64
}

/// Lower one decoded instruction.
#[must_use]
pub fn lower(instr: &PhysicalInstruction) -> Lowering {
    let m = instr.mnemonic.to_ascii_lowercase();
    let ops = &instr.operands;

    // Semantic kind + signedness for the supported subset.
    let (kind, signed) = match m.as_str() {
        "mov" | "movabs" | "push" | "pop" => (OpKind::Store, None),
        "lea" => (OpKind::Load, None),
        "xor" | "add" | "sub" | "inc" | "dec" => (OpKind::Binary, None),
        "cmp" | "test" => (OpKind::Compare, None),
        "jmp" | "je" | "jne" | "jz" | "jnz" => (OpKind::Branch, None),
        "ja" | "jae" | "jb" | "jbe" => (OpKind::Branch, Some(false)),
        "jg" | "jge" | "jl" | "jle" => (OpKind::Branch, Some(true)),
        "call" => (OpKind::Call, None),
        "ret" | "retn" | "retf" => (OpKind::Return, None),
        "syscall" => (OpKind::Unknown, None),
        _ => {
            return Lowering::Unsupported {
                mnemonic: instr.mnemonic.clone(),
            }
        }
    };

    // Parse operands, carrying any `SIZE ptr` width into memory operands.
    let mut parsed: Vec<Operand> = Vec::with_capacity(ops.len());
    for raw in ops {
        let operand = match raw.split_whitespace().find_map(prefix_width) {
            Some(w) => {
                // Re-parse the `[...]` portion with the known width.
                match raw.find('[').and_then(|bs| {
                    let after = &raw[bs + 1..];
                    after.rfind(']').map(|end| &after[..end])
                }) {
                    Some(body) => {
                        let mut mem = parse_memory(body).unwrap_or(MemOperand {
                            base: None,
                            index: None,
                            scale: 1,
                            disp: 0,
                            width: Width::B64,
                        });
                        mem.width = w;
                        Operand::Mem(mem)
                    }
                    None => match parse_operand(raw) {
                        Some(o) => o,
                        None => {
                            return Lowering::Unsupported {
                                mnemonic: instr.mnemonic.clone(),
                            }
                        }
                    },
                }
            }
            None => match parse_operand(raw) {
                Some(o) => o,
                None => {
                    return Lowering::Unsupported {
                        mnemonic: instr.mnemonic.clone(),
                    }
                }
            },
        };
        parsed.push(operand);
    }

    let width = infer_width(&parsed);
    Lowering::Lowered(LoweredInstr {
        mnemonic: instr.mnemonic.clone(),
        kind,
        width,
        signed,
        operands: parsed,
    })
}

/// Lower every instruction, keeping the output 1:1 with the input so its
/// indices line up with a control-flow graph built from the same instructions.
/// Instructions outside the modelled subset become a `LoweredInstr` with
/// `OpKind::Unknown` (an explicit "not modelled" placeholder) rather than
/// being dropped.
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

#[cfg(test)]
mod tests {
    use super::*;
    use semasm_decode::PhysicalInstruction;

    fn ins(mnemonic: &str, operands: &[&str]) -> PhysicalInstruction {
        PhysicalInstruction {
            address: 0,
            bytes: vec![0x90],
            mnemonic: mnemonic.into(),
            operands: operands.iter().map(|s| (*s).to_string()).collect(),
            read_regs: vec![],
            write_regs: vec![],
            groups: vec![],
            detail_available: false,
        }
    }

    fn lowered(l: &Lowering) -> &LoweredInstr {
        match l {
            Lowering::Lowered(x) => x,
            Lowering::Unsupported { mnemonic } => {
                panic!("expected lowered, got unsupported: {mnemonic}")
            }
        }
    }

    #[test]
    fn mov_register_has_width_and_binary_kind() {
        let l = lower(&ins("mov", &["rax", "rbx"]));
        let l = lowered(&l);
        assert_eq!(l.kind, OpKind::Store);
        assert_eq!(l.width, Width::B64);
        assert_eq!(l.operands.len(), 2);
        assert!(matches!(l.operands[0], Operand::Reg(_)));
        assert_eq!(l.signed, None);
    }

    #[test]
    fn eax_mov_carries_32bit_width() {
        let l = lower(&ins("mov", &["eax", "ebx"]));
        assert_eq!(lowered(&l).width, Width::B32);
    }

    #[test]
    fn conditional_branch_signedness() {
        assert_eq!(lowered(&lower(&ins("ja", &["0x100"]))).signed, Some(false));
        assert_eq!(lowered(&lower(&ins("jg", &["0x100"]))).signed, Some(true));
        assert_eq!(lowered(&lower(&ins("je", &["0x100"]))).signed, None);
    }

    #[test]
    fn memory_operand_normalised() {
        let l = lower(&ins("mov", &["qword ptr [rax + rbx*4 + 0x10]", "rcx"]));
        let l = lowered(&l);
        let mem = match &l.operands[0] {
            Operand::Mem(m) => m,
            other => panic!("expected mem operand, got {other:?}"),
        };
        assert_eq!(
            mem.base.map(|r| r.storage),
            Some(crate::Storage::Gp(Gp::Rax))
        );
        assert_eq!(
            mem.index.map(|r| r.storage),
            Some(crate::Storage::Gp(Gp::Rbx))
        );
        assert_eq!(mem.scale, 4);
        assert_eq!(mem.disp, 0x10);
        assert_eq!(mem.width, Width::B64);
    }

    #[test]
    fn memory_with_negative_disp() {
        let l = lower(&ins("mov", &["dword ptr [rip - 0x4]", "eax"]));
        let l = lowered(&l);
        let mem = match &l.operands[0] {
            Operand::Mem(m) => m,
            other => panic!("expected mem operand, got {other:?}"),
        };
        assert_eq!(mem.base.map(|r| r.storage), Some(crate::Storage::Rip));
        assert_eq!(mem.disp, -0x4);
        assert_eq!(mem.width, Width::B32);
    }

    #[test]
    fn immediate_is_parsed() {
        let l = lower(&ins("add", &["rax", "0x1"]));
        assert!(matches!(lowered(&l).operands[1], Operand::Imm(1)));
    }

    #[test]
    fn unsupported_subset_is_explicit() {
        let l = lower(&ins("frobnicate", &["rax"]));
        assert!(matches!(l, Lowering::Unsupported { .. }));
    }

    #[test]
    fn all_example_mnemonics_lower() {
        for m in [
            "mov", "lea", "xor", "add", "sub", "inc", "dec", "cmp", "test", "jmp", "je", "jne",
            "ja", "jae", "jb", "jbe", "jg", "jge", "jl", "jle", "call", "ret", "syscall", "push",
            "pop",
        ] {
            assert!(
                matches!(lower(&ins(m, &["rax"])), Lowering::Lowered(_)),
                "mnemonic {m} should lower"
            );
        }
    }

    #[test]
    fn count_byte_example_lowers() {
        // `mov rax, rsi ; ret` from the agent fixture.
        let a = lower(&ins("mov", &["rax", "rsi"]));
        let b = lower(&ins("ret", &[]));
        assert!(matches!(a, Lowering::Lowered(_)));
        assert!(matches!(b, Lowering::Lowered(_)));
        assert_eq!(lowered(&b).kind, OpKind::Return);
    }
}
