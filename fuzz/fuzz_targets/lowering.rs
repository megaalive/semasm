#![no_main]

use libfuzzer_sys::fuzz_target;
use semasm_decode::PhysicalInstruction;

fuzz_target!(|data: &[u8]| {
    let split = data.len().min(32);
    let mnemonic = String::from_utf8_lossy(&data[..split]).into_owned();
    let operands = data[split..]
        .chunks(16)
        .take(8)
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect();
    let instruction = PhysicalInstruction {
        address: 0,
        bytes: data.iter().copied().take(16).collect(),
        mnemonic,
        operands,
        read_regs: Vec::new(),
        write_regs: Vec::new(),
        groups: Vec::new(),
        detail_available: false,
    };
    let _ = semasm_x86::lower::lower(&instruction);
    let _ = semasm_aarch64::lower::lower(&instruction);
    let _ = semasm_riscv::lower::lower(&instruction);
});
