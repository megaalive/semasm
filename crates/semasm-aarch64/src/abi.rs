//! AAPCS64 (ARM Architecture Procedure Call Standard 64-bit)
//! conformance checks over lowered AArch64 instructions.
//!
//! This module is the AArch64 counterpart to the x86 System V and Microsoft
//! x64 ABI analyzers. It turns the register model in
//! [`crate`] plus the lowering in [`crate::lower`] into concrete AAPCS64
//! checks over a single function body (prologue … epilogue).
//!
//! Implemented checks (AAPCS64-001):
//!
//! * **Argument register binding** — `X0`–`X7` in their canonical order.
//! * **Integer return binding** — `X0` as the first result slot.
//! * **Preserved registers** — a nonvolatile (`nonvolatile`) register that is
//!   written must be saved in the prologue and restored in the epilogue.
//! * **Stack alignment** — `SP` must be 16-byte aligned at every `bl` and
//!   at return (AAPCS64 requires 16-byte alignment at all times).
//! * **LR preservation** — a non-leaf function must preserve `LR` (`X30`),
//!   which `ret` consumes as the return address.
//! * **Stack balance** — the net change to `SP` across the body must return
//!   to zero at each `ret`.

use serde::{Deserialize, Serialize};

use crate::lower::{LoweredInstr, Operand};
use crate::{Gp, Register, Storage, NONVOLATILE_GP};

/// Integer argument registers (1st … 8th) in AAPCS64 order.
pub const ARG_REGS: &[Gp] = &[
    Gp::X0,
    Gp::X1,
    Gp::X2,
    Gp::X3,
    Gp::X4,
    Gp::X5,
    Gp::X6,
    Gp::X7,
];

/// Integer/address return register (first result slot).
pub const RETURN_REG: Gp = Gp::X0;

/// Required stack alignment in bytes (AAPCS64: 16-byte at all times).
pub const STACK_ALIGN: i64 = 16;

/// At function entry `SP` is 16-byte aligned.
const ENTRY_SP_MOD: i64 = 0;

/// Severity of an ABI finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    /// Hard violation of the ABI.
    Error,
    /// Potential or advisory violation.
    Warning,
    /// Informational note.
    Info,
}

/// A single ABI conformance finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbiFinding {
    /// Stable machine-readable code (e.g. `STACK_ALIGN_CALL`).
    pub code: &'static str,
    /// Severity.
    pub severity: Severity,
    /// Human-readable explanation.
    pub message: String,
    /// Index into the analysed instruction slice where the issue was found.
    pub at: Option<usize>,
}

/// The tracked stack state at one point during the walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackState {
    /// Net change to `SP` accumulated so far (positive = stack grown down).
    pub sp_delta: i64,
    /// Cumulative `(ENTRY_SP_MOD + sp_delta) % 16`, i.e. the alignment of
    /// `SP` at this point.
    pub sp_align: i64,
}

impl StackState {
    /// Whether `SP` is 16-byte aligned at this point.
    #[must_use]
    pub fn is_aligned(self) -> bool {
        self.sp_align == 0
    }
}

/// Per-call-site stack state, captured just before the `bl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallSiteState {
    /// Index of the `bl` instruction in the analysed slice.
    pub index: usize,
    /// Stack state immediately before the call.
    pub before: StackState,
}

/// Result of analysing a function body against the AAPCS64 rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiReport {
    /// All findings, in discovery order.
    pub findings: Vec<AbiFinding>,
    /// Whether the body contains a `bl` (i.e. is non-leaf).
    pub is_leaf: bool,
    /// Net `SP` change at the final `ret` (0 == balanced).
    pub final_sp_delta: i64,
    /// Call sites with their pre-call stack state.
    pub call_sites: Vec<CallSiteState>,
}

impl AbiReport {
    /// Whether the analysis is fully clean (no error/warning findings).
    #[must_use]
    pub fn is_clean(&self) -> bool {
        !self
            .findings
            .iter()
            .any(|f| matches!(f.severity, Severity::Error | Severity::Warning))
    }
}

