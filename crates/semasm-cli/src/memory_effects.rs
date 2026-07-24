//! x86 memory-effect extraction for Region/Alias Evidence v1 (ADR 0006).

use std::collections::HashMap;

use semasm_contract::{AccessAddr, AccessMode, CheckedContract, ObservedMemoryAccess, SemType};
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
#[must_use]
pub fn collect_memory_effects(
    lowered: &[LoweredInstr],
    contract: &CheckedContract,
    abi: AbiConvention,
) -> Vec<ObservedMemoryAccess> {
    let mut affinity: HashMap<Gp, String> = HashMap::new();
    let mut stack_slots: HashMap<FrameSlot, String> = HashMap::new();
    seed_param_affinity(&mut affinity, contract, abi);

    let mut out = Vec::new();
    for instr in lowered {
        record_accesses(instr, &affinity, &mut out);
        update_affinity(instr, &mut affinity, &mut stack_slots);
    }
    out
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
            addr: classify_addr(mem, affinity),
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

fn classify_addr(mem: &MemOperand, affinity: &HashMap<Gp, String>) -> AccessAddr {
    if is_stack_frame(mem) {
        return AccessAddr::StackFrame;
    }
    let Some(base) = mem.base else {
        return AccessAddr::Unknown;
    };
    let Storage::Gp(gp) = base.storage else {
        return AccessAddr::Unknown;
    };
    if mem.index.is_some() && mem.scale > 0 {
        return AccessAddr::Unknown;
    }
    match affinity.get(&gp) {
        Some(param) => AccessAddr::Affine {
            base_param: param.clone(),
            offset: mem.disp,
        },
        None => AccessAddr::Unknown,
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

fn update_affinity(
    instr: &LoweredInstr,
    affinity: &mut HashMap<Gp, String>,
    stack_slots: &mut HashMap<FrameSlot, String>,
) {
    let mnemonic = instr.mnemonic.to_ascii_lowercase();
    match mnemonic.as_str() {
        "mov" | "movabs" => {
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
                // Reload / copy: mov reg, …
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
                            } else {
                                affinity.remove(&dst_gp);
                            }
                        }
                        Some(Operand::Mem(mem)) => {
                            // Reload spilled pointer params from the frame.
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
                        }
                    }
                }
                _ => {}
            }
        }
        "lea" => {
            let Some(Operand::Reg(dst)) = instr.operands.first() else {
                return;
            };
            let Storage::Gp(dst_gp) = dst.storage else {
                return;
            };
            if let Some(Operand::Mem(mem)) = instr.operands.get(1) {
                if let AccessAddr::Affine { base_param, .. } = classify_addr(mem, affinity) {
                    affinity.insert(dst_gp, base_param);
                    return;
                }
            }
            affinity.remove(&dst_gp);
        }
        "xor" | "pop" | "movzx" | "movsx" | "movsxd" | "and" | "or" | "imul" | "mul" | "div"
        | "idiv" | "neg" | "not" => {
            if let Some(Operand::Reg(dst)) = instr.operands.first() {
                if let Storage::Gp(dst_gp) = dst.storage {
                    affinity.remove(&dst_gp);
                }
            }
        }
        // inc/dec/add/sub/adc/sbb and other ops: keep existing param affinity
        // (pointer arithmetic does not change region identity in v1).
        _ => {}
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
}
