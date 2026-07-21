//! Implementations for self-contained CLI commands.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[cfg(feature = "capstone")]
use semasm_agent::verify::Coverage;
use semasm_agent::{
    evidence::{
        compare_reports, render_compare_markdown, render_evidence_card_json,
        render_evidence_card_markdown, EvidenceCardContext,
    },
    harness,
    verify::{
        ExecutableGate, ExecutionIsolation, GateStatus, SemanticGateError, SemanticGates,
        VerificationReport, VerificationStatus,
    },
    ContextBundle, TargetToolchain, TaskPacket,
};
use semasm_build::exec;
use semasm_build::report::{self, CommandRecordJson, ExecutionInfo};
use semasm_build::{BuildError, Pipeline};
use semasm_contract::{
    check_file, explain_code, format_diagnostics_terminal, CheckReportJson, ContractCode,
};
use semasm_obj::{ContainerKind, ObjectError};
use semasm_target::{tools, TargetIdentity};
#[cfg(feature = "capstone")]
use semasm_target::{Abi, Isa, ObjectFormat};

use crate::OutputFormat;

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

pub(crate) fn do_agent_packet(
    contract_path: &Path,
    target_str: &str,
    source: Option<&Path>,
    format: OutputFormat,
) -> ExitCode {
    let identity = match TargetIdentity::parse_known(target_str) {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };

    let contract_text = match std::fs::read_to_string(contract_path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "{}: error: failed to read file: {error}",
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

    let pipeline = Pipeline::discover(&identity);
    let toolchain = to_agent_toolchain(&pipeline.toolchain);
    let existing_source = match source {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(error) => {
                eprintln!("{}: error: failed to read source: {error}", path.display());
                return ExitCode::from(1);
            }
        },
        None => None,
    };

    let context = ContextBundle::generate(
        &checked,
        &identity,
        &toolchain,
        existing_source,
        Vec::new(),
        Vec::new(),
    );
    let packet = TaskPacket::new(
        "0.1.0",
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
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("failed to serialize packet: {error}");
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
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let (year, month, day, hour, minute, second) = epoch_to_utc(now);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Decompose a Unix timestamp (seconds) into UTC calendar fields.
#[allow(clippy::cast_possible_truncation)]
fn epoch_to_utc(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    const DAY: u64 = 86_400;
    let days = secs / DAY;
    let rem = secs % DAY;
    let hour = (rem / 3600) as u32;
    let minute = ((rem % 3600) / 60) as u32;
    let second = (rem % 60) as u32;

    let mut year: u64 = 1970;
    let mut rest = days;
    loop {
        let year_length = if is_leap(year) { 366 } else { 365 };
        if rest < year_length {
            break;
        }
        rest -= year_length;
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
    let mut remaining_days = rest;
    while remaining_days >= month_lengths[(month - 1) as usize] {
        remaining_days -= month_lengths[(month - 1) as usize];
        month += 1;
    }
    let day = (remaining_days + 1) as u32;
    (year as u32, month as u32, day, hour, minute, second)
}

fn is_leap(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Assemble, link, and run an agent-written `.asm` against the
/// synthesised behavioural test vectors, then evaluate the results.
#[allow(clippy::too_many_lines)]
pub(crate) fn do_agent_verify(
    source: &Path,
    contract_path: &Path,
    target_str: &str,
    format: OutputFormat,
    allow_execution: bool,
    card: Option<&Path>,
    card_json: bool,
) -> ExitCode {
    match run_agent_verify_core(source, contract_path, target_str, allow_execution) {
        VerifyCore::Early(code) => code,
        VerifyCore::Done {
            report,
            object_bytes,
            contract_bytes,
            exit,
        } => {
            let card_opts = CardOptions {
                path: card,
                as_json: card_json,
                contract_path,
                contract_bytes: &contract_bytes,
                source_path: source,
                object_bytes,
                allow_execution,
                target: target_str,
            };
            if !emit_verification_with_card(report.as_ref(), format, &card_opts) {
                return ExitCode::from(1);
            }
            exit
        }
    }
}

pub(crate) fn do_agent_compare(
    source_a: &Path,
    source_b: &Path,
    contract_path: &Path,
    target_str: &str,
    format: OutputFormat,
    allow_execution: bool,
) -> ExitCode {
    let a = match run_agent_verify_core(source_a, contract_path, target_str, allow_execution) {
        VerifyCore::Early(code) => return code,
        VerifyCore::Done { report, exit, .. } => (report, exit),
    };
    let b = match run_agent_verify_core(source_b, contract_path, target_str, allow_execution) {
        VerifyCore::Early(code) => return code,
        VerifyCore::Done { report, exit, .. } => (report, exit),
    };

    let label_a = source_a
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("a");
    let label_b = source_b
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("b");
    let compare = compare_reports(a.0.as_ref(), b.0.as_ref(), label_a, label_b);

    match format {
        OutputFormat::Json => match serde_json::to_string_pretty(&compare) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("failed to serialize compare report: {error}");
                return ExitCode::from(1);
            }
        },
        OutputFormat::Terminal => {
            print!("{}", render_compare_markdown(&compare, label_a, label_b));
        }
    }

    // Compare always emits a report when both sides produced verification
    // evidence; exit 0 only if at least one candidate verified.
    if a.0.status == VerificationStatus::Verified || b.0.status == VerificationStatus::Verified {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

struct CardOptions<'a> {
    path: Option<&'a Path>,
    as_json: bool,
    contract_path: &'a Path,
    contract_bytes: &'a [u8],
    source_path: &'a Path,
    object_bytes: u64,
    allow_execution: bool,
    target: &'a str,
}

enum VerifyCore {
    Early(ExitCode),
    Done {
        report: Box<VerificationReport>,
        object_bytes: u64,
        contract_bytes: Vec<u8>,
        exit: ExitCode,
    },
}

fn reproduce_verify_cmd(
    source: &Path,
    contract: &Path,
    target: &str,
    allow_execution: bool,
) -> String {
    let mut cmd = format!(
        "semasm agent verify {} {}",
        source.display(),
        contract.display()
    );
    if target != "x86_64-unknown-linux-gnu" {
        cmd.push_str(" --target ");
        cmd.push_str(target);
    }
    if allow_execution {
        cmd.push_str(" --allow-execution");
    }
    cmd
}

fn emit_verification_with_card(
    report: &VerificationReport,
    format: OutputFormat,
    card: &CardOptions<'_>,
) -> bool {
    if !emit_verification_report(report, format) {
        return false;
    }
    write_evidence_card_if_requested(report, card)
}

fn write_evidence_card_if_requested(report: &VerificationReport, card: &CardOptions<'_>) -> bool {
    let Some(path) = card.path else {
        return true;
    };
    let ctx = EvidenceCardContext {
        report,
        contract_path: card.contract_path,
        contract_bytes: card.contract_bytes,
        source_path: card.source_path,
        object_bytes: card.object_bytes,
        reproduce_cmd: &reproduce_verify_cmd(
            card.source_path,
            card.contract_path,
            card.target,
            card.allow_execution,
        ),
    };
    let body = if card.as_json {
        match render_evidence_card_json(&ctx) {
            Ok(json) => json,
            Err(error) => {
                eprintln!("failed to serialize evidence card: {error}");
                return false;
            }
        }
    } else {
        render_evidence_card_markdown(&ctx)
    };
    if let Err(error) = std::fs::write(path, body) {
        eprintln!("error: cannot write evidence card {}: {error}", path.display());
        return false;
    }
    eprintln!("wrote evidence card {}", path.display());
    true
}

#[allow(clippy::too_many_lines)]
fn run_agent_verify_core(
    source: &Path,
    contract_path: &Path,
    target_str: &str,
    allow_execution: bool,
) -> VerifyCore {
    let identity = match TargetIdentity::parse_known(target_str) {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("error: {error}");
            return VerifyCore::Early(ExitCode::from(2));
        }
    };
    let pipeline = Pipeline::discover(&identity);
    if !pipeline.all_tools_found() {
        eprintln!("error: toolchain incomplete for target `{target_str}`");
        eprintln!("  run `semasm target doctor {target_str}` for details");
        eprintln!(
            "  (assembler, linker, disassembler, and a runner such as qemu-x86_64 are required)"
        );
        return VerifyCore::Early(ExitCode::from(1));
    }
    let run_isolation = ExecutionIsolation::from_runner(pipeline.toolchain.runner.as_deref());

    let contract_bytes = match std::fs::read(contract_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("{}: error: {error}", contract_path.display());
            return VerifyCore::Early(ExitCode::from(1));
        }
    };
    let contract_text = match std::str::from_utf8(&contract_bytes) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("{}: error: {error}", contract_path.display());
            return VerifyCore::Early(ExitCode::from(1));
        }
    };
    let check = semasm_contract::check_str(contract_text);
    if !check.ok() {
        print!(
            "{}",
            format_diagnostics_terminal(&contract_path.display().to_string(), &check.diagnostics)
        );
        return VerifyCore::Early(ExitCode::from(1));
    }
    let checked = check.contract.expect("ok() implies Some");

    let vectors = harness::synthesize_vectors(&checked);
    if vectors.is_empty() {
        eprintln!(
            "error: no test vectors synthesised for `{}`; \
             the routine shape is not yet supported by the harness",
            checked.name
        );
        return VerifyCore::Early(ExitCode::from(1));
    }
    let routine_symbol = checked.name.clone();

    let directory = std::env::temp_dir().join(format!(
        "semasm-verify-{}-{}",
        std::process::id(),
        source
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("candidate")
    ));
    if let Err(error) = std::fs::create_dir_all(&directory) {
        eprintln!("error: cannot create scratch dir: {error}");
        return VerifyCore::Early(ExitCode::from(1));
    }
    let routine_object = directory.join("routine.o");
    let harness_object = directory.join("harness.o");
    let executable = if identity.object_format == semasm_target::ObjectFormat::PeCoff {
        directory.join("harness.exe")
    } else {
        directory.join("harness")
    };

    match pipeline.assemble_for_target(source, &routine_object) {
        Ok(output) if output.success() => {}
        Ok(output) => {
            eprintln!(
                "assemble routine failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let _ = std::fs::remove_dir_all(&directory);
            return VerifyCore::Early(ExitCode::from(1));
        }
        Err(error) => {
            eprintln!("assemble routine error: {error}");
            let _ = std::fs::remove_dir_all(&directory);
            return VerifyCore::Early(ExitCode::from(1));
        }
    }

    let object_bytes = std::fs::metadata(&routine_object).map_or(0, |meta| meta.len());

    let semantic = match verify_candidate_semantics(&routine_object, &identity, &routine_symbol) {
        Ok(gates) => gates,
        Err(error) => {
            eprintln!("semantic gate failed: {error}");
            let report = VerificationReport::from_parts(
                identity.name.clone(),
                routine_symbol,
                SemanticGates::from_error(&error, 0),
                ExecutableGate::skipped(),
                None,
                ExecutionIsolation::StaticOnly,
            );
            let _ = std::fs::remove_dir_all(&directory);
            return VerifyCore::Done {
                report: Box::new(report),
                object_bytes,
                contract_bytes,
                exit: ExitCode::from(1),
            };
        }
    };

    let harness_source = match harness::generate_harness(&routine_symbol, &vectors, identity.abi) {
        Ok(source) => source,
        Err(reason) => {
            eprintln!("behavioral harness unavailable: {reason}");
            let report = VerificationReport::from_parts(
                identity.name.clone(),
                routine_symbol,
                semantic,
                ExecutableGate::skipped(),
                None,
                ExecutionIsolation::StaticOnly,
            );
            let _ = std::fs::remove_dir_all(&directory);
            eprintln!(
                "execution denied: static semantic gates passed; behavioral harness not available for this target"
            );
            return VerifyCore::Done {
                report: Box::new(report),
                object_bytes,
                contract_bytes,
                exit: ExitCode::from(1),
            };
        }
    };

    let harness_ext = match identity.dialect {
        semasm_target::Dialect::GasUnified | semasm_target::Dialect::GasAtt => "S",
        semasm_target::Dialect::NasmIntel => "asm",
    };
    let harness_path = directory.join(format!("harness.{harness_ext}"));
    if let Err(error) = std::fs::write(&harness_path, &harness_source) {
        eprintln!("error: cannot write harness source: {error}");
        let _ = std::fs::remove_dir_all(&directory);
        return VerifyCore::Early(ExitCode::from(1));
    }
    match pipeline.assemble_for_target(&harness_path, &harness_object) {
        Ok(output) if output.success() => {}
        Ok(output) => {
            eprintln!(
                "assemble harness failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let _ = std::fs::remove_dir_all(&directory);
            return VerifyCore::Early(ExitCode::from(1));
        }
        Err(error) => {
            eprintln!("assemble harness error: {error}");
            let _ = std::fs::remove_dir_all(&directory);
            return VerifyCore::Early(ExitCode::from(1));
        }
    }

    let entry = match identity.abi {
        semasm_target::Abi::WindowsX64 => "main",
        _ => "_start",
    };
    match pipeline.link_for_target(&[&routine_object, &harness_object], &executable, entry) {
        Ok(output) if output.success() => {}
        Ok(output) => {
            eprintln!("link failed: {}", String::from_utf8_lossy(&output.stderr));
            let _ = std::fs::remove_dir_all(&directory);
            return VerifyCore::Early(ExitCode::from(1));
        }
        Err(error) => {
            eprintln!("link error: {error}");
            let _ = std::fs::remove_dir_all(&directory);
            return VerifyCore::Early(ExitCode::from(1));
        }
    }

    let (executable_gate, executable_error) = check_executable_object(&executable, &identity);
    if executable_gate.status == GateStatus::Failed {
        if let Some(error) = executable_error {
            eprintln!("executable object gate failed: {error}");
        }
        let report = VerificationReport::from_parts(
            identity.name.clone(),
            routine_symbol,
            semantic,
            executable_gate,
            None,
            ExecutionIsolation::StaticOnly,
        );
        let _ = std::fs::remove_dir_all(&directory);
        return VerifyCore::Done {
            report: Box::new(report),
            object_bytes,
            contract_bytes,
            exit: ExitCode::from(1),
        };
    }

    if !allow_execution {
        let report = VerificationReport::from_parts(
            identity.name.clone(),
            routine_symbol,
            semantic,
            executable_gate,
            None,
            ExecutionIsolation::StaticOnly,
        );
        let _ = std::fs::remove_dir_all(&directory);
        eprintln!(
            "execution denied: static semantic gates passed; rerun with --allow-execution to run the candidate"
        );
        return VerifyCore::Done {
            report: Box::new(report),
            object_bytes,
            contract_bytes,
            exit: ExitCode::from(1),
        };
    }

    let run = match pipeline.run(&executable) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("run error: {error}");
            let _ = std::fs::remove_dir_all(&directory);
            return VerifyCore::Early(ExitCode::from(1));
        }
    };
    let behavior = harness::evaluate(&run.stdout, &vectors);
    let _ = std::fs::remove_dir_all(&directory);
    let report = VerificationReport::from_parts(
        identity.name.clone(),
        routine_symbol,
        semantic,
        executable_gate,
        Some(behavior),
        run_isolation,
    );

    let exit = match report.status {
        VerificationStatus::Verified => ExitCode::SUCCESS,
        _ => ExitCode::from(1),
    };
    VerifyCore::Done {
        report: Box::new(report),
        object_bytes,
        contract_bytes,
        exit,
    }
}

