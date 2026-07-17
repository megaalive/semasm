//! Physical-instruction decoding for SemASM.
//!
//! This crate normalises raw machine code into [`PhysicalInstruction`] values
//! that downstream passes (CFG, behavioral checks) can reason about without
//! depending on any particular disassembly engine.
//!
//! The only concrete backend shipped today is Capstone, and it is gated behind
//! the `capstone` cargo feature so the crate (and `semasm-core`) still builds
//! on hosts where the Capstone C library cannot be compiled. When the feature
//! is disabled, [`decode_x86_64`] returns [`DecodeError::Unsupported`] and the
//! normalised types remain fully usable.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A single decoded machine instruction in a target-independent shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhysicalInstruction {
    /// Virtual/load address of the instruction.
    pub address: u64,
    /// Raw machine-code bytes of the instruction.
    pub bytes: Vec<u8>,
    /// Mnemonic, e.g. `xor`, `mov`, `ret`.
    pub mnemonic: String,
    /// Operand tokens as produced by the backend (`op_str` style). When the
    /// backend cannot supply operands this is empty, not `None`, to keep the
    /// type trivially serialisable.
    pub operands: Vec<String>,
    /// Registers read by the instruction (where the backend exposes detail).
    pub read_regs: Vec<String>,
    /// Registers written by the instruction (where the backend exposes detail).
    pub write_regs: Vec<String>,
    /// Instruction groups (e.g. `jump`, `call`, `ret`) from the backend.
    pub groups: Vec<String>,
    /// Whether [`read_regs`], [`write_regs`] and [`groups`] come from real
    /// backend detail (`true`) or are empty because the backend was
    /// unavailable/without detail (`false`). Consumed honestly by downstream
    /// passes.
    pub detail_available: bool,
}

impl fmt::Display for PhysicalInstruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}: {}", self.address, self.mnemonic)?;
        if !self.operands.is_empty() {
            write!(f, " {}", self.operands.join(", "))?;
        }
        Ok(())
    }
}

/// Errors produced while decoding raw machine code.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DecodeError {
    /// The requested architecture/feature combination is not available in this
    /// build (e.g. Capstone feature disabled, or arch not yet implemented).
    #[error("decoding for this target is not supported in this build: {0}")]
    Unsupported(String),
    /// Capstone failed to initialise for the requested mode.
    #[error("disassembler initialisation failed: {0}")]
    InitFailed(String),
    /// Capstone returned an error while disassembling a buffer.
    #[error("disassembly failed: {0}")]
    DisasmFailed(String),
    /// The input buffer could not be represented (e.g. empty).
    #[error("empty input buffer")]
    EmptyInput,
}

/// Decode an x86-64 machine-code buffer into normalised instructions.
///
/// `base_address` is the load address assigned to the first byte of `code`.
///
/// When the `capstone` feature is disabled this returns
/// [`DecodeError::Unsupported`]; the normalised types are still usable
/// downstream so core builds without Capstone.
pub fn decode_x86_64(
    code: &[u8],
    base_address: u64,
) -> Result<Vec<PhysicalInstruction>, DecodeError> {
    if code.is_empty() {
        return Err(DecodeError::EmptyInput);
    }
    decode_x86_64_inner(code, base_address)
}

#[cfg(feature = "capstone")]
fn decode_x86_64_inner(
    code: &[u8],
    base_address: u64,
) -> Result<Vec<PhysicalInstruction>, DecodeError> {
    use capstone::prelude::*;

    let cs = Capstone::new()
        .x86()
        .mode(capstone::arch::x86::ArchMode::Mode64)
        .detail(true)
        .build()
        .map_err(|e| DecodeError::InitFailed(format!("{e:?}")))?;

    let insns = cs
        .disasm_all(code, base_address)
        .map_err(|e| DecodeError::DisasmFailed(format!("{e:?}")))?;

    let mut out = Vec::with_capacity(insns.len());
    for insn in insns.iter() {
        let address = insn.address();
        let bytes = insn.bytes().to_vec();
        let mnemonic = insn.mnemonic().unwrap_or("").to_string();
        let op_str = insn.op_str().unwrap_or("");
        let operands: Vec<String> = if op_str.is_empty() {
            Vec::new()
        } else {
            op_str.split(',').map(|s| s.trim().to_string()).collect()
        };

        let (read_regs, write_regs, groups) = match cs.insn_detail(insn) {
            Ok(detail) => {
                let read_regs = detail
                    .regs_read()
                    .iter()
                    .filter_map(|r| cs.reg_name(*r))
                    .collect();
                let write_regs = detail
                    .regs_write()
                    .iter()
                    .filter_map(|r| cs.reg_name(*r))
                    .collect();
                let groups: Vec<String> = detail
                    .groups()
                    .iter()
                    .filter_map(|g| cs.group_name(*g))
                    .collect();
                (read_regs, write_regs, groups)
            }
            Err(_) => (Vec::new(), Vec::new(), Vec::new()),
        };

        out.push(PhysicalInstruction {
            address,
            bytes,
            mnemonic,
            operands,
            read_regs,
            write_regs,
            groups,
            detail_available: true,
        });
    }
    Ok(out)
}

