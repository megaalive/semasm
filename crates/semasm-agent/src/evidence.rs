//! One-page evidence cards for agent verification reports.
//!
//! Cards are human-readable Markdown (and optional JSON) summaries suitable
//! for pasting into a PR or issue. They do not replace [`VerificationReport`];
//! they package the same facts for non-experts.

use std::fmt::Write as _;
use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::verify::{GateStatus, VerificationReport, VerificationStatus};

/// Inputs needed to render a reproducible evidence card.
#[derive(Clone, Debug)]
pub struct EvidenceCardContext<'a> {
    /// Full verification report.
    pub report: &'a VerificationReport,
    /// Path to the contract file that was checked.
    pub contract_path: &'a Path,
    /// Raw contract file bytes (for a short content hash).
    pub contract_bytes: &'a [u8],
    /// Path to the candidate assembly source.
    pub source_path: &'a Path,
    /// Size in bytes of the assembled relocatable object (0 if unavailable).
    pub object_bytes: u64,
    /// Shell-shaped command that reproduces this verify run.
    pub reproduce_cmd: &'a str,
}

/// Machine-readable card payload (subset of the full report plus provenance).
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct EvidenceCardJson<'a> {
    /// Contract basename.
    pub contract: &'a str,
    /// First 12 hex chars of SHA-256 over contract bytes.
    pub contract_hash: String,
    /// Candidate source path (as provided).
    pub source: &'a str,
    /// Target triple / identity name.
    pub target: &'a str,
    /// Aggregate verification status.
    pub status: &'a str,
    /// Execution isolation label.
    pub isolation: &'a str,
    /// Assembled object size in bytes.
    pub object_bytes: u64,
    /// Executable bytes examined by semantic gates.
    pub executable_bytes: usize,
    /// Decode modeled/total.
    pub decode: String,
    /// Lowering modeled/total and percent.
    pub lowering: String,
    /// Gate status labels.
    pub object_policy: &'a str,
    /// Gate status labels.
    pub abi: &'a str,
    /// Gate status labels.
    pub capability: &'a str,
    /// Gate status labels.
    pub control: &'a str,
    /// Executable-container gate.
    pub executable: &'a str,
    /// Passed vector count (0 when behavior absent).
    pub vectors_passed: usize,
    /// Failed vector count (0 when behavior absent).
    pub vectors_failed: usize,
    /// Total vectors when behavior present.
    pub vectors_total: usize,
    /// Reproduce command.
    pub reproduce: &'a str,
}

