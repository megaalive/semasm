//! x86 memory-effect extraction for Region/Alias Evidence v1 (ADR 0006).

use std::collections::HashMap;

use semasm_contract::{AccessAddr, AccessMode, CheckedContract, ObservedMemoryAccess, SemType};
use semasm_decode::PhysicalInstruction;
use semasm_x86::lower::{LoweredInstr, MemOperand, Operand};
use semasm_x86::{Gp, Storage, Width};

/// Calling convention used to seed entry-parameter affinities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiConvention {
    /// System V AMD64: rdi, rsi, rdx, rcx, r8, r9.
    SysV,
    /// Windows x64: rcx, rdx, r8, r9.
    Win64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FrameSlot {
    Rbp(i64),
    Rsp(i64),
}

/// Collect observed memory accesses for Region/Alias Evidence v1.
#[cfg(test)]
#[must_use]
pub fn collect_memory_effects(
    lowered: &[LoweredInstr],
    contract: &CheckedContract,
    abi: AbiConvention,
) -> Vec<ObservedMemoryAccess> {
    collect_memory_effects_with_cfg(lowered, &[], contract, abi)
}

/// Collect memory effects with physical addresses available for narrow,
/// edge-confirmed loop induction.
#[must_use]
pub fn collect_memory_effects_with_cfg(
    lowered: &[LoweredInstr],
    physical: &[PhysicalInstruction],
    contract: &CheckedContract,
    abi: AbiConvention,
) -> Vec<ObservedMemoryAccess> {
    let mut affinity: HashMap<Gp, String> = HashMap::new();
    let mut stack_slots: HashMap<FrameSlot, String> = HashMap::new();
    // Fb5: GP registers known to hold a constant (mov imm / xor-zero).
    let mut consts: HashMap<Gp, u64> = HashMap::new();
    // Fb6: exclusive upper bounds from `cmp`+`jae`/`jge` fall-through guards.
    let mut uppers: HashMap<Gp, u64> = HashMap::new();
    let mut pending_cmp: Option<(Gp, u64)> = None;
    // Fb7: post-test count-up induction. Fb8: countdown (dec) induction.
    // Maps instruction index → (index GP, exclusive upper bound).
    let mut inductions = discover_post_test_inductions(lowered);
    for (k, v) in discover_countdown_inductions(lowered) {
        inductions.entry(k).or_insert(v);
    }
    for (k, v) in discover_cfg_pre_test_inductions(lowered, physical) {
        inductions.entry(k).or_insert(v);
    }
    for (k, v) in discover_cfg_post_test_inductions(lowered, physical) {
        inductions.entry(k).or_insert(v);
    }
    seed_param_affinity(&mut affinity, contract, abi);

    let mut out = Vec::new();
    for (idx, instr) in lowered.iter().enumerate() {
        let induction = inductions.get(&idx).copied();
        record_accesses(instr, &affinity, &consts, &uppers, induction, &mut out);
        update_affinity(
            instr,
            &mut affinity,
            &mut stack_slots,
            &mut consts,
            &mut uppers,
            &mut pending_cmp,
        );
    }
    out
}

/// Fb9a: a narrow CFG-confirmed pre-test loop:
/// `xor idx,idx; header: cmp idx,N; jae exit; ... access ...; inc idx; jmp header`.
///
/// Both branch destinations must resolve to physical instruction addresses,
/// and the body must not otherwise write the index. This is structured-loop
/// evidence only, not arbitrary invariant inference (Fb9c).
fn discover_cfg_pre_test_inductions(
    lowered: &[LoweredInstr],
    physical: &[PhysicalInstruction],
) -> HashMap<usize, (Gp, u64)> {
    let mut out = HashMap::new();
    if physical.len() != lowered.len() {
        return out;
    }
    let by_address: HashMap<u64, usize> = physical
        .iter()
        .enumerate()
        .map(|(idx, instr)| (instr.address, idx))
        .collect();

    for header in 0..lowered.len().saturating_sub(2) {
        let cmp = &lowered[header];
        if !cmp.mnemonic.eq_ignore_ascii_case("cmp") {
            continue;
        }
        let (Some(Operand::Reg(reg)), Some(Operand::Imm(bound))) =
            (cmp.operands.first(), cmp.operands.get(1))
        else {
            continue;
        };
        let Storage::Gp(gp) = reg.storage else {
            continue;
        };
        if *bound <= 0 || !index_was_zero_initialized(lowered, header, gp) {
            continue;
        }
        let guard = &lowered[header + 1];
        if !matches!(
            guard.mnemonic.to_ascii_lowercase().as_str(),
            "jae" | "jnb" | "jge"
        ) {
            continue;
        }
        let Some(exit) = branch_target_index(&physical[header + 1], &by_address) else {
            continue;
        };
        if exit <= header + 2 || exit > lowered.len() {
            continue;
        }
        let back = exit - 1;
        if !lowered[back].mnemonic.eq_ignore_ascii_case("jmp")
            || branch_target_index(&physical[back], &by_address) != Some(header)
        {
            continue;
        }

        let mut saw_inc = false;
        let mut accesses = Vec::new();
        let mut valid = true;
        for (at, instr) in lowered.iter().enumerate().take(back).skip(header + 2) {
            if writes_gp(instr, gp) {
                let is_inc = instr.mnemonic.eq_ignore_ascii_case("inc")
                    && matches!(
                        instr.operands.first(),
                        Some(Operand::Reg(r)) if r.storage == Storage::Gp(gp)
                    );
                if !is_inc || saw_inc {
                    valid = false;
                    break;
                }
                saw_inc = true;
            }
            if instr.operands.iter().any(|op| {
                matches!(
                    op,
                    Operand::Mem(mem)
                        if matches!(mem.index, Some(index) if index.storage == Storage::Gp(gp))
                )
            }) {
                accesses.push(at);
            }
        }
        if valid && saw_inc {
            for at in accesses {
                let Ok(bound_u) = u64::try_from(*bound) else {
                    continue;
                };
                out.insert(at, (gp, bound_u));
            }
        }
    }
    out
}

