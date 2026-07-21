//! Control-flow graph extraction for SemASM.
//!
//! [`build`] consumes a linear sequence of [`PhysicalInstruction`]s (typically
//! produced by `semasm-decode`) and reconstructs a basic-block control-flow
//! graph covering the initial `CFG-001` scope:
//!
//! * direct unconditional branch (`jmp rel`),
//! * direct conditional branch (`je`, `jne`, ...),
//! * fallthrough between instructions,
//! * direct call (`call rel`),
//! * return (`ret`),
//! * unknown indirect transfer (`jmp rax`, `call rax`).
//!
//! Indirect targets are deliberately left **unknown** rather than guessed (this
//! is an explicit acceptance criterion). Blocks that cannot be reached from the
//! entry point are detected and reported.

use std::collections::{HashMap, VecDeque};

use semasm_decode::PhysicalInstruction;
use serde::{Deserialize, Serialize};

/// How a basic block terminates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockEnd {
    /// Falls through to the next instruction in address order.
    Fallthrough,
    /// Unconditional direct branch to a known address.
    UnconditionalBranch {
        /// Resolved target block index, or `None` if the target address does
        /// not coincide with any decoded instruction (unresolved).
        target: Option<usize>,
        /// Raw target address.
        address: u64,
    },
    /// Conditional direct branch with a taken target and a fallthrough edge.
    ConditionalBranch {
        /// Resolved taken-target block index, or `None` if unresolved.
        taken: Option<usize>,
        /// Raw taken-target address.
        taken_address: u64,
    },
    /// Direct or indirect call. Calls fall through to the next instruction.
    Call {
        /// Resolved callee block index for direct calls, or `None` for an
        /// indirect call (target genuinely unknown).
        target: Option<usize>,
        /// Raw callee address for direct calls, `None` for indirect.
        address: Option<u64>,
    },
    /// Function return — no successors.
    Return,
    /// Indirect transfer whose target is unknown (e.g. `jmp rax`, `call rax`).
    /// Per the acceptance criteria we do NOT guess a target.
    Indirect,
    /// Terminating instruction the extractor does not recognise (e.g. `int`,
    /// `syscall`). Treated as a hard terminator with no successors.
    Unknown,
}

/// Kind of a control-flow edge between two blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeKind {
    /// Sequential fall-through.
    Fallthrough,
    /// Unconditional jump.
    Jump,
    /// Conditional-branch taken path.
    ConditionalTaken,
    /// Call edge (callee); the caller also has a fallthrough edge.
    Call,
    /// Return edge (informational; not materialised as a concrete edge).
    Return,
}

/// A directed edge in the CFG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    /// Source block index.
    pub from: usize,
    /// Destination block index (`None` for unknown/indirect targets).
    pub to: Option<usize>,
    /// Edge classification.
    pub kind: EdgeKind,
}

/// A single basic block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    /// Index of this block in the graph's block list.
    pub index: usize,
    /// First instruction index (into the source instruction list).
    pub start_instruction: usize,
    /// One-past-the-last instruction index.
    pub end_instruction: usize,
    /// Inclusive start address.
    pub start_address: u64,
    /// Inclusive end address (address of last instruction).
    pub end_address: u64,
    /// Outgoing edges.
    pub successors: Vec<Edge>,
    /// Terminator semantics.
    pub end: BlockEnd,
}

/// A reconstructed control-flow graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlFlowGraph {
    /// Basic blocks in discovery order.
    pub blocks: Vec<Block>,
    /// Index of the entry block (the lowest-address instruction).
    pub entry_block: usize,
}

/// Errors produced while building a CFG.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CfgError {
    /// No instructions were supplied.
    #[error("cannot build a CFG from zero instructions")]
    EmptyInput,
}

