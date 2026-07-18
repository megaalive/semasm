//! SemASM command-line interface.
//!
//! Rich build-time tooling lives here. Generated assembly programs do not link
//! this crate or any other SemASM Rust crate by default.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use semasm_agent::{harness, ContextBundle, TargetToolchain, TaskPacket};
use semasm_build::report::{self, CommandRecordJson};
use semasm_build::Pipeline;
use semasm_contract::{
    check_file, explain_code, format_diagnostics_terminal, CheckReportJson, ContractCode,
};
use semasm_core::SEMASM_VERSION;
#[cfg(feature = "capstone")]
use semasm_decode::{self, DecodeError};
use semasm_obj::{self, ObjectError};
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
    /// Object-file inspection (ELF/PE/Mach-O).
    Obj {
        /// Path to the object file to inspect.
        path: PathBuf,
        /// Optional target triple; when given, architecture mismatch is a
        /// hard error.
        #[arg(long)]
        target: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    /// Disassemble raw machine code (Capstone-backed).
    #[cfg(feature = "capstone")]
    Decode {
        /// Path to a raw binary blob to disassemble.
        path: PathBuf,
        /// Base address assigned to the first byte (default: 0; `0x` hex ok).
        #[arg(long, default_value = "0")]
        base: String,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
    },
    /// Build a control-flow graph from raw machine code (Capstone-backed).
    #[cfg(feature = "capstone")]
    Cfg {
        /// Path to a raw binary blob to analyse.
        path: PathBuf,
        /// Base address assigned to the first byte (default:0; `0x` hex ok).
        #[arg(long, default_value = "0")]
        base: String,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
    },
    /// Check a function body against the System V AMD64 ABI
    /// (decode + lower + ABI analysis).
    #[cfg(feature = "capstone")]
    Abi {
        /// Path to a raw binary blob containing one function body.
        path: PathBuf,
        /// Base address assigned to the first byte (default:0; `0x` hex ok).
        #[arg(long, default_value = "0")]
        base: String,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
    },
    /// Forward data-flow analysis over a function body (decode + lower +
    /// control-flow graph + abstract interpretation).
    #[cfg(feature = "capstone")]
    Analyze {
        /// Path to a raw binary blob containing one function body.
        path: PathBuf,
        /// Base address assigned to the first byte (default:0; `0x` hex ok).
        #[arg(long, default_value = "0")]
        base: String,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
    },
    /// Check a function body against the Microsoft x64 ABI
    /// (decode + lower + ABI analysis).
    #[cfg(feature = "capstone")]
    Win64Abi {
        /// Path to a raw binary blob containing one function body.
        path: PathBuf,
        /// Base address assigned to the first byte (default:0; `0x` hex ok).
        #[arg(long, default_value = "0")]
        base: String,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
    },
    /// Check a function body against the AAPCS64 ABI
    /// (decode + lower + ABI analysis).
    #[cfg(feature = "capstone")]
    Aarch64Abi {
        /// Path to a raw binary blob containing one function body.
        path: PathBuf,
        /// Base address assigned to the first byte (default:0; `0x` hex ok).
        #[arg(long, default_value = "0")]
        base: String,
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
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
    /// Assemble, link, and run an agent-written `.asm` against synthesized
    /// test vectors, reporting per-vector pass/fail.
    Verify {
        /// Path to the agent-written `.asm` routine (must `global` the
        /// routine symbol named by the contract).
        source: PathBuf,
        /// Path to the `*.sem.toml` contract.
        contract: PathBuf,
        /// Target triple (default: `x86_64-unknown-linux-gnu`).
        #[arg(long, default_value = "x86_64-unknown-linux-gnu")]
        target: String,
        /// Output format: `terminal` (human) or `json` (HarnessReport).
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

#[allow(clippy::too_many_lines)]
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
            match semasm_target::capability::CapabilityManifest::parse(include_str!(
                "../../../capabilities.toml"
            )) {
                Ok(manifest) => {
                    print!("{}", manifest.render_status(SEMASM_VERSION));
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("error: failed to load embedded capability manifest: {error}");
                    ExitCode::from(1)
                }
            }
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
            AgentCmd::Verify {
                source,
                contract,
                target,
                format,
            } => do_agent_verify(&source, &contract, &target, format),
        },
        Some(Commands::Obj {
            path,
            target,
            format,
        }) => do_obj_inspect(&path, target.as_deref(), format),
        #[cfg(feature = "capstone")]
        Some(Commands::Decode { path, base, format }) => {
            let base = match parse_base(&base) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(2);
                }
            };
            do_decode_inspect(&path, base, format)
        }
        #[cfg(feature = "capstone")]
        Some(Commands::Cfg { path, base, format }) => {
            let base = match parse_base(&base) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(2);
                }
            };
            do_cfg_inspect(&path, base, format)
        }
        #[cfg(feature = "capstone")]
        Some(Commands::Abi { path, base, format }) => {
            let base = match parse_base(&base) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(2);
                }
            };
            do_abi_inspect(&path, base, format)
        }
        #[cfg(feature = "capstone")]
        Some(Commands::Analyze { path, base, format }) => {
            let base = match parse_base(&base) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(2);
                }
            };
            do_analyze_inspect(&path, base, format)
        }
        #[cfg(feature = "capstone")]
        Some(Commands::Win64Abi { path, base, format }) => {
            let base = match parse_base(&base) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(2);
                }
            };
            do_win64_abi_inspect(&path, base, format)
        }
        #[cfg(feature = "capstone")]
        Some(Commands::Aarch64Abi { path, base, format }) => {
            let base = match parse_base(&base) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(2);
                }
            };
            do_aarch64_abi_inspect(&path, base, format)
        }
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

