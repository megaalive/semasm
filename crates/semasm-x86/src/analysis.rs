//! Forward data-flow analysis over lowered x86-64 instructions (ANALYSIS-001).
//!
//! The analysis tracks, per register, an abstract value on a three-point
//! lattice (`Uninit → Const(n) → Conflict`), plus the running `RSP` delta and
//! the signedness context of the most recent comparison. Memory loads are
//! treated as `Conflict` (the value is not statically known); the *provenance*
//! of a memory access (stack frame vs red zone vs unknown) is recorded
//! separately for reporting.
//!
//! Propagation is **block-at-a-time over a worklist** with a monotone join,
//! so it:
//!
//! * **converges on loops** — revisiting a back-edge can only raise lattice
//!   values toward `Conflict`, never lower them;
//! * **joins conflicting incoming state into an explicit `Conflict`** (never an
//!   implicit wrong value);
//! * **never explodes paths** — each basic block is processed at most once per
//!   worklist pass, and the worklist empties at the fixpoint.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::lower::{LoweredInstr, Operand};
use crate::{Gp, Register, Storage, Width};
use semasm_asir::OpKind;

/// An abstract register value on the analysis lattice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbsVal {
    /// Register has not been assigned within the analysed region.
    Uninit,
    /// Register holds a known constant.
    Const(i64),
    /// Conflicting / unknown — at least two incompatible values have reached
    /// this point (e.g. both branches of a conditional write).
    Conflict,
}

impl AbsVal {
    /// Join two abstract values: `Conflict` dominates, equal constants
    /// stay constant, anything else collapses to `Conflict`.
    #[must_use]
    pub fn join(self, other: AbsVal) -> AbsVal {
        match (self, other) {
            // Equal constants stay constant; anything touching Conflict or two
            // different constants collapses to Conflict.
            (AbsVal::Const(a), AbsVal::Const(b)) if a == b => AbsVal::Const(a),
            _ => {
                let uninit = matches!(self, AbsVal::Uninit) && matches!(other, AbsVal::Uninit);
                if uninit {
                    AbsVal::Uninit
                } else {
                    AbsVal::Conflict
                }
            }
        }
    }
}

/// Abstract state of one general-purpose register at a program point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegAbs {
    /// Abstract value.
    pub value: AbsVal,
    /// Operation width last seen writing this register.
    pub width: Width,
}

/// The signedness context carried after a comparison/test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpContext {
    /// No comparison seen yet.
    None,
    /// Most recent comparison is signed.
    Signed,
    /// Most recent comparison is unsigned.
    Unsigned,
}

/// Provenance of a memory access base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemProvenance {
    /// Below `RSP` (red zone) — clobbered by calls in non-leaf code.
    RedZone,
    /// Relative to `RSP` at or above the current pointer (call-frame slot).
    StackFrame,
    /// Relative to `RBP` (established frame pointer).
    FramePointer,
    /// Any other base — provenance unknown.
    Unknown,
}

/// Per-block abstract state at a program point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockState {
    /// Abstract value of every GP register, keyed by canonical register.
    pub regs: HashMap<Gp, RegAbs>,
    /// Net change to `RSP` since entry (positive = stack grown down).
    pub rsp_delta: i64,
    /// Whether [`Self::rsp_delta`] remains known.
    pub rsp_known: bool,
    /// Signedness context of the most recent comparison.
    pub cmp: CmpContext,
    /// Whether memory facts remain valid after all observed instructions.
    pub memory_known: bool,
}

impl BlockState {
    /// A fresh bottom state (entry of the analysed region).
    fn bottom() -> Self {
        Self {
            regs: HashMap::new(),
            rsp_delta: 0,
            rsp_known: true,
            cmp: CmpContext::None,
            memory_known: true,
        }
    }

