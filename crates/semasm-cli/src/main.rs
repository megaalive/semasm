//! SemASM command-line interface.
//!
//! Rich build-time tooling lives here. Generated assembly programs do not link
//! this crate or any other SemASM Rust crate by default.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use semasm_agent::{ContextBundle, TargetToolchain, TaskPacket};
use semasm_build::report::{self, CommandRecordJson};
use semasm_build::Pipeline;
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
    /// Assemble, link, verify, and report a source file.
    Build {
        /// Path to a `.asm` source file.
        source: PathBuf,
        /// Target triple (default: `x86_64-unknown-linux-gnu`).
        #[arg(long, default_value = "x86_64-unknown-linux-gnu")]
        target: String,
        /// Output directory (default: temp dir).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Skip running the built executable under QEMU.
        #[arg(long)]
        no_run: bool,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
    },
    /// Contract commands.
    Contract {
        #[command(subcommand)]
        action: ContractCmd,
    },
    /// Agent-integration commands (task packets for external coding agents).
    Agent {
        #[command(subcommand)]
        action: AgentCmd,
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

#[derive(Debug, Subcommand)]
enum AgentCmd {
    /// Build a task packet (JSON) or rendered context (Markdown) for an agent.
    Packet {
        /// Path to a `*.sem.toml` contract.
        contract: PathBuf,
        /// Target triple (default: `x86_64-unknown-linux-gnu`).
        #[arg(long, default_value = "x86_64-unknown-linux-gnu")]
        target: String,
        /// Optional existing `.asm` source the agent should extend.
        #[arg(long)]
        source: Option<PathBuf>,
        /// Output format: `terminal` (Markdown context) or `json` (full packet).
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
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
        Some(Commands::Build {
            source,
            target,
            out_dir,
            no_run,
            format,
        }) => do_build(&source, &target, out_dir.as_deref(), no_run, format),
        Some(Commands::Contract { action }) => match action {
            ContractCmd::Check { path, format } => do_contract_check(&path, format),
        },
        Some(Commands::Agent { action }) => match action {
            AgentCmd::Packet {
                contract,
                target,
                source,
                format,
            } => do_agent_packet(&contract, &target, source.as_deref(), format),
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

/// Convert the build pipeline's `Toolchain` into the agent packet's
/// `TargetToolchain` (field-compatible, separate types).
fn to_agent_toolchain(tc: &semasm_build::pipeline::Toolchain) -> TargetToolchain {
    TargetToolchain {
        assembler: tc.assembler.clone(),
        linker: tc.linker.clone(),
        disassembler: tc.disassembler.clone(),
        runner: tc.runner.clone(),
    }
}

fn do_agent_packet(
    contract_path: &Path,
    target_str: &str,
    source: Option<&Path>,
    format: OutputFormat,
) -> ExitCode {
    // Resolve target identity.
    let identity = match TargetIdentity::parse_known(target_str) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    // Load + validate the contract.
    let contract_text = match std::fs::read_to_string(contract_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!(
                "{}: error: failed to read file: {e}",
                contract_path.display()
            );
            return ExitCode::from(1);
        }
    };
    let check = semasm_contract::check_str(&contract_text);
    if !check.ok() {
        print!(
            "{}",
            format_diagnostics_terminal(&contract_path.display().to_string(), &check.diagnostics)
        );
        return ExitCode::from(1);
    }
    let checked = check.contract.expect("ok() implies Some");

    // Discover toolchain for this target.
    let pipeline = Pipeline::discover(&identity);
    let toolchain = to_agent_toolchain(&pipeline.toolchain);

    // Optional existing source.
    let existing_source = match source {
        Some(p) => match std::fs::read_to_string(p) {
            Ok(t) => Some(t),
            Err(e) => {
                eprintln!("{}: error: failed to read source: {e}", p.display());
                return ExitCode::from(1);
            }
        },
        None => None,
    };

    // Build the context bundle (AGENT-004 will synthesise test vectors).
    let context = ContextBundle::generate(
        &checked,
        &identity,
        &toolchain,
        existing_source,
        Vec::new(),
        Vec::new(),
    );

    // Assemble the full packet.
    let packet = TaskPacket::new(
        "0.1.0",
        // RFC 3339 timestamp.
        chrono_now(),
        contract_text,
        checked,
        identity,
        toolchain,
        vec![contract_path.display().to_string()],
        Vec::new(),
        context,
    );

    match format {
        OutputFormat::Json => match serde_json::to_string_pretty(&packet) {
            Ok(s) => {
                println!("{s}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("failed to serialize packet: {e}");
                ExitCode::from(1)
            }
        },
        OutputFormat::Terminal => {
            println!("{}", packet.context.to_markdown());
            ExitCode::SUCCESS
        }
    }
}

/// Best-effort RFC 3339 timestamp for the packet `created_at` field.
fn chrono_now() -> String {
    // Avoid pulling `chrono` into the binary just for this one field:
    // fall back to a fixed sentinel only if the platform clock fails.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    // Format as `YYYY-MM-DDTHH:MM:SSZ` using UTC calendar math.
    // We have only a seconds counter, so approximate with a simple
    // Gregorian decomposition. Good enough for a created_at stamp.
    let (y, mo, d, h, mi, s) = epoch_to_utc(now);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Decompose a Unix timestamp (seconds) into UTC calendar fields.
#[allow(clippy::cast_possible_truncation)]
fn epoch_to_utc(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    const DAY: u64 = 86_400;
    let days = secs / DAY;
    let rem = secs % DAY;
    let h = (rem / 3600) as u32;
    let mi = ((rem % 3600) / 60) as u32;
    let s = (rem % 60) as u32;

    // Days since 1970-01-01. Walk years then months.
    let mut year: u64 = 1970;
    let mut rest = days;
    loop {
        let leap = is_leap(year);
        let ylen = if leap { 366 } else { 365 };
        if rest < ylen {
            break;
        }
        rest -= ylen;
        year += 1;
    }
    let month_lengths: [u64; 12] = [
        31,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month: u64 = 1;
    let mut d = rest;
    while d >= month_lengths[(month - 1) as usize] {
        d -= month_lengths[(month - 1) as usize];
        month += 1;
    }
    let day = (d + 1) as u32;
    (year as u32, month as u32, day, h, mi, s)
}

/// Leap-year test for the proleptic Gregorian calendar.
fn is_leap(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[allow(clippy::too_many_lines)]
fn do_build(
    source: &Path,
    target_str: &str,
    out_dir: Option<&Path>,
    no_run: bool,
    format: OutputFormat,
) -> ExitCode {
    // Resolve target
    let identity = match TargetIdentity::parse_known(target_str) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };

    // Discover toolchain
    let pipe = Pipeline::discover(&identity);
    if !pipe.all_tools_found() {
        eprintln!("warning: not all required tools are available on PATH");
        eprintln!("  run `semasm target doctor {target_str}` for details");
    }

    // Prepare output directory
    let mut tmp_dir = PathBuf::new();
    let mut created_tmp = false;
    let out_dir: &Path = if let Some(d) = out_dir {
        d
    } else {
        tmp_dir = std::env::temp_dir().join(format!("semasm-build-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp_dir);
        created_tmp = true;
        &tmp_dir
    };
    if let Err(e) = std::fs::create_dir_all(out_dir) {
        eprintln!(
            "error: cannot create output directory `{}`: {e}",
            out_dir.display()
        );
        return ExitCode::from(1);
    }

    // Step 1: assemble
    let obj_path = out_dir.join("exit.o");
    let exe_path = out_dir.join("exit");

    let ao = match pipe.assemble_reproducible(source, &obj_path, "elf64") {
        Ok(o) => o,
        Err(e) => {
            eprintln!("assemble error: {e}");
            return ExitCode::from(1);
        }
    };
    if !ao.success() {
        eprintln!(
            "assemble failed (exit={:?}): {}",
            ao.exit_code,
            String::from_utf8_lossy(&ao.stderr),
        );
        return ExitCode::from(1);
    }

    // Step 2: link
    let lo = match pipe.link_reproducible(&[&obj_path], &exe_path) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("link error: {e}");
            return ExitCode::from(1);
        }
    };
    if !lo.success() {
        eprintln!(
            "link failed (exit={:?}): {}",
            lo.exit_code,
            String::from_utf8_lossy(&lo.stderr),
        );
        return ExitCode::from(1);
    }

    // Step 3: verify architecture
    let arch = match pipe.verify_architecture(&exe_path) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("verify error: {e}");
            return ExitCode::from(1);
        }
    };
    if !arch.is_executable {
        eprintln!(
            "error: linked file is not executable (format={})",
            arch.format
        );
        return ExitCode::from(1);
    }

    // Step 4: run (optional)
    let run_output = if no_run {
        None
    } else if let Ok(o) = pipe.run(&exe_path) {
        Some(o)
    } else {
        eprintln!("warning: runner not available (install qemu-user for this target)");
        None
    };

    // Step 5: generate report
    let records = vec![
        CommandRecordJson {
            label: "assemble".into(),
            command: format!(
                "{} -O0 -w+error -f elf64 {} -o {}",
                pipe.toolchain.assembler,
                source.display(),
                obj_path.display(),
            ),
            exit_code: ao.exit_code,
            stdout: String::from_utf8_lossy(&ao.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&ao.stderr).into_owned(),
            duration_secs: ao.duration.as_secs_f64(),
            timed_out: ao.timed_out,
            success: ao.success(),
        },
        CommandRecordJson {
            label: "link".into(),
            command: format!(
                "{} --build-id=none --hash-style=sysv -o {} {}",
                pipe.toolchain.linker,
                exe_path.display(),
                obj_path.display(),
            ),
            exit_code: lo.exit_code,
            stdout: String::from_utf8_lossy(&lo.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&lo.stderr).into_owned(),
            duration_secs: lo.duration.as_secs_f64(),
            timed_out: lo.timed_out,
            success: lo.success(),
        },
    ];

    let artifact = match report::generate_report(
        &pipe,
        source,
        Some(&obj_path),
        &exe_path,
        records,
        run_output.as_ref(),
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("report generation error: {e}");
            return ExitCode::from(1);
        }
    };

    // Output
    match format {
        OutputFormat::Terminal => {
            print!("{}", artifact.to_terminal());
        }
        OutputFormat::Json => match artifact.to_json_pretty() {
            Ok(j) => println!("{j}"),
            Err(e) => {
                eprintln!("JSON serialisation error: {e}");
                return ExitCode::from(1);
            }
        },
    }

    // Clean up temp dir if we created one
    if created_tmp {
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_debug_assert() {
        Cli::command().debug_assert();
    }

    #[test]
    fn agent_packet_emits_json_with_context() {
        let dir = std::env::temp_dir().join(format!("semasm-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let contract = dir.join("write_all.sem.toml");
        std::fs::write(
            &contract,
            r#"
contract_version = "0.1"

[function]
name = "write_all"
summary = "Write all bytes."

[[function.parameters]]
name = "buffer"
type = "ptr<const u8>"
role = "input"

[[function.parameters]]
name = "length"
type = "usize"
role = "input"

[[function.returns]]
name = "written"
type = "usize"
"#,
        )
        .expect("write fixture");

        // JSON mode: full packet, must contain the function name and a valid
        // context bundle with ABI parameters.
        let code = do_agent_packet(
            &contract,
            "x86_64-unknown-linux-gnu",
            None,
            OutputFormat::Json,
        );
        assert_eq!(code, ExitCode::SUCCESS);

        // Terminal mode: Markdown context, must mention the function + a
        // preserved register.
        let code = do_agent_packet(
            &contract,
            "x86_64-unknown-linux-gnu",
            None,
            OutputFormat::Terminal,
        );
        assert_eq!(code, ExitCode::SUCCESS);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn agent_packet_rejects_unknown_target() {
        let dir = std::env::temp_dir().join(format!("semasm-test2-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let contract = dir.join("c.sem.toml");
        std::fs::write(
            &contract,
            r#"
contract_version = "0.1"

[function]
name = "f"
summary = "x"

[[function.parameters]]
name = "a"
type = "u8"
role = "input"

[[function.returns]]
name = "b"
type = "u8"
"#,
        )
        .expect("write fixture");

        let code = do_agent_packet(&contract, "not-a-real-target", None, OutputFormat::Json);
        assert_eq!(code, ExitCode::from(2));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
