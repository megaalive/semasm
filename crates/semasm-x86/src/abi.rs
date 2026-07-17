//! System V AMD64 ABI conformance checks over lowered x86-64 instructions.
//!
//! This module turns the pure register model in [`crate`] plus the lowering in
//! [`crate::lower`] into a set of concrete calling-convention checks. The
//! checks operate on a contiguous slice of [`crate::lower::LoweredInstr`]
//! that represents a single function body (prologue … epilogue).
//!
//! Implemented checks (ABI-SYSV-001):
//!
//! * **Argument register binding** — the integer argument registers in their
//!   canonical System V order.
//! * **Integer return binding** — `RAX` as the integer/address return slot.
//! * **Preserved registers** — a callee-saved (`nonvolatile`) register that is
//!   written must be saved in the prologue and restored in the epilogue.
//! * **Call-site stack alignment** — at every `call` the stack pointer must be
//!   16-byte aligned (`(%rsp) % 16 == 0`).
//! * **Stack balance** — the net change to `RSP` across the body must return to
//!   zero at each `ret`.
//! * **Red-zone policy for leaf functions** — a function that performs a call
//!   must not rely on the 128-byte red zone below `RSP`.
//! * **Syscall clobbers** — `syscall` is modelled separately from the function
//!   ABI: it clobbers all caller-saved GP registers (plus `RCX`/`R11`).

use serde::{Deserialize, Serialize};

use crate::lower::{LoweredInstr, Operand};
use crate::{Gp, Register, Storage, NONVOLATILE_GP, VOLATILE_GP};

/// Integer argument registers (1st … 6th) in System V AMD64 order.
pub const ARG_REGS: &[Gp] = &[Gp::Rdi, Gp::Rsi, Gp::Rdx, Gp::Rcx, Gp::R8, Gp::R9];

/// Integer/address return register.
pub const RETURN_REG: Gp = Gp::Rax;

/// System V AMD64 red-zone size in bytes (below `RSP`).
pub const RED_ZONE_BYTES: i64 = 128;

/// Required stack alignment in bytes.
pub const STACK_ALIGN: i64 = 16;

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

/// Result of analysing a function body against the System V AMD64 rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiReport {
    /// All findings, in discovery order.
    pub findings: Vec<AbiFinding>,
    /// Whether the body contains a `call` (i.e. is non-leaf).
    pub is_leaf: bool,
    /// Whether the body contains a `syscall`.
    pub has_syscall: bool,
    /// Net `RSP` change at the final `ret` (0 == balanced).
    pub final_rsp_delta: i64,
    /// Deepest red-zone access observed (most-negative displacement), or 0.
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
/// Returns `None` once the six GP argument registers are exhausted — further
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

/// The set of GP registers clobbered by a `syscall`.
///
/// The Linux x86-64 syscall ABI clobbers every caller-saved (volatile) GP
/// register — `RAX`, `RCX`, `RDX`, `RSI`, `RDI`, `R8`, `R9`, `R10`, `R11` —
/// which is exactly the System V volatile set (already including `RCX`/`R11`).
#[must_use]
pub fn syscall_clobbers() -> Vec<Register> {
    VOLATILE_GP.iter().map(|g| g.full()).collect()
}

/// Names of the volatile (caller-saved) GP registers.
#[must_use]
pub fn volatile_gp() -> &'static [Gp] {
    VOLATILE_GP
}

/// Names of the nonvolatile (callee-saved) GP registers.
#[must_use]
pub fn nonvolatile_gp() -> &'static [Gp] {
    NONVOLATILE_GP
}

/// Whether `reg` is the stack pointer.
#[must_use]
fn is_rsp(r: Register) -> bool {
    matches!(r.storage, Storage::Gp(Gp::Rsp))
}