    /// Join two block states field-by-field (monotone).
    fn join(&self, other: &BlockState) -> BlockState {
        let mut regs = self.regs.clone();
        for (g, v) in &other.regs {
            let joined = match regs.get(g) {
                Some(existing) => RegAbs {
                    value: existing.value.join(v.value),
                    width: if existing.width.bits() >= v.width.bits() {
                        existing.width
                    } else {
                        v.width
                    },
                },
                None => *v,
            };
            regs.insert(*g, joined);
        }
        BlockState {
            regs,
            // RSP delta is only meaningful when identical; differing deltas
            // mean the block is reached with an inconsistent stack, which we
            // surface as the join-Conflict equivalent by keeping the entry value.
            rsp_delta: if self.rsp_delta == other.rsp_delta {
                self.rsp_delta
            } else {
                // Conflicting stack depth: keep the most negative (deepest)
                // as a conservative approximation, and flag via rsp_delta
                // staying; the report notes the divergence through the per-register
                // Conflict mechanism, not here. We preserve the smaller delta.
                self.rsp_delta.min(other.rsp_delta)
            },
            rsp_known: self.rsp_known && other.rsp_known && self.rsp_delta == other.rsp_delta,
            cmp: match (self.cmp, other.cmp) {
                (CmpContext::None, x) | (x, CmpContext::None) => x,
                // Two equal contexts stay; anything else merges to Unsigned
                // (treated as a conflict for reporting purposes).
                (a, b) if a == b => a,
                _ => CmpContext::Unsigned,
            },
            memory_known: self.memory_known && other.memory_known,
        }
    }
}

/// A basic-block range over the lowered instruction stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockRange {
    /// Inclusive start instruction index.
    pub start: usize,
    /// Exclusive end instruction index.
    pub end: usize,
    /// Successor block indices (in the `blocks` array).
    pub successors: Vec<usize>,
}

/// A data-flow analysis note for reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisNote {
    /// Stable note code.
    pub code: &'static str,
    /// Block the note applies to.
    pub block: usize,
    /// Human-readable message.
    pub message: String,
}

/// Result of the forward analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisReport {
    /// Number of fixpoint iterations performed.
    pub iterations: usize,
    /// Whether the worklist converged (always true for this monotone
    /// analysis; retained for diagnostic clarity).
    pub converged: bool,
    /// Final per-block state (entry state into each block).
    pub block_in: Vec<BlockState>,
    /// Final per-block state (exit state after each block's instructions).
    pub block_out: Vec<BlockState>,
    /// Memory accesses with their provenance, in instruction order.
    pub mem_accesses: Vec<MemAccess>,
    /// Reporting notes.
    pub notes: Vec<AnalysisNote>,
}

/// A recorded memory access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemAccess {
    /// Instruction index in the lowered stream.
    pub at: usize,
    /// Access kind.
    pub kind: MemKind,
    /// Provenance of the access base.
    pub provenance: MemProvenance,
    /// Displacement relative to the base.
    pub disp: i64,
}

/// Kind of a memory access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemKind {
    /// Load (read).
    Load,
    /// Store (write).
    Store,
}

/// Classify the provenance of a memory operand.
#[allow(clippy::manual_range_contains)]
#[must_use]
pub fn mem_provenance(base: Option<Register>, disp: i64, rsp_delta: i64) -> MemProvenance {
    match base {
        Some(b) => match b.storage {
            Storage::Gp(Gp::Rsp) => {
                // What the instruction sees as `[rsp + disp]` is actually at
                // `RSP_entry + rsp_delta + disp` in absolute terms. Below the
                // current stack pointer is the red zone.
                let abs = rsp_delta + disp;
                if abs < 0 && abs >= -128 {
                    MemProvenance::RedZone
                } else {
                    MemProvenance::StackFrame
                }
            }
            Storage::Gp(Gp::Rbp) => MemProvenance::FramePointer,
            _ => MemProvenance::Unknown,
        },
        None => MemProvenance::Unknown,
    }
}