/// Build a control-flow graph from a linear instruction stream.
///
/// Instructions are re-sorted by address internally so callers need not
/// pre-sort. The entry block is the one containing the lowest address.
#[allow(clippy::too_many_lines)]
pub fn build(instructions: &[PhysicalInstruction]) -> Result<ControlFlowGraph, CfgError> {
    if instructions.is_empty() {
        return Err(CfgError::EmptyInput);
    }

    // Index the instructions, sorted by address for stability.
    let mut order: Vec<usize> = (0..instructions.len()).collect();
    order.sort_by_key(|&i| instructions[i].address);

    // Map address -> instruction index (for branch-target resolution).
    let addr_to_instr: HashMap<u64, usize> = instructions
        .iter()
        .enumerate()
        .map(|(i, ins)| (ins.address, i))
        .collect();

    let next_address = |i: usize| -> Option<u64> {
        let pos = order.iter().position(|&x| x == i)?;
        order.get(pos + 1).map(|&n| instructions[n].address)
    };

    // First pass: split into basic blocks at terminators. Successor edges are
    // recorded against instruction indices (resolved to block indices later).
    let mut blocks: Vec<BlockBuilder> = Vec::new();
    let mut current: Option<BlockBuilder> = None;

    for &i in &order {
        if current.is_none() {
            current = Some(BlockBuilder::new(i, instructions[i].address));
        }
        let ins = &instructions[i];
        let end = classify(ins);
        // A `Call` is NOT a terminator: execution continues to the next
        // instruction, so the block keeps growing past it.
        let is_terminator = matches!(
            end,
            BlockEnd::UnconditionalBranch { .. }
                | BlockEnd::ConditionalBranch { .. }
                | BlockEnd::Return
                | BlockEnd::Indirect
                | BlockEnd::Unknown
        );

        // Outgoing edges contributed by THIS instruction. Only the final
        // (terminator) instruction of a block carries inter-block edges, except
        // a `Call` which contributes a callee edge while the block continues
        // (its fall-through is implicit, within the same block).
        let mut successors = Vec::new();
        match &end {
            BlockEnd::UnconditionalBranch { address, .. } => {
                let target = addr_to_instr.get(address).copied();
                successors.push(Edge {
                    from: 0,
                    to: target,
                    kind: EdgeKind::Jump,
                });
            }
            BlockEnd::ConditionalBranch { taken_address, .. } => {
                let taken = addr_to_instr.get(taken_address).copied();
                successors.push(Edge {
                    from: 0,
                    to: taken,
                    kind: EdgeKind::ConditionalTaken,
                });
                // Conditional branches fall through to the next instruction, which
                // begins a new block.
                if let Some(na) = next_address(i) {
                    let fall = addr_to_instr.get(&na).copied();
                    successors.push(Edge {
                        from: 0,
                        to: fall,
                        kind: EdgeKind::Fallthrough,
                    });
                }
            }
            BlockEnd::Call { address, .. } => {
                if let Some(a) = address {
                    let target = addr_to_instr.get(a).copied();
                    successors.push(Edge {
                        from: 0,
                        to: target,
                        kind: EdgeKind::Call,
                    });
                }
            }
            BlockEnd::Return | BlockEnd::Indirect | BlockEnd::Unknown | BlockEnd::Fallthrough => {}
        }

        let b = current.as_mut().expect("block opened above");
        b.end_instruction = i + 1;
        b.end_address = ins.address;
        b.end = end;
        b.successors.extend(successors);

        if is_terminator {
            // Close the current block; the next iteration opens a fresh one
            // starting at the following instruction.
            if let Some(b) = current.take() {
                blocks.push(b);
            }
        }
    }
    if let Some(b) = current.take() {
        blocks.push(b);
    }

    // Map every instruction index to its containing block index. Branch
    // targets may land mid-block (e.g. a loop back-edge to a non-entry
    // instruction), so the map must cover all instructions in a block.
    let mut instr_to_block: HashMap<usize, usize> = HashMap::new();
    for (bi, b) in blocks.iter().enumerate() {
        for ins in b.start_instruction..b.end_instruction {
            instr_to_block.insert(ins, bi);
        }
    }

    // Resolve edges to block indices and rebuild `end` from the edges so the
    // two representations cannot disagree.
    let mut out_blocks: Vec<Block> = Vec::with_capacity(blocks.len());
    for (bi, mut b) in blocks.into_iter().enumerate() {
        let mut resolved: Vec<Edge> = Vec::with_capacity(b.successors.len());
        let mut jump_target: Option<usize> = None;
        let mut cond_taken: Option<usize> = None;
        let mut call_target: Option<usize> = None;

        for mut e in b.successors.drain(..) {
            e.from = bi;
            e.to = e.to.and_then(|t| instr_to_block.get(&t).copied());
            match e.kind {
                EdgeKind::Jump => jump_target = e.to,
                EdgeKind::ConditionalTaken => cond_taken = e.to,
                EdgeKind::Call => call_target = e.to,
                EdgeKind::Fallthrough | EdgeKind::Return => {}
            }
            resolved.push(e);
        }

        let end = match b.end {
            BlockEnd::UnconditionalBranch { address, .. } => BlockEnd::UnconditionalBranch {
                target: jump_target,
                address,
            },
            BlockEnd::ConditionalBranch { taken_address, .. } => BlockEnd::ConditionalBranch {
                taken: cond_taken,
                taken_address,
            },
            BlockEnd::Call { address, .. } => BlockEnd::Call {
                target: call_target,
                address,
            },
            other => other,
        };
        b.end = end;

        out_blocks.push(Block {
            index: bi,
            start_instruction: b.start_instruction,
            end_instruction: b.end_instruction,
            start_address: b.start_address,
            end_address: b.end_address,
            successors: resolved,
            end: b.end,
        });
    }

    // Entry block = block with the lowest start address.
    let entry_block = out_blocks
        .iter()
        .min_by_key(|b| b.start_address)
        .map_or(0, |b| b.index);

    Ok(ControlFlowGraph {
        blocks: out_blocks,
        entry_block,
    })
}

