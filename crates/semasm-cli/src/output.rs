//! Output adapters for CLI reports.

/// Print an x86 analysis report in human-readable form.
pub(crate) fn print_analysis_terminal(r: &semasm_x86::analysis::AnalysisReport) {
    println!("fixpoint iterations: {}", r.iterations);
    println!("blocks analysed:      {}", r.block_out.len());
    println!("memory accesses:       {}", r.mem_accesses.len());
    println!();
    if r.notes.is_empty() {
        println!("ANALYSIS: no register/memory notes (all state unknown/empty)");
    } else {
        for n in &r.notes {
            println!("[{}] @block {}: {}", n.code, n.block, n.message);
        }
    }
}

/// A JSON-friendly view of [`semasm_x86::analysis::AnalysisReport`].
#[derive(serde::Serialize)]
pub(crate) struct JsonAnalysisReport {
    iterations: usize,
    converged: bool,
    block_count: usize,
    mem_access_count: usize,
    notes: Vec<JsonAnalysisNote>,
}

#[derive(serde::Serialize)]
struct JsonAnalysisNote {
    code: String,
    block: usize,
    message: String,
}

pub(crate) fn json_analysis_report(r: &semasm_x86::analysis::AnalysisReport) -> JsonAnalysisReport {
    JsonAnalysisReport {
        iterations: r.iterations,
        converged: r.converged,
        block_count: r.block_out.len(),
        mem_access_count: r.mem_accesses.len(),
        notes: r
            .notes
            .iter()
            .map(|n| JsonAnalysisNote {
                code: n.code.to_string(),
                block: n.block,
                message: n.message.clone(),
            })
            .collect(),
    }
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct UnsupportedInstruction {
    pub(crate) address: u64,
    pub(crate) bytes: Vec<u8>,
    pub(crate) mnemonic: String,
    pub(crate) operands: Vec<String>,
}

pub(crate) fn unsupported_instruction(
    instruction: &semasm_decode::PhysicalInstruction,
    mnemonic: String,
) -> UnsupportedInstruction {
    UnsupportedInstruction {
        address: instruction.address,
        bytes: instruction.bytes.clone(),
        mnemonic,
        operands: instruction.operands.clone(),
    }
}

fn analysis_status(unsupported: &[UnsupportedInstruction]) -> &'static str {
    if unsupported.is_empty() {
        "complete"
    } else {
        "incomplete"
    }
}

#[derive(serde::Serialize)]
pub(crate) struct JsonAbiReport {
    instructions_decoded: usize,
    instructions_lowered: usize,
    status: &'static str,
    unsupported: Vec<UnsupportedInstruction>,
    is_leaf: bool,
    has_syscall: bool,
    final_rsp_delta: i64,
    call_site_count: usize,
    max_red_zone_disp: i64,
    clean: bool,
    findings: Vec<JsonAbiFinding>,
}

#[derive(serde::Serialize)]
struct JsonAbiFinding {
    code: String,
    severity: String,
    at: Option<usize>,
    message: String,
}

pub(crate) fn json_abi_report(
    r: &semasm_x86::abi::AbiReport,
    decoded_count: usize,
    lowered_count: usize,
    unsupported: &[UnsupportedInstruction],
) -> JsonAbiReport {
    JsonAbiReport {
        instructions_decoded: decoded_count,
        instructions_lowered: lowered_count,
        status: analysis_status(unsupported),
        unsupported: unsupported.to_vec(),
        is_leaf: r.is_leaf,
        has_syscall: r.has_syscall,
        final_rsp_delta: r.final_rsp_delta,
        call_site_count: r.call_sites.len(),
        max_red_zone_disp: r.max_red_zone_disp.min(0),
        clean: r.is_clean() && unsupported.is_empty(),
        findings: r
            .findings
            .iter()
            .map(|f| JsonAbiFinding {
                code: f.code.to_string(),
                severity: f.severity_str().to_string(),
                at: f.at,
                message: f.message.clone(),
            })
            .collect(),
    }
}

#[derive(serde::Serialize)]
pub(crate) struct JsonWin64AbiReport {
    instructions_decoded: usize,
    instructions_lowered: usize,
    status: &'static str,
    unsupported: Vec<UnsupportedInstruction>,
    is_leaf: bool,
    final_rsp_delta: i64,
    call_site_count: usize,
    max_below_rsp_disp: i64,
    clean: bool,
    findings: Vec<JsonWin64AbiFinding>,
}

#[derive(serde::Serialize)]
struct JsonWin64AbiFinding {
    code: String,
    severity: String,
    at: Option<usize>,
    message: String,
}

pub(crate) fn json_win64_abi_report(
    r: &semasm_x86::abi_win64::AbiReport,
    decoded_count: usize,
    lowered_count: usize,
    unsupported: &[UnsupportedInstruction],
) -> JsonWin64AbiReport {
    JsonWin64AbiReport {
        instructions_decoded: decoded_count,
        instructions_lowered: lowered_count,
        status: analysis_status(unsupported),
        unsupported: unsupported.to_vec(),
        is_leaf: r.is_leaf,
        final_rsp_delta: r.final_rsp_delta,
        call_site_count: r.call_sites.len(),
        max_below_rsp_disp: r.max_red_zone_disp.min(0),
        clean: r.is_clean() && unsupported.is_empty(),
        findings: r
            .findings
            .iter()
            .map(|f| JsonWin64AbiFinding {
                code: f.code.to_string(),
                severity: f.severity_str().to_string(),
                at: f.at,
                message: f.message.clone(),
            })
            .collect(),
    }
}

#[derive(serde::Serialize)]
pub(crate) struct JsonAarch64AbiReport {
    instructions_decoded: usize,
    instructions_lowered: usize,
    status: &'static str,
    unsupported: Vec<UnsupportedInstruction>,
    is_leaf: bool,
    final_sp_delta: i64,
    call_site_count: usize,
    clean: bool,
    findings: Vec<JsonAarch64AbiFinding>,
}

#[derive(serde::Serialize)]
struct JsonAarch64AbiFinding {
    code: String,
    severity: String,
    at: Option<usize>,
    message: String,
}

pub(crate) fn json_aarch64_abi_report(
    r: &semasm_aarch64::abi::AbiReport,
    decoded_count: usize,
    lowered_count: usize,
    unsupported: &[UnsupportedInstruction],
) -> JsonAarch64AbiReport {
    JsonAarch64AbiReport {
        instructions_decoded: decoded_count,
        instructions_lowered: lowered_count,
        status: analysis_status(unsupported),
        unsupported: unsupported.to_vec(),
        is_leaf: r.is_leaf,
        final_sp_delta: r.final_sp_delta,
        call_site_count: r.call_sites.len(),
        clean: r.is_clean() && unsupported.is_empty(),
        findings: r
            .findings
            .iter()
            .map(|f| JsonAarch64AbiFinding {
                code: f.code.to_string(),
                severity: f.severity_str().to_string(),
                at: f.at,
                message: f.message.clone(),
            })
            .collect(),
    }
}
