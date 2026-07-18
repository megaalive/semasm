//! SemASM command-line interface.
//!
//! Rich build-time tooling lives here. Generated assembly programs do not link
//! this crate or any other SemASM Rust crate by default.

#![forbid(unsafe_code)]

mod commands;
#[cfg(feature = "capstone")]
mod output;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use commands::{
    do_agent_packet, do_agent_verify, do_build, do_contract_check, do_explain, do_target_doctor,
};
#[cfg(feature = "capstone")]
use output::{
    json_aarch64_abi_report, json_abi_report, json_analysis_report, json_win64_abi_report,
    print_analysis_terminal, unsupported_instruction, UnsupportedInstruction,
};
use semasm_core::SEMASM_VERSION;
#[cfg(feature = "capstone")]
use semasm_decode::{self, DecodeError};
use semasm_obj::{self, ObjectError};
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
        /// Output format: `terminal` (human) or `json` (HarnessReport).
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
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

#[cfg(feature = "capstone")]
fn lower_x86_with_evidence(
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
fn analysis_exit_code(clean: bool, complete: bool, allow_incomplete: bool) -> ExitCode {
    if clean && (complete || allow_incomplete) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// Decode a raw binary blob, lower it, and run the System V AMD64 ABI
/// analysis over the (single) function body.
#[cfg(feature = "capstone")]
fn do_abi_inspect(
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

/// Decode a raw binary blob, lower it, and run the Microsoft x64 ABI
/// analysis over the (single) function body.
#[cfg(feature = "capstone")]
fn do_win64_abi_inspect(
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
fn do_aarch64_abi_inspect(
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
