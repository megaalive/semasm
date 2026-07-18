//! Implementations for self-contained CLI commands.

use std::path::Path;
use std::process::ExitCode;

use semasm_contract::{
    check_file, explain_code, format_diagnostics_terminal, CheckReportJson, ContractCode,
};
use semasm_target::{tools, TargetIdentity};

use crate::OutputFormat;

pub(crate) fn do_explain(code: &str) -> ExitCode {
    if let Some(text) = explain_code(code) {
        println!("{text}");
        ExitCode::SUCCESS
    } else {
        eprintln!("unknown diagnostic code `{code}`");
        eprintln!("known contract codes:");
        for code in ContractCode::all() {
            eprintln!("  {}", code.as_str());
        }
        ExitCode::from(2)
    }
}

pub(crate) fn do_target_doctor(target_str: &str, format: OutputFormat) -> ExitCode {
    let identity = match TargetIdentity::parse_known(target_str) {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };

    let report = tools::doctor(&identity);
    match format {
        OutputFormat::Terminal => print_doctor_terminal(&report),
        OutputFormat::Json => {
            let json = DoctorReportJson::from_report(&report);
            if let Err(error) = serde_json::to_string_pretty(&json).map(|json| println!("{json}")) {
                eprintln!("failed to serialize JSON report: {error}");
                return ExitCode::from(1);
            }
        }
    }

    if report.all_found() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn print_doctor_terminal(report: &tools::DoctorReport) {
    let separator = "─".repeat(56);
    println!("Toolchain report for {}", report.target);
    println!("{separator}");
    println!();

    for slot in &report.slots {
        for probe in &slot.probes {
            let label = format!("{} ({})", slot.role, probe.kind);
            if probe.found {
                let version = probe.version.as_deref().unwrap_or("<version unknown>");
                println!("  {label:40} ✓  {version}");
            } else {
                println!("  {label:40} ✗  not found");
                for hint in probe.kind.install_hint() {
                    println!("  → install: {hint}");
                }
            }
        }
        println!();
    }

    let found = report.found_count();
    let total = report.total_count();
    if report.all_found() {
        println!("Result: {found}/{total} — all tool roles resolved ✓");
        println!("  target kit: ready");
    } else {
        println!("Result: {found}/{total} tool roles resolved");
        println!(
            "  target kit: ⚠ {}/{} tool roles MISSING",
            total - found,
            total
        );
    }
}

#[derive(serde::Serialize)]
struct DoctorReportJson {
    target: String,
    all_found: bool,
    found_count: usize,
    total_count: usize,
    tools: Vec<ToolSlotJson>,
}

#[derive(serde::Serialize)]
struct ToolSlotJson {
    role: String,
    resolved: Option<String>,
    candidates: Vec<ToolProbeJson>,
}

#[derive(serde::Serialize)]
struct ToolProbeJson {
    tool: String,
    found: bool,
    version: Option<String>,
    install_hints: Vec<String>,
}

impl DoctorReportJson {
    fn from_report(report: &tools::DoctorReport) -> Self {
        Self {
            target: report.target.clone(),
            all_found: report.all_found(),
            found_count: report.found_count(),
            total_count: report.total_count(),
            tools: report
                .slots
                .iter()
                .map(|slot| ToolSlotJson {
                    role: slot.role.to_string(),
                    resolved: slot
                        .resolved
                        .map(|index| slot.candidates[index].label().to_string()),
                    candidates: slot
                        .probes
                        .iter()
                        .map(|probe| ToolProbeJson {
                            tool: probe.kind.label().to_string(),
                            found: probe.found,
                            version: probe.version.clone(),
                            install_hints: probe
                                .kind
                                .install_hint()
                                .into_iter()
                                .map(ToString::to_string)
                                .collect(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

pub(crate) fn do_contract_check(path: &Path, format: OutputFormat) -> ExitCode {
    let path_display = path.display().to_string();
    let result = match check_file(path) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("{path_display}: error: failed to read file: {error}");
            return ExitCode::from(1);
        }
    };

    match format {
        OutputFormat::Terminal => {
            if result.diagnostics.is_empty() {
                if let Some(contract) = &result.contract {
                    println!("{path_display}: ok: contract `{}` is valid", contract.name);
                } else {
                    println!("{path_display}: ok");
                }
            } else {
                print!(
                    "{}",
                    format_diagnostics_terminal(&path_display, &result.diagnostics)
                );
            }
        }
        OutputFormat::Json => {
            let report = CheckReportJson::from_result(&path_display, &result);
            if let Err(error) = serde_json::to_string_pretty(&report).map(|json| println!("{json}"))
            {
                eprintln!("failed to serialize JSON report: {error}");
                return ExitCode::from(1);
            }
        }
    }

    if result.ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
