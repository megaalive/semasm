//! Microsoft x64 calling-convention conformance checks over lowered x86-64
//! instructions.
//!
//! This module is the Windows counterpart to [`crate::abi`] (System V). It
//! turns the register model in [`crate`] plus the lowering in
//! [`crate::lower`] into concrete Microsoft x64 ABI checks. The checks
//! operate on a contiguous slice of [`crate::lower::LoweredInstr`] that
//! represents a single function body (prologue … epilogue).
//!
//! Implemented checks (WIN64-002):
//!
//! * **Argument register binding** — the integer argument registers in their
//!   canonical Windows x64 order (`RCX`, `RDX`, `R8`, `R9`).
//! * **Integer return binding** — `RAX` as the integer/address return slot.
//! * **Preserved registers** — a nonvolatile (`nonvolatile`) register that is
//!   written must be saved in the prologue and restored in the epilogue.
//! * **Call-site alignment** — at every `call` the stack pointer must be
//!   16-byte aligned (`(%rsp) % 16 == 0`).
//! * **Shadow space** — at every `call` the caller must have reserved the
//!   32-byte "shadow store" (`RSP` must have been decreased by at least 32
//!   bytes since entry, in addition to alignment).
//! * **Stack balance** — the net change to `RSP` across the body must return
//!   to zero at each `ret`.
//! * **No red zone** — Windows has no red zone; a function that reads or
//!   writes below `RSP` is flagged.

use serde::{Deserialize, Serialize};

use crate::lower::{LoweredInstr, MemOperand, Operand};
use crate::{Gp, Register, Storage};

/// Integer argument registers (1st … 4th) in Microsoft x64 order.
pub const ARG_REGS: &[Gp] = &[Gp::Rcx, Gp::Rdx, Gp::R8, Gp::R9];

/// Integer/address return register.
pub const RETURN_REG: Gp = Gp::Rax;

/// Required stack alignment in bytes (before a `call`).
pub const STACK_ALIGN: i64 = 16;

/// Microsoft x64 shadow-store size in bytes, reserved at every call site.
pub const SHADOW_BYTES: i64 = 32;

/// At function entry `RSP` is 16-byte aligned minus the 8-byte return address
/// pushed by the caller, i.e. `RSP % 16 == 8`.
const ENTRY_RSP_MOD: i64 = 8;

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
    /// Net change to `RSP` accumulated so far (positive = stack grown down).
    pub rsp_delta: i64,
    /// Cumulative `(ENTRY_RSP_MOD + rsp_delta) % 16`, i.e. the alignment of
    /// `RSP` at this point.
    pub rsp_align: i64,
}

impl StackState {
    /// Whether `RSP` is 16-byte aligned at this point.
    #[must_use]
    pub fn is_aligned(self) -> bool {
        self.rsp_align == 0
    }
}

/// Per-call-site stack state, captured just before the `call`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallSiteState {
    /// Index of the `call` instruction in the analysed slice.
    pub index: usize,
    /// Stack state immediately before the call.
    pub before: StackState,
}

/// Result of analysing a function body against the Microsoft x64 rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiReport {
    /// All findings, in discovery order.
    pub findings: Vec<AbiFinding>,
    /// Whether the body contains a `call` (i.e. is non-leaf).
    pub is_leaf: bool,
    /// Net `RSP` change at the final `ret` (0 == balanced).
    pub final_rsp_delta: i64,
    /// Deepest (most-negative) `RSP`-relative access observed below `RSP`, or 0.
    pub max_red_zone_disp: i64,
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
/// Returns `None` once the four GP argument registers are exhausted — further
/// arguments live on the stack.
#[must_use]
pub fn argument_register(index: usize) -> Option<Register> {
    ARG_REGS.get(index).map(|g| g.full())
}

/// The integer/address return register (`RAX`).
#[must_use]
pub fn return_register() -> Register {
    RETURN_REG.full()
}

/// Whether `reg` is the stack pointer.
#[must_use]
fn is_rsp(r: Register) -> bool {
    matches!(r.storage, Storage::Gp(Gp::Rsp))
}

/// The nonvolatile (callee-saved) GP registers under the Microsoft x64 ABI.
///
/// `RSP` is intentionally excluded: it is tracked separately by the stack
/// walk, and a function must always restore it by balancing its own
/// adjustments rather than via a push/pop.
pub const NONVOLATILE_WIN64: &[Gp] = &[
    Gp::Rbx,
    Gp::Rbp,
    Gp::Rdi,
    Gp::Rsi,
    Gp::R12,
    Gp::R13,
    Gp::R14,
    Gp::R15,
];