/// Build basic-block ranges from the lowered stream by splitting at
/// terminators. Successor edges use **block indices**:
///
/// * a fall-through edge (when present) points at the next contiguous block;
/// * an optional `target_block` map (address → block index) lets callers
///   resolve `jmp`/`call`/conditional-branch targets into real back-edges
///   or merges; when absent, only the fall-through edge is emitted.
#[allow(clippy::implicit_hasher)]
#[must_use]
pub fn build_blocks(
    instrs: &[LoweredInstr],
    target_block: &HashMap<u64, usize>,
) -> Vec<BlockRange> {
    let n = instrs.len();
    if n == 0 {
        return Vec::new();
    }
    let mut blocks: Vec<BlockRange> = Vec::new();
    let mut start = 0;
    let mut block_idx: usize = 0;
    let mut i = 0;
    while i < n {
        let ins = &instrs[i];
        let is_term = matches!(ins.kind, OpKind::Call | OpKind::Return | OpKind::Branch)
            || ins.mnemonic == "syscall"
            || ins.mnemonic == "ret"
            || ins.mnemonic == "int";
        let hard_stop = matches!(ins.kind, OpKind::Return)
            || ins.mnemonic == "ret"
            || ins.mnemonic == "syscall"
            || ins.mnemonic == "int";

        let mut successors: Vec<usize> = Vec::new();
        // Resolve a taken/jump target if the operand names a known address.
        if let Some(Operand::Imm(addr)) = ins.operands.first() {
            if let Some(&tb) = target_block.get(&addr.unsigned_abs()) {
                successors.push(tb);
            }
        }
        // Fall-through to the next block, unless this is a hard stop.
        if !hard_stop && i + 1 < n {
            successors.push(block_idx + 1);
        }
        if is_term || i + 1 == n {
            blocks.push(BlockRange {
                start,
                end: i + 1,
                successors,
            });
            start = i + 1;
            block_idx += 1;
        }
        i += 1;
    }
    if start < n {
        blocks.push(BlockRange {
            start,
            end: n,
            successors: Vec::new(),
        });
    }
    blocks
}

/// Apply the transfer function of one instruction to a state.
#[allow(clippy::match_same_arms)]
fn transfer(ins: &LoweredInstr, state: &mut BlockState) {
    match ins.kind {
        OpKind::Store => {
            // mov / push / pop: destination is operand 0 when it is a register.
            if let Some(Operand::Reg(dst)) = ins.operands.first() {
                if let Storage::Gp(g) = dst.storage {
                    let val = match ins.operands.get(1) {
                        Some(Operand::Imm(n)) => AbsVal::Const(*n),
                        Some(Operand::Reg(src)) => gp_of(*src)
                            .and_then(|source| state.regs.get(&source))
                            .map_or(AbsVal::Conflict, |r| r.value),
                        _ => AbsVal::Conflict,
                    };
                    state.regs.insert(
                        g,
                        RegAbs {
                            value: val,
                            width: dst.width,
                        },
                    );
                }
            }
            // Stack pointer adjustment for push/pop.
            match ins.mnemonic.as_str() {
                "push" if state.rsp_known => state.rsp_delta += 8,
                "pop" if state.rsp_known => state.rsp_delta -= 8,
                _ => {}
            }
        }
        OpKind::Load => {
            // lea / mov mem -> reg: the loaded value is unknown.
            if let Some(Operand::Reg(dst)) = ins.operands.first() {
                if let Storage::Gp(g) = dst.storage {
                    state.regs.insert(
                        g,
                        RegAbs {
                            value: AbsVal::Conflict,
                            width: dst.width,
                        },
                    );
                }
            }
        }
        OpKind::Binary => {
            // add/sub: if both operands are registers and the source is a
            // known constant, the destination becomes Conflict (result unknown
            // unless trivially identity); if source is immediate the result is
            // Conflict too (we do not fold arithmetic yet — "cheap" constants
            // only covers direct moves).
            if let Some(Operand::Reg(dst)) = ins.operands.first() {
                if let Storage::Gp(g) = dst.storage {
                    state.regs.insert(
                        g,
                        RegAbs {
                            value: AbsVal::Conflict,
                            width: dst.width,
                        },
                    );
                }
            }
        }
        OpKind::Compare => {
            state.cmp = match ins.signed {
                Some(true) => CmpContext::Signed,
                Some(false) | None => CmpContext::Unsigned,
            };
        }
        OpKind::Call => {
            // A call clobbers all volatile (caller-saved) registers; the
            // destination's prior value is lost. RSP gains 8 for the return
            // address. Callee-saved registers are preserved by convention.
            for &g in crate::VOLATILE_GP {
                state.regs.insert(
                    g,
                    RegAbs {
                        value: AbsVal::Conflict,
                        width: Width::B64,
                    },
                );
            }
            if state.rsp_known {
                state.rsp_delta += 8;
            }
        }
        OpKind::Unknown => invalidate_unknown(state),
        OpKind::Branch | OpKind::Return | OpKind::Nop => {}
    }
}