/// Assemble, link, and run an agent-written `.asm` against the
/// synthesised behavioural test vectors, then evaluate the results.
#[allow(clippy::too_many_lines)]
fn do_agent_verify(
    source: &Path,
    contract_path: &Path,
    target_str: &str,
    format: OutputFormat,
) -> ExitCode {
    // Resolve target + toolchain.
    let identity = match TargetIdentity::parse_known(target_str) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let pipeline = Pipeline::discover(&identity);
    if !pipeline.all_tools_found() {
        eprintln!("error: toolchain incomplete for target `{target_str}`");
        eprintln!("  run `semasm target doctor {target_str}` for details");
        eprintln!(
            "  (assembler, linker, disassembler, and a runner such as qemu-x86_64 are required)"
        );
        return ExitCode::from(1);
    }

    // Load + validate the contract.
    let contract_text = match std::fs::read_to_string(contract_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{}: error: {e}", contract_path.display());
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

    // Synthesise vectors + generate the harness source.
    let vectors = harness::synthesize_vectors(&checked);
    if vectors.is_empty() {
        eprintln!(
            "error: no test vectors synthesised for `{}`; \
             the routine shape is not yet supported by the harness",
            checked.name
        );
        return ExitCode::from(1);
    }
    let harness_src = harness::generate_harness(&checked.name, &vectors);
    let routine_symbol = checked.name.clone();

    // Prepare a scratch directory.
    let dir = std::env::temp_dir().join(format!("semasm-verify-{}", std::process::id()));
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("error: cannot create scratch dir: {e}");
        return ExitCode::from(1);
    }
    let routine_obj = dir.join("routine.o");
    let harness_obj = dir.join("harness.o");
    let exe = dir.join("harness");

    // Assemble the routine.
    match pipeline.assemble_reproducible(source, &routine_obj, "elf64") {
        Ok(o) if o.success() => {}
        Ok(o) => {
            eprintln!(
                "assemble routine failed: {}",
                String::from_utf8_lossy(&o.stderr)
            );
            let _ = std::fs::remove_dir_all(&dir);
            return ExitCode::from(1);
        }
        Err(e) => {
            eprintln!("assemble routine error: {e}");
            let _ = std::fs::remove_dir_all(&dir);
            return ExitCode::from(1);
        }
    }

    // Assemble the harness.
    let harness_path = dir.join("harness.asm");
    if let Err(e) = std::fs::write(&harness_path, &harness_src) {
        eprintln!("error: cannot write harness source: {e}");
        let _ = std::fs::remove_dir_all(&dir);
        return ExitCode::from(1);
    }
    match pipeline.assemble_reproducible(&harness_path, &harness_obj, "elf64") {
        Ok(o) if o.success() => {}
        Ok(o) => {
            eprintln!(
                "assemble harness failed: {}",
                String::from_utf8_lossy(&o.stderr)
            );
            let _ = std::fs::remove_dir_all(&dir);
            return ExitCode::from(1);
        }
        Err(e) => {
            eprintln!("assemble harness error: {e}");
            let _ = std::fs::remove_dir_all(&dir);
            return ExitCode::from(1);
        }
    }

    // Link both objects.
    match pipeline.link_reproducible(&[&routine_obj, &harness_obj], &exe) {
        Ok(o) if o.success() => {}
        Ok(o) => {
            eprintln!("link failed: {}", String::from_utf8_lossy(&o.stderr));
            let _ = std::fs::remove_dir_all(&dir);
            return ExitCode::from(1);
        }
        Err(e) => {
            eprintln!("link error: {e}");
            let _ = std::fs::remove_dir_all(&dir);
            return ExitCode::from(1);
        }
    }

    // Run under the configured runner.
    let run = match pipeline.run(&exe) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("run error: {e}");
            let _ = std::fs::remove_dir_all(&dir);
            return ExitCode::from(1);
        }
    };

    let report = harness::evaluate(&run.stdout, &vectors);

    // Clean up scratch.
    let _ = std::fs::remove_dir_all(&dir);

    match format {
        OutputFormat::Json => match serde_json::to_string_pretty(&report) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("failed to serialize report: {e}");
                return ExitCode::from(1);
            }
        },
        OutputFormat::Terminal => {
            println!("Routine: {routine_symbol}");
            println!("Vectors: {}", report.cases.len());
            println!();
            for (i, c) in report.cases.iter().enumerate() {
                let status = if c.passed { "PASS" } else { "FAIL" };
                println!(
                    "{i}. [{status}] {}  expected={} observed={}",
                    c.name, c.expected, c.observed
                );
            }
            println!(
                "\nResult: {}",
                if report.all_passed {
                    "all vectors passed"
                } else {
                    "one or more vectors failed"
                }
            );
        }
    }

    if report.all_passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
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