/// Whether `mem` reads/writes below the stack pointer (Windows has no red
/// zone): base `RSP` with a negative displacement.
fn below_rsp_displacement(mem: &MemOperand) -> Option<i64> {
    match mem.base {
        Some(b) if is_rsp(b) && mem.index.is_none() => {
            if mem.disp < 0 {
                Some(mem.disp)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Collect the nonvolatile GP registers **written as data** (e.g. `mov rbx,
/// ...`) anywhere in the body. `push`/`pop` of a callee-saved register are the
/// save/restore mechanism itself and are deliberately excluded.
fn written_nonvolatile(instrs: &[LoweredInstr]) -> Vec<Gp> {
    let mut out: Vec<Gp> = Vec::new();
    for ins in instrs {
        if matches!(ins.mnemonic.as_str(), "push" | "pop") {
            continue;
        }
        for op in &ins.operands {
            if let Operand::Reg(r) = op {
                if let Storage::Gp(g) = r.storage {
                    if NONVOLATILE_WIN64.contains(&g) && !is_rsp(*r) && !out.contains(&g) {
                        out.push(g);
                    }
                }
            }
        }
    }
    out
}

/// A register is "saved in the prologue" if it is `push`ed before the first
/// instruction that is neither a `push` nor a `sub rsp`/`add rsp`.
fn prologue_saved(instrs: &[LoweredInstr]) -> Vec<Gp> {
    let mut out: Vec<Gp> = Vec::new();
    for ins in instrs {
        match ins.mnemonic.as_str() {
            "push" => {
                if let Some(Operand::Reg(r)) = ins.operands.first() {
                    if let Storage::Gp(g) = r.storage {
                        if !out.contains(&g) {
                            out.push(g);
                        }
                    }
                }
            }
            "sub" | "add" => {
                if let Some(Operand::Reg(r)) = ins.operands.first() {
                    if is_rsp(*r) {
                        break;
                    }
                }
            }
            _ => break,
        }
    }
    out
}

/// The nonvolatile GP registers restored in the epilogue, scanned backwards.
/// `ret`, stack adjustment (`add rsp`), and `RSP` pops terminate the scan; a
/// `pop` of a GP register records it as restored.
fn epilogue_restored(instrs: &[LoweredInstr]) -> Vec<Gp> {
    let mut out: Vec<Gp> = Vec::new();
    for ins in instrs.iter().rev() {
        match ins.mnemonic.as_str() {
            "ret" | "retn" | "retf" | "add" | "sub" => {}
            "pop" => {
                if let Some(Operand::Reg(r)) = ins.operands.first() {
                    if let Storage::Gp(g) = r.storage {
                        if is_rsp(*r) {
                            break;
                        }
                        out.push(g);
                    }
                }
            }
            _ => break,
        }
    }
    out
}

/// Effect of one instruction on the ABI walk.
struct Step {
    /// Change applied to `RSP` delta by this instruction.
    rsp_change: i64,
    /// Whether this instruction is a `call`.
    is_call: bool,
    /// Whether this instruction is a `ret`.
    is_ret: bool,
    /// Deepest (most-negative) below-RSP displacement accessed, if any.
    below_rsp_disp: Option<i64>,
}

/// True when this is `mov rsp, rbp` (frame restore).
fn is_mov_rsp_from_rbp(ins: &LoweredInstr) -> bool {
    if !matches!(ins.mnemonic.as_str(), "mov" | "movabs") {
        return false;
    }
    matches!(
        (ins.operands.first(), ins.operands.get(1)),
        (Some(Operand::Reg(dst)), Some(Operand::Reg(src)))
            if is_rsp(*dst) && matches!(src.storage, Storage::Gp(Gp::Rbp))
    )
}

/// Classify one lowered instruction into its effect on the ABI walk.
fn step(ins: &LoweredInstr) -> Step {
    let mut s = Step {
        rsp_change: 0,
        is_call: false,
        is_ret: false,
        below_rsp_disp: None,
    };
    match ins.mnemonic.as_str() {
        "push" => s.rsp_change += 8,
        "pop" => s.rsp_change -= 8,
        "sub" | "add" => {
            if let (Some(Operand::Reg(r)), Some(Operand::Imm(n))) =
                (ins.operands.first(), ins.operands.get(1))
            {
                if is_rsp(*r) {
                    s.rsp_change += if ins.mnemonic == "sub" { *n } else { -*n };
                }
            }
        }
        "call" => s.is_call = true,
        "ret" | "retn" | "retf" => s.is_ret = true,
        _ => {}
    }
    if matches!(
        ins.kind,
        semasm_asir::OpKind::Store | semasm_asir::OpKind::Load
    ) {
        for op in &ins.operands {
            if let Operand::Mem(m) = op {
                if let Some(d) = below_rsp_displacement(m) {
                    s.below_rsp_disp = Some(d);
                }
            }
        }
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
                    "nonvolatile register {g:?} is written but never pushed in the \
                     prologue; the Microsoft x64 ABI requires callee-saved registers \
                     to be preserved"
                ),
                at: None,
            });
        }
        if !restored.contains(g) {
            findings.push(AbiFinding {
                code: "CALLEE_SAVED_NOT_RESTORED",
                severity: Severity::Error,
                message: format!(
                    "nonvolatile register {g:?} is written but never popped in the \
                     epilogue; the Microsoft x64 ABI requires callee-saved registers \
                     to be restored"
                ),
                at: None,
            });
        }
    }
}