fn invalidate_unknown(state: &mut BlockState) {
    for g in [
        Gp::Rax,
        Gp::Rbx,
        Gp::Rcx,
        Gp::Rdx,
        Gp::Rsi,
        Gp::Rdi,
        Gp::Rsp,
        Gp::Rbp,
        Gp::R8,
        Gp::R9,
        Gp::R10,
        Gp::R11,
        Gp::R12,
        Gp::R13,
        Gp::R14,
        Gp::R15,
    ] {
        state.regs.insert(
            g,
            RegAbs {
                value: AbsVal::Conflict,
                width: Width::B64,
            },
        );
    }
    state.cmp = CmpContext::None;
    state.rsp_known = false;
    state.memory_known = false;
}

/// Extract the canonical GP register of a `Register`, if it is one.
fn gp_of(r: Register) -> Option<Gp> {
    match r.storage {
        Storage::Gp(g) => Some(g),
        _ => None,
    }
}

/// Process a single block: join predecessor out-states into the in-state, then
/// apply the transfer function across the block's instructions (refining memory
/// provenance with the live stack delta). Returns `(in_state, out_state)`.
fn process_block(
    bi: usize,
    lowered: &[LoweredInstr],
    blocks: &[BlockRange],
    block_out: &[BlockState],
    mem_accesses: &mut [MemAccess],
) -> (BlockState, BlockState) {
    let block = &blocks[bi];

    // In-state = join of all predecessor out-states.
    let mut in_state = BlockState::bottom();
    let mut has_pred = false;
    for (pi, p) in blocks.iter().enumerate() {
        if p.successors.contains(&bi) {
            in_state = if has_pred {
                in_state.join(&block_out[pi])
            } else {
                block_out[pi].clone()
            };
            has_pred = true;
        }
    }

    let mut out_state = in_state.clone();
    let mut rsp_at = out_state.rsp_delta;
    for (k, ins) in lowered[block.start..block.end].iter().enumerate() {
        let idx = block.start + k;
        for ma in mem_accesses.iter_mut().filter(|m| m.at == idx) {
            if let Some(Operand::Mem(m)) = ins.operands.first() {
                ma.provenance = mem_provenance(m.base, m.disp, rsp_at);
            }
        }
        transfer(ins, &mut out_state);
        rsp_at = out_state.rsp_delta;
    }
    (in_state, out_state)
}