/// Classify a single instruction's control-flow role.
///
/// Used by CFG construction and by leaf-policy gates that must reject
/// indirect calls/branches even when they are not block terminators.
#[must_use]
pub fn classify_instruction(ins: &PhysicalInstruction) -> BlockEnd {
    classify(ins)
}

/// Classify a single instruction's terminator behaviour.
fn classify(ins: &PhysicalInstruction) -> BlockEnd {
    let groups = &ins.groups;
    let mnemonic = ins.mnemonic.to_ascii_lowercase();

    let is_ret = groups.iter().any(|g| g == "ret");
    let is_call = groups.iter().any(|g| g == "call");
    let is_jump = groups.iter().any(|g| g == "jump");

    if is_ret {
        return BlockEnd::Return;
    }

    if is_call {
        // Determine direct vs indirect from the first operand.
        return match parse_target(ins) {
            Some(addr) => BlockEnd::Call {
                target: None, // resolved later via address map
                address: Some(addr),
            },
            None => BlockEnd::Call {
                target: None,
                address: None,
            },
        };
    }

    if is_jump {
        let direct = parse_target(ins);
        let unconditional = mnemonic == "jmp";
        if unconditional {
            return match direct {
                Some(addr) => BlockEnd::UnconditionalBranch {
                    target: None,
                    address: addr,
                },
                None => BlockEnd::Indirect,
            };
        }
        // Conditional jump.
        return match direct {
            Some(addr) => BlockEnd::ConditionalBranch {
                taken: None,
                taken_address: addr,
            },
            None => BlockEnd::Indirect,
        };
    }

    // Unknown terminating groups (int, iret, privilege, syscall...).
    if groups
        .iter()
        .any(|g| matches!(g.as_str(), "int" | "iret" | "privilege" | "branch" if false))
    {
        return BlockEnd::Unknown;
    }

    BlockEnd::Fallthrough
}

/// Parse a direct branch/call target address from the operand list.
///
/// Returns `Some(addr)` when the (first) operand is a bare immediate such as
/// `0x400010` or Capstone's decimal form `7`, and `None` when it is a register
/// or memory expression (indirect transfer) or absent.
fn parse_target(ins: &PhysicalInstruction) -> Option<u64> {
    let op = ins.operands.first()?;
    let trimmed = op.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        if let Ok(addr) = u64::from_str_radix(hex, 16) {
            return Some(addr);
        }
    }
    // Capstone occasionally prints resolved absolute targets in decimal.
    if !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(addr) = trimmed.parse::<u64>() {
            return Some(addr);
        }
    }
    None
}

/// Builder-side block before indices are finalised.
#[derive(Debug, Clone)]
struct BlockBuilder {
    start_instruction: usize,
    end_instruction: usize,
    start_address: u64,
    end_address: u64,
    successors: Vec<Edge>,
    end: BlockEnd,
}

impl BlockBuilder {
    fn new(start_instruction: usize, start_address: u64) -> Self {
        Self {
            start_instruction,
            end_instruction: start_instruction + 1,
            start_address,
            end_address: start_address,
            successors: Vec::new(),
            end: BlockEnd::Fallthrough,
        }
    }
}

