//! Capstone-backed inspection commands.

use std::path::Path;
use std::process::ExitCode;

use semasm_decode::DecodeError;

use crate::output::{
    json_aarch64_abi_report, json_abi_report, json_analysis_report, json_win64_abi_report,
    print_analysis_terminal, unsupported_instruction, UnsupportedInstruction,
};
use crate::OutputFormat;

/// Parse a `--base` value, accepting decimal or `0x`-prefixed hex.
#[cfg(feature = "capstone")]
#[allow(clippy::items_after_test_module)]
pub(crate) fn parse_base(s: &str) -> Result<u64, String> {
    let trimmed = s.trim();
    if let Some(hex) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).map_err(|e| format!("invalid hex base `{s}`: {e}"))
    } else {
        trimmed
            .parse::<u64>()
            .map_err(|e| format!("invalid base `{s}`: {e}"))
    }
}

/// Disassemble a raw binary blob and emit normalised physical instructions.
#[cfg(feature = "capstone")]
pub(crate) fn do_decode_inspect(path: &Path, base: u64, format: OutputFormat) -> ExitCode {
    let code = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{}: error: failed to read file: {e}", path.display());
            return ExitCode::from(1);
        }
    };

    let instrs = match semasm_decode::decode_x86_64(&code, base) {
        Ok(i) => i,
        Err(DecodeError::Unsupported(_)) => {
            eprintln!(
                "error: x86-64 decoding is not compiled into this build; \
                 rebuild `semasm-cli` with the `capstone` feature"
            );
            return ExitCode::from(1);
        }
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    match format {
        OutputFormat::Json => match semasm_decode::to_json(&instrs) {
            Ok(s) => {
                println!("{s}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("failed to serialize JSON: {e}");
                ExitCode::from(1)
            }
        },
        OutputFormat::Terminal => {
            if instrs.is_empty() {
                println!("(no instructions decoded)");
            }
            for insn in &instrs {
                print!("{insn}");
                if insn.detail_available {
                    if !insn.read_regs.is_empty() {
                        print!("  ; r:{}", insn.read_regs.join("/"));
                    }
                    if !insn.write_regs.is_empty() {
                        print!("  w:{}", insn.write_regs.join("/"));
                    }
                    if !insn.groups.is_empty() {
                        print!("  [{}]", insn.groups.join("/"));
                    }
                }
                println!();
            }
            ExitCode::SUCCESS
        }
    }
}

/// Build a control-flow graph from a raw binary blob and emit it.
#[cfg(feature = "capstone")]
pub(crate) fn do_cfg_inspect(path: &Path, base: u64, format: OutputFormat) -> ExitCode {
    let code = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{}: error: failed to read file: {e}", path.display());
            return ExitCode::from(1);
        }
    };

    let instrs = match semasm_decode::decode_x86_64(&code, base) {
        Ok(i) => i,
        Err(DecodeError::Unsupported(_)) => {
            eprintln!(
                "error: x86-64 decoding is not compiled into this build; \
                 rebuild `semasm-cli` with the `capstone` feature"
            );
            return ExitCode::from(1);
        }
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    let graph = match semasm_cfg::build(&instrs) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    match format {
        OutputFormat::Json => match graph.to_json() {
            Ok(s) => {
                println!("{s}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("failed to serialize JSON: {e}");
                ExitCode::from(1)
            }
        },
        OutputFormat::Terminal => {
            print!("{}", graph.to_terminal());
            ExitCode::SUCCESS
        }
    }
}

#[cfg(feature = "capstone")]
pub(crate) fn lower_x86_with_evidence(
    instructions: &[semasm_decode::PhysicalInstruction],
) -> (
    Vec<semasm_x86::lower::LoweredInstr>,
    Vec<UnsupportedInstruction>,
) {
    let mut lowered = Vec::with_capacity(instructions.len());
    let mut unsupported = Vec::new();
    for instruction in instructions {
        match semasm_x86::lower::lower(instruction) {
            semasm_x86::lower::Lowering::Lowered(value) => lowered.push(value),
            semasm_x86::lower::Lowering::Unsupported { mnemonic } => {
                unsupported.push(unsupported_instruction(instruction, mnemonic));
            }
        }
    }
    (lowered, unsupported)
}

#[cfg(feature = "capstone")]
fn lower_aarch64_with_evidence(
    instructions: &[semasm_decode::PhysicalInstruction],
) -> (
    Vec<semasm_aarch64::lower::LoweredInstr>,
    Vec<UnsupportedInstruction>,
) {
    let mut lowered = Vec::with_capacity(instructions.len());
    let mut unsupported = Vec::new();
    for instruction in instructions {
        match semasm_aarch64::lower::lower(instruction) {
            semasm_aarch64::lower::Lowering::Lowered(value) => lowered.push(value),
            semasm_aarch64::lower::Lowering::Unsupported { mnemonic } => {
                unsupported.push(unsupported_instruction(instruction, mnemonic));
            }
        }
    }
    (lowered, unsupported)
}

#[cfg(feature = "capstone")]
fn print_incomplete_terminal(unsupported: &[UnsupportedInstruction], allow_incomplete: bool) {
    println!("instructions unsupported: {}", unsupported.len());
    for instruction in unsupported {
        println!(
            "[ANALYSIS_INCOMPLETE] {:#x}: {} {} (bytes: {:02x?})",
            instruction.address,
            instruction.mnemonic,
            instruction.operands.join(", "),
            instruction.bytes
        );
    }
    if !unsupported.is_empty() && allow_incomplete {
        println!("incomplete analysis explicitly allowed by --allow-incomplete");
    }
}

#[cfg(feature = "capstone")]
pub(crate) fn analysis_exit_code(clean: bool, complete: bool, allow_incomplete: bool) -> ExitCode {
    if clean && (complete || allow_incomplete) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// Decode a raw binary blob, lower it, and run the System V AMD64 ABI
/// analysis over the (single) function body.
#[cfg(feature = "capstone")]
pub(crate) fn do_abi_inspect(
    path: &Path,
    base: u64,
    format: OutputFormat,
    allow_incomplete: bool,
) -> ExitCode {
    let code = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{}: error: failed to read file: {e}", path.display());
            return ExitCode::from(1);
        }
    };

    let instrs = match semasm_decode::decode_x86_64(&code, base) {
        Ok(i) => i,
        Err(DecodeError::Unsupported(_)) => {
            eprintln!(
                "error: x86-64 decoding is not compiled into this build; \
                 rebuild `semasm-cli` with the `capstone` feature"
            );
            return ExitCode::from(1);
        }
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    let (lowered, unsupported) = lower_x86_with_evidence(&instrs);

    let report = semasm_x86::abi::analyze(&lowered);

    match format {
        OutputFormat::Json => {
            match serde_json::to_string_pretty(&json_abi_report(
                &report,
                instrs.len(),
                lowered.len(),
                &unsupported,
            )) {
                Ok(s) => {
                    println!("{s}");
                    analysis_exit_code(report.is_clean(), unsupported.is_empty(), allow_incomplete)
                }
                Err(e) => {
                    eprintln!("failed to serialize JSON: {e}");
                    ExitCode::from(1)
                }
            }
        }
        OutputFormat::Terminal => {
            println!("instructions decoded: {}", instrs.len());
            println!("instructions lowered: {}", lowered.len());
            print_incomplete_terminal(&unsupported, allow_incomplete);
            println!("leaf function:       {}", report.is_leaf);
            println!("contains syscall:    {}", report.has_syscall);
            println!("final RSP delta:    {}", report.final_rsp_delta);
            println!("call sites:          {}", report.call_sites.len());
            println!(
                "max red-zone disp:   {}",
                if report.max_red_zone_disp < 0 {
                    report.max_red_zone_disp
                } else {
                    0
                }
            );
            println!();
            if report.findings.is_empty() && unsupported.is_empty() {
                println!("ABI: clean — no System V AMD64 violations detected ✓");
            } else {
                for f in &report.findings {
                    let tag = match f.severity {
                        semasm_x86::abi::Severity::Error => "ERROR  ",
                        semasm_x86::abi::Severity::Warning => "warning",
                        semasm_x86::abi::Severity::Info => "info   ",
                    };
                    let where_ = match f.at {
                        Some(i) => format!(" @{i}"),
                        None => String::new(),
                    };
                    println!("[{tag}] {} ({}{})", f.code, f.severity_str(), where_);
                    println!("        {}", f.message);
                }
            }
            analysis_exit_code(report.is_clean(), unsupported.is_empty(), allow_incomplete)
        }
    }
}

/// Decode a raw binary blob, lower it, build a control-flow graph, and run
/// the forward data-flow analysis (ANALYSIS-001).
#[cfg(feature = "capstone")]
pub(crate) fn do_analyze_inspect(path: &Path, base: u64, format: OutputFormat) -> ExitCode {
    let code = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{}: error: failed to read file: {e}", path.display());
            return ExitCode::from(1);
        }
    };

    let instrs = match semasm_decode::decode_x86_64(&code, base) {
        Ok(i) => i,
        Err(DecodeError::Unsupported(_)) => {
            eprintln!(
                "error: x86-64 decoding is not compiled into this build; \
                 rebuild `semasm-cli` with the `capstone` feature"
            );
            return ExitCode::from(1);
        }
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    // Lower every decoded instruction, keeping a 1:1 mapping with the
    // decoded list so CFG instruction indices line up.
    let lowered = semasm_x86::lower::lower_keep_all(&instrs);

    let cfg = match semasm_cfg::build(&instrs) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    // Convert CFG blocks into analysis block ranges. Successor edges carry
    // real block indices (including back-edges and merges).
    let blocks: Vec<semasm_x86::analysis::BlockRange> = cfg
        .blocks
        .iter()
        .map(|b| semasm_x86::analysis::BlockRange {
            start: b.start_instruction,
            end: b.end_instruction,
            successors: b.successors.iter().filter_map(|e| e.to).collect(),
        })
        .collect();

    let report = semasm_x86::analysis::analyze(&lowered, &blocks);

    match format {
        OutputFormat::Json => match serde_json::to_string_pretty(&json_analysis_report(&report)) {
            Ok(s) => {
                println!("{s}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("failed to serialize JSON: {e}");
                ExitCode::from(1)
            }
        },
        OutputFormat::Terminal => {
            print_analysis_terminal(&report);
            ExitCode::SUCCESS
        }
    }
}

/// Decode a raw binary blob, lower it, and run the Microsoft x64 ABI
/// analysis over the (single) function body.
#[cfg(feature = "capstone")]
pub(crate) fn do_win64_abi_inspect(
    path: &Path,
    base: u64,
    format: OutputFormat,
    allow_incomplete: bool,
) -> ExitCode {
    let code = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{}: error: failed to read file: {e}", path.display());
            return ExitCode::from(1);
        }
    };

    let instrs = match semasm_decode::decode_x86_64(&code, base) {
        Ok(i) => i,
        Err(DecodeError::Unsupported(_)) => {
            eprintln!(
                "error: x86-64 decoding is not compiled into this build; \
                 rebuild `semasm-cli` with the `capstone` feature"
            );
            return ExitCode::from(1);
        }
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    let (lowered, unsupported) = lower_x86_with_evidence(&instrs);

    let report = semasm_x86::abi_win64::analyze(&lowered);

    match format {
        OutputFormat::Json => {
            match serde_json::to_string_pretty(&json_win64_abi_report(
                &report,
                instrs.len(),
                lowered.len(),
                &unsupported,
            )) {
                Ok(s) => {
                    println!("{s}");
                    analysis_exit_code(report.is_clean(), unsupported.is_empty(), allow_incomplete)
                }
                Err(e) => {
                    eprintln!("failed to serialize JSON: {e}");
                    ExitCode::from(1)
                }
            }
        }
        OutputFormat::Terminal => {
            println!("instructions decoded: {}", instrs.len());
            println!("instructions lowered: {}", lowered.len());
            print_incomplete_terminal(&unsupported, allow_incomplete);
            println!("leaf function:       {}", report.is_leaf);
            println!("final RSP delta:    {}", report.final_rsp_delta);
            println!("call sites:          {}", report.call_sites.len());
            println!(
                "max below-RSP disp:  {}",
                if report.max_red_zone_disp < 0 {
                    report.max_red_zone_disp
                } else {
                    0
                }
            );
            println!();
            if report.findings.is_empty() && unsupported.is_empty() {
                println!("WIN64 ABI: clean — no Microsoft x64 violations detected");
            } else {
                for f in &report.findings {
                    let tag = match f.severity {
                        semasm_x86::abi_win64::Severity::Error => "ERROR  ",
                        semasm_x86::abi_win64::Severity::Warning => "warning",
                        semasm_x86::abi_win64::Severity::Info => "info   ",
                    };
                    let where_ = match f.at {
                        Some(i) => format!(" @{i}"),
                        None => String::new(),
                    };
                    println!("[{}] {} ({}{})", f.code, f.severity_str(), tag, where_);
                    println!("        {}", f.message);
                }
            }
            analysis_exit_code(report.is_clean(), unsupported.is_empty(), allow_incomplete)
        }
    }
}