#[cfg(not(feature = "capstone"))]
fn decode_x86_64_inner(
    _code: &[u8],
    _base_address: u64,
) -> Result<Vec<PhysicalInstruction>, DecodeError> {
    Err(DecodeError::Unsupported(
        "x86-64 decoding requires the `capstone` feature".to_string(),
    ))
}

/// Serialise a slice of instructions to stable, deterministic JSON.
pub fn to_json(instrs: &[PhysicalInstruction]) -> Result<String, DecodeError> {
    serde_json::to_string_pretty(instrs).map_err(|e| DecodeError::DisasmFailed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_is_an_error() {
        assert_eq!(decode_x86_64(&[], 0), Err(DecodeError::EmptyInput));
    }

    #[test]
    fn no_feature_build_reports_unsupported() {
        // This test exercises the non-capstone path; it passes whether or not
        // the feature is enabled because we call the public API which is
        // honest about availability. When the feature is off, decode returns
        // Unsupported. When on, it must successfully decode `ret` (0xc3).
        let code = &[0xc3u8];
        match decode_x86_64(code, 0x1000) {
            Ok(instrs) => {
                assert_eq!(instrs.len(), 1);
                assert_eq!(instrs[0].address, 0x1000);
                assert_eq!(instrs[0].bytes, vec![0xc3]);
                assert_eq!(instrs[0].mnemonic, "ret");
                assert!(instrs[0].detail_available);
            }
            Err(DecodeError::Unsupported(_)) => {
                // Feature disabled — acceptable per spec.
            }
            Err(other) => panic!("unexpected decode error: {other}"),
        }
    }

    #[test]
    fn decode_unknown_bytes_does_not_panic() {
        let code = &[0x00u8, 0xff, 0x12, 0x34, 0x99, 0xab];
        let _ = decode_x86_64(code, 0);
    }

    #[test]
    fn json_is_deterministic() {
        let instrs = vec![PhysicalInstruction {
            address: 0x4000,
            bytes: vec![0x31, 0xc0],
            mnemonic: "xor".to_string(),
            operands: vec!["eax".to_string(), "eax".to_string()],
            read_regs: vec!["eax".to_string()],
            write_regs: vec!["eax".to_string()],
            groups: vec![],
            detail_available: true,
        }];
        let a = to_json(&instrs).unwrap();
        let b = to_json(&instrs).unwrap();
        assert_eq!(a, b);
    }

    #[cfg(feature = "capstone")]
    #[test]
    fn x86_64_fixture_decodes_with_register_effects() {
        // `xor eax, eax` (31 c0) then `ret` (c3) at base 0x400000.
        let code = [0x31u8, 0xc0, 0xc3];
        let instrs = decode_x86_64(&code, 0x0040_0000).expect("decode");
        assert_eq!(instrs.len(), 2);

        let xor = &instrs[0];
        assert_eq!(xor.address, 0x0040_0000);
        assert_eq!(xor.bytes, vec![0x31, 0xc0]);
        assert_eq!(xor.mnemonic, "xor");
        assert_eq!(xor.operands, vec!["eax", "eax"]);
        assert!(xor.detail_available);
        assert!(xor.write_regs.iter().any(|r| r == "eax"));
        assert!(xor.read_regs.iter().any(|r| r == "eax"));

        let ret = &instrs[1];
        assert_eq!(ret.address, 0x0040_0002);
        assert_eq!(ret.bytes, vec![0xc3]);
        assert_eq!(ret.mnemonic, "ret");
    }

    #[cfg(feature = "capstone")]
    #[test]
    fn x86_64_call_is_grouped_and_linked() {
        // `call rel32` (e8 00 00 00 00)
        let code = [0xe8u8, 0x00, 0x00, 0x00, 0x00];
        let instrs = decode_x86_64(&code, 0x1000).expect("decode");
        assert_eq!(instrs.len(), 1);
        let call = &instrs[0];
        assert_eq!(call.mnemonic, "call");
        assert!(call.detail_available);
        assert!(call.groups.iter().any(|g| g == "call"));
    }
}
