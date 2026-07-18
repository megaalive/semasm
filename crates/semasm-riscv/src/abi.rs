//! RISC-V LP64 (AArch64 psABI equivalent) ABI conformance checks over
//! lowered RISC-V instructions.
//!
//! This module is the RISC-V counterpart to the x86 System V and Microsoft x64
//! ABI analyzers. It turns the register model in
//! [`crate`] plus the lowering in [`crate::lower`] into concrete LP64 checks
//! over a single function body (prologue … epilogue).
//!
//! Implemented checks (RVABI-001):
//!
#![allow(
    clippy::bind_instead_of_map,
    clippy::collapsible_if,
    clippy::manual_let_else,
    dead_code,
    clippy::unnecessary_map_or,
    clippy::get_first,
    clippy::map_unwrap_or
)]
//! * **Argument register binding** — `a0`–`a7` (`x10`–`x17`) in their canonical
//!   order.
//! * **Integer return binding** — `a0` (`x10`) as the first result slot.
//! * **Preserved registers** — a nonvolatile register that is written must be
//!   saved in the prologue and restored in the epilogue.
//! * **Stack alignment** — `sp` must be 16-byte aligned at every `call`/`jalr`
//!   and at return (RISC-V LP64 requires 16-byte alignment at all times).
//! * **RA preservation** — a non-leaf function must preserve `ra` (`x1`),
//!   which `ret` (expanded to `jalr zero, 0(ra)`) consumes as the return address.
//! * **Stack balance** — the net change to `sp` across the body must return to
//!   zero at each `ret`.

use serde::{Deserialize, Serialize};

use crate::lower::{LoweredInstr, Operand};
use crate::{Gpr, Register, Storage, NONVOLATILE_GPR};
use semasm_asir::OpKind;

/// Integer argument registers (1st … 8th) in LP64 order.
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

/// Required stack alignment in bytes (LP64: 16-byte at all times).
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

/// Per-call-site stack state, captured just before the `jalr`/`call`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallSiteState {
    /// Index of the call instruction in the analysed slice.
    pub index: usize,
    /// Stack state immediately before the call.
    pub before: StackState,
}

/// Result of analysing a function body against the LP64 rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiReport {
    /// All findings, in discovery order.
    pub findings: Vec<AbiFinding>,
    /// Whether the body contains a call (i.e. is non-leaf).
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

/// Whether `reg` is the stack pointer.
#[must_use]
fn is_sp(r: Register) -> bool {
    matches!(r.storage, Storage::Gpr(Gpr::Sp))
}

/// Collect the nonvolatile GPR registers **written as data** (e.g. `add s0, ...`)
/// anywhere in the body. `sd`/`sw`/`sdsp` of a callee-saved register are the
/// save/restore mechanism itself and are deliberately excluded.
fn written_nonvolatile(instrs: &[LoweredInstr]) -> Vec<Gpr> {
    let mut out: Vec<Gpr> = Vec::new();
    for ins in instrs {
        for op in &ins.operands {
            if let Operand::Reg(r) = op {
                let Storage::Gpr(g) = r.storage;
                if NONVOLATILE_GPR.contains(&g) && !is_sp(*r) && !out.contains(&g) {
                    out.push(g);
                }
            }
        }
    }
    out
}

/// A register is "saved in the prologue" if a store (`sd`/`sw`/`sdsp`) writes it
/// to memory with `SP` as the base (with a pre-/post-index negative displacement
/// or an explicit negative offset).
fn prologue_saved(instrs: &[LoweredInstr]) -> Vec<Gpr> {
    let mut out: Vec<Gpr> = Vec::new();
    for ins in instrs {
        for g in sp_store_targets(ins) {
            if !out.contains(&g) {
                out.push(g);
            }
        }
        if sp_store_targets(ins).is_empty() && !out.is_empty() {
            break;
        }
    }
    out
}

/// The GPR registers stored by an `SP`-based store prologue instruction.
/// For store-pair instructions (e.g., `sd s0, s1, -16(sp)`), returns both registers.
fn sp_store_targets(ins: &LoweredInstr) -> Vec<Gpr> {
    if !matches!(ins.kind, OpKind::Store) {
        return Vec::new();
    }
    let mem = match ins.operands.iter().find_map(|o| match o {
        Operand::Mem(m) => Some(m),
        _ => None,
    }) {
        Some(m) => m,
        None => return Vec::new(),
    };
    if !mem
        .base
        .map(|r| matches!(r.storage, Storage::Gpr(Gpr::Sp)))
        .unwrap_or(false)
    {
        return Vec::new();
    }
    // All register operands except the memory operand are source registers being stored
    let mut regs = Vec::new();
    for o in &ins.operands {
        if let Operand::Reg(r) = o {
            let Storage::Gpr(g) = r.storage;
            regs.push(g);
        }
    }
    regs
}