/// Analyse a single function body against the Microsoft x64 rules.
///
/// `instrs` is expected to be the lowered, contiguous slice for one function
/// (prologue through epilogue). The walk derives the stack state and emits a
/// finding for every rule violation it can statically determine.
#[must_use]
pub fn analyze(instrs: &[LoweredInstr]) -> AbiReport {
    let mut findings: Vec<AbiFinding> = Vec::new();
    let mut rsp_delta: i64 = 0;
    let mut call_sites: Vec<CallSiteState> = Vec::new();
    let mut is_leaf = true;
    let mut max_below_rsp_disp: i64 = 0;

    for (i, ins) in instrs.iter().enumerate() {
        // Standard frame epilogue: `mov rsp, rbp` restores RSP to the post-`push rbp`
        // depth (delta 8). HlaX64 and many compilers use this instead of `add rsp, N`.
        if is_mov_rsp_from_rbp(ins) {
            rsp_delta = 8;
            continue;
        }

        let state = StackState {
            rsp_delta,
            rsp_align: ((ENTRY_RSP_MOD + rsp_delta) % STACK_ALIGN + STACK_ALIGN) % STACK_ALIGN,
        };
        let s = step(ins);

        if s.is_call {
            is_leaf = false;
            call_sites.push(CallSiteState {
                index: i,
                before: state,
            });
            // Microsoft x64: RSP must be 16-byte aligned immediately before a
            // `call`, and the 32-byte shadow store must be reserved.
            if !state.is_aligned() {
                findings.push(AbiFinding {
                    code: "STACK_ALIGN_CALL",
                    severity: Severity::Error,
                    message: format!(
                        "call at index {i}: RSP is {}-byte aligned (delta {rsp_delta}); \
                         the Microsoft x64 ABI requires 16-byte alignment immediately \
                         before every call",
                        state.rsp_align
                    ),
                    at: Some(i),
                });
            }
            if rsp_delta < SHADOW_BYTES {
                findings.push(AbiFinding {
                    code: "SHADOW_SPACE_MISSING",
                    severity: Severity::Error,
                    message: format!(
                        "call at index {i}: only {rsp_delta} bytes of stack reserved \
                         since entry, expected at least the {SHADOW_BYTES}-byte shadow \
                         store for the Microsoft x64 ABI"
                    ),
                    at: Some(i),
                });
            }
        }
        if s.is_ret && rsp_delta != 0 {
            findings.push(AbiFinding {
                code: "STACK_BALANCE_RET",
                severity: Severity::Error,
                message: format!(
                    "ret at index {i}: RSP delta is {rsp_delta}, expected 0; \
                     the stack must be balanced before returning"
                ),
                at: Some(i),
            });
        }
        if let Some(d) = s.below_rsp_disp {
            if d < max_below_rsp_disp {
                max_below_rsp_disp = d;
            }
        }
        rsp_delta += s.rsp_change;
    }

    // Callee-saved registers that are written must be saved + restored.
    check_callee_saved(instrs, &mut findings);

    // Windows has no red zone: any access below RSP is an error.
    if max_below_rsp_disp < 0 {
        findings.push(AbiFinding {
            code: "RED_ZONE_NOT_ALLOWED",
            severity: Severity::Error,
            message: format!(
                "function accesses RSP-relative memory at displacement \
                 {max_below_rsp_disp} below the stack pointer; the Microsoft x64 ABI \
                 has no red zone and such accesses are unsafe across calls"
            ),
            at: None,
        });
    }

    AbiReport {
        findings,
        is_leaf,
        final_rsp_delta: rsp_delta,
        max_red_zone_disp: max_below_rsp_disp,
        call_sites,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::{LoweredInstr, MemOperand, Operand};
    use crate::{Gp, Width};
    use semasm_asir::OpKind as Kind;

    /// Build a `LoweredInstr` from a mnemonic and operand list.
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

    fn imm(n: i64) -> Operand {
        Operand::Imm(n)
    }

    // --- binding helpers ------------------------------------------------

    #[test]
    fn argument_register_order_matches_win64() {
        let expected = [Gp::Rcx, Gp::Rdx, Gp::R8, Gp::R9];
        for (i, g) in expected.iter().enumerate() {
            assert_eq!(argument_register(i), Some(g.full()));
        }
        // The 5th integer argument is passed on the stack.
        assert_eq!(argument_register(4), None);
    }

    #[test]
    fn return_register_is_rax() {
        assert_eq!(return_register(), Gp::Rax.full());
    }

    // --- call-site alignment + shadow space -----------------------------

    #[test]
    fn aligned_call_with_shadow_is_clean() {
        // entry RSP%16==8; `sub rsp, 0x28` (40) -> RSP%16==0 and >=32 reserved.
        let body = vec![
            ins("sub", Kind::Binary, vec![reg(Gp::Rsp), imm(0x28)]),
            ins("call", Kind::Call, vec![imm(0x1000)]),
            ins("add", Kind::Binary, vec![reg(Gp::Rsp), imm(0x28)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        assert!(!r.is_leaf);
        assert!(r.is_clean(), "expected clean: {:?}", r.findings);
        assert_eq!(r.final_rsp_delta, 0);
    }

    #[test]
    fn misaligned_call_produces_error() {
        // No `sub rsp` -> at call RSP%16==8 -> misaligned + no shadow.
        let body = vec![
            ins("call", Kind::Call, vec![imm(0x1000)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        assert!(r.findings.iter().any(|f| f.code == "STACK_ALIGN_CALL"));
        assert!(r.findings.iter().any(|f| f.code == "SHADOW_SPACE_MISSING"));
    }

    #[test]
    fn shadow_reserved_but_misaligned_is_error() {
        // Only 8 bytes reserved: aligned (8+8)%16==0? RSP%16==8+8==0 yes aligned,
        // but shadow (<32) missing.
        let body = vec![
            ins("sub", Kind::Binary, vec![reg(Gp::Rsp), imm(8)]),
            ins("call", Kind::Call, vec![imm(0x1000)]),
            ins("add", Kind::Binary, vec![reg(Gp::Rsp), imm(8)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        assert!(r.findings.iter().any(|f| f.code == "SHADOW_SPACE_MISSING"));
        assert!(!r.findings.iter().any(|f| f.code == "STACK_ALIGN_CALL"));
    }

    // --- stack balance ---------------------------------------------------

    #[test]
    fn unbalanced_stack_at_ret_is_error() {
        let body = vec![
            ins("push", Kind::Store, vec![reg(Gp::Rax)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        let f = r
            .findings
            .iter()
            .find(|f| f.code == "STACK_BALANCE_RET")
            .expect("expected STACK_BALANCE_RET");
        assert_eq!(f.severity, Severity::Error);
        assert_eq!(r.final_rsp_delta, 8);
    }

    // --- callee-saved preservation --------------------------------------

    #[test]
    fn written_callee_saved_must_be_saved_and_restored() {
        // RBX is nonvolatile on Windows; writing it without push/pop violates ABI.
        let body = vec![
            ins("mov", Kind::Store, vec![reg(Gp::Rbx), reg(Gp::Rcx)]),
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
        let body = vec![
            ins("push", Kind::Store, vec![reg(Gp::Rbx)]),
            ins("mov", Kind::Store, vec![reg(Gp::Rbx), reg(Gp::Rcx)]),
            ins("pop", Kind::Store, vec![reg(Gp::Rbx)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        assert!(r.is_clean(), "expected clean: {:?}", r.findings);
    }

    // --- no red zone ----------------------------------------------------

    fn below_rsp_load(disp: i64) -> LoweredInstr {
        ins(
            "mov",
            Kind::Load,
            vec![Operand::Mem(MemOperand {
                base: Some(Gp::Rsp.full()),
                index: None,
                scale: 1,
                disp,
                width: Width::B64,
            })],
        )
    }

    #[test]
    fn below_rsp_access_is_error() {
        // Even a leaf function reading RSP-8 is illegal on Windows (no red zone).
        let body = vec![below_rsp_load(-8), ins("ret", Kind::Return, vec![])];
        let r = analyze(&body);
        let f = r
            .findings
            .iter()
            .find(|f| f.code == "RED_ZONE_NOT_ALLOWED")
            .expect("expected RED_ZONE_NOT_ALLOWED");
        assert_eq!(f.severity, Severity::Error);
    }
}
