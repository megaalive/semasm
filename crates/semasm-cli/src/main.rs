//! SemASM command-line interface.
//!
//! Rich build-time tooling lives here. Generated assembly programs do not link
//! this crate or any other SemASM Rust crate by default.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use semasm_contract::{
    check_file, explain_code, format_diagnostics_terminal, CheckReportJson, ContractCode,
};
use semasm_core::SEMASM_VERSION;
use semasm_target::tools;
use semasm_target::TargetIdentity;

/// Semantic infrastructure for assembly programs (build-time tooling only).
#[derive(Debug, Parser)]
#[command(
    name = "semasm",
    version = SEMASM_VERSION,
    about = "SemASM: semantic contracts, target kits, and verification for assembly",
    long_about = "SemASM provides portable semantic contracts, multi-target kits, \
                  and verification pipelines around hand-written or agent-written \
                  assembly. It is not a high-level language and does not ship a \
                  runtime into generated programs."
)]
struct Cli {
    /// Explain a stable diagnostic code (for example CTR003) and exit.
    #[arg(long = "explain", value_name = "CODE")]
    explain: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Print version and workspace status.
    Version,
    /// Show high-level project status.
    Status,
    /// Explain a diagnostic code (same as `--explain`).
    Explain {
        /// Code such as `CTR003`.
        code: String,
    },
    /// Target-kit commands.
    Target {
        #[command(subcommand)]
        action: TargetCmd,
    },
    /// Contract commands.
    Contract {
        #[command(subcommand)]
        action: ContractCmd,
    },
}

#[derive(Debug, Subcommand)]
enum TargetCmd {
    /// Probe the host toolchain for a given target and report status.
    Doctor {
        /// Target triple (e.g. `x86_64-unknown-linux-gnu`).
        target: String,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
    },
}

#[derive(Debug, Subcommand)]
enum ContractCmd {
    /// Parse and validate a semantic contract file.
    Check {
        /// Path to a `*.sem.toml` contract.
        path: PathBuf,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    /// Human-readable diagnostics.
    Terminal,
    /// Machine-readable JSON report.
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if let Some(code) = cli.explain.as_deref() {
        return do_explain(code);
    }

    match cli.command {
        None | Some(Commands::Version) => {
            println!("semasm {SEMASM_VERSION}");
            ExitCode::SUCCESS
        }
        Some(Commands::Status) => {
            println!("semasm {SEMASM_VERSION}");
            println!("phase: VS-01 contract parser");
            println!("status: contract check available; no architecture backends yet");
            println!(
                "crates: semasm-core, semasm-contract, semasm-asir, semasm-target, semasm-cli"
            );
            println!("note: generated programs do not link SemASM by default");
            ExitCode::SUCCESS
        }
        Some(Commands::Explain { code }) => do_explain(&code),
        Some(Commands::Target { action }) => match action {
            TargetCmd::Doctor { target, format } => do_target_doctor(&target, format),
        },
        Some(Commands::Contract { action }) => match action {
            ContractCmd::Check { path, format } => do_contract_check(&path, format),
        },
    }
}

fn do_explain(code: &str) -> ExitCode {
    if let Some(text) = explain_code(code) {
        println!("{text}");
        ExitCode::SUCCESS
    } else {
        eprintln!("unknown diagnostic code `{code}`");
        eprintln!("known contract codes:");
        for c in ContractCode::all() {
            eprintln!("  {}", c.as_str());
        }
        ExitCode::from(2)
    }
}

fn do_target_doctor(target_str: &str, format: OutputFormat) -> ExitCode {
    let identity = match TargetIdentity::parse_known(target_str) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    let report = tools::doctor(&identity);

    match format {
        OutputFormat::Terminal => {
            let sep = "─".repeat(56);
            println!("Toolchain report for {}", report.target);
            println!("{sep}");
            println!();

            for slot in &report.slots {
                for probe in &slot.probes {
                    let label = format!("{} ({})", slot.role, probe.kind);
                    if probe.found {
                        let ver = probe.version.as_deref().unwrap_or("<version unknown>");
                        println!("  {label:40} ✓  {ver}");
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
        OutputFormat::Json => {
            let json = DoctorReportJson::from_report(&report);
            match serde_json::to_string_pretty(&json) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("failed to serialize JSON report: {e}");
                    return ExitCode::from(1);
                }
            }
        }
    }

    if report.all_found() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
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
                        .map(|i| slot.candidates[i].label().to_string()),
                    candidates: slot
                        .probes
                        .iter()
                        .map(|p| ToolProbeJson {
                            tool: p.kind.label().to_string(),
                            found: p.found,
                            version: p.version.clone(),
                            install_hints: p
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

fn do_contract_check(path: &std::path::Path, format: OutputFormat) -> ExitCode {
    let path_display = path.display().to_string();
    let result = match check_file(path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{path_display}: error: failed to read file: {e}");
            return ExitCode::from(1);
        }
    };

    match format {
        OutputFormat::Terminal => {
            if result.diagnostics.is_empty() {
                if let Some(c) = &result.contract {
                    println!("{path_display}: ok: contract `{}` is valid", c.name);
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
            match serde_json::to_string_pretty(&report) {
                Ok(s) => println!("{s}"),
                Err(e) => {
                    eprintln!("failed to serialize JSON report: {e}");
                    return ExitCode::from(1);
                }
            }
        }
    }

    if result.ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_debug_assert() {
        Cli::command().debug_assert();
    }
}