/// The GPR registers loaded from an `SP`-based load epilogue instruction.
/// For load-pair instructions (e.g., `ld s0, s1, 16(sp)`), returns both registers.
fn sp_load_targets(ins: &LoweredInstr) -> Vec<Gpr> {
    if !matches!(ins.kind, OpKind::Load) {
        return Vec::new();
    }
    let mem = match ins.operands.iter().find_map(|o| match o {
        Operand::Mem(m) => Some(m),
        _ => None,
    }) {
        Some(m) => m,
        None => return Vec::new(),
    };
    if !mem
        .base
        .map(|r| matches!(r.storage, Storage::Gpr(Gpr::Sp)))
        .unwrap_or(false)
    {
        return Vec::new();
    }
    let mut regs = Vec::new();
    for o in &ins.operands {
        if let Operand::Reg(r) = o {
            let Storage::Gpr(g) = r.storage;
            regs.push(g);
        }
    }
    regs
}

/// The nonvolatile GPR registers restored in the epilogue, scanned backwards.
/// `ret`, stack adjustment (`addi sp, ...`), and `SP` loads terminate the scan;
/// a `Load` from `SP` records its target as restored.
fn epilogue_restored(instrs: &[LoweredInstr]) -> Vec<Gpr> {
    let mut out: Vec<Gpr> = Vec::new();
    for ins in instrs.iter().rev() {
        match ins.kind {
            OpKind::Return | OpKind::Binary | OpKind::Unknown => {}
            _ => {
                let targets = sp_load_targets(ins);
                if targets.is_empty() {
                    break;
                }
                for g in targets {
                    out.push(g);
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
    /// Whether this instruction is a call (`jalr`/`call`).
    is_call: bool,
    /// Whether this instruction is a return (`ret` / `jalr zero, 0(ra)`).
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
        "sd" | "sw" | "sdsp" | "swsp" => {
            if let Some(mem) = ins.operands.iter().find_map(|o| match o {
                Operand::Mem(m) => Some(m),
                _ => None,
            }) {
                if mem
                    .base
                    .and_then(|r| Some(matches!(r.storage, Storage::Gpr(Gpr::Sp))))
                    .unwrap_or(false)
                {
                    if mem.disp < 0 {
                        s.sp_change += -mem.disp;
                    } else if mem.disp > 0 {
                        s.sp_change -= mem.disp;
                    }
                }
            }
        }
        "ld" | "lw" => {
            // Stack restore via load with positive displacement
            if let Some(mem) = ins.operands.iter().find_map(|o| match o {
                Operand::Mem(m) => Some(m),
                _ => None,
            }) {
                if mem
                    .base
                    .and_then(|r| Some(matches!(r.storage, Storage::Gpr(Gpr::Sp))))
                    .unwrap_or(false)
                {
                    if mem.disp > 0 {
                        s.sp_change -= mem.disp;
                    }
                }
            }
        }
        "addi" | "add" => {
            // RISC-V: addi rd, rs1, imm — check if rd is sp and rs1 is sp
            if ins.operands.len() >= 3 {
                if let (Some(Operand::Reg(rd)), Some(Operand::Reg(rs1)), Some(Operand::Imm(n))) = (
                    ins.operands.get(0),
                    ins.operands.get(1),
                    ins.operands.get(2),
                ) {
                    if matches!(rd.storage, Storage::Gpr(Gpr::Sp))
                        && matches!(rs1.storage, Storage::Gpr(Gpr::Sp))
                    {
                        // sp = sp + n; stack grows when sp decreases (n negative), so sp_change = -n
                        s.sp_change -= *n;
                    }
                }
            }
        }
        "jalr" => {
            // Heuristic: `jalr x1, offset(xN)` is a call; `jalr x0, 0(x1)` is a return.
            let rd_is_ra = ins.operands.first().map_or(
                false,
                |o| matches!(o, Operand::Reg(r) if matches!(r.storage, Storage::Gpr(Gpr::Ra))),
            );
            let rd_is_zero = ins.operands.first().map_or(
                false,
                |o| matches!(o, Operand::Reg(r) if matches!(r.storage, Storage::Gpr(Gpr::Zero))),
            );
            if rd_is_zero {
                // Check if base is ra for return
                if let Some(Operand::Mem(m)) = ins.operands.get(1) {
                    if m.base
                        .and_then(|r| Some(matches!(r.storage, Storage::Gpr(Gpr::Ra))))
                        .unwrap_or(false)
                    {
                        s.is_ret = true;
                    }
                }
            } else if rd_is_ra {
                s.is_call = true;
            }
        }
        "call" => s.is_call = true, // pseudo-instruction
        "ret" => s.is_ret = true,   // pseudo-instruction
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
                     prologue; LP64 requires callee-saved registers to be preserved"
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
                     epilogue; LP64 requires callee-saved registers to be restored"
                ),
                at: None,
            });
        }
    }
}