fn branch_target_index(
    instr: &PhysicalInstruction,
    by_address: &HashMap<u64, usize>,
) -> Option<usize> {
    let raw = instr.operands.first()?.trim();
    let raw = raw.strip_prefix("0x").unwrap_or(raw);
    let target = u64::from_str_radix(raw, 16).ok()?;
    by_address.get(&target).copied()
}

/// Fb9b: CFG-confirmed post-test count-up:
/// `xor idx,idx; header: access; inc idx; cmp idx,N; jb header`.
///
/// The conditional back-edge must resolve to the access instruction, and no
/// other body write may touch `idx`. Complements Fb9a; arbitrary invariant
/// inference remains Fb9c locked.
fn discover_cfg_post_test_inductions(
    lowered: &[LoweredInstr],
    physical: &[PhysicalInstruction],
) -> HashMap<usize, (Gp, u64)> {
    let mut out = HashMap::new();
    if physical.len() != lowered.len() {
        return out;
    }
    let by_address: HashMap<u64, usize> = physical
        .iter()
        .enumerate()
        .map(|(idx, instr)| (instr.address, idx))
        .collect();

    for access_at in 0..lowered.len().saturating_sub(3) {
        let access = &lowered[access_at];
        let Some(idx_gp) = indexed_gp_in_access(access) else {
            continue;
        };
        if !index_was_zero_initialized(lowered, access_at, idx_gp) {
            continue;
        }

        let mut at = access_at + 1;
        // Allow a short stretch of non-index-writing ops before the inc.
        while at < lowered.len() && !writes_gp(&lowered[at], idx_gp) {
            if at - access_at > 4 {
                break;
            }
            at += 1;
        }
        if at >= lowered.len() {
            continue;
        }
        let inc = &lowered[at];
        let is_inc = inc.mnemonic.eq_ignore_ascii_case("inc")
            && matches!(
                inc.operands.first(),
                Some(Operand::Reg(r)) if r.storage == Storage::Gp(idx_gp)
            );
        let is_add1 = inc.mnemonic.eq_ignore_ascii_case("add")
            && matches!(
                (inc.operands.first(), inc.operands.get(1)),
                (Some(Operand::Reg(r)), Some(Operand::Imm(1)))
                    if r.storage == Storage::Gp(idx_gp)
            );
        if !is_inc && !is_add1 {
            continue;
        }

        let cmp_at = at + 1;
        let br_at = at + 2;
        if br_at >= lowered.len() {
            continue;
        }
        let cmp = &lowered[cmp_at];
        if !cmp.mnemonic.eq_ignore_ascii_case("cmp") {
            continue;
        }
        let (Some(Operand::Reg(reg)), Some(Operand::Imm(bound))) =
            (cmp.operands.first(), cmp.operands.get(1))
        else {
            continue;
        };
        if reg.storage != Storage::Gp(idx_gp) || *bound <= 0 {
            continue;
        }
        let br = &lowered[br_at];
        if !matches!(
            br.mnemonic.to_ascii_lowercase().as_str(),
            "jb" | "jnae" | "jl" | "jnge"
        ) {
            continue;
        }
        if branch_target_index(&physical[br_at], &by_address) != Some(access_at) {
            continue;
        }
        let Ok(bound_u) = u64::try_from(*bound) else {
            continue;
        };
        out.insert(access_at, (idx_gp, bound_u));
    }
    out
}

fn indexed_gp_in_access(instr: &LoweredInstr) -> Option<Gp> {
    for op in &instr.operands {
        let Operand::Mem(mem) = op else {
            continue;
        };
        let index = mem.index?;
        if let Storage::Gp(gp) = index.storage {
            return Some(gp);
        }
    }
    None
}

/// Fb7: detect post-test counted loops of the form
/// `… access [base+idx] …; inc idx; cmp idx, N; jb/jl …`
/// where `idx` was zero-initialized earlier.
///
/// Maps instruction index of the memory access → (index GP, exclusive upper bound N).
/// Honesty: linear pattern match ≠ CFG-sound arbitrary loop induction.
fn discover_post_test_inductions(lowered: &[LoweredInstr]) -> HashMap<usize, (Gp, u64)> {
    let mut out = HashMap::new();
    for (idx, instr) in lowered.iter().enumerate() {
        for op in &instr.operands {
            let Operand::Mem(mem) = op else {
                continue;
            };
            let Some(index) = mem.index else {
                continue;
            };
            let Storage::Gp(idx_gp) = index.storage else {
                continue;
            };
            if !index_was_zero_initialized(lowered, idx, idx_gp) {
                continue;
            }
            if let Some(bound) = post_test_bound_after(lowered, idx + 1, idx_gp) {
                out.insert(idx, (idx_gp, bound));
            }
        }
    }
    out
}

