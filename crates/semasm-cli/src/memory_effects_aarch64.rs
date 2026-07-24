//! AArch64 memory-effect extraction for Region/Alias Evidence v1 (ADR 0008).

use std::collections::HashMap;

use semasm_aarch64::lower::{LoweredInstr, MemOperand, Operand};
use semasm_aarch64::{Gp, Storage, Width};
use semasm_asir::OpKind;
use semasm_contract::{AccessAddr, AccessMode, CheckedContract, ObservedMemoryAccess, SemType};

/// Collect observed memory accesses for Region/Alias Evidence v1 (AAPCS64).
#[must_use]
pub fn collect_memory_effects(
    lowered: &[LoweredInstr],
    contract: &CheckedContract,
) -> Vec<ObservedMemoryAccess> {
    let mut affinity: HashMap<Gp, String> = HashMap::new();
    seed_param_affinity(&mut affinity, contract);

    let mut out = Vec::new();
    for instr in lowered {
        record_accesses(instr, &affinity, &mut out);
        update_affinity(instr, &mut affinity);
    }
    out
}

fn seed_param_affinity(affinity: &mut HashMap<Gp, String>, contract: &CheckedContract) {
    // AAPCS64 integer/pointer args: x0..x7.
    let regs = [
        Gp::X0,
        Gp::X1,
        Gp::X2,
        Gp::X3,
        Gp::X4,
        Gp::X5,
        Gp::X6,
        Gp::X7,
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
    affinity: &HashMap<Gp, String>,
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
        "adr" | "adrp" => None,
        _ => match kind {
            OpKind::Store => Some(AccessMode::Store),
            _ => Some(AccessMode::Load),
        },
    }
}

fn classify_addr(mem: &MemOperand, affinity: &HashMap<Gp, String>) -> AccessAddr {
    if is_stack_frame(mem) {
        return AccessAddr::StackFrame;
    }
    let Some(base) = mem.base else {
        return AccessAddr::Unknown;
    };
    if mem.index.is_some() && mem.scale != 0 {
        return AccessAddr::Unknown;
    }
    match base.storage {
        Storage::Gp(gp) => match affinity.get(&gp) {
            Some(param) => AccessAddr::Affine {
                base_param: param.clone(),
                offset: mem.disp,
            },
            None => AccessAddr::Unknown,
        },
        Storage::Sp | Storage::Nzcv => AccessAddr::StackFrame,
    }
}

fn is_stack_frame(mem: &MemOperand) -> bool {
    match mem.base {
        Some(base) => match base.storage {
            Storage::Sp => true,
            Storage::Gp(Gp::Fp) if mem.index.is_none() => true,
            _ => false,
        },
        None => false,
    }
}

fn width_bytes(width: Width) -> u32 {
    width.bits() / 8
}

fn update_affinity(instr: &LoweredInstr, affinity: &mut HashMap<Gp, String>) {
    let mnemonic = instr.mnemonic.to_ascii_lowercase();
    match mnemonic.as_str() {
        "mov" | "movz" | "movn" => {
            let Some(Operand::Reg(dst)) = instr.operands.first() else {
                return;
            };
            let Storage::Gp(dst_gp) = dst.storage else {
                return;
            };
            if dst_gp == Gp::Zr {
                return;
            }
            match instr.operands.get(1) {
                Some(Operand::Reg(src)) => {
                    if let Storage::Gp(src_gp) = src.storage {
                        if src_gp == Gp::Zr {
                            affinity.remove(&dst_gp);
                        } else if let Some(name) = affinity.get(&src_gp).cloned() {
                            affinity.insert(dst_gp, name);
                        } else {
                            affinity.remove(&dst_gp);
                        }
                    } else {
                        affinity.remove(&dst_gp);
                    }
                }
                _ => {
                    affinity.remove(&dst_gp);
                }
            }
        }
        "add" | "sub" => {
            // add/sub dst, src, … — preserve/copy affinity from src GP.
            let Some(Operand::Reg(dst)) = instr.operands.first() else {
                return;
            };
            let Storage::Gp(dst_gp) = dst.storage else {
                return;
            };
            if let Some(Operand::Reg(src)) = instr.operands.get(1) {
                if let Storage::Gp(src_gp) = src.storage {
                    if let Some(name) = affinity.get(&src_gp).cloned() {
                        affinity.insert(dst_gp, name);
                        return;
                    }
                }
            }
            // Keep existing dst affinity on pointer bump when only imm changes
            // and dst already known (e.g. add x3, x3, #1 already handled above).
            let _ = dst_gp;
        }
        "ldr" | "ldrb" | "ldrh" | "ldrsb" | "ldrsh" | "ldrsw" | "and" | "orr" | "eor" | "mul"
        | "sdiv" | "udiv" | "movk" => {
            if let Some(Operand::Reg(dst)) = instr.operands.first() {
                if let Storage::Gp(dst_gp) = dst.storage {
                    if dst_gp != Gp::Zr {
                        affinity.remove(&dst_gp);
                    }
                }
            }
        }
        _ => {}
    }
}

#[cfg(all(test, feature = "capstone"))]
mod tests {
    use super::*;
    use semasm_aarch64::lower::lower;
    use semasm_decode::PhysicalInstruction;

    fn phys(mnemonic: &str, operands: &[&str]) -> PhysicalInstruction {
        PhysicalInstruction {
            address: 0,
            bytes: vec![0xd5, 0x03, 0x20, 0x1f],
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
            phys("mov", &["x3", "x0"]),
            phys("mov", &["x4", "x1"]),
            phys("ldrb", &["w5", "[x4]"]),
            phys("strb", &["w5", "[x3]"]),
            phys("add", &["x3", "x3", "#1"]),
            phys("add", &["x4", "x4", "#1"]),
            phys("ret", &[]),
        ];
        let lowered: Vec<_> = instrs
            .iter()
            .filter_map(|p| match lower(p) {
                semasm_aarch64::lower::Lowering::Lowered(l) => Some(l),
                semasm_aarch64::lower::Lowering::Unsupported { .. } => None,
            })
            .collect();
        let effects = collect_memory_effects(&lowered, &contract);
        let unknowns = effects
            .iter()
            .filter(|e| matches!(e.addr, AccessAddr::Unknown))
            .count();
        assert_eq!(unknowns, 0, "{effects:?}");
        assert!(effects.iter().any(|e| matches!(
            &e.addr,
            AccessAddr::Affine { base_param, .. } if base_param == "src"
        )));
        assert!(effects.iter().any(|e| matches!(
            &e.addr,
            AccessAddr::Affine { base_param, .. } if base_param == "dst"
        )));
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

        let instrs = [phys("ldrb", &["w0", "[x9]"])];
        let lowered: Vec<_> = instrs
            .iter()
            .filter_map(|p| match lower(p) {
                semasm_aarch64::lower::Lowering::Lowered(l) => Some(l),
                semasm_aarch64::lower::Lowering::Unsupported { .. } => None,
            })
            .collect();
        let effects = collect_memory_effects(&lowered, &contract);
        assert!(effects
            .iter()
            .any(|e| matches!(e.addr, AccessAddr::Unknown)));
    }
}