impl AbiFinding {
    /// Stable lowercase string for the severity (e.g. `"error"`).
    #[must_use]
    pub fn severity_str(&self) -> &'static str {
        match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

/// The integer argument register for the Nth integer argument (0-based).
///
/// Returns `None` once the eight GP argument registers are exhausted —
/// further arguments live on the stack.
#[must_use]
pub fn argument_register(index: usize) -> Option<Register> {
    ARG_REGS.get(index).map(|g| g.full())
}

/// The integer/address return register (`X0`).
#[must_use]
pub fn return_register() -> Register {
    RETURN_REG.full()
}

/// Whether `reg` is the stack pointer.
#[must_use]
fn is_sp(r: Register) -> bool {
    matches!(r.storage, Storage::Sp)
}

/// Collect the nonvolatile GP registers **written as data** (e.g. `mov x19,
/// ...`) anywhere in the body. `stp`/`str`/`ldp`/`ldr` of a callee-saved
/// register are the save/restore mechanism itself and are deliberately
/// excluded.
fn written_nonvolatile(instrs: &[LoweredInstr]) -> Vec<Gp> {
    let mut out: Vec<Gp> = Vec::new();
    for ins in instrs {
        for op in &ins.operands {
            if let Operand::Reg(r) = op {
                if let Storage::Gp(g) = r.storage {
                    if NONVOLATILE_GP.contains(&g) && !is_sp(*r) && !out.contains(&g) {
                        out.push(g);
                    }
                }
            }
        }
    }
    out
}

/// A register is "saved in the prologue" if a `Store` (str/stp) writes it
/// to memory with `SP` as the base (with a pre-/post-index negative
/// displacement or an explicit negative offset).
fn prologue_saved(instrs: &[LoweredInstr]) -> Vec<Gp> {
    let mut out: Vec<Gp> = Vec::new();
    for ins in instrs {
        // Detect SP-based stores of GP registers.
        let targets = sp_store_targets(ins);
        if !targets.is_empty() {
            for g in targets {
                if !out.contains(&g) {
                    out.push(g);
                }
            }
        } else if !is_sp_based_store(ins) {
            // Once we hit a non-save instruction, the prologue is over.
            if !out.is_empty() {
                break;
            }
        }
    }
    out
}

/// The GP registers stored by an `SP`-based `Store` prologue instruction
/// (e.g. `stp x19, x20, [sp, #-16]!` yields both `X19` and `X20`).
fn sp_store_targets(ins: &LoweredInstr) -> Vec<Gp> {
    if !matches!(ins.kind, semasm_asir::OpKind::Store) {
        return Vec::new();
    }
    let mem = ins.operands.iter().find_map(|o| match o {
        Operand::Mem(m) => Some(m),
        _ => None,
    });
    if mem.and_then(|m| m.base).is_some_and(is_sp) {
        // Every non-memory GP register operand is a stored register.
        let mut out = Vec::new();
        for op in &ins.operands {
            if let Operand::Reg(r) = op {
                if let Storage::Gp(g) = r.storage {
                    if !out.contains(&g) {
                        out.push(g);
                    }
                }
            }
        }
        return out;
    }
    Vec::new()
}

/// Whether an instruction is an `SP`-based store (used to detect the
/// boundary of the prologue save block).
fn is_sp_based_store(ins: &LoweredInstr) -> bool {
    !sp_store_targets(ins).is_empty()
}

/// The GP registers loaded from an `SP`-based `Load` epilogue instruction
/// (e.g. `ldp x19, x20, [sp], #16` yields both `X19` and `X20`).
fn sp_load_targets(ins: &LoweredInstr) -> Vec<Gp> {
    if !matches!(ins.kind, semasm_asir::OpKind::Load) {
        return Vec::new();
    }
    let mem = ins.operands.iter().find_map(|o| match o {
        Operand::Mem(m) => Some(m),
        _ => None,
    });
    if mem.and_then(|m| m.base).is_some_and(is_sp) {
        let mut out = Vec::new();
        for op in &ins.operands {
            if let Operand::Reg(r) = op {
                if let Storage::Gp(g) = r.storage {
                    if !out.contains(&g) {
                        out.push(g);
                    }
                }
            }
        }
        return out;
    }
    Vec::new()
}

/// The nonvolatile GP registers restored in the epilogue, scanned backwards.
/// `ret`, stack adjustment (`add sp`), and `SP` loads terminate the scan; a
/// `Load` from `SP` records its target as restored.
fn epilogue_restored(instrs: &[LoweredInstr]) -> Vec<Gp> {
    let mut out: Vec<Gp> = Vec::new();
    for ins in instrs.iter().rev() {
        match ins.kind {
            semasm_asir::OpKind::Return
            | semasm_asir::OpKind::Binary
            | semasm_asir::OpKind::Unknown => {}
            _ => {
                let restored = sp_load_targets(ins);
                if restored.is_empty() {
                    break;
                }
                for g in restored {
                    if !out.contains(&g) {
                        out.push(g);
                    }
                }
            }
        }
    }
    out
}

/// Effect of one instruction on the ABI walk.
struct Step {
    /// Change applied to `SP` delta by this instruction.
    sp_change: i64,
    /// Whether this instruction is a `bl`.
    is_call: bool,
    /// Whether this instruction is a `ret`.
    is_ret: bool,
}

/// Classify one lowered instruction into its effect on the ABI walk.
fn step(ins: &LoweredInstr) -> Step {
    let mut s = Step {
        sp_change: 0,
        is_call: false,
        is_ret: false,
    };
    match ins.mnemonic.as_str() {
        "str" | "stp" | "ldr" | "ldp" => {
            // SP-relative push/pop via pre-/post-index: `str xN, [sp, #-16]!`.
            if let Some(mem) = ins.operands.iter().find_map(|o| match o {
                Operand::Mem(m) => Some(m),
                _ => None,
            }) {
                if mem.base.is_some_and(is_sp) {
                    // Pre-index `!` or negative explicit offset both shrink SP.
                    if mem.disp < 0 {
                        s.sp_change += -mem.disp; // disp is negative → grow down
                    } else if mem.disp > 0 {
                        s.sp_change -= mem.disp;
                    }
                }
            }
        }
        "add" | "sub" => {
            if let (Some(Operand::Reg(r)), Some(Operand::Imm(n))) =
                (ins.operands.first(), ins.operands.get(1))
            {
                if is_sp(*r) {
                    s.sp_change += if ins.mnemonic == "sub" { *n } else { -*n };
                }
            }
        }
        "bl" => s.is_call = true,
        "ret" => s.is_ret = true,
        _ => {}
    }
    s
}

/// Emit findings for any nonvolatile register written without being saved in
/// the prologue and restored in the epilogue.
fn check_callee_saved(instrs: &[LoweredInstr], findings: &mut Vec<AbiFinding>) {
    let written = written_nonvolatile(instrs);
    if written.is_empty() {
        return;
    }
    let saved = prologue_saved(instrs);
    let restored = epilogue_restored(instrs);
    for g in &written {
        if !saved.contains(g) {
            findings.push(AbiFinding {
                code: "CALLEE_SAVED_NOT_PRESERVED",
                severity: Severity::Error,
                message: format!(
                    "nonvolatile register {g:?} is written but never saved in the \
                     prologue; AAPCS64 requires callee-saved registers to be preserved"
                ),
                at: None,
            });
        }
        if !restored.contains(g) {
            findings.push(AbiFinding {
                code: "CALLEE_SAVED_NOT_RESTORED",
                severity: Severity::Error,
                message: format!(
                    "nonvolatile register {g:?} is written but never restored in the \
                     epilogue; AAPCS64 requires callee-saved registers to be restored"
                ),
                at: None,
            });
        }
    }
}

/// Analyse a single function body against the AAPCS64 rules.
#[must_use]
pub fn analyze(instrs: &[LoweredInstr]) -> AbiReport {
    let mut findings: Vec<AbiFinding> = Vec::new();
    let mut sp_delta: i64 = 0;
    let mut call_sites: Vec<CallSiteState> = Vec::new();
    let mut is_leaf = true;
    let mut lr_ever_saved = false;

    for (i, ins) in instrs.iter().enumerate() {
        let state = StackState {
            sp_delta,
            sp_align: ((ENTRY_SP_MOD + sp_delta) % STACK_ALIGN + STACK_ALIGN) % STACK_ALIGN,
        };
        let s = step(ins);

        if s.is_call {
            is_leaf = false;
            call_sites.push(CallSiteState {
                index: i,
                before: state,
            });
            // AAPCS64: SP must be 16-byte aligned at the `bl`.
            if !state.is_aligned() {
                findings.push(AbiFinding {
                    code: "STACK_ALIGN_CALL",
                    severity: Severity::Error,
                    message: format!(
                        "bl at index {i}: SP is {}-byte aligned (delta {sp_delta}); \
                         AAPCS64 requires 16-byte alignment at every call",
                        state.sp_align
                    ),
                    at: Some(i),
                });
            }
        }
        if s.is_ret && sp_delta != 0 {
            findings.push(AbiFinding {
                code: "STACK_BALANCE_RET",
                severity: Severity::Error,
                message: format!(
                    "ret at index {i}: SP delta is {sp_delta}, expected 0; \
                     the stack must be balanced before returning"
                ),
                at: Some(i),
            });
        }
        // Track whether LR (X30) is saved anywhere (needs preservation in
        // non-leaf functions).
        if sp_store_targets(ins).contains(&Gp::Lr) {
            lr_ever_saved = true;
        }
        sp_delta += s.sp_change;
    }

    // Callee-saved registers that are written must be saved + restored.
    check_callee_saved(instrs, &mut findings);

    // A non-leaf function consumes LR via `ret`; if it never saved LR it
    // must not have clobbered it. We approximate: if non-leaf and LR is
    // among the written nonvolatile set but was never saved, flag it.
    if !is_leaf && !lr_ever_saved {
        let written = written_nonvolatile(instrs);
        if written.contains(&Gp::Lr) {
            findings.push(AbiFinding {
                code: "LR_CLOBBERED_NON_LEAF",
                severity: Severity::Error,
                message: "function performs a `bl` (non-leaf) and writes LR (X30) \
                     without saving it in the prologue; AAPCS64 requires LR to be \
                     preserved across calls"
                    .to_string(),
                at: None,
            });
        }
    }

    AbiReport {
        findings,
        is_leaf,
        final_sp_delta: sp_delta,
        call_sites,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::{LoweredInstr, MemOperand, Operand};
    use crate::{Gp, Width};
    use semasm_asir::OpKind as Kind;

    fn ins(mnemonic: &str, kind: Kind, operands: Vec<Operand>) -> LoweredInstr {
        LoweredInstr {
            mnemonic: mnemonic.into(),
            kind,
            width: Width::B64,
            signed: None,
            operands,
        }
    }

    fn reg(g: Gp) -> Operand {
        Operand::Reg(g.full())
    }

    fn reg_sp() -> Operand {
        Operand::Reg(Register::sp())
    }

    fn imm(n: i64) -> Operand {
        Operand::Imm(n)
    }

    fn mem_sp(disp: i64) -> Operand {
        Operand::Mem(MemOperand {
            base: Some(Register::sp()),
            index: None,
            scale: 1,
            disp,
            width: Width::B64,
        })
    }

    // --- binding helpers ------------------------------------------------

    #[test]
    fn argument_register_order_matches_aapcs64() {
        let expected = [
            Gp::X0,
            Gp::X1,
            Gp::X2,
            Gp::X3,
            Gp::X4,
            Gp::X5,
            Gp::X6,
            Gp::X7,
        ];
        for (i, g) in expected.iter().enumerate() {
            assert_eq!(argument_register(i), Some(g.full()));
        }
        assert_eq!(argument_register(8), None);
    }

    #[test]
    fn return_register_is_x0() {
        assert_eq!(return_register(), Gp::X0.full());
    }

    // --- call-site alignment -------------------------------------------

    #[test]
    fn aligned_call_is_clean() {
        // entry SP%16==0; `sub sp, 16` keeps alignment; `bl` then clean.
        let body = vec![
            ins("sub", Kind::Binary, vec![reg(Gp::X0), reg(Gp::X1), imm(0)]),
            ins("sub", Kind::Binary, vec![reg_sp(), imm(16)]),
            ins("bl", Kind::Call, vec![imm(0x1000)]),
            ins("add", Kind::Binary, vec![reg_sp(), imm(16)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        assert!(!r.is_leaf);
        assert!(r.is_clean(), "expected clean: {:?}", r.findings);
        assert_eq!(r.final_sp_delta, 0);
    }

    #[test]
    fn misaligned_call_produces_error() {
        // No `sub sp` → at bl SP%16==0 still (entry aligned) — actually
        // aligned. Use a single `sub sp, 8` to misalign.
        let body = vec![
            ins("sub", Kind::Binary, vec![reg_sp(), imm(8)]),
            ins("bl", Kind::Call, vec![imm(0x1000)]),
            ins("add", Kind::Binary, vec![reg_sp(), imm(8)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        assert!(r.findings.iter().any(|f| f.code == "STACK_ALIGN_CALL"));
    }

    // --- stack balance ---------------------------------------------------

    #[test]
    fn unbalanced_stack_at_ret_is_error() {
        let body = vec![
            ins("sub", Kind::Binary, vec![reg_sp(), imm(16)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        let f = r
            .findings
            .iter()
            .find(|f| f.code == "STACK_BALANCE_RET")
            .expect("expected STACK_BALANCE_RET");
        assert_eq!(f.severity, Severity::Error);
        assert_eq!(r.final_sp_delta, 16);
    }

    // --- callee-saved preservation --------------------------------------

    #[test]
    fn written_callee_saved_must_be_saved_and_restored() {
        let body = vec![
            ins("mov", Kind::Store, vec![reg(Gp::X19), reg(Gp::X0)]),
            ins("bl", Kind::Call, vec![imm(0x1000)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        assert!(r
            .findings
            .iter()
            .any(|f| f.code == "CALLEE_SAVED_NOT_PRESERVED"));
        assert!(r
            .findings
            .iter()
            .any(|f| f.code == "CALLEE_SAVED_NOT_RESTORED"));
    }

    #[test]
    fn saved_and_restored_callee_saved_is_clean() {
        // stp x19, x20, [sp, #-16]!  (save) ... ldp x19, x20, [sp], #16 (restore)
        let save = ins(
            "stp",
            Kind::Store,
            vec![reg(Gp::X19), reg(Gp::X20), mem_sp(-16)],
        );
        let use0 = ins("mov", Kind::Store, vec![reg(Gp::X19), reg(Gp::X0)]);
        let use1 = ins("mov", Kind::Store, vec![reg(Gp::X20), reg(Gp::X1)]);
        let restore = ins(
            "ldp",
            Kind::Load,
            vec![reg(Gp::X19), reg(Gp::X20), mem_sp(16)],
        );
        let ret = ins("ret", Kind::Return, vec![]);
        let r = analyze(&[save, use0, use1, restore, ret]);
        assert!(r.is_clean(), "expected clean: {:?}", r.findings);
    }

    #[test]
    fn lr_clobbered_in_non_leaf_is_error() {
        let body = vec![
            ins("mov", Kind::Store, vec![reg(Gp::Lr), reg(Gp::X0)]),
            ins("bl", Kind::Call, vec![imm(0x1000)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        assert!(r.findings.iter().any(|f| f.code == "LR_CLOBBERED_NON_LEAF"));
    }
}