fn emit_verification_report(report: &VerificationReport, format: OutputFormat) -> bool {
    match format {
        OutputFormat::Json => match serde_json::to_string_pretty(report) {
            Ok(json) => {
                println!("{json}");
                true
            }
            Err(error) => {
                eprintln!("failed to serialize report: {error}");
                false
            }
        },
        OutputFormat::Terminal => {
            print_verification_terminal(report);
            true
        }
    }
}

fn print_verification_terminal(report: &VerificationReport) {
    let semantic = &report.semantic;
    println!("Status: {}", report.status.as_str());
    println!("Target: {}", report.target);
    println!("Routine: {}", report.routine_symbol);
    println!("Isolation: {}", report.isolation.as_str());
    println!(
        "Semantic gates: object={} decode={}/{} lowering={}/{} ({}%) abi={} capability={} control={}",
        semantic.object_policy.as_str(),
        semantic.decode.modeled,
        semantic.decode.total,
        semantic.lowering.modeled,
        semantic.lowering.total,
        semantic.lowering.percent_modeled(),
        semantic.abi.as_str(),
        semantic.capability.as_str(),
        semantic.control.as_str(),
    );
    println!("Executable gate: {}", report.executable.status.as_str());

    match &report.behavior {
        Some(behavior) => {
            println!("Vectors: {}", behavior.cases.len());
            println!();
            for (index, case) in behavior.cases.iter().enumerate() {
                let status = if case.passed { "PASS" } else { "FAIL" };
                println!(
                    "{index}. [{status}] {}  expected={} observed={}",
                    case.name, case.expected, case.observed
                );
            }
            println!(
                "\nResult: {}",
                if behavior.all_passed {
                    "all vectors passed"
                } else {
                    "one or more vectors failed"
                }
            );
        }
        None => {
            println!("Behavior: skipped (execution not allowed)");
        }
    }
}