/// Analyse a single function body against the RISC-V LP64 rules.
#[must_use]
pub fn analyze(instrs: &[LoweredInstr]) -> AbiReport {
    let mut findings: Vec<AbiFinding> = Vec::new();
    let mut sp_delta: i64 = 0;
    let mut call_sites: Vec<CallSiteState> = Vec::new();
    let mut is_leaf = true;
    let mut ra_ever_saved = false;

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
            if !state.is_aligned() {
                findings.push(AbiFinding {
                    code: "STACK_ALIGN_CALL",
                    severity: Severity::Error,
                    message: format!(
                        "call at index {i}: SP is {}-byte aligned (delta {sp_delta}); \
                         LP64 requires 16-byte alignment at every call",
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
        if sp_store_targets(ins).contains(&Gpr::Ra) {
            ra_ever_saved = true;
        }
        sp_delta += s.sp_change;
    }

    check_callee_saved(instrs, &mut findings);

    if !is_leaf && !ra_ever_saved {
        let written = written_nonvolatile(instrs);
        // Also check for RA being written (RA is volatile but must be preserved by non-leaf functions)
        let ra_written = instrs.iter().any(|ins| {
            ins.operands.iter().any(
                |op| matches!(op, Operand::Reg(r) if matches!(r.storage, Storage::Gpr(Gpr::Ra))),
            )
        });
        if written.contains(&Gpr::Ra) || ra_written {
            findings.push(AbiFinding {
                code: "RA_CLOBBERED_NON_LEAF",
                severity: Severity::Error,
                message: "function performs a call (non-leaf) and writes RA (x1) \
                     without saving it in the prologue; LP64 requires RA to be \
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
    use crate::{Gpr, Width};
    use OpKind as Kind;

    fn ins(mnemonic: &str, kind: Kind, operands: Vec<Operand>) -> LoweredInstr {
        LoweredInstr {
            mnemonic: mnemonic.into(),
            kind,
            width: Width::B64,
            signed: None,
            operands,
        }
    }

    fn reg(g: Gpr) -> Operand {
        Operand::Reg(g.full())
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

    #[test]
    fn argument_register_order_matches_lp64() {
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
    fn return_register_is_a0() {
        assert_eq!(return_register(), Gpr::A0.full());
    }

    #[test]
    fn aligned_call_is_clean() {
        let body = vec![
            ins(
                "addi",
                Kind::Binary,
                vec![reg(Gpr::Sp), reg(Gpr::Sp), imm(-16)],
            ),
            ins("call", Kind::Call, vec![imm(0x1000)]),
            ins(
                "addi",
                Kind::Binary,
                vec![reg(Gpr::Sp), reg(Gpr::Sp), imm(16)],
            ),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        assert!(!r.is_leaf);
        assert!(r.is_clean(), "expected clean: {:?}", r.findings);
        assert_eq!(r.final_sp_delta, 0);
    }

    #[test]
    fn misaligned_call_produces_error() {
        let body = vec![
            ins(
                "addi",
                Kind::Binary,
                vec![reg(Gpr::Sp), reg(Gpr::Sp), imm(-8)],
            ),
            ins("call", Kind::Call, vec![imm(0x1000)]),
            ins(
                "addi",
                Kind::Binary,
                vec![reg(Gpr::Sp), reg(Gpr::Sp), imm(8)],
            ),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        assert!(r.findings.iter().any(|f| f.code == "STACK_ALIGN_CALL"));
    }

    #[test]
    fn unbalanced_stack_at_ret_is_error() {
        let body = vec![
            ins(
                "addi",
                Kind::Binary,
                vec![reg(Gpr::Sp), reg(Gpr::Sp), imm(-16)],
            ),
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

    #[test]
    fn written_callee_saved_must_be_saved_and_restored() {
        let body = vec![
            ins(
                "add",
                Kind::Binary,
                vec![reg(Gpr::S0), reg(Gpr::A0), reg(Gpr::Zero)],
            ),
            ins("call", Kind::Call, vec![imm(0x1000)]),
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
        let save = ins(
            "sd",
            Kind::Store,
            vec![reg(Gpr::S0), reg(Gpr::S1), mem_sp(-16)],
        );
        let use0 = ins(
            "add",
            Kind::Binary,
            vec![reg(Gpr::S0), reg(Gpr::A0), reg(Gpr::Zero)],
        );
        let use1 = ins(
            "add",
            Kind::Binary,
            vec![reg(Gpr::S1), reg(Gpr::A1), reg(Gpr::Zero)],
        );
        let restore = ins(
            "ld",
            Kind::Load,
            vec![reg(Gpr::S0), reg(Gpr::S1), mem_sp(16)],
        );
        let ret = ins("ret", Kind::Return, vec![]);
        let r = analyze(&[save, use0, use1, restore, ret]);
        assert!(r.is_clean(), "expected clean: {:?}", r.findings);
    }

    #[test]
    fn ra_clobbered_in_non_leaf_is_error() {
        let body = vec![
            ins("mv", Kind::Store, vec![reg(Gpr::Ra), reg(Gpr::A0)]),
            ins("call", Kind::Call, vec![imm(0x1000)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let r = analyze(&body);
        assert!(r.findings.iter().any(|f| f.code == "RA_CLOBBERED_NON_LEAF"));
    }
}