/// Discover the program entry symbol from a COFF/PE object's exported
/// (global, defined) symbols. Prefers `main` or `_start`; otherwise the
/// first exported symbol is used.
#[must_use]
fn link_entry_of(obj: &Path) -> Option<String> {
    let info = semasm_obj::read(obj).ok()?;
    if info.exports.is_empty() {
        return None;
    }
    info.exports
        .iter()
        .find(|s| *s == "main" || *s == "_start" || *s == "mainCRTStartup")
        .or_else(|| info.exports.first())
        .cloned()
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

    // Step 1: assemble (format + object/executable naming are target-specific)
    let obj_ext = if identity.object_format == semasm_target::ObjectFormat::PeCoff {
        "obj"
    } else {
        "o"
    };
    let obj_path = out_dir.join(format!("exit.{obj_ext}"));
    let exe_path = if identity.object_format == semasm_target::ObjectFormat::PeCoff {
        out_dir.join("exit.exe")
    } else {
        out_dir.join("exit")
    };

    let ao = match pipe.assemble_reproducible(source, &obj_path, identity.nasm_format()) {
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
    let lo = if identity.object_format == semasm_target::ObjectFormat::PeCoff {
        let entry = link_entry_of(&obj_path).unwrap_or_else(|| "main".to_string());
        match pipe.link_pe(&[&obj_path], &exe_path, &entry, "console") {
            Ok(o) => o,
            Err(e) => {
                eprintln!("link error: {e}");
                return ExitCode::from(1);
            }
        }
    } else {
        match pipe.link_reproducible(&[&obj_path], &exe_path) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("link error: {e}");
                return ExitCode::from(1);
            }
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
            stdout_capture: ao.stdout_capture.clone(),
            stderr_capture: ao.stderr_capture.clone(),
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
            stdout_capture: lo.stdout_capture.clone(),
            stderr_capture: lo.stderr_capture.clone(),
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

    #[test]
    fn agent_verify_reports_incomplete_toolchain() {
        // On a host without the full toolchain (notably no qemu-user
        // runner), `verify` must report the missing tools and exit non-zero
        // rather than attempting to run a half-built binary.
        let dir = std::env::temp_dir().join(format!("semasm-verify-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let contract = dir.join("count_byte.sem.toml");
        std::fs::write(
            &contract,
            r#"
contract_version = "0.1"

[function]
name = "count_byte"
summary = "Count a byte in a buffer."

[[function.parameters]]
name = "buffer"
type = "ptr<const u8>"
role = "input"

[[function.parameters]]
name = "length"
type = "usize"
role = "input"

[[function.parameters]]
name = "needle"
type = "u8"
role = "input"

[[function.returns]]
name = "count"
type = "usize"
"#,
        )
        .expect("write fixture");
        let routine = dir.join("count_byte.asm");
        std::fs::write(
            &routine,
            "BITS 64\nDEFAULT REL\nglobal count_byte\nsection .text\ncount_byte:\n mov rax, rsi\n ret\n",
        )
        .expect("write routine");

        let code = do_agent_verify(
            &routine,
            &contract,
            "x86_64-unknown-linux-gnu",
            OutputFormat::Terminal,
        );
        // Without a runner (qemu) the toolchain is incomplete → non-zero.
        assert!(code != ExitCode::SUCCESS);

        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Inspect an object file and emit its normalised view.
#[allow(clippy::items_after_test_module)]
fn do_obj_inspect(path: &Path, target: Option<&str>, format: OutputFormat) -> ExitCode {
    let info = match target {
        Some(t) => {
            let identity = match TargetIdentity::parse_known(t) {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(2);
                }
            };
            match semasm_obj::read_for_target(path, &identity) {
                Ok(i) => i,
                Err(ObjectError::ArchitectureMismatch { actual, expected }) => {
                    eprintln!("error: architecture mismatch: object `{actual}` but target requires `{expected}`");
                    return ExitCode::from(2);
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(1);
                }
            }
        }
        None => match semasm_obj::read(path) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(1);
            }
        },
    };

    match format {
        OutputFormat::Json => match info.to_json() {
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
            println!("format:      {:?}", info.format);
            println!(
                "architecture: {} ({})",
                info.architecture, info.architecture_raw
            );
            println!("endian:      {}", info.endian);
            println!("entry:       {:#x}", info.entry);
            println!("sections:    {}", info.sections.len());
            println!("symbols:     {}", info.symbols.len());
            println!("relocations: {}", info.relocations.len());
            println!("imports:     {}", info.imports.len());
            println!("exports:     {}", info.exports.len());
            ExitCode::SUCCESS
        }
    }
}

/// Parse a `--base` value, accepting decimal or `0x`-prefixed hex.
#[cfg(feature = "capstone")]
fn parse_base(s: &str) -> Result<u64, String> {
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
fn do_decode_inspect(path: &Path, base: u64, format: OutputFormat) -> ExitCode {
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
fn do_cfg_inspect(path: &Path, base: u64, format: OutputFormat) -> ExitCode {
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

/// Decode a raw binary blob, lower it, and run the System V AMD64 ABI
/// analysis over the (single) function body.
#[cfg(feature = "capstone")]
fn do_abi_inspect(path: &Path, base: u64, format: OutputFormat) -> ExitCode {
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

    // Lower every decoded instruction; Unsupported ones are dropped (explicit
    // "not modelled" signal) before the ABI walk.
    let lowered: Vec<semasm_x86::lower::LoweredInstr> = instrs
        .iter()
        .filter_map(|p| match semasm_x86::lower::lower(p) {
            semasm_x86::lower::Lowering::Lowered(l) => Some(l),
            semasm_x86::lower::Lowering::Unsupported { .. } => None,
        })
        .collect();

    let report = semasm_x86::abi::analyze(&lowered);

    match format {
        OutputFormat::Json => {
            match serde_json::to_string_pretty(&json_abi_report(&report, lowered.len())) {
                Ok(s) => {
                    println!("{s}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("failed to serialize JSON: {e}");
                    ExitCode::from(1)
                }
            }
        }
        OutputFormat::Terminal => {
            println!("instructions lowered: {}", lowered.len());
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
            if report.findings.is_empty() {
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
            if report.is_clean() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
    }
}

/// A JSON-friendly view of [`semasm_x86::abi::AbiReport`].
#[cfg(feature = "capstone")]
#[derive(serde::Serialize)]
struct JsonAbiReport {
    instructions_lowered: usize,
    is_leaf: bool,
    has_syscall: bool,
    final_rsp_delta: i64,
    call_site_count: usize,
    max_red_zone_disp: i64,
    clean: bool,
    findings: Vec<JsonAbiFinding>,
}

#[cfg(feature = "capstone")]
#[derive(serde::Serialize)]
struct JsonAbiFinding {
    code: String,
    severity: String,
    at: Option<usize>,
    message: String,
}

#[cfg(feature = "capstone")]
fn json_abi_report(r: &semasm_x86::abi::AbiReport, lowered_count: usize) -> JsonAbiReport {
    JsonAbiReport {
        instructions_lowered: lowered_count,
        is_leaf: r.is_leaf,
        has_syscall: r.has_syscall,
        final_rsp_delta: r.final_rsp_delta,
        call_site_count: r.call_sites.len(),
        max_red_zone_disp: if r.max_red_zone_disp < 0 {
            r.max_red_zone_disp
        } else {
            0
        },
        clean: r.is_clean(),
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

/// Decode a raw binary blob, lower it, build a control-flow graph, and run
/// the forward data-flow analysis (ANALYSIS-001).
#[cfg(feature = "capstone")]
fn do_analyze_inspect(path: &Path, base: u64, format: OutputFormat) -> ExitCode {
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

/// Print the analysis report in human-readable form.
#[cfg(feature = "capstone")]
fn print_analysis_terminal(r: &semasm_x86::analysis::AnalysisReport) {
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
#[cfg(feature = "capstone")]
#[derive(serde::Serialize)]
struct JsonAnalysisReport {
    iterations: usize,
    converged: bool,
    block_count: usize,
    mem_access_count: usize,
    notes: Vec<JsonAnalysisNote>,
}

#[cfg(feature = "capstone")]
#[derive(serde::Serialize)]
struct JsonAnalysisNote {
    code: String,
    block: usize,
    message: String,
}

#[cfg(feature = "capstone")]
fn json_analysis_report(r: &semasm_x86::analysis::AnalysisReport) -> JsonAnalysisReport {
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

/// Decode a raw binary blob, lower it, and run the Microsoft x64 ABI
/// analysis over the (single) function body.
#[cfg(feature = "capstone")]
fn do_win64_abi_inspect(path: &Path, base: u64, format: OutputFormat) -> ExitCode {
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

    // Lower every decoded instruction; Unsupported ones are dropped (explicit
    // "not modelled" signal) before the ABI walk.
    let lowered: Vec<semasm_x86::lower::LoweredInstr> = instrs
        .iter()
        .filter_map(|p| match semasm_x86::lower::lower(p) {
            semasm_x86::lower::Lowering::Lowered(l) => Some(l),
            semasm_x86::lower::Lowering::Unsupported { .. } => None,
        })
        .collect();

    let report = semasm_x86::abi_win64::analyze(&lowered);

    match format {
        OutputFormat::Json => {
            match serde_json::to_string_pretty(&json_win64_abi_report(&report, lowered.len())) {
                Ok(s) => {
                    println!("{s}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("failed to serialize JSON: {e}");
                    ExitCode::from(1)
                }
            }
        }
        OutputFormat::Terminal => {
            println!("instructions lowered: {}", lowered.len());
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
            if report.findings.is_empty() {
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
            if report.is_clean() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
    }
}

/// A JSON-friendly view of [`semasm_x86::abi_win64::AbiReport`].
#[cfg(feature = "capstone")]
#[derive(serde::Serialize)]
struct JsonWin64AbiReport {
    instructions_lowered: usize,
    is_leaf: bool,
    final_rsp_delta: i64,
    call_site_count: usize,
    max_below_rsp_disp: i64,
    clean: bool,
    findings: Vec<JsonWin64AbiFinding>,
}

#[cfg(feature = "capstone")]
#[derive(serde::Serialize)]
struct JsonWin64AbiFinding {
    code: String,
    severity: String,
    at: Option<usize>,
    message: String,
}

#[cfg(feature = "capstone")]
fn json_win64_abi_report(
    r: &semasm_x86::abi_win64::AbiReport,
    lowered_count: usize,
) -> JsonWin64AbiReport {
    JsonWin64AbiReport {
        instructions_lowered: lowered_count,
        is_leaf: r.is_leaf,
        final_rsp_delta: r.final_rsp_delta,
        call_site_count: r.call_sites.len(),
        max_below_rsp_disp: if r.max_red_zone_disp < 0 {
            r.max_red_zone_disp
        } else {
            0
        },
        clean: r.is_clean(),
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

/// Decode a raw binary blob, lower it, and run the AAPCS64 ABI
/// analysis over the (single) function body.
#[cfg(feature = "capstone")]
fn do_aarch64_abi_inspect(path: &Path, base: u64, format: OutputFormat) -> ExitCode {
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

    // Lower every decoded instruction; Unsupported ones are dropped before
    // the ABI walk.
    let lowered: Vec<semasm_aarch64::lower::LoweredInstr> = instrs
        .iter()
        .filter_map(|p| match semasm_aarch64::lower::lower(p) {
            semasm_aarch64::lower::Lowering::Lowered(l) => Some(l),
            semasm_aarch64::lower::Lowering::Unsupported { .. } => None,
        })
        .collect();

    let report = semasm_aarch64::abi::analyze(&lowered);

    match format {
        OutputFormat::Json => {
            match serde_json::to_string_pretty(&json_aarch64_abi_report(&report, lowered.len())) {
                Ok(s) => {
                    println!("{s}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("failed to serialize JSON: {e}");
                    ExitCode::from(1)
                }
            }
        }
        OutputFormat::Terminal => {
            println!("instructions lowered: {}", lowered.len());
            println!("leaf function:       {}", report.is_leaf);
            println!("final SP delta:      {}", report.final_sp_delta);
            println!("call sites:          {}", report.call_sites.len());
            println!();
            if report.findings.is_empty() {
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
            if report.is_clean() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
    }
}

/// A JSON-friendly view of [`semasm_aarch64::abi::AbiReport`].
#[cfg(feature = "capstone")]
#[derive(serde::Serialize)]
struct JsonAarch64AbiReport {
    instructions_lowered: usize,
    is_leaf: bool,
    final_sp_delta: i64,
    call_site_count: usize,
    clean: bool,
    findings: Vec<JsonAarch64AbiFinding>,
}

#[cfg(feature = "capstone")]
#[derive(serde::Serialize)]
struct JsonAarch64AbiFinding {
    code: String,
    severity: String,
    at: Option<usize>,
    message: String,
}

#[cfg(feature = "capstone")]
fn json_aarch64_abi_report(
    r: &semasm_aarch64::abi::AbiReport,
    lowered_count: usize,
) -> JsonAarch64AbiReport {
    JsonAarch64AbiReport {
        instructions_lowered: lowered_count,
        is_leaf: r.is_leaf,
        final_sp_delta: r.final_sp_delta,
        call_site_count: r.call_sites.len(),
        clean: r.is_clean(),
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