fn check_executable_object(
    executable: &Path,
    identity: &TargetIdentity,
) -> (ExecutableGate, Option<String>) {
    match semasm_obj::read_for_target(executable, identity) {
        Ok(info) if info.kind == ContainerKind::Executable => (ExecutableGate::passed(), None),
        Ok(info) => (
            ExecutableGate::failed(),
            Some(format!("produced {:?} container", info.kind)),
        ),
        Err(error) => (ExecutableGate::failed(), Some(error.to_string())),
    }
}

#[cfg(feature = "capstone")]
fn verify_candidate_semantics(
    object_path: &Path,
    identity: &TargetIdentity,
    routine_symbol: &str,
) -> Result<SemanticGates, SemanticGateError> {
    require_semantic_target(identity)?;
    check_candidate_object_policy(object_path, identity, routine_symbol)?;

    match (identity.isa, identity.abi, identity.object_format) {
        (Isa::X86_64, Abi::SysVAmd64, ObjectFormat::Elf)
        | (Isa::X86_64, Abi::WindowsX64, ObjectFormat::PeCoff) => {
            let (physical, code_bytes) =
                decode_candidate_code(object_path, identity, DecodeIsa::X86_64)?;
            let decode_coverage = Coverage::complete(physical.len());
            let lowered = lower_x86_instructions(&physical, decode_coverage)?;
            let lowering_coverage = Coverage::complete(lowered.len());
            check_x86_abi_capability(&lowered, identity.abi, decode_coverage, lowering_coverage)?;
            check_x86_cfg_leaf(&physical, decode_coverage, lowering_coverage)?;
            Ok(SemanticGates {
                object_policy: GateStatus::Passed,
                executable_bytes: code_bytes,
                decode: decode_coverage,
                lowering: lowering_coverage,
                abi: GateStatus::Passed,
                capability: GateStatus::Passed,
                control: GateStatus::Passed,
            })
        }
        (Isa::AArch64, Abi::Aapcs64, ObjectFormat::Elf) => {
            let (physical, code_bytes) =
                decode_candidate_code(object_path, identity, DecodeIsa::AArch64)?;
            let decode_coverage = Coverage::complete(physical.len());
            let lowered = lower_aarch64_instructions(&physical, decode_coverage)?;
            let lowering_coverage = Coverage::complete(lowered.len());
            check_aarch64_abi_capability(&lowered, decode_coverage, lowering_coverage)?;
            Ok(SemanticGates {
                object_policy: GateStatus::Passed,
                executable_bytes: code_bytes,
                decode: decode_coverage,
                lowering: lowering_coverage,
                abi: GateStatus::Passed,
                capability: GateStatus::Passed,
                // CFG leaf policy is x86-only in this slice.
                control: GateStatus::Passed,
            })
        }
        (Isa::Riscv64, Abi::Riscv, ObjectFormat::Elf) => {
            let (physical, code_bytes) =
                decode_candidate_code(object_path, identity, DecodeIsa::Riscv64)?;
            let decode_coverage = Coverage::complete(physical.len());
            let lowered = lower_riscv_instructions(&physical, decode_coverage)?;
            let lowering_coverage = Coverage::complete(lowered.len());
            check_riscv_abi_capability(&lowered, decode_coverage, lowering_coverage)?;
            Ok(SemanticGates {
                object_policy: GateStatus::Passed,
                executable_bytes: code_bytes,
                decode: decode_coverage,
                lowering: lowering_coverage,
                abi: GateStatus::Passed,
                capability: GateStatus::Passed,
                // CFG leaf policy is x86-only in this slice.
                control: GateStatus::Passed,
            })
        }
        _ => Err(SemanticGateError::new(
            "target",
            format!(
                "agent verification has no semantic-gate dispatch for `{}`",
                identity.name
            ),
        )),
    }
}