/// Whether `mem` reads/writes within the red zone: base `RSP` with a
/// non-positive displacement in `[-RED_ZONE_BYTES, 0]`.
fn red_zone_displacement(mem: &crate::lower::MemOperand) -> Option<i64> {
    match mem.base {
        Some(b) if is_rsp(b) && mem.index.is_none() => {
            if mem.disp <= 0 && mem.disp >= -RED_ZONE_BYTES {
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
                    if NONVOLATILE_GP.contains(&g) && !is_rsp(*r) && !out.contains(&g) {
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
    /// Whether this instruction is a `syscall`.
    is_syscall: bool,
    /// Deepest (most-negative) red-zone displacement accessed, if any.
    red_zone_disp: Option<i64>,
}

/// Classify one lowered instruction into its effect on the ABI walk.
fn step(ins: &LoweredInstr) -> Step {
    let mut s = Step {
        rsp_change: 0,
        is_call: false,
        is_ret: false,
        is_syscall: false,
        red_zone_disp: None,
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
        "syscall" => s.is_syscall = true,
        _ => {}
    }
    if matches!(
        ins.kind,
        semasm_asir::OpKind::Store | semasm_asir::OpKind::Load
    ) {
        for op in &ins.operands {
            if let Operand::Mem(m) = op {
                if let Some(d) = red_zone_displacement(m) {
                    s.red_zone_disp = Some(d);
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
                     prologue; System V requires callee-saved registers to be preserved"
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
                     epilogue; System V requires callee-saved registers to be restored"
                ),
                at: None,
            });
        }
    }
}

/// Analyse a single function body against the System V AMD64 rules.
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
    let mut has_syscall = false;
    let mut max_red_zone_disp: i64 = 0;

    for (i, ins) in instrs.iter().enumerate() {
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
            if !state.is_aligned() {
                findings.push(AbiFinding {
                    code: "STACK_ALIGN_CALL",
                    severity: Severity::Error,
                    message: format!(
                        "call at index {i}: RSP is {}-byte aligned (delta {rsp_delta}); \
                         System V requires 16-byte alignment at every call site",
                        state.rsp_align
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
        if s.is_syscall {
            has_syscall = true;
        }
        if let Some(d) = s.red_zone_disp {
            if d < max_red_zone_disp {
                max_red_zone_disp = d;
            }
        }
        rsp_delta += s.rsp_change;
    }

    // Callee-saved registers that are written must be saved + restored.
    check_callee_saved(instrs, &mut findings);

    // Red-zone policy: a non-leaf function must not rely on the red zone.
    if !is_leaf && max_red_zone_disp < 0 {
        findings.push(AbiFinding {
            code: "RED_ZONE_NON_LEAF",
            severity: Severity::Error,
            message: format!(
                "function performs a call yet accesses the red zone at displacement \
                 {max_red_zone_disp} (RSP-relative, below the stack pointer); the red zone \
                 is clobbered by calls and may only be used by leaf functions"
            ),
            at: None,
        });
    }

    AbiReport {
        findings,
        is_leaf,
        has_syscall,
        final_rsp_delta: rsp_delta,
        max_red_zone_disp,
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
    fn argument_register_order_matches_sysv() {
        let expected = [Gp::Rdi, Gp::Rsi, Gp::Rdx, Gp::Rcx, Gp::R8, Gp::R9];
        for (i, g) in expected.iter().enumerate() {
            assert_eq!(argument_register(i), Some(g.full()));
        }
        // The 7th integer argument is passed on the stack.
        assert_eq!(argument_register(6), None);
    }

    #[test]
    fn return_register_is_rax() {
        assert_eq!(return_register(), Gp::Rax.full());
    }

    #[test]
    fn syscall_clobbers_all_volatile() {
        let clobbered: Vec<Gp> = syscall_clobbers()
            .into_iter()
            .map(|r| match r.storage {
                Storage::Gp(g) => g,
                _ => panic!("syscall clobbers must be GP"),
            })
            .collect();
        assert_eq!(clobbered, VOLATILE_GP.to_vec());
    }

    // --- stack alignment at call site -----------------------------------

    #[test]
    fn aligned_call_is_clean() {
        // entry RSP%16==8; `sub rsp, 8` -> RSP%16==0 -> aligned call.
        let body = vec![
            ins("sub", Kind::Binary, vec![reg(Gp::Rsp), imm(8)]),
            ins("call", Kind::Call, vec![imm(0x1000)]),
            ins("add", Kind::Binary, vec![reg(Gp::Rsp), imm(8)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        assert!(!r.is_leaf);
        assert!(r.is_clean(), "expected clean: {:?}", r.findings);
        assert_eq!(r.final_rsp_delta, 0);
    }

    #[test]
    fn misaligned_call_produces_error() {
        // No `sub rsp` -> at call RSP%16==8 -> misaligned.
        let body = vec![
            ins("call", Kind::Call, vec![imm(0x1000)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        let f = r
            .findings
            .iter()
            .find(|f| f.code == "STACK_ALIGN_CALL")
            .expect("expected STACK_ALIGN_CALL");
        assert_eq!(f.severity, Severity::Error);
        assert_eq!(f.at, Some(0));
    }

    // --- stack balance ---------------------------------------------------

    #[test]
    fn unbalanced_stack_at_ret_is_error() {
        // push once, then ret without popping -> delta +8.
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

    #[test]
    fn push_pop_balances() {
        let body = vec![
            ins("push", Kind::Store, vec![reg(Gp::Rbx)]),
            ins("pop", Kind::Store, vec![reg(Gp::Rbx)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        assert!(r.is_clean(), "expected clean: {:?}", r.findings);
    }

    // --- callee-saved preservation --------------------------------------

    #[test]
    fn written_callee_saved_must_be_saved_and_restored() {
        // RBX is nonvolatile; writing it without push/pop is a violation.
        let body = vec![
            ins("mov", Kind::Store, vec![reg(Gp::Rbx), reg(Gp::Rdi)]),
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
            ins("mov", Kind::Store, vec![reg(Gp::Rbx), reg(Gp::Rdi)]),
            ins("pop", Kind::Store, vec![reg(Gp::Rbx)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        assert!(r.is_clean(), "expected clean: {:?}", r.findings);
    }

    // --- red-zone policy ------------------------------------------------

    fn red_zone_load(disp: i64) -> LoweredInstr {
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
    fn leaf_function_may_use_red_zone() {
        // Leaf function (no call) reading RSP-8 is fine.
        let body = vec![red_zone_load(-8), ins("ret", Kind::Return, vec![])];
        let r = analyze(&body);
        assert!(r.is_leaf);
        assert!(r.is_clean(), "expected clean: {:?}", r.findings);
        assert_eq!(r.max_red_zone_disp, -8);
    }

    #[test]
    fn non_leaf_red_zone_use_is_error() {
        // Non-leaf function reads the red zone: must be flagged.
        let body = vec![
            red_zone_load(-16),
            ins("call", Kind::Call, vec![imm(0x1000)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        let f = r
            .findings
            .iter()
            .find(|f| f.code == "RED_ZONE_NON_LEAF")
            .expect("expected RED_ZONE_NON_LEAF");
        assert_eq!(f.severity, Severity::Error);
    }

    // --- syscall ---------------------------------------------------------

    #[test]
    fn syscall_detected() {
        let body = vec![ins("syscall", Kind::Unknown, vec![])];
        let r = analyze(&body);
        assert!(r.has_syscall);
    }
}