/// Short SHA-256 prefix for contract provenance.
#[must_use]
pub fn contract_hash_short(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(12);
    for byte in digest.iter().take(6) {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Render a Markdown evidence card.
#[must_use]
pub fn render_evidence_card_markdown(ctx: &EvidenceCardContext<'_>) -> String {
    let report = ctx.report;
    let semantic = &report.semantic;
    let contract_name = ctx
        .contract_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| ctx.contract_path.to_str().unwrap_or("contract"));
    let hash = contract_hash_short(ctx.contract_bytes);
    let source = ctx.source_path.to_string_lossy();
    let (passed, failed, total) = vector_counts(report);
    let lowering_pct = semantic.lowering.percent_modeled();

    format!(
        "\
# SemASM Evidence Card

| Field | Value |
| --- | --- |
| Contract | `{contract_name}` (`{hash}`) |
| Source | `{source}` |
| Target | `{target}` |
| Status | **{status}** |
| Isolation | `{isolation}` |
| Object size | {object_bytes} bytes |
| Executable bytes | {executable_bytes} |
| Decode | {decode_modeled}/{decode_total} |
| Lowering | {lowering_modeled}/{lowering_total} ({lowering_pct}%) |
| Object policy | `{object_policy}` |
| ABI | `{abi}` |
| Capability | `{capability}` |
| Control | `{control}` |
| Executable gate | `{executable}` |
| Vectors | {passed} passed / {failed} failed / {total} total |
{oracle_rows}
## Reproduce

```
{reproduce}
```
",
        target = report.target,
        status = report.status.as_str(),
        isolation = report.isolation.as_str(),
        object_bytes = ctx.object_bytes,
        executable_bytes = semantic.executable_bytes,
        decode_modeled = semantic.decode.modeled,
        decode_total = semantic.decode.total,
        lowering_modeled = semantic.lowering.modeled,
        lowering_total = semantic.lowering.total,
        object_policy = semantic.object_policy.as_str(),
        abi = semantic.abi.as_str(),
        capability = semantic.capability.as_str(),
        control = semantic.control.as_str(),
        executable = report.executable.status.as_str(),
        oracle_rows = match &report.behavior_oracle {
            Some(oracle) => format!(
                "| Behavior oracle | `{id}` v{version} |\n| Proof basis | `{basis}` |\n| Contract ensures | {ensures} |\n| Oracle claim | {claim} |\n| Oracle evidence | `{hash}` |\n",
                id = oracle.id,
                version = oracle.version,
                basis = oracle.proof_basis.as_str(),
                ensures = if oracle.contract_ensures.is_empty() {
                    "_(none)_".to_string()
                } else {
                    oracle
                        .contract_ensures
                        .iter()
                        .map(|e| format!("`{e}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                },
                claim = oracle.claim,
                hash = oracle.evidence_hash,
            ),
            None => String::new(),
        },
        reproduce = ctx.reproduce_cmd,
    )
}

/// Render a JSON evidence card.
///
/// # Errors
///
/// Returns an error when serialization fails.
pub fn render_evidence_card_json(
    ctx: &EvidenceCardContext<'_>,
) -> Result<String, serde_json::Error> {
    let report = ctx.report;
    let semantic = &report.semantic;
    let contract_name = ctx
        .contract_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| ctx.contract_path.to_str().unwrap_or("contract"));
    let source = ctx.source_path.to_string_lossy();
    let (passed, failed, total) = vector_counts(report);
    let card = EvidenceCardJson {
        contract: contract_name,
        contract_hash: contract_hash_short(ctx.contract_bytes),
        source: source.as_ref(),
        target: report.target.as_str(),
        status: report.status.as_str(),
        isolation: report.isolation.as_str(),
        object_bytes: ctx.object_bytes,
        executable_bytes: semantic.executable_bytes,
        decode: format!("{}/{}", semantic.decode.modeled, semantic.decode.total),
        lowering: format!(
            "{}/{} ({}%)",
            semantic.lowering.modeled,
            semantic.lowering.total,
            semantic.lowering.percent_modeled()
        ),
        object_policy: semantic.object_policy.as_str(),
        abi: semantic.abi.as_str(),
        capability: semantic.capability.as_str(),
        control: semantic.control.as_str(),
        executable: report.executable.status.as_str(),
        vectors_passed: passed,
        vectors_failed: failed,
        vectors_total: total,
        reproduce: ctx.reproduce_cmd,
    };
    serde_json::to_string_pretty(&card)
}

/// Diff two verification reports for candidate compare.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct CandidateCompareReport {
    /// Target identity shared by both runs.
    pub target: String,
    /// Routine symbol.
    pub routine_symbol: String,
    /// Status for candidate A.
    pub status_a: String,
    /// Status for candidate B.
    pub status_b: String,
    /// Gate fields that differ (`object_policy`, `abi`, …).
    pub gate_diffs: Vec<String>,
    /// Vector names present in both with different pass/fail.
    pub vector_diffs: Vec<String>,
    /// Preferred candidate when one is verified and the other is not.
    pub preferred: Option<String>,
}

/// Build a compare report from two verification outcomes.
#[must_use]
pub fn compare_reports(
    report_a: &VerificationReport,
    report_b: &VerificationReport,
    label_a: &str,
    label_b: &str,
) -> CandidateCompareReport {
    let mut gate_diffs = Vec::new();
    push_gate_diff(
        &mut gate_diffs,
        "object_policy",
        report_a.semantic.object_policy,
        report_b.semantic.object_policy,
    );
    push_gate_diff(
        &mut gate_diffs,
        "abi",
        report_a.semantic.abi,
        report_b.semantic.abi,
    );
    push_gate_diff(
        &mut gate_diffs,
        "capability",
        report_a.semantic.capability,
        report_b.semantic.capability,
    );
    push_gate_diff(
        &mut gate_diffs,
        "control",
        report_a.semantic.control,
        report_b.semantic.control,
    );
    push_gate_diff(
        &mut gate_diffs,
        "memory",
        report_a.semantic.memory,
        report_b.semantic.memory,
    );
    if report_a.semantic.decode != report_b.semantic.decode {
        gate_diffs.push(format!(
            "decode: {}/{} vs {}/{}",
            report_a.semantic.decode.modeled,
            report_a.semantic.decode.total,
            report_b.semantic.decode.modeled,
            report_b.semantic.decode.total
        ));
    }
    if report_a.semantic.lowering != report_b.semantic.lowering {
        gate_diffs.push(format!(
            "lowering: {}/{} vs {}/{}",
            report_a.semantic.lowering.modeled,
            report_a.semantic.lowering.total,
            report_b.semantic.lowering.modeled,
            report_b.semantic.lowering.total
        ));
    }

    let mut vector_diffs = Vec::new();
    if let (Some(a), Some(b)) = (&report_a.behavior, &report_b.behavior) {
        for case_a in &a.cases {
            if let Some(case_b) = b.cases.iter().find(|c| c.name == case_a.name) {
                if case_a.passed != case_b.passed {
                    vector_diffs.push(format!(
                        "{}: {} vs {}",
                        case_a.name,
                        if case_a.passed { "PASS" } else { "FAIL" },
                        if case_b.passed { "PASS" } else { "FAIL" }
                    ));
                }
            }
        }
    } else if report_a.behavior.is_some() != report_b.behavior.is_some() {
        vector_diffs.push("behavior present on only one candidate".into());
    }

    let preferred = match (report_a.status, report_b.status) {
        (a, b) if verification_rank(a) > verification_rank(b) => Some(label_a.to_string()),
        (a, b) if verification_rank(b) > verification_rank(a) => Some(label_b.to_string()),
        _ => None,
    };

    CandidateCompareReport {
        target: report_a.target.clone(),
        routine_symbol: report_a.routine_symbol.clone(),
        status_a: report_a.status.as_str().to_string(),
        status_b: report_b.status.as_str().to_string(),
        gate_diffs,
        vector_diffs,
        preferred,
    }
}

/// Rank for candidate preference: higher is better. Verified beats
/// VerifiedUnderPreconditions; both beat non-success outcomes.
fn verification_rank(status: VerificationStatus) -> u8 {
    match status {
        VerificationStatus::Verified => 3,
        VerificationStatus::VerifiedUnderPreconditions => 2,
        _ => 0,
    }
}

/// Render a compare report as Markdown.
#[must_use]
pub fn render_compare_markdown(
    compare: &CandidateCompareReport,
    label_a: &str,
    label_b: &str,
) -> String {
    let mut out = String::new();
    out.push_str("# SemASM Candidate Compare\n\n");
    let _ = write!(
        out,
        "| Field | {label_a} | {label_b} |\n| --- | --- | --- |\n"
    );
    let _ = writeln!(
        out,
        "| Status | `{}` | `{}` |",
        compare.status_a, compare.status_b
    );
    let _ = writeln!(out, "| Target | `{}` | _(same)_ |", compare.target);
    let _ = write!(
        out,
        "| Routine | `{}` | _(same)_ |\n\n",
        compare.routine_symbol
    );
    if compare.gate_diffs.is_empty() {
        out.push_str("## Gate diffs\n\n_(none)_\n\n");
    } else {
        out.push_str("## Gate diffs\n\n");
        for diff in &compare.gate_diffs {
            let _ = writeln!(out, "- {diff}");
        }
        out.push('\n');
    }
    if compare.vector_diffs.is_empty() {
        out.push_str("## Vector diffs\n\n_(none)_\n\n");
    } else {
        out.push_str("## Vector diffs\n\n");
        for diff in &compare.vector_diffs {
            let _ = writeln!(out, "- {diff}");
        }
        out.push('\n');
    }
    if let Some(preferred) = &compare.preferred {
        let _ = writeln!(out, "**Preferred:** `{preferred}`");
    }
    out
}

fn vector_counts(report: &VerificationReport) -> (usize, usize, usize) {
    match &report.behavior {
        Some(behavior) => {
            let passed = behavior.cases.iter().filter(|c| c.passed).count();
            let total = behavior.cases.len();
            (passed, total - passed, total)
        }
        None => (0, 0, 0),
    }
}

fn push_gate_diff(out: &mut Vec<String>, name: &str, a: GateStatus, b: GateStatus) {
    if a != b {
        out.push(format!("{name}: {} vs {}", a.as_str(), b.as_str()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::{
        Coverage, ExecutableGate, ExecutionIsolation, GateStatus, SemanticGates, VerificationReport,
    };

    fn sample_report(status_control: GateStatus) -> VerificationReport {
        let semantic = SemanticGates {
            object_policy: GateStatus::Passed,
            executable_bytes: 32,
            decode: Coverage::complete(4),
            lowering: Coverage::complete(4),
            abi: GateStatus::Passed,
            capability: GateStatus::Passed,
            control: status_control,
            memory: GateStatus::Passed,
        };
        VerificationReport::from_parts(
            "x86_64-unknown-linux-gnu".into(),
            "count_byte".into(),
            semantic,
            ExecutableGate::passed(),
            None,
            ExecutionIsolation::StaticOnly,
        )
    }

    #[test]
    fn markdown_card_mentions_status_and_control() {
        let report = sample_report(GateStatus::Passed);
        let contract = b"contract_version = \"0.1\"\n";
        let md = render_evidence_card_markdown(&EvidenceCardContext {
            report: &report,
            contract_path: Path::new("fixtures/contracts/count_byte.sem.toml"),
            contract_bytes: contract,
            source_path: Path::new("fixtures/asm/count_byte.asm"),
            object_bytes: 128,
            reproduce_cmd: "semasm agent verify fixtures/asm/count_byte.asm fixtures/contracts/count_byte.sem.toml",
        });
        assert!(md.contains("execution_denied"));
        assert!(md.contains("Control"));
        assert!(md.contains("128 bytes"));
        assert!(md.contains("Reproduce"));
    }

    #[test]
    fn compare_detects_control_gate_diff() {
        let a = sample_report(GateStatus::Passed);
        let mut b = sample_report(GateStatus::Failed);
        // force semantic_failed on b
        b = VerificationReport::from_parts(
            b.target,
            b.routine_symbol,
            SemanticGates {
                object_policy: GateStatus::Passed,
                executable_bytes: 32,
                decode: Coverage::complete(4),
                lowering: Coverage::complete(4),
                abi: GateStatus::Passed,
                capability: GateStatus::Passed,
                control: GateStatus::Failed,
                memory: GateStatus::Passed,
            },
            ExecutableGate::skipped(),
            None,
            ExecutionIsolation::StaticOnly,
        );
        let cmp = compare_reports(&a, &b, "a.asm", "b.asm");
        assert_eq!(cmp.status_a, "execution_denied");
        assert_eq!(cmp.status_b, "semantic_failed");
        assert!(cmp.gate_diffs.iter().any(|d| d.starts_with("control:")));
    }
}