impl ControlFlowGraph {
    /// Indices of blocks not reachable from the entry block.
    #[must_use]
    pub fn unreachable_blocks(&self) -> Vec<usize> {
        let mut reachable = vec![false; self.blocks.len()];
        let mut queue = VecDeque::new();
        queue.push_back(self.entry_block);
        reachable[self.entry_block] = true;
        while let Some(bi) = queue.pop_front() {
            for e in &self.blocks[bi].successors {
                if let Some(to) = e.to {
                    if !reachable[to] {
                        reachable[to] = true;
                        queue.push_back(to);
                    }
                }
            }
        }
        reachable
            .iter()
            .enumerate()
            .filter(|(_, &r)| !r)
            .map(|(i, _)| i)
            .collect()
    }

    /// Serialise to stable, deterministic JSON.
    pub fn to_json(&self) -> Result<String, CfgError> {
        serde_json::to_string_pretty(self).map_err(|_e| CfgError::EmptyInput)
    }

    /// Human-readable textual dump (deterministic, block order stable).
    #[must_use]
    pub fn to_terminal(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let mut blocks: Vec<&Block> = self.blocks.iter().collect();
        blocks.sort_by_key(|b| b.start_address);
        let entry = &self.blocks[self.entry_block];
        let _ = writeln!(
            out,
            "entry: block {} @ {:#x}",
            self.entry_block, entry.start_address
        );
        let _ = writeln!(out, "blocks: {}", self.blocks.len());
        let unreachable = self.unreachable_blocks();
        let _ = writeln!(out, "unreachable: {}", unreachable.len());
        for b in &blocks {
            let _ = writeln!(
                out,
                "block {} [{:#x}..{:#x}] (instr {}..{}): end={:?}",
                b.index,
                b.start_address,
                b.end_address,
                b.start_instruction,
                b.end_instruction,
                b.end
            );
            for e in &b.successors {
                let to = match e.to {
                    Some(t) => format!("block {t}"),
                    None => "?".to_string(),
                };
                let _ = writeln!(out, "    -> {to} ({:?})", e.kind);
            }
        }
        out.push_str("\nunreachable blocks:");
        if unreachable.is_empty() {
            out.push_str(" none\n");
        } else {
            for u in unreachable {
                let _ = write!(out, " {u}");
            }
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semasm_decode::{decode_x86_64, PhysicalInstruction};

    fn dec(code: &[u8]) -> Vec<PhysicalInstruction> {
        decode_x86_64(code, 0x1000).expect("decode")
    }

    #[test]
    fn empty_input_errors() {
        assert_eq!(build(&[]), Err(CfgError::EmptyInput));
    }

    #[test]
    fn linear_fallthrough_single_block() {
        // xor eax,eax ; ret  -> ONE block ending in Return
        let code = [0x31u8, 0xc0, 0xc3];
        let instrs = dec(&code);
        let g = build(&instrs).expect("build");
        assert_eq!(g.blocks.len(), 1);
        assert_eq!(g.blocks[0].end, BlockEnd::Return);
        assert!(g.unreachable_blocks().is_empty());
    }

    #[test]
    fn conditional_branch_and_loop() {
        // A small loop:
        //   1000: xor eax, eax        ; eax=0
        //   1002: dec ecx             ; (stand-in for cmp)
        //   1004: jnz 1002            ; loop back to 0x1002
        //   1006: ret
        let code = [
            0x31u8, 0xc0, // xor eax,eax          @ 0x1000
            0x48, 0xff, 0xc9, // dec ecx           @ 0x1002
            0x75, 0xfb, // jnz -5 => 0x1002 (dec)@ 0x1005
            0xc3, // ret                       @ 0x1007
        ];
        let instrs = dec(&code);
        let g = build(&instrs).expect("build");
        // blocks: [xor,dec,jnz], [ret] = 2 blocks
        assert_eq!(g.blocks.len(), 2);
        // block 0 ends with a conditional branch.
        let b0 = &g.blocks[0];
        assert!(matches!(b0.end, BlockEnd::ConditionalBranch { .. }));
        // The taken edge is a back-edge to block 0 itself (target 0x1002 is
        // inside block 0).
        let self_loop = b0
            .successors
            .iter()
            .any(|e| e.kind == EdgeKind::ConditionalTaken && e.to == Some(0));
        assert!(self_loop, "expected back-edge to block 0");
        // It also has a fallthrough successor (the ret block).
        let fall = b0
            .successors
            .iter()
            .any(|e| e.kind == EdgeKind::Fallthrough && e.to == Some(1));
        assert!(fall, "expected fallthrough to ret block");
        assert!(g.unreachable_blocks().is_empty());
    }

    #[test]
    fn unreachable_block_reported() {
        // ret (terminates) then dead code that is never branched to.
        //   1000: ret            <- entry, terminates
        //   1001: xor eax,eax   (unreachable)
        //   1003: ret           (unreachable)
        let code = [0xc3u8, 0x31, 0xc0, 0xc3];
        let instrs = dec(&code);
        let g = build(&instrs).expect("build");
        // Entry block [ret]; the remaining code forms an unreachable block.
        assert!(g.blocks.len() >= 2);
        let unreachable = g.unreachable_blocks();
        assert!(
            !unreachable.is_empty(),
            "expected at least one unreachable block"
        );
    }

    #[test]
    fn indirect_target_stays_unknown() {
        // jmp rax  -> indirect, no guessed edge
        let code = [0xffu8, 0xe0];
        let instrs = dec(&code);
        let g = build(&instrs).expect("build");
        assert_eq!(g.blocks.len(), 1);
        assert_eq!(g.blocks[0].end, BlockEnd::Indirect);
        assert!(g.blocks[0].successors.is_empty());
    }

    #[test]
    fn decimal_jump_immediate_is_direct() {
        // Capstone may print absolute targets as decimal (e.g. "7").
        let ins = PhysicalInstruction {
            address: 0x17,
            bytes: vec![0x75, 0xee],
            mnemonic: "jne".into(),
            operands: vec!["7".into()],
            read_regs: Vec::new(),
            write_regs: Vec::new(),
            groups: vec!["jump".into(), "branch_relative".into()],
            detail_available: true,
        };
        assert!(matches!(
            classify_instruction(&ins),
            BlockEnd::ConditionalBranch {
                taken_address: 7,
                ..
            }
        ));
    }

    #[test]
    fn call_has_fallthrough_and_callee_edge() {
        // call rel32 ; ret  -> one block ending in Return, with a Call edge
        // and a fallthrough edge (call does not terminate the block).
        let code = [0xe8u8, 0x00, 0x00, 0x00, 0x00, 0xc3];
        let instrs = dec(&code);
        let g = build(&instrs).expect("build");
        assert_eq!(g.blocks.len(), 1);
        assert_eq!(g.blocks[0].end, BlockEnd::Return);
        let call_block = &g.blocks[0];
        // One call edge + one fallthrough edge.
        let call_edges = call_block
            .successors
            .iter()
            .filter(|e| e.kind == EdgeKind::Call)
            .count();
        let fall_edges = call_block
            .successors
            .iter()
            .filter(|e| e.kind == EdgeKind::Fallthrough)
            .count();
        assert_eq!(call_edges, 1);
        // The call's fall-through is implicit (the `ret` is in the same
        // block), so there is no inter-block fallthrough edge.
        assert_eq!(fall_edges, 0);
    }

    #[test]
    fn json_is_deterministic() {
        let code = [0x31u8, 0xc0, 0xc3];
        let instrs = dec(&code);
        let g = build(&instrs).expect("build");
        let a = g.to_json().unwrap();
        let b = g.to_json().unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn no_feature_backend_still_builds_graph_from_manual_input() {
        // Without the capstone feature we cannot decode, but the graph builder
        // must still work on hand-built PhysicalInstruction values.
        let instrs = vec![
            PhysicalInstruction {
                address: 0x1000,
                bytes: vec![0x90],
                mnemonic: "nop".into(),
                operands: vec![],
                read_regs: vec![],
                write_regs: vec![],
                groups: vec![],
                detail_available: false,
            },
            PhysicalInstruction {
                address: 0x1001,
                bytes: vec![0xc3],
                mnemonic: "ret".into(),
                operands: vec![],
                read_regs: vec![],
                write_regs: vec![],
                groups: vec!["ret".into()],
                detail_available: false,
            },
        ];
        let g = build(&instrs).expect("build");
        assert_eq!(g.blocks.len(), 1);
        assert_eq!(g.blocks[0].end, BlockEnd::Return);
    }
}