#[cfg(feature = "capstone")]
fn require_semantic_target(identity: &TargetIdentity) -> Result<(), SemanticGateError> {
    let supported = matches!(
        (identity.isa, identity.abi, identity.object_format),
        (Isa::X86_64, Abi::SysVAmd64, ObjectFormat::Elf)
            | (Isa::X86_64, Abi::WindowsX64, ObjectFormat::PeCoff)
            | (Isa::AArch64, Abi::Aapcs64, ObjectFormat::Elf)
            | (Isa::Riscv64, Abi::Riscv, ObjectFormat::Elf)
    );
    if !supported {
        return Err(SemanticGateError::new(
            "target",
            format!(
                "agent verification currently has complete semantic gates for \
                 x86_64 SysV ELF, x86_64 Win64 PE, AArch64 Linux ELF, and RV64 Linux ELF, not `{}`",
                identity.name
            ),
        ));
    }
    Ok(())
}

#[cfg(feature = "capstone")]
fn check_candidate_object_policy(
    object_path: &Path,
    identity: &TargetIdentity,
    routine_symbol: &str,
) -> Result<(), SemanticGateError> {
    let info = semasm_obj::read_for_target(object_path, identity)
        .map_err(|error| SemanticGateError::new("object", error.to_string()))?;
    if info.kind != ContainerKind::Relocatable {
        return Err(SemanticGateError::new(
            "object",
            format!("candidate must be relocatable, found {:?}", info.kind),
        ));
    }
    if !info.exports.iter().any(|symbol| symbol == routine_symbol) {
        return Err(SemanticGateError::new(
            "object",
            format!("required routine symbol `{routine_symbol}` is not exported"),
        ));
    }
    if !info.imports.is_empty() {
        return Err(SemanticGateError::new(
            "object",
            format!(
                "candidate has forbidden external capabilities/imports: {}",
                info.imports.join(", ")
            ),
        ));
    }
    let wx: Vec<&str> = info
        .sections
        .iter()
        .filter(|section| section.writable && section.executable)
        .map(|section| section.name.as_str())
        .collect();
    if !wx.is_empty() {
        return Err(SemanticGateError::new(
            "object",
            format!(
                "candidate has forbidden writable+executable section(s): {}",
                wx.join(", ")
            ),
        ));
    }
    Ok(())
}