/// Decode a raw binary blob, lower it, and run the AAPCS64 ABI
/// analysis over the (single) function body.
#[cfg(feature = "capstone")]
pub(crate) fn do_aarch64_abi_inspect(
    path: &Path,
    base: u64,
    format: OutputFormat,
    allow_incomplete: bool,
) -> ExitCode {
    let code = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("{}: error: failed to read file: {e}", path.display());
            return ExitCode::from(1);
        }
    };

    let instrs = match semasm_decode::decode_aarch64(&code, base) {
        Ok(i) => i,
        Err(DecodeError::Unsupported(_)) => {
            eprintln!(
                "error: AArch64 decoding is not compiled into this build; \
                 rebuild `semasm-cli` with the `capstone` feature"
            );
            return ExitCode::from(1);
        }
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(1);
        }
    };

    let (lowered, unsupported) = lower_aarch64_with_evidence(&instrs);

    let report = semasm_aarch64::abi::analyze(&lowered);

    match format {
        OutputFormat::Json => {
            match serde_json::to_string_pretty(&json_aarch64_abi_report(
                &report,
                instrs.len(),
                lowered.len(),
                &unsupported,
            )) {
                Ok(s) => {
                    println!("{s}");
                    analysis_exit_code(report.is_clean(), unsupported.is_empty(), allow_incomplete)
                }
                Err(e) => {
                    eprintln!("failed to serialize JSON: {e}");
                    ExitCode::from(1)
                }
            }
        }
        OutputFormat::Terminal => {
            println!("instructions decoded: {}", instrs.len());
            println!("instructions lowered: {}", lowered.len());
            print_incomplete_terminal(&unsupported, allow_incomplete);
            println!("leaf function:       {}", report.is_leaf);
            println!("final SP delta:      {}", report.final_sp_delta);
            println!("call sites:          {}", report.call_sites.len());
            println!();
            if report.findings.is_empty() && unsupported.is_empty() {
                println!("AAPCS64 ABI: clean — no AArch64 violations detected");
            } else {
                for f in &report.findings {
                    let tag = match f.severity {
                        semasm_aarch64::abi::Severity::Error => "ERROR  ",
                        semasm_aarch64::abi::Severity::Warning => "warning",
                        semasm_aarch64::abi::Severity::Info => "info   ",
                    };
                    let where_ = match f.at {
                        Some(i) => format!(" @{i}"),
                        None => String::new(),
                    };
                    println!("[{}] {} ({}{})", f.code, f.severity_str(), tag, where_);
                    println!("        {}", f.message);
                }
            }
            analysis_exit_code(report.is_clean(), unsupported.is_empty(), allow_incomplete)
        }
    }
}
