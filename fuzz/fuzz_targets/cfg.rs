#![no_main]

use libfuzzer_sys::fuzz_target;
use semasm_decode::PhysicalInstruction;

fuzz_target!(|data: &[u8]| {
    let instructions: Vec<_> = data
        .iter()
        .take(64)
        .enumerate()
        .map(|(index, byte)| {
            let mnemonic = match byte % 7 {
                0 => "jmp",
                1 => "jne",
                2 => "call",
                3 => "ret",
                4 => "syscall",
                5 => "mov",
                _ => "unknown",
            };
            PhysicalInstruction {
                address: 0x1000 + u64::try_from(index).unwrap_or(0),
                bytes: vec![*byte],
                mnemonic: mnemonic.to_string(),
                operands: vec![format!("0x{:x}", 0x1000_u64 + u64::from(*byte))],
                read_regs: Vec::new(),
                write_regs: Vec::new(),
                groups: Vec::new(),
                detail_available: false,
            }
        })
        .collect();
    let _ = semasm_cfg::build(&instructions);
});