fn index_was_zero_initialized(lowered: &[LoweredInstr], before: usize, gp: Gp) -> bool {
    for instr in lowered[..before].iter().rev() {
        let mnemonic = instr.mnemonic.to_ascii_lowercase();
        match mnemonic.as_str() {
            "xor" => {
                if let (Some(Operand::Reg(dst)), Some(Operand::Reg(src))) =
                    (instr.operands.first(), instr.operands.get(1))
                {
                    if let (Storage::Gp(d), Storage::Gp(s)) = (dst.storage, src.storage) {
                        if d == gp && s == gp {
                            return true;
                        }
                        if d == gp {
                            return false;
                        }
                    }
                }
            }
            "mov" | "movabs" => {
                if let Some(Operand::Reg(dst)) = instr.operands.first() {
                    if let Storage::Gp(d) = dst.storage {
                        if d == gp {
                            return matches!(instr.operands.get(1), Some(Operand::Imm(0)));
                        }
                    }
                }
            }
            "inc" | "dec" | "add" | "sub" | "adc" | "sbb" | "pop" | "movzx" | "movsx"
            | "movsxd" | "and" | "or" | "lea" | "imul" | "mul" | "neg" | "not" | "shl" | "shr" => {
                if let Some(Operand::Reg(dst)) = instr.operands.first() {
                    if let Storage::Gp(d) = dst.storage {
                        if d == gp {
                            return false;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    false
}

fn post_test_bound_after(lowered: &[LoweredInstr], from: usize, gp: Gp) -> Option<u64> {
    let mut saw_inc = false;
    for (offset, instr) in lowered[from..].iter().enumerate() {
        let at = from + offset;
        let mnemonic = instr.mnemonic.to_ascii_lowercase();
        match mnemonic.as_str() {
            "inc" => {
                if let Some(Operand::Reg(dst)) = instr.operands.first() {
                    if let Storage::Gp(d) = dst.storage {
                        if d == gp {
                            if saw_inc {
                                return None;
                            }
                            saw_inc = true;
                        }
                    }
                }
            }
            "add" => {
                if let (Some(Operand::Reg(dst)), Some(Operand::Imm(1))) =
                    (instr.operands.first(), instr.operands.get(1))
                {
                    if let Storage::Gp(d) = dst.storage {
                        if d == gp {
                            if saw_inc {
                                return None;
                            }
                            saw_inc = true;
                            continue;
                        }
                    }
                }
                if let Some(Operand::Reg(dst)) = instr.operands.first() {
                    if let Storage::Gp(d) = dst.storage {
                        if d == gp {
                            return None;
                        }
                    }
                }
            }
            "cmp" if saw_inc => {
                if let (Some(Operand::Reg(reg)), Some(Operand::Imm(imm))) =
                    (instr.operands.first(), instr.operands.get(1))
                {
                    if let Storage::Gp(r) = reg.storage {
                        if r == gp && *imm > 0 {
                            let next = lowered.get(at + 1)?;
                            let br = next.mnemonic.to_ascii_lowercase();
                            // jb/jl: continue while index < bound after the inc.
                            if matches!(br.as_str(), "jb" | "jnae" | "jl" | "jnge") {
                                return u64::try_from(*imm).ok();
                            }
                        }
                    }
                }
                return None;
            }
            "dec" | "sub" | "mov" | "movabs" | "xor" | "pop" | "lea" | "movzx" | "and" | "or" => {
                if let Some(Operand::Reg(dst)) = instr.operands.first() {
                    if let Storage::Gp(d) = dst.storage {
                        if d == gp {
                            return None;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Fb8: detect countdown loops of the form
/// `mov idx, N; …; dec idx; access [base+idx]; …; jnz/jns …`
/// where N > 0 is a literal. After `dec`, idx ∈ [0, N) at the access.
///
/// Honesty: linear countdown pattern ≠ CFG-sound arbitrary loop induction
/// (Fb9c). Complements Fb7 count-up post-test induction.
fn discover_countdown_inductions(lowered: &[LoweredInstr]) -> HashMap<usize, (Gp, u64)> {
    let mut out = HashMap::new();
    for (idx, instr) in lowered.iter().enumerate() {
        for op in &instr.operands {
            let Operand::Mem(mem) = op else {
                continue;
            };
            let Some(index) = mem.index else {
                continue;
            };
            let Storage::Gp(idx_gp) = index.storage else {
                continue;
            };
            let Some(bound) = countdown_bound_for_access(lowered, idx, idx_gp) else {
                continue;
            };
            out.insert(idx, (idx_gp, bound));
        }
    }
    out
}

fn countdown_bound_for_access(lowered: &[LoweredInstr], access_at: usize, gp: Gp) -> Option<u64> {
    // Immediately before the access (allowing only non-gp-writing ops), require `dec gp`.
    let mut dec_at = None;
    for i in (0..access_at).rev() {
        let instr = &lowered[i];
        let mnemonic = instr.mnemonic.to_ascii_lowercase();
        if mnemonic == "dec" {
            if let Some(Operand::Reg(dst)) = instr.operands.first() {
                if let Storage::Gp(d) = dst.storage {
                    if d == gp {
                        dec_at = Some(i);
                        break;
                    }
                }
            }
        }
        if writes_gp(instr, gp) {
            return None;
        }
        // Skip a few non-writing ops between dec and access.
        if access_at - i > 4 {
            return None;
        }
    }
    let dec_at = dec_at?;

    // After the access, require jnz/jns (continue while non-zero / non-negative).
    let mut saw_back = false;
    for instr in &lowered[access_at + 1..] {
        let mnemonic = instr.mnemonic.to_ascii_lowercase();
        if matches!(mnemonic.as_str(), "jnz" | "jne" | "jns") {
            saw_back = true;
            break;
        }
        if writes_gp(instr, gp) || matches!(mnemonic.as_str(), "jmp" | "ret" | "jae" | "jge" | "je")
        {
            break;
        }
        // Bound the lookahead.
        if !matches!(
            mnemonic.as_str(),
            "nop" | "test" | "cmp" | "and" | "or" | "xor" | "mov" | "movzx"
        ) {
            // Allow a short unrelated stretch; stop on other control flow.
            if matches!(
                mnemonic.as_str(),
                "call" | "ja" | "jb" | "jg" | "jl" | "jle" | "jbe"
            ) {
                break;
            }
        }
    }
    if !saw_back {
        return None;
    }

    // Before the dec, require `mov gp, Imm(N)` with N > 0 (and no other writes).
    for i in (0..dec_at).rev() {
        let instr = &lowered[i];
        let mnemonic = instr.mnemonic.to_ascii_lowercase();
        match mnemonic.as_str() {
            "mov" | "movabs" => {
                if let (Some(Operand::Reg(dst)), Some(Operand::Imm(imm))) =
                    (instr.operands.first(), instr.operands.get(1))
                {
                    if let Storage::Gp(d) = dst.storage {
                        if d == gp {
                            if *imm > 0 {
                                return u64::try_from(*imm).ok();
                            }
                            return None;
                        }
                    }
                }
                if writes_gp(instr, gp) {
                    return None;
                }
            }
            _ if writes_gp(instr, gp) => return None,
            _ => {}
        }
    }
    None
}

fn writes_gp(instr: &LoweredInstr, gp: Gp) -> bool {
    let mnemonic = instr.mnemonic.to_ascii_lowercase();
    match mnemonic.as_str() {
        "mov" | "movabs" | "movzx" | "movsx" | "movsxd" | "lea" | "pop" | "xor" | "and" | "or"
        | "add" | "sub" | "adc" | "sbb" | "inc" | "dec" | "neg" | "not" | "imul" | "mul"
        | "shl" | "shr" | "sal" | "sar" | "rol" | "ror" => {
            if let Some(Operand::Reg(dst)) = instr.operands.first() {
                if let Storage::Gp(d) = dst.storage {
                    return d == gp;
                }
            }
            false
        }
        _ => false,
    }
}

fn seed_param_affinity(
    affinity: &mut HashMap<Gp, String>,
    contract: &CheckedContract,
    abi: AbiConvention,
) {
    let regs: &[Gp] = match abi {
        AbiConvention::SysV => &[Gp::Rdi, Gp::Rsi, Gp::Rdx, Gp::Rcx, Gp::R8, Gp::R9],
        AbiConvention::Win64 => &[Gp::Rcx, Gp::Rdx, Gp::R8, Gp::R9],
    };
    for (slot, param) in contract.parameters.iter().enumerate() {
        if slot >= regs.len() {
            break;
        }
        if matches!(param.ty, SemType::Ptr { .. }) {
            affinity.insert(regs[slot], param.name.clone());
        }
    }
}

fn record_accesses(
    instr: &LoweredInstr,
    affinity: &HashMap<Gp, String>,
    consts: &HashMap<Gp, u64>,
    uppers: &HashMap<Gp, u64>,
    induction: Option<(Gp, u64)>,
    out: &mut Vec<ObservedMemoryAccess>,
) {
    let mnemonic = instr.mnemonic.to_ascii_lowercase();
    for (idx, op) in instr.operands.iter().enumerate() {
        let Operand::Mem(mem) = op else {
            continue;
        };
        let Some(mode) = access_mode(&mnemonic, idx, instr) else {
            continue;
        };
        out.push(ObservedMemoryAccess {
            mode,
            width_bytes: width_bytes(mem.width),
            addr: classify_addr(mem, affinity, consts, uppers, induction),
            mnemonic: mnemonic.clone(),
            instruction_offset: 0,
        });
    }
}

fn access_mode(mnemonic: &str, operand_index: usize, instr: &LoweredInstr) -> Option<AccessMode> {
    match mnemonic {
        "lea" => None,
        "mov" | "movabs" | "movzx" | "movsx" | "movsxd" => {
            if operand_index == 0 {
                if matches!(instr.operands.first(), Some(Operand::Mem(_))) {
                    Some(AccessMode::Store)
                } else {
                    None
                }
            } else if matches!(instr.operands.get(1), Some(Operand::Mem(_))) {
                Some(AccessMode::Load)
            } else {
                None
            }
        }
        "push" | "stosb" | "stosw" | "stosd" | "stosq" => Some(AccessMode::Store),
        "add" | "sub" | "adc" | "sbb" | "and" | "or" | "xor" | "inc" | "dec" | "not" | "neg"
        | "shl" | "shr" | "sal" | "sar" | "rol" | "ror" | "xchg" => {
            if operand_index == 0 && matches!(instr.operands.first(), Some(Operand::Mem(_))) {
                Some(AccessMode::Store)
            } else if matches!(instr.operands.get(operand_index), Some(Operand::Mem(_))) {
                Some(AccessMode::Load)
            } else {
                None
            }
        }
        // Includes pop/lods* and any other mnemonic with a memory operand.
        _ => Some(AccessMode::Load),
    }
}

fn classify_addr(
    mem: &MemOperand,
    affinity: &HashMap<Gp, String>,
    consts: &HashMap<Gp, u64>,
    uppers: &HashMap<Gp, u64>,
    induction: Option<(Gp, u64)>,
) -> AccessAddr {
    if is_stack_frame(mem) {
        return AccessAddr::StackFrame;
    }
    let Some(base) = mem.base else {
        return AccessAddr::Unknown;
    };
    let Storage::Gp(gp) = base.storage else {
        return AccessAddr::Unknown;
    };
    let Some(param) = affinity.get(&gp) else {
        return AccessAddr::Unknown;
    };
    // Fb4: model indexed form instead of collapsing to Unknown.
    // Fb5: attach index_const when the index GP holds a known constant.
    // Fb6: else attach index_max_exclusive from an active range guard.
    // Fb7: else attach induction max from a post-test counted-loop pattern.
    if let Some(index) = mem.index {
        let (index_const, index_max_exclusive) = match index.storage {
            Storage::Gp(idx_gp) => {
                // Fb7 post-test induction at this site subsumes a live Fb5
                // constant (often 0 from xor-zero before the first iteration):
                // the same access is reached with idx ∈ [0, N).
                if let Some((igp, bound)) = induction {
                    if igp == idx_gp {
                        (None, Some(bound))
                    } else if let Some(c) = consts.get(&idx_gp).copied() {
                        (Some(c), None)
                    } else {
                        (None, uppers.get(&idx_gp).copied())
                    }
                } else if let Some(c) = consts.get(&idx_gp).copied() {
                    (Some(c), None)
                } else {
                    (None, uppers.get(&idx_gp).copied())
                }
            }
            _ => (None, None),
        };
        return AccessAddr::Indexed {
            base_param: param.clone(),
            scale: mem.scale.max(1),
            displacement: mem.disp,
            index_const,
            index_max_exclusive,
        };
    }
    AccessAddr::Affine {
        base_param: param.clone(),
        offset: mem.disp,
    }
}

fn is_stack_frame(mem: &MemOperand) -> bool {
    match mem.base {
        Some(base) if matches!(base.storage, Storage::Gp(Gp::Rsp | Gp::Rbp)) => mem.index.is_none(),
        _ => false,
    }
}

fn frame_slot(mem: &MemOperand) -> Option<FrameSlot> {
    if mem.index.is_some() {
        return None;
    }
    let base = mem.base?;
    match base.storage {
        Storage::Gp(Gp::Rbp) => Some(FrameSlot::Rbp(mem.disp)),
        Storage::Gp(Gp::Rsp) => Some(FrameSlot::Rsp(mem.disp)),
        _ => None,
    }
}

fn width_bytes(width: Width) -> u32 {
    width.bits() / 8
}

#[allow(clippy::too_many_lines)] // Fb affinity / cmp-bound state machine is intentionally dense.
fn update_affinity(
    instr: &LoweredInstr,
    affinity: &mut HashMap<Gp, String>,
    stack_slots: &mut HashMap<FrameSlot, String>,
    consts: &mut HashMap<Gp, u64>,
    uppers: &mut HashMap<Gp, u64>,
    pending_cmp: &mut Option<(Gp, u64)>,
) {
    let mnemonic = instr.mnemonic.to_ascii_lowercase();
    match mnemonic.as_str() {
        "cmp" => {
            // Fb6: remember `cmp reg, imm` so a following jae/jge can arm a
            // fall-through upper bound (reg < imm).
            *pending_cmp = None;
            if let (Some(Operand::Reg(reg)), Some(Operand::Imm(imm))) =
                (instr.operands.first(), instr.operands.get(1))
            {
                if let Storage::Gp(gp) = reg.storage {
                    if let Ok(bound) = u64::try_from(*imm) {
                        *pending_cmp = Some((gp, bound));
                    }
                }
            }
        }
        "jae" | "jnb" | "jge" => {
            // Fall-through path: index < bound from the preceding cmp.
            if let Some((gp, bound)) = pending_cmp.take() {
                uppers.insert(gp, bound);
            }
        }
        "ja" | "jb" | "jbe" | "jg" | "jl" | "jle" | "je" | "jne" | "jz" | "jnz" | "jmp" => {
            // Other branches invalidate a pending cmp without arming a bound.
            *pending_cmp = None;
        }
        "mov" | "movabs" => {
            *pending_cmp = None;
            match (instr.operands.first(), instr.operands.get(1)) {
                // Spill: mov [rbp/rsp+disp], reg — keep param identity in the slot.
                (Some(Operand::Mem(mem)), Some(Operand::Reg(src))) => {
                    if let (Some(slot), Storage::Gp(src_gp)) = (frame_slot(mem), src.storage) {
                        if let Some(name) = affinity.get(&src_gp).cloned() {
                            stack_slots.insert(slot, name);
                        } else {
                            stack_slots.remove(&slot);
                        }
                    }
                }
                // Reload / copy / imm: mov reg, …
                (Some(Operand::Reg(dst)), src) => {
                    let Storage::Gp(dst_gp) = dst.storage else {
                        return;
                    };
                    match src {
                        Some(Operand::Reg(src)) => {
                            if let Storage::Gp(src_gp) = src.storage {
                                if let Some(name) = affinity.get(&src_gp).cloned() {
                                    affinity.insert(dst_gp, name);
                                } else {
                                    affinity.remove(&dst_gp);
                                }
                                if let Some(c) = consts.get(&src_gp).copied() {
                                    consts.insert(dst_gp, c);
                                } else {
                                    consts.remove(&dst_gp);
                                }
                                if let Some(u) = uppers.get(&src_gp).copied() {
                                    uppers.insert(dst_gp, u);
                                } else {
                                    uppers.remove(&dst_gp);
                                }
                            } else {
                                affinity.remove(&dst_gp);
                                consts.remove(&dst_gp);
                                uppers.remove(&dst_gp);
                            }
                        }
                        Some(Operand::Imm(imm)) => {
                            affinity.remove(&dst_gp);
                            if let Ok(c) = u64::try_from(*imm) {
                                consts.insert(dst_gp, c);
                            } else {
                                consts.remove(&dst_gp);
                            }
                            uppers.remove(&dst_gp);
                        }
                        Some(Operand::Mem(mem)) => {
                            // Reload spilled pointer params from the frame.
                            consts.remove(&dst_gp);
                            uppers.remove(&dst_gp);
                            if let Some(slot) = frame_slot(mem) {
                                if let Some(name) = stack_slots.get(&slot).cloned() {
                                    affinity.insert(dst_gp, name);
                                    return;
                                }
                            }
                            affinity.remove(&dst_gp);
                        }
                        _ => {
                            affinity.remove(&dst_gp);
                            consts.remove(&dst_gp);
                            uppers.remove(&dst_gp);
                        }
                    }
                }
                _ => {}
            }
        }
        "lea" => {
            *pending_cmp = None;
            let Some(Operand::Reg(dst)) = instr.operands.first() else {
                return;
            };
            let Storage::Gp(dst_gp) = dst.storage else {
                return;
            };
            consts.remove(&dst_gp);
            uppers.remove(&dst_gp);
            if let Some(Operand::Mem(mem)) = instr.operands.get(1) {
                if let AccessAddr::Affine { base_param, .. } =
                    classify_addr(mem, affinity, consts, uppers, None)
                {
                    affinity.insert(dst_gp, base_param);
                    return;
                }
            }
            affinity.remove(&dst_gp);
        }
        "xor" => {
            *pending_cmp = None;
            // xor reg,reg → constant 0 (Fb5); other xor clears maps.
            if let (Some(Operand::Reg(dst)), Some(Operand::Reg(src))) =
                (instr.operands.first(), instr.operands.get(1))
            {
                if let (Storage::Gp(dst_gp), Storage::Gp(src_gp)) = (dst.storage, src.storage) {
                    if dst_gp == src_gp {
                        affinity.remove(&dst_gp);
                        consts.insert(dst_gp, 0);
                        uppers.remove(&dst_gp);
                        return;
                    }
                    affinity.remove(&dst_gp);
                    consts.remove(&dst_gp);
                    uppers.remove(&dst_gp);
                }
            } else if let Some(Operand::Reg(dst)) = instr.operands.first() {
                if let Storage::Gp(dst_gp) = dst.storage {
                    affinity.remove(&dst_gp);
                    consts.remove(&dst_gp);
                    uppers.remove(&dst_gp);
                }
            }
        }
        "pop" | "movzx" | "movsx" | "movsxd" | "and" | "or" | "imul" | "mul" | "div" | "idiv"
        | "neg" | "not" => {
            *pending_cmp = None;
            if let Some(Operand::Reg(dst)) = instr.operands.first() {
                if let Storage::Gp(dst_gp) = dst.storage {
                    affinity.remove(&dst_gp);
                    consts.remove(&dst_gp);
                    uppers.remove(&dst_gp);
                }
            }
        }
        // Pointer arithmetic keeps param affinity but loses constant / range
        // knowledge (Fb6: clear upper after inc so post-inc accesses need a
        // fresh guard).
        "inc" | "dec" | "add" | "sub" | "adc" | "sbb" | "shl" | "shr" | "sal" | "sar" | "rol"
        | "ror" => {
            *pending_cmp = None;
            if let Some(Operand::Reg(dst)) = instr.operands.first() {
                if let Storage::Gp(dst_gp) = dst.storage {
                    consts.remove(&dst_gp);
                    uppers.remove(&dst_gp);
                }
            }
        }
        _ => {
            *pending_cmp = None;
        }
    }
}

#[cfg(all(test, feature = "capstone"))]
mod tests {
    use super::*;
    use semasm_decode::PhysicalInstruction;
    use semasm_x86::lower::lower;

    fn phys(mnemonic: &str, operands: &[&str]) -> PhysicalInstruction {
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

    fn phys_at(address: u64, mnemonic: &str, operands: &[&str]) -> PhysicalInstruction {
        PhysicalInstruction {
            address,
            ..phys(mnemonic, operands)
        }
    }

    fn count_byte_contract() -> CheckedContract {
        semasm_contract::check_str(
            r#"
contract_version = "0.1"
[function]
name = "count_byte"
[[function.parameters]]
name = "buffer"
type = "ptr<const u8>"
[[function.parameters]]
name = "length"
type = "usize"
[[function.parameters]]
name = "needle"
type = "u8"
[[function.returns]]
name = "count"
type = "usize"
[[function.memory.regions]]
name = "buffer"
base = "buffer"
length = "length"
access = "read"
"#,
        )
        .contract
        .expect("contract")
    }

    #[test]
    fn memcpy_style_accesses_stay_affine() {
        let contract = semasm_contract::check_str(
            r#"
contract_version = "0.1"
[function]
name = "memcpy"
[[function.parameters]]
name = "dst"
type = "ptr<u8>"
[[function.parameters]]
name = "src"
type = "ptr<const u8>"
[[function.parameters]]
name = "length"
type = "usize"
[[function.returns]]
name = "status"
type = "usize"
[[function.memory.regions]]
name = "src"
base = "src"
length = "length"
access = "read"
[[function.memory.regions]]
name = "dst"
base = "dst"
length = "length"
access = "write"
[[function.memory.relations]]
left = "src"
right = "dst"
require = "disjoint"
"#,
        )
        .contract
        .expect("contract");

        let instrs = [
            phys("xor", &["eax", "eax"]),
            phys("test", &["rdx", "rdx"]),
            phys("jz", &["0x20"]),
            phys("mov", &["cl", "byte ptr [rsi]"]),
            phys("mov", &["byte ptr [rdi]", "cl"]),
            phys("inc", &["rdi"]),
            phys("inc", &["rsi"]),
            phys("dec", &["rdx"]),
            phys("jnz", &["0x10"]),
            phys("ret", &[]),
        ];
        let lowered: Vec<_> = instrs
            .iter()
            .filter_map(|p| match lower(p) {
                semasm_x86::lower::Lowering::Lowered(l) => Some(l),
                semasm_x86::lower::Lowering::Unsupported { .. } => None,
            })
            .collect();
        let effects = collect_memory_effects(&lowered, &contract, AbiConvention::SysV);
        let unknowns = effects
            .iter()
            .filter(|e| matches!(e.addr, AccessAddr::Unknown))
            .count();
        assert_eq!(unknowns, 0, "{effects:?}");
    }

    #[test]
    fn win64_frame_spill_reload_keeps_buffer_affine() {
        // HlaX64-style prologue: spill rcx (buffer) to [rbp-8], reload into r10,
        // then byte-load through r10. Without slot tracking this becomes Unknown.
        let contract = count_byte_contract();
        let instrs = [
            phys("push", &["rbp"]),
            phys("mov", &["rbp", "rsp"]),
            phys("sub", &["rsp", "0x20"]),
            phys("mov", &["qword ptr [rbp - 8]", "rcx"]),
            phys("mov", &["qword ptr [rbp - 0x10]", "rdx"]),
            phys("mov", &["qword ptr [rbp - 0x18]", "r8"]),
            phys("mov", &["r8", "0"]),
            phys("mov", &["r9", "0"]),
            phys("mov", &["r10", "qword ptr [rbp - 8]"]),
            phys("movzx", &["r11", "byte ptr [r10]"]),
            phys("add", &["r10", "1"]),
            phys("ret", &[]),
        ];
        let lowered: Vec<_> = instrs
            .iter()
            .filter_map(|p| match lower(p) {
                semasm_x86::lower::Lowering::Lowered(l) => Some(l),
                semasm_x86::lower::Lowering::Unsupported { .. } => None,
            })
            .collect();
        let effects = collect_memory_effects(&lowered, &contract, AbiConvention::Win64);
        let unknowns = effects
            .iter()
            .filter(|e| matches!(e.addr, AccessAddr::Unknown))
            .count();
        assert_eq!(unknowns, 0, "{effects:?}");
        assert!(
            effects.iter().any(|e| {
                matches!(
                    &e.addr,
                    AccessAddr::Affine { base_param, .. } if base_param == "buffer"
                ) && e.width_bytes == 1
            }),
            "expected affine byte load through reloaded buffer pointer: {effects:?}"
        );
    }

    #[test]
    fn unknown_base_is_marked_unknown() {
        let contract = semasm_contract::check_str(
            r#"
contract_version = "0.1"
[function]
name = "f"
[[function.parameters]]
name = "dst"
type = "ptr<u8>"
[[function.returns]]
name = "status"
type = "usize"
[[function.memory.regions]]
name = "dst"
base = "dst"
length = "1"
access = "write"
[[function.memory.relations]]
left = "dst"
right = "dst"
require = "equal"
"#,
        )
        .contract
        .expect("contract");

        let instrs = [phys("mov", &["byte ptr [rax]", "cl"])];
        let lowered: Vec<_> = instrs
            .iter()
            .filter_map(|p| match lower(p) {
                semasm_x86::lower::Lowering::Lowered(l) => Some(l),
                semasm_x86::lower::Lowering::Unsupported { .. } => None,
            })
            .collect();
        let effects = collect_memory_effects(&lowered, &contract, AbiConvention::SysV);
        assert!(effects
            .iter()
            .any(|e| matches!(e.addr, AccessAddr::Unknown)));
    }

    #[test]
    fn constant_index_attaches_index_const() {
        // mov eax, 3; movzx ecx, byte [rdi + rax]  (SysV: rdi = buffer)
        let contract = semasm_contract::check_str(
            r#"
contract_version = "0.1"
[function]
name = "load_at_3"
[[function.parameters]]
name = "buffer"
type = "ptr<const u8>"
[[function.returns]]
name = "value"
type = "usize"
[[function.memory.regions]]
name = "cell"
base = "buffer"
length = "8"
access = "read"
"#,
        )
        .contract
        .expect("contract");

        let instrs = [
            phys("mov", &["eax", "3"]),
            phys("movzx", &["ecx", "byte ptr [rdi + rax*1]"]),
            phys("ret", &[]),
        ];
        let lowered: Vec<_> = instrs
            .iter()
            .filter_map(|p| match lower(p) {
                semasm_x86::lower::Lowering::Lowered(l) => Some(l),
                semasm_x86::lower::Lowering::Unsupported { .. } => None,
            })
            .collect();
        let effects = collect_memory_effects(&lowered, &contract, AbiConvention::SysV);
        assert!(
            effects.iter().any(|e| matches!(
                &e.addr,
                AccessAddr::Indexed {
                    base_param,
                    scale: 1,
                    displacement: 0,
                    index_const: Some(3),
                    ..
                } if base_param == "buffer"
            )),
            "expected Indexed with index_const=3: {effects:?}"
        );
    }

    #[test]
    fn range_guard_attaches_index_max_exclusive() {
        // cmp eax, 8; jae done; movzx ecx, byte [rdi + rax]
        let contract = semasm_contract::check_str(
            r#"
contract_version = "0.1"
[function]
name = "load_guarded"
[[function.parameters]]
name = "buffer"
type = "ptr<const u8>"
[[function.returns]]
name = "value"
type = "usize"
[[function.memory.regions]]
name = "cell"
base = "buffer"
length = "8"
access = "read"
"#,
        )
        .contract
        .expect("contract");

        let instrs = [
            phys("cmp", &["eax", "8"]),
            phys("jae", &["0x20"]),
            phys("movzx", &["ecx", "byte ptr [rdi + rax*1]"]),
            phys("ret", &[]),
        ];
        let lowered: Vec<_> = instrs
            .iter()
            .filter_map(|p| match lower(p) {
                semasm_x86::lower::Lowering::Lowered(l) => Some(l),
                semasm_x86::lower::Lowering::Unsupported { .. } => None,
            })
            .collect();
        let effects = collect_memory_effects(&lowered, &contract, AbiConvention::SysV);
        assert!(
            effects.iter().any(|e| matches!(
                &e.addr,
                AccessAddr::Indexed {
                    base_param,
                    scale: 1,
                    displacement: 0,
                    index_const: None,
                    index_max_exclusive: Some(8),
                } if base_param == "buffer"
            )),
            "expected Indexed with index_max_exclusive=8: {effects:?}"
        );
    }

    #[test]
    fn post_test_loop_induction_attaches_index_max_exclusive() {
        // xor eax,eax; movzx ecx, byte [rdi+rax]; inc eax; cmp eax,8; jb loop
        let contract = semasm_contract::check_str(
            r#"
contract_version = "0.1"
[function]
name = "load_post_test"
[[function.parameters]]
name = "buffer"
type = "ptr<const u8>"
[[function.returns]]
name = "value"
type = "usize"
[[function.memory.regions]]
name = "cell"
base = "buffer"
length = "8"
access = "read"
"#,
        )
        .contract
        .expect("contract");

        let instrs = [
            phys("xor", &["eax", "eax"]),
            phys("movzx", &["ecx", "byte ptr [rdi + rax*1]"]),
            phys("inc", &["eax"]),
            phys("cmp", &["eax", "8"]),
            phys("jb", &["0x10"]),
            phys("ret", &[]),
        ];
        let lowered: Vec<_> = instrs
            .iter()
            .filter_map(|p| match lower(p) {
                semasm_x86::lower::Lowering::Lowered(l) => Some(l),
                semasm_x86::lower::Lowering::Unsupported { .. } => None,
            })
            .collect();
        let effects = collect_memory_effects(&lowered, &contract, AbiConvention::SysV);
        assert!(
            effects.iter().any(|e| matches!(
                &e.addr,
                AccessAddr::Indexed {
                    base_param,
                    scale: 1,
                    displacement: 0,
                    index_const: None,
                    index_max_exclusive: Some(8),
                } if base_param == "buffer"
            )),
            "expected Fb7 post-test induction index_max_exclusive=8: {effects:?}"
        );
    }

    #[test]
    fn countdown_loop_induction_attaches_index_max_exclusive() {
        // mov eax, 8; dec eax; movzx ecx, byte [rdi+rax]; jnz loop
        let contract = semasm_contract::check_str(
            r#"
contract_version = "0.1"
[function]
name = "load_countdown"
[[function.parameters]]
name = "buffer"
type = "ptr<const u8>"
[[function.returns]]
name = "value"
type = "usize"
[[function.memory.regions]]
name = "cell"
base = "buffer"
length = "8"
access = "read"
"#,
        )
        .contract
        .expect("contract");

        let instrs = [
            phys("mov", &["eax", "8"]),
            phys("dec", &["eax"]),
            phys("movzx", &["ecx", "byte ptr [rdi + rax*1]"]),
            phys("jnz", &["0x10"]),
            phys("ret", &[]),
        ];
        let lowered: Vec<_> = instrs
            .iter()
            .filter_map(|p| match lower(p) {
                semasm_x86::lower::Lowering::Lowered(l) => Some(l),
                semasm_x86::lower::Lowering::Unsupported { .. } => None,
            })
            .collect();
        let effects = collect_memory_effects(&lowered, &contract, AbiConvention::SysV);
        assert!(
            effects.iter().any(|e| matches!(
                &e.addr,
                AccessAddr::Indexed {
                    base_param,
                    scale: 1,
                    displacement: 0,
                    index_const: None,
                    index_max_exclusive: Some(8),
                } if base_param == "buffer"
            )),
            "expected Fb8 countdown induction index_max_exclusive=8: {effects:?}"
        );
    }

    #[test]
    fn cfg_pre_test_loop_attaches_index_max_exclusive() {
        // xor eax,eax; header: cmp eax,8; jae exit; load; inc; jmp header
        let contract = semasm_contract::check_str(
            r#"
contract_version = "0.1"
[function]
name = "load_pre_test"
[[function.parameters]]
name = "buffer"
type = "ptr<const u8>"
[[function.returns]]
name = "value"
type = "usize"
[[function.memory.regions]]
name = "cell"
base = "buffer"
length = "8"
access = "read"
"#,
        )
        .contract
        .expect("contract");
        let physical = vec![
            phys_at(0x1000, "xor", &["eax", "eax"]),
            phys_at(0x1002, "cmp", &["eax", "8"]),
            phys_at(0x1005, "jae", &["0x1010"]),
            phys_at(0x1007, "movzx", &["ecx", "byte ptr [rdi + rax*1]"]),
            phys_at(0x100b, "inc", &["eax"]),
            phys_at(0x100d, "jmp", &["0x1002"]),
            phys_at(0x1010, "ret", &[]),
        ];
        let lowered: Vec<_> = physical
            .iter()
            .filter_map(|p| match lower(p) {
                semasm_x86::lower::Lowering::Lowered(l) => Some(l),
                semasm_x86::lower::Lowering::Unsupported { .. } => None,
            })
            .collect();
        let effects =
            collect_memory_effects_with_cfg(&lowered, &physical, &contract, AbiConvention::SysV);
        assert!(
            effects.iter().any(|e| matches!(
                &e.addr,
                AccessAddr::Indexed {
                    base_param,
                    index_const: None,
                    index_max_exclusive: Some(8),
                    ..
                } if base_param == "buffer"
            )),
            "expected Fb9a CFG-confirmed upper bound: {effects:?}"
        );
    }

    #[test]
    fn cfg_pre_test_loop_rejects_non_back_edge() {
        let physical = vec![
            phys_at(0x1000, "xor", &["eax", "eax"]),
            phys_at(0x1002, "cmp", &["eax", "8"]),
            phys_at(0x1005, "jae", &["0x1010"]),
            phys_at(0x1007, "movzx", &["ecx", "byte ptr [rdi + rax]"]),
            phys_at(0x100b, "inc", &["eax"]),
            phys_at(0x100d, "jmp", &["0x1000"]),
            phys_at(0x1010, "ret", &[]),
        ];
        let lowered: Vec<_> = physical
            .iter()
            .filter_map(|p| match lower(p) {
                semasm_x86::lower::Lowering::Lowered(l) => Some(l),
                semasm_x86::lower::Lowering::Unsupported { .. } => None,
            })
            .collect();
        assert!(discover_cfg_pre_test_inductions(&lowered, &physical).is_empty());
    }

    #[test]
    fn cfg_post_test_loop_attaches_index_max_exclusive() {
        // xor; header: load; inc; cmp 8; jb header
        let contract = semasm_contract::check_str(
            r#"
contract_version = "0.1"
[function]
name = "load_post_test_cfg"
[[function.parameters]]
name = "buffer"
type = "ptr<const u8>"
[[function.returns]]
name = "value"
type = "usize"
[[function.memory.regions]]
name = "cell"
base = "buffer"
length = "8"
access = "read"
"#,
        )
        .contract
        .expect("contract");
        let physical = vec![
            phys_at(0x1000, "xor", &["eax", "eax"]),
            phys_at(0x1002, "movzx", &["ecx", "byte ptr [rdi + rax*1]"]),
            phys_at(0x1006, "inc", &["eax"]),
            phys_at(0x1008, "cmp", &["eax", "8"]),
            phys_at(0x100b, "jb", &["0x1002"]),
            phys_at(0x100d, "ret", &[]),
        ];
        let lowered: Vec<_> = physical
            .iter()
            .filter_map(|p| match lower(p) {
                semasm_x86::lower::Lowering::Lowered(l) => Some(l),
                semasm_x86::lower::Lowering::Unsupported { .. } => None,
            })
            .collect();
        let effects =
            collect_memory_effects_with_cfg(&lowered, &physical, &contract, AbiConvention::SysV);
        assert!(
            effects.iter().any(|e| matches!(
                &e.addr,
                AccessAddr::Indexed {
                    base_param,
                    index_const: None,
                    index_max_exclusive: Some(8),
                    ..
                } if base_param == "buffer"
            )),
            "expected Fb9b CFG-confirmed post-test bound: {effects:?}"
        );
    }

    #[test]
    fn cfg_post_test_loop_rejects_wrong_back_edge() {
        let physical = vec![
            phys_at(0x1000, "xor", &["eax", "eax"]),
            phys_at(0x1002, "movzx", &["ecx", "byte ptr [rdi + rax*1]"]),
            phys_at(0x1006, "inc", &["eax"]),
            phys_at(0x1008, "cmp", &["eax", "8"]),
            phys_at(0x100b, "jb", &["0x1000"]),
            phys_at(0x100d, "ret", &[]),
        ];
        let lowered: Vec<_> = physical
            .iter()
            .filter_map(|p| match lower(p) {
                semasm_x86::lower::Lowering::Lowered(l) => Some(l),
                semasm_x86::lower::Lowering::Unsupported { .. } => None,
            })
            .collect();
        assert!(discover_cfg_post_test_inductions(&lowered, &physical).is_empty());
    }
}
