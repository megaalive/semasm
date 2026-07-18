//! Implementations for self-contained CLI commands.

use std::path::Path;
use std::process::ExitCode;

use semasm_agent::{harness, ContextBundle, TargetToolchain, TaskPacket};
use semasm_build::Pipeline;
use semasm_contract::{
    check_file, explain_code, format_diagnostics_terminal, CheckReportJson, ContractCode,
};
use semasm_target::{tools, TargetIdentity};

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
) -> ExitCode {
    let identity = match TargetIdentity::parse_known(target_str) {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("error: {error}");
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

    let contract_text = match std::fs::read_to_string(contract_path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("{}: error: {error}", contract_path.display());
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

    let vectors = harness::synthesize_vectors(&checked);
    if vectors.is_empty() {
        eprintln!(
            "error: no test vectors synthesised for `{}`; \
             the routine shape is not yet supported by the harness",
            checked.name
        );
        return ExitCode::from(1);
    }
    let harness_source = harness::generate_harness(&checked.name, &vectors);
    let routine_symbol = checked.name.clone();

    let directory = std::env::temp_dir().join(format!("semasm-verify-{}", std::process::id()));
    if let Err(error) = std::fs::create_dir_all(&directory) {
        eprintln!("error: cannot create scratch dir: {error}");
        return ExitCode::from(1);
    }
    let routine_object = directory.join("routine.o");
    let harness_object = directory.join("harness.o");
    let executable = directory.join("harness");

    match pipeline.assemble_reproducible(source, &routine_object, "elf64") {
        Ok(output) if output.success() => {}
        Ok(output) => {
            eprintln!(
                "assemble routine failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let _ = std::fs::remove_dir_all(&directory);
            return ExitCode::from(1);
        }
        Err(error) => {
            eprintln!("assemble routine error: {error}");
            let _ = std::fs::remove_dir_all(&directory);
            return ExitCode::from(1);
        }
    }

    let harness_path = directory.join("harness.asm");
    if let Err(error) = std::fs::write(&harness_path, &harness_source) {
        eprintln!("error: cannot write harness source: {error}");
        let _ = std::fs::remove_dir_all(&directory);
        return ExitCode::from(1);
    }
    match pipeline.assemble_reproducible(&harness_path, &harness_object, "elf64") {
        Ok(output) if output.success() => {}
        Ok(output) => {
            eprintln!(
                "assemble harness failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let _ = std::fs::remove_dir_all(&directory);
            return ExitCode::from(1);
        }
        Err(error) => {
            eprintln!("assemble harness error: {error}");
            let _ = std::fs::remove_dir_all(&directory);
            return ExitCode::from(1);
        }
    }

    match pipeline.link_reproducible(&[&routine_object, &harness_object], &executable) {
        Ok(output) if output.success() => {}
        Ok(output) => {
            eprintln!("link failed: {}", String::from_utf8_lossy(&output.stderr));
            let _ = std::fs::remove_dir_all(&directory);
            return ExitCode::from(1);
        }
        Err(error) => {
            eprintln!("link error: {error}");
            let _ = std::fs::remove_dir_all(&directory);
            return ExitCode::from(1);
        }
    }

    let run = match pipeline.run(&executable) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("run error: {error}");
            let _ = std::fs::remove_dir_all(&directory);
            return ExitCode::from(1);
        }
    };
    let report = harness::evaluate(&run.stdout, &vectors);
    let _ = std::fs::remove_dir_all(&directory);

    match format {
        OutputFormat::Json => match serde_json::to_string_pretty(&report) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("failed to serialize report: {error}");
                return ExitCode::from(1);
            }
        },
        OutputFormat::Terminal => {
            println!("Routine: {routine_symbol}");
            println!("Vectors: {}", report.cases.len());
            println!();
            for (index, case) in report.cases.iter().enumerate() {
                let status = if case.passed { "PASS" } else { "FAIL" };
                println!(
                    "{index}. [{status}] {}  expected={} observed={}",
                    case.name, case.expected, case.observed
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