#[cfg(feature = "capstone")]
#[derive(Clone, Copy)]
enum DecodeIsa {
    X86_64,
    AArch64,
    Riscv64,
}

#[cfg(feature = "capstone")]
fn decode_candidate_code(
    object_path: &Path,
    identity: &TargetIdentity,
    isa: DecodeIsa,
) -> Result<(Vec<semasm_decode::PhysicalInstruction>, usize), SemanticGateError> {
    let sections = semasm_obj::read_code_sections(object_path, identity)
        .map_err(|error| SemanticGateError::new("object", error.to_string()))?;
    if sections.is_empty() {
        return Err(SemanticGateError::new(
            "object",
            "candidate contains no executable code section",
        ));
    }

    let mut physical = Vec::new();
    let mut code_bytes = 0usize;
    for section in sections {
        code_bytes += section.bytes.len();
        let mut decoded = match isa {
            DecodeIsa::X86_64 => semasm_decode::decode_x86_64(&section.bytes, section.address),
            DecodeIsa::AArch64 => semasm_decode::decode_aarch64(&section.bytes, section.address),
            DecodeIsa::Riscv64 => semasm_decode::decode_riscv64(&section.bytes, section.address),
        }
        .map_err(|error| {
            SemanticGateError::new(
                "decode",
                format!("decode failed for {}: {error}", section.name),
            )
        })?;
        physical.append(&mut decoded);
    }
    let decoded_bytes = physical
        .iter()
        .map(|instruction| instruction.bytes.len())
        .sum::<usize>();
    if decoded_bytes != code_bytes {
        return Err(SemanticGateError {
            stage: "decode",
            message: format!(
                "decode coverage incomplete: decoded {decoded_bytes} of {code_bytes} executable bytes"
            ),
            decode: None,
            lowering: None,
        });
    }
    Ok((physical, code_bytes))
}

#[cfg(feature = "capstone")]
fn lower_x86_instructions(
    physical: &[semasm_decode::PhysicalInstruction],
    decode_coverage: Coverage,
) -> Result<Vec<semasm_x86::lower::LoweredInstr>, SemanticGateError> {
    let mut lowered = Vec::with_capacity(physical.len());
    for instruction in physical {
        match semasm_x86::lower::lower(instruction) {
            semasm_x86::lower::Lowering::Lowered(instruction) => lowered.push(instruction),
            semasm_x86::lower::Lowering::Unsupported { mnemonic } => {
                let modeled = lowered.len();
                let total = physical.len();
                return Err(SemanticGateError {
                    stage: "lowering",
                    message: format!(
                        "lowering coverage incomplete at {:#x}: unsupported `{mnemonic}`",
                        instruction.address
                    ),
                    decode: Some(decode_coverage),
                    lowering: Some(Coverage {
                        total,
                        modeled,
                        unknown: total - modeled,
                    }),
                });
            }
        }
    }
    Ok(lowered)
}

#[cfg(feature = "capstone")]
fn lower_aarch64_instructions(
    physical: &[semasm_decode::PhysicalInstruction],
    decode_coverage: Coverage,
) -> Result<Vec<semasm_aarch64::lower::LoweredInstr>, SemanticGateError> {
    let mut lowered = Vec::with_capacity(physical.len());
    for instruction in physical {
        match semasm_aarch64::lower::lower(instruction) {
            semasm_aarch64::lower::Lowering::Lowered(instruction) => lowered.push(instruction),
            semasm_aarch64::lower::Lowering::Unsupported { mnemonic } => {
                let modeled = lowered.len();
                let total = physical.len();
                return Err(SemanticGateError {
                    stage: "lowering",
                    message: format!(
                        "lowering coverage incomplete at {:#x}: unsupported `{mnemonic}`",
                        instruction.address
                    ),
                    decode: Some(decode_coverage),
                    lowering: Some(Coverage {
                        total,
                        modeled,
                        unknown: total - modeled,
                    }),
                });
            }
        }
    }
    Ok(lowered)
}

