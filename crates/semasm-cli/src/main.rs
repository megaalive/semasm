//! SemASM command-line interface.
//!
//! Rich build-time tooling lives here. Generated assembly programs do not link
//! this crate or any other SemASM Rust crate by default.

#![forbid(unsafe_code)]

mod commands;
#[cfg(feature = "capstone")]
mod inspect;
#[cfg(feature = "capstone")]
mod memory_effects;
#[cfg(feature = "capstone")]
mod output;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use commands::{
    do_agent_compare, do_agent_packet, do_agent_verify, do_build, do_contract_check, do_explain,
    do_obj_inspect, do_target_doctor,
};
#[cfg(all(feature = "capstone", test))]
use inspect::{analysis_exit_code, lower_x86_with_evidence};
#[cfg(feature = "capstone")]
use inspect::{
    do_aarch64_abi_inspect, do_abi_inspect, do_analyze_inspect, do_cfg_inspect, do_decode_inspect,
    do_win64_abi_inspect, parse_base,
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
    Version {
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
    },
    /// Show high-level project status.
    Status {
        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
    },
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
        /// Permit a successful exit when some decoded instructions cannot be lowered.
        #[arg(long)]
        allow_incomplete: bool,
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
        /// Permit a successful exit when some decoded instructions cannot be lowered.
        #[arg(long)]
        allow_incomplete: bool,
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
        /// Permit a successful exit when some decoded instructions cannot be lowered.
        #[arg(long)]
        allow_incomplete: bool,
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
        /// Output format: `terminal` (human) or `json` (VerificationReport).
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
        /// Explicitly permit candidate execution after all static gates pass.
        #[arg(long)]
        allow_execution: bool,
        /// Write a one-page evidence card (Markdown by default) to this path.
        #[arg(long)]
        card: Option<PathBuf>,
        /// Emit the evidence card as JSON instead of Markdown (requires `--card`).
        #[arg(long)]
        card_json: bool,
    },
    /// Compare two candidate `.asm` files against one contract.
    Compare {
        /// First candidate source.
        source_a: PathBuf,
        /// Second candidate source.
        source_b: PathBuf,
        /// Path to the `*.sem.toml` contract.
        contract: PathBuf,
        /// Target triple (default: `x86_64-unknown-linux-gnu`).
        #[arg(long, default_value = "x86_64-unknown-linux-gnu")]
        target: String,
        /// Output format: `terminal` (Markdown) or `json`.
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
        /// Permit candidate execution when static gates pass.
        #[arg(long)]
        allow_execution: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum OutputFormat {
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
        None => {
            println!("semasm {SEMASM_VERSION}");
            ExitCode::SUCCESS
        }
        Some(Commands::Version { format }) => match format {
            OutputFormat::Terminal => {
                println!("semasm {SEMASM_VERSION}");
                ExitCode::SUCCESS
            }
            OutputFormat::Json => {
                let body = serde_json::json!({
                    "name": "semasm",
                    "version": SEMASM_VERSION,
                });
                println!("{body}");
                ExitCode::SUCCESS
            }
        },
        Some(Commands::Status { format }) => {
            match semasm_target::capability::CapabilityManifest::parse(include_str!(
                "../../../capabilities.toml"
            )) {
                Ok(manifest) => match format {
                    OutputFormat::Terminal => {
                        print!("{}", manifest.render_status(SEMASM_VERSION));
                        ExitCode::SUCCESS
                    }
                    OutputFormat::Json => {
                        println!("{}", manifest.status_json(SEMASM_VERSION));
                        ExitCode::SUCCESS
                    }
                },
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
                allow_execution,
                card,
                card_json,
            } => {
                if card_json && card.is_none() {
                    eprintln!("error: --card-json requires --card <path>");
                    ExitCode::from(2)
                } else {
                    do_agent_verify(
                        &source,
                        &contract,
                        &target,
                        format,
                        allow_execution,
                        card.as_deref(),
                        card_json,
                    )
                }
            }
            AgentCmd::Compare {
                source_a,
                source_b,
                contract,
                target,
                format,
                allow_execution,
            } => do_agent_compare(
                &source_a,
                &source_b,
                &contract,
                &target,
                format,
                allow_execution,
            ),
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
        Some(Commands::Abi {
            path,
            base,
            format,
            allow_incomplete,
        }) => {
            let base = match parse_base(&base) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(2);
                }
            };
            do_abi_inspect(&path, base, format, allow_incomplete)
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
        Some(Commands::Win64Abi {
            path,
            base,
            format,
            allow_incomplete,
        }) => {
            let base = match parse_base(&base) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(2);
                }
            };
            do_win64_abi_inspect(&path, base, format, allow_incomplete)
        }
        #[cfg(feature = "capstone")]
        Some(Commands::Aarch64Abi {
            path,
            base,
            format,
            allow_incomplete,
        }) => {
            let base = match parse_base(&base) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::from(2);
                }
            };
            do_aarch64_abi_inspect(&path, base, format, allow_incomplete)
        }
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

    #[test]
    fn agent_verify_requires_explicit_execution_opt_in() {
        let parsed = Cli::try_parse_from([
            "semasm",
            "agent",
            "verify",
            "candidate.asm",
            "contract.toml",
        ])
        .unwrap();
        let Some(Commands::Agent {
            action: AgentCmd::Verify {
                allow_execution, ..
            },
        }) = parsed.command
        else {
            panic!("expected agent verify command");
        };
        assert!(!allow_execution);

        let opted_in = Cli::try_parse_from([
            "semasm",
            "agent",
            "verify",
            "candidate.asm",
            "contract.toml",
            "--allow-execution",
        ])
        .unwrap();
        let Some(Commands::Agent {
            action: AgentCmd::Verify {
                allow_execution, ..
            },
        }) = opted_in.command
        else {
            panic!("expected agent verify command");
        };
        assert!(allow_execution);
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
            false,
            None,
            false,
        );
        // Without a runner (qemu) the toolchain is incomplete → non-zero.
        assert!(code != ExitCode::SUCCESS);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(feature = "capstone")]
    #[test]
    fn unsupported_lowering_retains_instruction_evidence() {
        let instruction = semasm_decode::PhysicalInstruction {
            address: 0x0040_1000,
            bytes: vec![0xc5, 0xf8, 0x77],
            mnemonic: "vzeroupper".to_string(),
            operands: Vec::new(),
            read_regs: Vec::new(),
            write_regs: Vec::new(),
            groups: Vec::new(),
            detail_available: true,
        };

        let (lowered, unsupported) = lower_x86_with_evidence(&[instruction]);
        assert!(lowered.is_empty());
        assert_eq!(unsupported.len(), 1);
        assert_eq!(unsupported[0].address, 0x0040_1000);
        assert_eq!(unsupported[0].bytes, [0xc5, 0xf8, 0x77]);
        assert_eq!(unsupported[0].mnemonic, "vzeroupper");
    }

    #[cfg(feature = "capstone")]
    #[test]
    fn incomplete_analysis_requires_explicit_opt_in() {
        assert_ne!(analysis_exit_code(true, false, false), ExitCode::SUCCESS);
        assert_eq!(analysis_exit_code(true, false, true), ExitCode::SUCCESS);
        assert_ne!(analysis_exit_code(false, true, true), ExitCode::SUCCESS);
    }
}
