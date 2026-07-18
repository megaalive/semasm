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