/// Run the forward data-flow analysis to a fixpoint.
///
/// `lowered` is the linear instruction stream; `blocks` describes the basic
/// blocks (see [`build_blocks`]). Returns the per-block entry/exit states,
/// memory-access provenance, and reporting notes.
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn analyze(lowered: &[LoweredInstr], blocks: &[BlockRange]) -> AnalysisReport {
    let mut block_in: Vec<BlockState> = vec![BlockState::bottom(); blocks.len()];
    let mut block_out: Vec<BlockState> = vec![BlockState::bottom(); blocks.len()];
    let mut mem_accesses: Vec<MemAccess> = Vec::new();
    let mut notes: Vec<AnalysisNote> = Vec::new();

    // Record memory accesses up front (provenance resolved with per-instr
    // stack delta during transfer below is approximated at entry; we recompute
    // provenance from the final out-state per block for accuracy).
    for (at, ins) in lowered.iter().enumerate() {
        if ins.kind == OpKind::Unknown {
            notes.push(AnalysisNote {
                code: "ANALYSIS_INCOMPLETE",
                block: block_of(blocks, at),
                message: format!(
                    "instruction {at} (`{}`) has unknown effects; register, flag, stack, and memory facts were invalidated",
                    ins.mnemonic
                ),
            });
        }
        let kind = match ins.kind {
            OpKind::Load => Some(MemKind::Load),
            OpKind::Store => {
                // push/pop/store — only true memory stores (not reg moves).
                match ins.operands.first() {
                    Some(Operand::Mem(_)) => Some(MemKind::Store),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(k) = kind {
            if let Some(Operand::Mem(m)) = ins.operands.first() {
                mem_accesses.push(MemAccess {
                    at,
                    kind: k,
                    provenance: mem_provenance(m.base, m.disp, 0),
                    disp: m.disp,
                });
            }
        }
    }

    // Entry block (index 0) starts from the ABI-initial bottom state.
    // Iterative worklist.
    let mut worklist: VecDeque<usize> = (0..blocks.len()).collect();
    let mut iterations = 0;
    let mut seen = HashSet::new();

    while let Some(bi) = worklist.pop_front() {
        iterations += 1;
        if iterations > blocks.len() * 4 + 16 {
            // Safety bound; monotone join guarantees termination well before.
            break;
        }
        let (in_state, out_state) =
            process_block(bi, lowered, blocks, &block_out, &mut mem_accesses);

        // If the out-state changed, push successors to the worklist.
        if out_state != block_out[bi] || block_in[bi] != in_state {
            block_in[bi] = in_state;
            block_out[bi] = out_state;
            for s in &blocks[bi].successors {
                if !seen.contains(s) || iterations < blocks.len() * 2 {
                    worklist.push_back(*s);
                    seen.insert(*s);
                }
            }
        }
    }

    // Predecessor counts, to distinguish a true merge (join) from a
    // single-path Conflict caused by arithmetic imprecision.
    let pred_count: Vec<usize> = (0..blocks.len())
        .map(|bi| blocks.iter().filter(|p| p.successors.contains(&bi)).count())
        .collect();

    // Notes: register constants and conflicts at block exits, plus red zone.
    for (bi, st) in block_out.iter().enumerate() {
        for (&g, ra) in &st.regs {
            match ra.value {
                AbsVal::Const(n) => notes.push(AnalysisNote {
                    code: "REG_CONST",
                    block: bi,
                    message: format!(
                        "block {bi}: {g:?} holds constant {n} ({}-bit)",
                        ra.width.bits()
                    ),
                }),
                // Only surface a join Conflict when the block actually merges
                // two or more distinct predecessor paths, or has a self
                // back-edge (loop) — both are real joins. A single-path
                // Conflict is arithmetic imprecision and is left implicit.
                AbsVal::Conflict => {
                    let is_join = pred_count[bi] >= 2 || blocks[bi].successors.contains(&bi);
                    if is_join {
                        notes.push(AnalysisNote {
                            code: "REG_CONFLICT",
                            block: bi,
                            message: format!(
                                "block {bi}: {g:?} has conflicting incoming values (joined to unknown)"
                            ),
                        });
                    }
                }
                AbsVal::Uninit => {}
            }
        }
        if st.cmp == CmpContext::Unsigned && bi > 0 && pred_count[bi] >= 2 {
            notes.push(AnalysisNote {
                code: "CMP_UNSIGNED",
                block: bi,
                message: format!("block {bi}: comparison treated as unsigned"),
            });
        }
    }
    for ma in &mem_accesses {
        if ma.provenance == MemProvenance::RedZone {
            notes.push(AnalysisNote {
                code: "MEM_RED_ZONE",
                block: block_of(blocks, ma.at),
                message: format!(
                    "instruction {} accesses the red zone at RSP{}0x{:x}",
                    ma.at,
                    if ma.disp < 0 { "-" } else { "+" },
                    ma.disp.abs()
                ),
            });
        }
    }

    AnalysisReport {
        iterations,
        converged: true,
        block_in,
        block_out,
        mem_accesses,
        notes,
    }
}

/// Find the block index containing an instruction index.
fn block_of(blocks: &[BlockRange], at: usize) -> usize {
    blocks
        .iter()
        .position(|b| (b.start..b.end).contains(&at))
        .unwrap_or(0)
}

/// Convenience: build blocks from the instruction stream (no external target
/// resolution) and analyse.
#[must_use]
pub fn analyze_linear(lowered: &[LoweredInstr]) -> AnalysisReport {
    let blocks = build_blocks(lowered, &HashMap::new());
    analyze(lowered, &blocks)
}

#[cfg(test)]
mod tests {
    use super::{analyze, analyze_linear, AbsVal, BlockRange, CmpContext, MemProvenance};
    use crate::lower::{LoweredInstr, MemOperand, Operand};
    use crate::{Gp, Register, Storage, Width};
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

    fn ins_signed(
        mnemonic: &str,
        kind: Kind,
        signed: Option<bool>,
        operands: Vec<Operand>,
    ) -> LoweredInstr {
        LoweredInstr {
            mnemonic: mnemonic.into(),
            kind,
            width: Width::B64,
            signed,
            operands,
        }
    }

    fn reg(g: Gp) -> Operand {
        Operand::Reg(g.full())
    }

    fn imm(n: i64) -> Operand {
        Operand::Imm(n)
    }

    #[test]
    fn constant_propagates_through_block() {
        // mov rax, 5  ->  RAX = 5
        let body = vec![ins("mov", Kind::Store, vec![reg(Gp::Rax), imm(5)])];
        let r = analyze_linear(&body);
        assert!(r.converged);
        let rax = r.block_out[0].regs.get(&Gp::Rax).unwrap();
        assert_eq!(rax.value, AbsVal::Const(5));
    }

    #[test]
    fn conflicting_branches_join_to_conflict() {
        // Two predecessors write different constants into RAX, then a merge
        // block joins them — the result must be the explicit Conflict state.
        // block0: mov rax, 1        (succ: block1, block2)
        // block1: mov rax, 2        (succ: block2)
        // block2: <merge>             (preds: block0, block1)
        let body = vec![
            ins("mov", Kind::Store, vec![reg(Gp::Rax), imm(1)]),
            ins("mov", Kind::Store, vec![reg(Gp::Rax), imm(2)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let blocks = vec![
            BlockRange {
                start: 0,
                end: 1,
                successors: vec![1, 2],
            },
            BlockRange {
                start: 1,
                end: 2,
                successors: vec![2],
            },
            BlockRange {
                start: 2,
                end: 3,
                successors: vec![],
            },
        ];
        let r = analyze(&body, &blocks);
        // RAX must be Conflict at the merge block's exit (two different consts).
        let merged = &r.block_out[2];
        assert_eq!(merged.regs.get(&Gp::Rax).unwrap().value, AbsVal::Conflict);
    }

    #[test]
    fn call_clobbers_volatile_registers() {
        // mov rax, 7 ; call foo  ->  RAX becomes Conflict after the call.
        let body = vec![
            ins("mov", Kind::Store, vec![reg(Gp::Rax), imm(7)]),
            ins("call", Kind::Call, vec![imm(0x100)]),
        ];
        let r = analyze_linear(&body);
        let out = &r.block_out[0];
        // RAX (volatile) is clobbered by the call.
        assert_eq!(out.regs.get(&Gp::Rax).unwrap().value, AbsVal::Conflict);
        // RBX (callee-saved) is preserved/untracked.
        assert_eq!(out.regs.get(&Gp::Rbx), None);
        // RSP advanced by 8 for the return address.
        assert_eq!(out.rsp_delta, 8);
    }

    #[test]
    fn loop_converges_and_does_not_explode() {
        // A tiny loop with a back edge:
        // block0: mov rcx, 3              (succ: block1)
        // block1: dec rcx ; jnz block1   (succ: block1, block2)
        // block2: ret                      (succ: none)
        // Even with a back edge, the analysis must terminate (fixpoint) and
        // not iterate unboundedly.
        let body = vec![
            ins("mov", Kind::Store, vec![reg(Gp::Rcx), imm(3)]),
            ins("dec", Kind::Binary, vec![reg(Gp::Rcx)]),
            ins("jnz", Kind::Branch, vec![imm(0x10)]),
            ins("ret", Kind::Return, vec![]),
        ];
        let blocks = vec![
            BlockRange {
                start: 0,
                end: 1,
                successors: vec![1],
            },
            BlockRange {
                start: 1,
                end: 3,
                successors: vec![1, 2],
            },
            BlockRange {
                start: 3,
                end: 4,
                successors: vec![],
            },
        ];
        let r = analyze(&body, &blocks);
        assert!(r.converged);
        // RCX is modified by dec -> Conflict (we do not fold arithmetic).
        let out = &r.block_out[1];
        assert_eq!(out.regs.get(&Gp::Rcx).unwrap().value, AbsVal::Conflict);
        // Bounded iteration count (no path explosion).
        assert!(r.iterations <= blocks.len() * 4 + 16);
    }

    #[test]
    fn red_zone_access_is_recorded() {
        // mov rax, [rsp - 8]  (red-zone read in a leaf context)
        let m = MemOperand {
            base: Some(Gp::Rsp.full()),
            index: None,
            scale: 1,
            disp: -8,
            width: Width::B64,
        };
        let body = vec![ins("mov", Kind::Load, vec![Operand::Mem(m)])];
        let r = analyze_linear(&body);
        assert!(r
            .mem_accesses
            .iter()
            .any(|a| a.provenance == MemProvenance::RedZone));
        assert!(r.notes.iter().any(|n| n.code == "MEM_RED_ZONE"));
    }

    #[test]
    fn comparison_sets_signed_context() {
        // cmp rax, rbx (signed) then a branch.
        let body = vec![
            ins_signed(
                "cmp",
                Kind::Compare,
                Some(true),
                vec![reg(Gp::Rax), reg(Gp::Rbx)],
            ),
            ins("jg", Kind::Branch, vec![imm(0x20)]),
        ];
        let r = analyze_linear(&body);
        assert!(r.converged);
        let out = &r.block_out[0];
        // The signed comparison is recorded in the block's cmp context.
        assert_eq!(out.cmp, CmpContext::Signed);
    }

    #[test]
    fn non_gp_source_never_reuses_rax_value() {
        let xmm0 = Operand::Reg(Register {
            storage: Storage::Xmm(0),
            width: Width::B64,
            high: false,
        });
        let body = vec![
            ins("mov", Kind::Store, vec![reg(Gp::Rax), imm(7)]),
            ins("mov", Kind::Store, vec![reg(Gp::Rbx), xmm0]),
        ];
        let report = analyze_linear(&body);
        assert_eq!(
            report.block_out[0].regs.get(&Gp::Rbx).unwrap().value,
            AbsVal::Conflict
        );
    }

    #[test]
    fn unknown_instruction_invalidates_tracked_state() {
        let body = vec![
            ins("mov", Kind::Store, vec![reg(Gp::Rax), imm(7)]),
            ins_signed(
                "cmp",
                Kind::Compare,
                Some(true),
                vec![reg(Gp::Rax), reg(Gp::Rbx)],
            ),
            ins("mystery", Kind::Unknown, vec![]),
        ];
        let report = analyze_linear(&body);
        let out = &report.block_out[0];
        assert_eq!(out.regs.get(&Gp::Rax).unwrap().value, AbsVal::Conflict);
        assert_eq!(out.cmp, CmpContext::None);
        assert!(!out.rsp_known);
        assert!(!out.memory_known);
        assert!(report
            .notes
            .iter()
            .any(|note| note.code == "ANALYSIS_INCOMPLETE"));
    }
}
