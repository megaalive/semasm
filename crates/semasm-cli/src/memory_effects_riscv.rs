//! RISC-V memory-effect extraction for Region/Alias Evidence v1 (ADR 0008).

use std::collections::HashMap;

use semasm_asir::OpKind;
use semasm_contract::{AccessAddr, AccessMode, CheckedContract, ObservedMemoryAccess, SemType};
use semasm_riscv::lower::{LoweredInstr, MemOperand, Operand};
use semasm_riscv::{Gpr, Storage, Width};

/// Collect observed memory accesses for Region/Alias Evidence v1 (RV LP64).
#[must_use]
pub fn collect_memory_effects(
    lowered: &[LoweredInstr],
    contract: &CheckedContract,
) -> Vec<ObservedMemoryAccess> {
    let mut affinity: HashMap<Gpr, String> = HashMap::new();
    seed_param_affinity(&mut affinity, contract);

    let mut out = Vec::new();
    for instr in lowered {
        record_accesses(instr, &affinity, &mut out);
        update_affinity(instr, &mut affinity);
    }
    out
}

fn seed_param_affinity(affinity: &mut HashMap<Gpr, String>, contract: &CheckedContract) {
    // RV LP64 integer/pointer args: a0..a7.
    let regs = [
        Gpr::A0,
        Gpr::A1,
        Gpr::A2,
        Gpr::A3,
        Gpr::A4,
        Gpr::A5,
        Gpr::A6,
        Gpr::A7,
    ];
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
    affinity: &HashMap<Gpr, String>,
    out: &mut Vec<ObservedMemoryAccess>,
) {
    let mnemonic = instr.mnemonic.to_ascii_lowercase();
    for op in &instr.operands {
        let Operand::Mem(mem) = op else {
            continue;
        };
        let Some(mode) = access_mode(&mnemonic, instr.kind) else {
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

fn access_mode(mnemonic: &str, kind: OpKind) -> Option<AccessMode> {
    match mnemonic {
        "auipc" => None,
        _ => match kind {
            OpKind::Store => Some(AccessMode::Store),
            _ => Some(AccessMode::Load),
        },
    }
}

fn classify_addr(mem: &MemOperand, affinity: &HashMap<Gpr, String>) -> AccessAddr {
    if is_stack_frame(mem) {
        return AccessAddr::StackFrame;
    }
    let Some(base) = mem.base else {
        return AccessAddr::Unknown;
    };
    if mem.index.is_some() && mem.scale != 0 {
        return AccessAddr::Unknown;
    }
    let Storage::Gpr(gpr) = base.storage;
    match affinity.get(&gpr) {
        Some(param) => AccessAddr::Affine {
            base_param: param.clone(),
            offset: mem.disp,
        },
        None => AccessAddr::Unknown,
    }
}

fn is_stack_frame(mem: &MemOperand) -> bool {
    match mem.base {
        Some(base) => {
            matches!(base.storage, Storage::Gpr(Gpr::Sp | Gpr::S0) if mem.index.is_none())
        }
        None => false,
    }
}

fn width_bytes(width: Width) -> u32 {
    width.bits() / 8
}

fn update_affinity(instr: &LoweredInstr, affinity: &mut HashMap<Gpr, String>) {
    let mnemonic = instr.mnemonic.to_ascii_lowercase();
    match mnemonic.as_str() {
        "mv" | "li" => {
            let Some(Operand::Reg(dst)) = instr.operands.first() else {
                return;
            };
            let Storage::Gpr(dst_gpr) = dst.storage;
            if dst_gpr == Gpr::Zero {
                return;
            }
            match instr.operands.get(1) {
                Some(Operand::Reg(src)) => {
                    let Storage::Gpr(src_gpr) = src.storage;
                    if src_gpr == Gpr::Zero {
                        affinity.remove(&dst_gpr);
                    } else if let Some(name) = affinity.get(&src_gpr).cloned() {
                        affinity.insert(dst_gpr, name);
                    } else {
                        affinity.remove(&dst_gpr);
                    }
                }
                _ => {
                    affinity.remove(&dst_gpr);
                }
            }
        }
        "addi" | "add" | "addiw" | "addw" | "sub" | "subw" => {
            let Some(Operand::Reg(dst)) = instr.operands.first() else {
                return;
            };
            let Storage::Gpr(dst_gpr) = dst.storage;
            if let Some(Operand::Reg(src)) = instr.operands.get(1) {
                let Storage::Gpr(src_gpr) = src.storage;
                if let Some(name) = affinity.get(&src_gpr).cloned() {
                    affinity.insert(dst_gpr, name);
                }
            }
        }
        "ld" | "lb" | "lh" | "lw" | "lbu" | "lhu" | "lwu" | "and" | "andi" | "or" | "ori"
        | "xor" | "mul" | "sll" | "slli" => {
            if let Some(Operand::Reg(dst)) = instr.operands.first() {
                let Storage::Gpr(dst_gpr) = dst.storage;
                if dst_gpr != Gpr::Zero {
                    affinity.remove(&dst_gpr);
                }
            }
        }
        _ => {}
    }
}

#[cfg(all(test, feature = "capstone"))]
mod tests {
    use super::*;
    use semasm_decode::PhysicalInstruction;
    use semasm_riscv::lower::lower;

    fn phys(mnemonic: &str, operands: &[&str]) -> PhysicalInstruction {
        PhysicalInstruction {
            address: 0,
            bytes: vec![0x13, 0x00, 0x00, 0x00],
            mnemonic: mnemonic.into(),
            operands: operands.iter().map(|s| (*s).to_string()).collect(),
            read_regs: vec![],
            write_regs: vec![],
            groups: vec![],
            detail_available: false,
        }
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
            phys("mv", &["t0", "a0"]),
            phys("mv", &["t1", "a1"]),
            phys("lbu", &["t3", "0(t1)"]),
            phys("sb", &["t3", "0(t0)"]),
            phys("addi", &["t0", "t0", "1"]),
            phys("addi", &["t1", "t1", "1"]),
            phys("ret", &[]),
        ];
        let lowered: Vec<_> = instrs
            .iter()
            .filter_map(|p| match lower(p) {
                semasm_riscv::lower::Lowering::Lowered(l) => Some(l),
                semasm_riscv::lower::Lowering::Unsupported { .. } => None,
            })
            .collect();
        let effects = collect_memory_effects(&lowered, &contract);
        let unknowns = effects
            .iter()
            .filter(|e| matches!(e.addr, AccessAddr::Unknown))
            .count();
        assert_eq!(unknowns, 0, "{effects:?}");
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

        let instrs = [phys("lbu", &["a0", "0(t6)"])];
        let lowered: Vec<_> = instrs
            .iter()
            .filter_map(|p| match lower(p) {
                semasm_riscv::lower::Lowering::Lowered(l) => Some(l),
                semasm_riscv::lower::Lowering::Unsupported { .. } => None,
            })
            .collect();
        let effects = collect_memory_effects(&lowered, &contract);
        assert!(effects
            .iter()
            .any(|e| matches!(e.addr, AccessAddr::Unknown)));
    }
}