#[cfg(feature = "capstone")]
fn lower_riscv_instructions(
    physical: &[semasm_decode::PhysicalInstruction],
    decode_coverage: Coverage,
) -> Result<Vec<semasm_riscv::lower::LoweredInstr>, SemanticGateError> {
    let mut lowered = Vec::with_capacity(physical.len());
    for instruction in physical {
        match semasm_riscv::lower::lower(instruction) {
            semasm_riscv::lower::Lowering::Lowered(instruction) => lowered.push(instruction),
            semasm_riscv::lower::Lowering::Unsupported { mnemonic } => {
                let modeled = lowered.len();
                let total = physical.len();
                return Err(SemanticGateError {
                    stage: "lowering",
                    message: format!(
                        "lowering coverage incomplete at {:#x}: unsupported `{mnemonic}`",
                        instruction.address
                    ),
                    decode: Some(decode_coverage),
                    lowering: Some(Coverage {
                        total,
                        modeled,
                        unknown: total - modeled,
                    }),
                });
            }
        }
    }
    Ok(lowered)
}

#[cfg(feature = "capstone")]
fn check_x86_abi_capability(
    lowered: &[semasm_x86::lower::LoweredInstr],
    abi: Abi,
    decode_coverage: Coverage,
    lowering_coverage: Coverage,
) -> Result<(), SemanticGateError> {
    match abi {
        Abi::SysVAmd64 => {
            let report = semasm_x86::abi::analyze(lowered);
            if !report.is_clean() {
                let findings = report
                    .findings
                    .iter()
                    .map(|finding| format!("{}: {}", finding.code, finding.message))
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(SemanticGateError {
                    stage: "abi",
                    message: format!("System V ABI verification failed: {findings}"),
                    decode: Some(decode_coverage),
                    lowering: Some(lowering_coverage),
                });
            }
            if report.has_syscall {
                return Err(SemanticGateError {
                    stage: "capability",
                    message: "candidate requests the forbidden syscall capability".into(),
                    decode: Some(decode_coverage),
                    lowering: Some(lowering_coverage),
                });
            }
        }
        Abi::WindowsX64 => {
            let report = semasm_x86::abi_win64::analyze(lowered);
            if !report.is_clean() {
                let findings = report
                    .findings
                    .iter()
                    .map(|finding| format!("{}: {}", finding.code, finding.message))
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(SemanticGateError {
                    stage: "abi",
                    message: format!("Microsoft x64 ABI verification failed: {findings}"),
                    decode: Some(decode_coverage),
                    lowering: Some(lowering_coverage),
                });
            }
            if lowered.iter().any(|ins| ins.mnemonic == "syscall") {
                return Err(SemanticGateError {
                    stage: "capability",
                    message: "candidate requests the forbidden syscall capability".into(),
                    decode: Some(decode_coverage),
                    lowering: Some(lowering_coverage),
                });
            }
        }
        _ => {
            return Err(SemanticGateError::new(
                "abi",
                format!("unexpected x86 ABI `{abi}`"),
            ));
        }
    }
    Ok(())
}

