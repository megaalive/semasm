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
    /// Contract commands.
    Contract {
        #[command(subcommand)]
        action: ContractCmd,
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