#[cfg(feature = "capstone")]
fn check_x86_cfg_leaf(
    physical: &[semasm_decode::PhysicalInstruction],
    decode_coverage: Coverage,
    lowering_coverage: Coverage,
) -> Result<(), SemanticGateError> {
    use semasm_cfg::BlockEnd;

    for instruction in physical {
        match semasm_cfg::classify_instruction(instruction) {
            BlockEnd::Indirect => {
                return Err(SemanticGateError {
                    stage: "cfg",
                    message: format!(
                        "leaf control-flow policy rejected indirect branch at {:#x}",
                        instruction.address
                    ),
                    decode: Some(decode_coverage),
                    lowering: Some(lowering_coverage),
                });
            }
            BlockEnd::Unknown => {
                return Err(SemanticGateError {
                    stage: "cfg",
                    message: format!(
                        "leaf control-flow policy rejected unknown terminator at {:#x}",
                        instruction.address
                    ),
                    decode: Some(decode_coverage),
                    lowering: Some(lowering_coverage),
                });
            }
            BlockEnd::Call { address: None, .. } => {
                return Err(SemanticGateError {
                    stage: "cfg",
                    message: format!(
                        "leaf control-flow policy rejected indirect call at {:#x}",
                        instruction.address
                    ),
                    decode: Some(decode_coverage),
                    lowering: Some(lowering_coverage),
                });
            }
            _ => {}
        }
    }

    let cfg = semasm_cfg::build(physical).map_err(|error| SemanticGateError {
        stage: "cfg",
        message: format!("CFG build failed: {error}"),
        decode: Some(decode_coverage),
        lowering: Some(lowering_coverage),
    })?;

    for block in &cfg.blocks {
        match &block.end {
            BlockEnd::UnconditionalBranch {
                target: None,
                address,
            } => {
                return Err(SemanticGateError {
                    stage: "cfg",
                    message: format!(
                        "leaf control-flow policy rejected incomplete jump from {:#x} to {:#x}",
                        block.end_address, address
                    ),
                    decode: Some(decode_coverage),
                    lowering: Some(lowering_coverage),
                });
            }
            BlockEnd::ConditionalBranch {
                taken: None,
                taken_address,
                ..
            } => {
                return Err(SemanticGateError {
                    stage: "cfg",
                    message: format!(
                        "leaf control-flow policy rejected incomplete conditional from {:#x} to {:#x}",
                        block.end_address, taken_address
                    ),
                    decode: Some(decode_coverage),
                    lowering: Some(lowering_coverage),
                });
            }
            BlockEnd::Indirect | BlockEnd::Unknown => {
                return Err(SemanticGateError {
                    stage: "cfg",
                    message: format!(
                        "leaf control-flow policy rejected non-direct terminator at {:#x}",
                        block.end_address
                    ),
                    decode: Some(decode_coverage),
                    lowering: Some(lowering_coverage),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(feature = "capstone")]
fn check_aarch64_abi_capability(
    lowered: &[semasm_aarch64::lower::LoweredInstr],
    decode_coverage: Coverage,
    lowering_coverage: Coverage,
) -> Result<(), SemanticGateError> {
    let report = semasm_aarch64::abi::analyze(lowered);
    if !report.is_clean() {
        let findings = report
            .findings
            .iter()
            .map(|finding| format!("{}: {}", finding.code, finding.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(SemanticGateError {
            stage: "abi",
            message: format!("AAPCS64 ABI verification failed: {findings}"),
            decode: Some(decode_coverage),
            lowering: Some(lowering_coverage),
        });
    }
    if lowered.iter().any(|ins| ins.mnemonic == "svc") {
        return Err(SemanticGateError {
            stage: "capability",
            message: "candidate requests the forbidden svc capability".into(),
            decode: Some(decode_coverage),
            lowering: Some(lowering_coverage),
        });
    }
    Ok(())
}

#[cfg(feature = "capstone")]
fn check_riscv_abi_capability(
    lowered: &[semasm_riscv::lower::LoweredInstr],
    decode_coverage: Coverage,
    lowering_coverage: Coverage,
) -> Result<(), SemanticGateError> {
    let report = semasm_riscv::abi::analyze(lowered);
    if !report.is_clean() {
        let findings = report
            .findings
            .iter()
            .map(|finding| format!("{}: {}", finding.code, finding.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(SemanticGateError {
            stage: "abi",
            message: format!("RISC-V LP64 ABI verification failed: {findings}"),
            decode: Some(decode_coverage),
            lowering: Some(lowering_coverage),
        });
    }
    if lowered.iter().any(|ins| ins.mnemonic == "ecall") {
        return Err(SemanticGateError {
            stage: "capability",
            message: "candidate requests the forbidden ecall capability".into(),
            decode: Some(decode_coverage),
            lowering: Some(lowering_coverage),
        });
    }
    Ok(())
}

#[cfg(not(feature = "capstone"))]
fn verify_candidate_semantics(
    _object_path: &Path,
    _identity: &TargetIdentity,
    _routine_symbol: &str,
) -> Result<SemanticGates, SemanticGateError> {
    Err(SemanticGateError::new(
        "decode",
        "agent verification requires a build with the `capstone` feature",
    ))
}

#[cfg(all(test, feature = "capstone"))]
mod semantic_gate_tests {
    use super::*;
    use std::process::Command;

    #[test]
    #[ignore = "requires nasm on PATH"]
    fn canonical_candidate_passes_static_semantic_gates() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source = workspace.join("fixtures/asm/count_byte.asm");
        let scratch =
            std::env::temp_dir().join(format!("semasm-semantic-gate-{}", std::process::id()));
        std::fs::create_dir_all(&scratch).unwrap();
        let object = scratch.join("count_byte.o");
        let output = Command::new("nasm")
            .args(["-f", "elf64"])
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "nasm failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let target = TargetIdentity::parse_known("x86_64-unknown-linux-gnu").unwrap();
        let gates = verify_candidate_semantics(&object, &target, "count_byte").unwrap();
        assert!(gates.all_passed());
        assert_eq!(gates.lowering.unknown, 0);
        assert_eq!(gates.lowering.modeled, gates.lowering.total);
        assert_eq!(gates.abi, GateStatus::Passed);
        assert_eq!(gates.capability, GateStatus::Passed);
        assert_eq!(gates.control, GateStatus::Passed);
        let _ = std::fs::remove_dir_all(scratch);
    }

    #[test]
    #[ignore = "requires nasm on PATH"]
    fn win64_candidate_passes_static_semantic_gates() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source = workspace.join("fixtures/asm/count_byte_win64.asm");
        let scratch =
            std::env::temp_dir().join(format!("semasm-semantic-win64-{}", std::process::id()));
        std::fs::create_dir_all(&scratch).unwrap();
        let object = scratch.join("count_byte.obj");
        let output = Command::new("nasm")
            .args(["-f", "win64"])
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "nasm failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let target = TargetIdentity::parse_known("x86_64-pc-windows-msvc").unwrap();
        let gates = verify_candidate_semantics(&object, &target, "count_byte").unwrap();
        assert!(gates.all_passed());
        assert_eq!(gates.lowering.unknown, 0);
        assert_eq!(gates.abi, GateStatus::Passed);
        assert_eq!(gates.capability, GateStatus::Passed);
        assert_eq!(gates.control, GateStatus::Passed);
        let _ = std::fs::remove_dir_all(scratch);
    }

    #[test]
    #[ignore = "requires aarch64-linux-gnu-as on PATH"]
    fn aarch64_candidate_passes_static_semantic_gates() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source = workspace.join("fixtures/asm/count_byte_aarch64.S");
        let scratch =
            std::env::temp_dir().join(format!("semasm-semantic-a64-{}", std::process::id()));
        std::fs::create_dir_all(&scratch).unwrap();
        let object = scratch.join("count_byte.o");
        let output = match Command::new("aarch64-linux-gnu-as")
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .output()
        {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("skipping: aarch64-linux-gnu-as not on PATH");
                let _ = std::fs::remove_dir_all(&scratch);
                return;
            }
            Err(error) => panic!("failed to spawn aarch64-linux-gnu-as: {error}"),
        };
        assert!(
            output.status.success(),
            "as failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let target = TargetIdentity::parse_known("aarch64-unknown-linux-gnu").unwrap();
        let gates = verify_candidate_semantics(&object, &target, "count_byte").unwrap();
        assert!(gates.all_passed());
        assert_eq!(gates.abi, GateStatus::Passed);
        assert_eq!(gates.capability, GateStatus::Passed);
        let _ = std::fs::remove_dir_all(scratch);
    }

    #[test]
    #[ignore = "requires riscv64-linux-gnu-as on PATH"]
    fn riscv64_candidate_passes_static_semantic_gates() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source = workspace.join("fixtures/asm/count_byte_riscv64.S");
        let scratch =
            std::env::temp_dir().join(format!("semasm-semantic-rv64-{}", std::process::id()));
        std::fs::create_dir_all(&scratch).unwrap();
        let object = scratch.join("count_byte.o");
        let output = match Command::new("riscv64-linux-gnu-as")
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .output()
        {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("skipping: riscv64-linux-gnu-as not on PATH");
                let _ = std::fs::remove_dir_all(&scratch);
                return;
            }
            Err(error) => panic!("failed to spawn riscv64-linux-gnu-as: {error}"),
        };
        assert!(
            output.status.success(),
            "as failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let target = TargetIdentity::parse_known("riscv64gc-unknown-linux-gnu").unwrap();
        let gates = verify_candidate_semantics(&object, &target, "count_byte").unwrap();
        assert!(gates.all_passed());
        assert_eq!(gates.abi, GateStatus::Passed);
        assert_eq!(gates.capability, GateStatus::Passed);
        let _ = std::fs::remove_dir_all(scratch);
    }
}

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
pub(crate) fn do_build(
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

    let assemble_spec = pipe.assemble_for_target_spec(source, &obj_path);
    let ao = match exec::exec(&assemble_spec) {
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
    let entry = if identity.object_format == semasm_target::ObjectFormat::PeCoff {
        link_entry_of(&obj_path).unwrap_or_else(|| "main".to_string())
    } else if matches!(
        identity.dialect,
        semasm_target::Dialect::GasUnified | semasm_target::Dialect::GasAtt
    ) {
        link_entry_of(&obj_path).unwrap_or_else(|| "_start".to_string())
    } else {
        "_start".to_string()
    };
    let link_spec = pipe.link_for_target_spec(&[&obj_path], &exe_path, &entry);
    let lo = match exec::exec(&link_spec) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("link error: {error}");
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

    // Step 4: run unless explicitly disabled. Preserve failures in the report
    // and return a non-zero status after emitting the available evidence.
    let (execution, execution_failed) = if no_run {
        (ExecutionInfo::NotRequested, false)
    } else {
        match pipe.run(&exe_path) {
            Ok(output) => (ExecutionInfo::succeeded(&output), false),
            Err(BuildError::ProgramNotFound(reason)) => {
                eprintln!("execution unavailable: {reason}");
                (ExecutionInfo::unavailable(reason), true)
            }
            Err(error) => {
                let error = error.to_string();
                eprintln!("execution failed: {error}");
                (ExecutionInfo::failed(error), true)
            }
        }
    };

    // Step 5: generate report
    let records = vec![
        CommandRecordJson {
            label: "assemble".into(),
            command: assemble_spec.to_string(),
            program: assemble_spec.program.clone(),
            arguments: assemble_spec.args.clone(),
            exit_code: ao.exit_code,
            stdout: String::from_utf8_lossy(&ao.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&ao.stderr).into_owned(),
            duration_secs: ao.duration.as_secs_f64(),
            timed_out: ao.timed_out,
            success: ao.success(),
            stdout_capture: ao.stdout_capture.clone(),
            stderr_capture: ao.stderr_capture.clone(),
            termination: ao.termination.clone(),
        },
        CommandRecordJson {
            label: "link".into(),
            command: link_spec.to_string(),
            program: link_spec.program.clone(),
            arguments: link_spec.args.clone(),
            exit_code: lo.exit_code,
            stdout: String::from_utf8_lossy(&lo.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&lo.stderr).into_owned(),
            duration_secs: lo.duration.as_secs_f64(),
            timed_out: lo.timed_out,
            success: lo.success(),
            stdout_capture: lo.stdout_capture.clone(),
            stderr_capture: lo.stderr_capture.clone(),
            termination: lo.termination.clone(),
        },
    ];

    let artifact = match report::generate_report(
        &pipe,
        source,
        Some(&obj_path),
        &exe_path,
        records,
        execution,
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

    if execution_failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
/// Inspect an object file and emit its normalised view.
#[allow(clippy::items_after_test_module)]
pub(crate) fn do_obj_inspect(path: &Path, target: Option<&str>, format: OutputFormat) -> ExitCode {
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
