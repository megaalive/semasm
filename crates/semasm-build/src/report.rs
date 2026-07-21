//! Artifact report generation for build pipelines.
//!
//! Collects source hashes, tool versions, command records, section
//! sizes, symbols, dynamic-dependency status, and execution results
//! into a single serialisable structure.

use std::fmt::Write as FmtWrite;
use std::io::Read;
use std::path::Path;

use semasm_target::ExecutionIsolation;
use serde::ser::Error as _;
use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::exec::{self, BuildError, CaptureInfo, CommandOutput, CommandSpec, TerminationInfo};
use crate::pipeline::Pipeline;

/// Current serialized artifact-report schema.
pub const ARTIFACT_REPORT_SCHEMA_VERSION: &str = "0.4";

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Hash of a file (SHA-256, hex-encoded).
#[derive(Debug, Clone, Serialize)]
pub struct FileHash {
    /// Hex SHA-256 of the file contents.
    pub sha256: String,
}

/// Information about a source file.
#[derive(Debug, Clone, Serialize)]
pub struct SourceInfo {
    /// Path relative to the project root (or absolute).
    pub path: String,
    /// File hash.
    pub hash: FileHash,
}

/// Tool version reported by `--version`.
#[derive(Debug, Clone, Serialize)]
pub struct ToolVersionInfo {
    /// Binary name (e.g. `nasm`, `ld.lld`).
    pub tool: String,
    /// Version string (first line of `--version`).
    pub version: String,
}

/// Section header from `objdump -h`.
#[derive(Debug, Clone, Serialize)]
pub struct SectionInfo {
    /// Section name (e.g. `.text`, `.data`, `.bss`).
    pub name: String,
    /// Size in bytes (decoded from hex).
    pub size: u64,
    /// Virtual address.
    pub vma: u64,
    /// Raw flags string (e.g. `TEXT`, `DATA`).
    pub flags: String,
}

/// Symbol table entry from `objdump -t`.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolInfo {
    /// Symbol name.
    pub name: String,
    /// Address.
    pub address: u64,
    /// Size in bytes.
    pub size: u64,
    /// Section name this symbol belongs to.
    pub section: String,
    /// Symbol type character (e.g. `F` for function, `O` for object, `N` for
    /// debugging).
    pub kind: char,
}

/// Information about a built artifact (object or executable).
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactFileInfo {
    /// Path to the artifact.
    pub path: String,
    /// File hash.
    pub hash: FileHash,
    /// File size in bytes.
    pub size: u64,
    /// Section headers (from `objdump -h`).
    pub sections: Vec<SectionInfo>,
    /// Symbol table (from `objdump -t`).
    pub symbols: Vec<SymbolInfo>,
    /// Whether the file has a `.dynamic` section (dynamic executable).
    pub is_dynamic: bool,
    /// Raw `objdump -h` output (for debugging / full transparency).
    pub raw_sections: String,
    /// Raw `objdump -t` output.
    pub raw_symbols: String,
    /// Raw `objdump -p` output (dynamic / private headers).
    pub raw_private_headers: String,
}

/// Explicit execution evidence for an artifact.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ExecutionInfo {
    /// Execution was deliberately disabled.
    NotRequested,
    /// No suitable runner was available.
    Unavailable {
        /// Why no runner could be invoked.
        reason: String,
    },
    /// The runner was available but invocation failed.
    Failed {
        /// Invocation failure suitable for reports.
        error: String,
    },
    /// The runner completed and produced output.
    Succeeded {
        /// Exit code, or `null` when killed or signalled.
        exit_code: Option<i32>,
        /// Whether the process was killed by timeout.
        timed_out: bool,
        /// Stdout as lossy UTF-8.
        stdout: String,
        /// Stderr as lossy UTF-8.
        stderr: String,
        /// Bounded stdout capture metadata.
        stdout_capture: CaptureInfo,
        /// Bounded stderr capture metadata.
        stderr_capture: CaptureInfo,
        /// Process-tree termination diagnostics, when applicable.
        termination: Option<TerminationInfo>,
    },
}

impl ExecutionInfo {
    /// Record that no suitable runner was available.
    #[must_use]
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self::Unavailable {
            reason: reason.into(),
        }
    }

    /// Record that runner invocation failed.
    #[must_use]
    pub fn failed(error: impl Into<String>) -> Self {
        Self::Failed {
            error: error.into(),
        }
    }

    /// Convert captured command output into successful execution evidence.
    #[must_use]
    pub fn succeeded(output: &CommandOutput) -> Self {
        Self::Succeeded {
            exit_code: output.exit_code,
            timed_out: output.timed_out,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            stdout_capture: output.stdout_capture.clone(),
            stderr_capture: output.stderr_capture.clone(),
            termination: output.termination.clone(),
        }
    }
}

/// Complete build artifact report, fully serialisable as JSON.
#[derive(Debug, Clone)]
pub struct ArtifactReport {
    /// Serialized schema identity used for compatibility checks.
    pub schema_version: &'static str,
    /// Normalized compilation target identity.
    pub target: String,
    /// Source file information.
    pub source: SourceInfo,
    /// Tool versions used in the build.
    pub tool_versions: Vec<ToolVersionInfo>,
    /// Commands that were executed (with specs and outputs).
    pub command_records: Vec<CommandRecordJson>,
    /// Intermediate object file info, if available.
    pub object: Option<ArtifactFileInfo>,
    /// Final executable info.
    pub executable: ArtifactFileInfo,
    /// Explicit execution evidence.
    pub execution: ExecutionInfo,
    /// How execution was isolated (or that only static build steps ran).
    pub isolation: ExecutionIsolation,
}

/// Compatibility of an artifact report with this reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportSchemaCompatibility {
    /// The report uses the current schema.
    Current,
    /// The report uses an older minor revision from the same major line.
    CompatibleOlder,
    /// A newer minor revision accepted through explicit reader opt-in.
    ForwardOptIn,
}

/// Controls artifact-report schema compatibility checks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReportReadOptions {
    /// Permit newer minor revisions from the same major schema line.
    pub allow_newer_minor: bool,
}

/// Check whether serialized artifact-report JSON is readable by this version.
pub fn check_report_schema_compatibility(json: &str) -> Result<ReportSchemaCompatibility, String> {
    check_report_schema_compatibility_with_options(json, ReportReadOptions::default())
}

/// Check report compatibility using explicit forward-read policy.
pub fn check_report_schema_compatibility_with_options(
    json: &str,
    options: ReportReadOptions,
) -> Result<ReportSchemaCompatibility, String> {
    let document: serde_json::Value =
        serde_json::from_str(json).map_err(|error| format!("invalid report JSON: {error}"))?;
    let version = document
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "artifact report is missing string field 'schema_version'".to_string())?;
    let actual = parse_schema_version(version)?;
    let supported = parse_schema_version(ARTIFACT_REPORT_SCHEMA_VERSION)?;

    if actual.0 != supported.0 {
        return Err(format!(
            "unsupported artifact report schema '{version}'; reader supports '{ARTIFACT_REPORT_SCHEMA_VERSION}'"
        ));
    }
    if actual.1 > supported.1 {
        if options.allow_newer_minor {
            return Ok(ReportSchemaCompatibility::ForwardOptIn);
        }
        return Err(format!(
            "unsupported artifact report schema '{version}'; reader supports '{ARTIFACT_REPORT_SCHEMA_VERSION}'"
        ));
    }
    if actual == supported {
        Ok(ReportSchemaCompatibility::Current)
    } else {
        Ok(ReportSchemaCompatibility::CompatibleOlder)
    }
}

fn parse_schema_version(version: &str) -> Result<(u64, u64), String> {
    let (major, minor) = version
        .split_once('.')
        .ok_or_else(|| format!("invalid schema version '{version}'; expected MAJOR.MINOR"))?;
    if minor.contains('.') {
        return Err(format!(
            "invalid schema version '{version}'; expected MAJOR.MINOR"
        ));
    }
    let major = major
        .parse()
        .map_err(|_| format!("invalid schema version '{version}'; expected MAJOR.MINOR"))?;
    let minor = minor
        .parse()
        .map_err(|_| format!("invalid schema version '{version}'; expected MAJOR.MINOR"))?;
    Ok((major, minor))
}

/// A JSON-friendly version of [`crate::record::CommandRecord`].
#[derive(Debug, Clone, Serialize)]
pub struct CommandRecordJson {
    /// Step label.
    pub label: String,
    /// Command line string.
    pub command: String,
    /// Executable identity, kept separate from display formatting.
    pub program: String,
    /// Structured command arguments before canonical path normalization.
    pub arguments: Vec<String>,
    /// Exit code.
    pub exit_code: Option<i32>,
    /// Stdout (lossy UTF-8).
    pub stdout: String,
    /// Stderr (lossy UTF-8).
    pub stderr: String,
    /// Duration in seconds.
    pub duration_secs: f64,
    /// Whether it timed out.
    pub timed_out: bool,
    /// Whether it succeeded (exit code 0).
    pub success: bool,
    /// Bounded stdout capture metadata.
    pub stdout_capture: CaptureInfo,
    /// Bounded stderr capture metadata.
    pub stderr_capture: CaptureInfo,
    /// Process-tree termination diagnostics, when applicable.
    pub termination: Option<TerminationInfo>,
}

#[derive(Serialize)]
struct CanonicalEvidence<'a> {
    schema_version: &'a str,
    target: &'a str,
    source_sha256: &'a str,
    tools: Vec<(&'a String, &'a String)>,
    commands: Vec<CanonicalCommandOutcome<'a>>,
    object: Option<CanonicalArtifact<'a>>,
    executable: CanonicalArtifact<'a>,
    execution: CanonicalExecutionOutcome,
}

#[derive(Serialize)]
struct SerializedArtifactReport<'a> {
    schema_version: &'a str,
    deterministic_evidence: CanonicalEvidence<'a>,
    deterministic_evidence_sha256: String,
    volatile_metadata: VolatileMetadata<'a>,
}

#[derive(Serialize)]
struct VolatileMetadata<'a> {
    source_path: &'a str,
    commands: &'a [CommandRecordJson],
    object: Option<VolatileArtifact<'a>>,
    executable: VolatileArtifact<'a>,
    execution: &'a ExecutionInfo,
    isolation: ExecutionIsolation,
}

#[derive(Serialize)]
struct VolatileArtifact<'a> {
    path: &'a str,
    raw_sections: &'a str,
    raw_symbols: &'a str,
    raw_private_headers: &'a str,
}

#[derive(Serialize)]
struct CanonicalCommandOutcome<'a> {
    label: &'a str,
    program: String,
    arguments: Vec<String>,
    exit_code: Option<i32>,
    timed_out: bool,
    success: bool,
}

#[derive(Serialize)]
struct CanonicalArtifact<'a> {
    sha256: &'a str,
    size: u64,
    sections: Vec<(&'a str, u64, u64, &'a str)>,
    symbols: Vec<(&'a str, u64, u64, &'a str, char)>,
    is_dynamic: bool,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum CanonicalExecutionOutcome {
    NotRequested,
    Unavailable,
    Failed,
    Succeeded {
        exit_code: Option<i32>,
        timed_out: bool,
    },
}

fn canonical_artifact(artifact: &ArtifactFileInfo) -> CanonicalArtifact<'_> {
    let mut sections: Vec<_> = artifact
        .sections
        .iter()
        .map(|section| {
            (
                section.name.as_str(),
                section.size,
                section.vma,
                section.flags.as_str(),
            )
        })
        .collect();
    sections.sort_unstable();
    let mut symbols: Vec<_> = artifact
        .symbols
        .iter()
        .map(|symbol| {
            (
                symbol.name.as_str(),
                symbol.address,
                symbol.size,
                symbol.section.as_str(),
                symbol.kind,
            )
        })
        .collect();
    symbols.sort_unstable();
    CanonicalArtifact {
        sha256: &artifact.hash.sha256,
        size: artifact.size,
        sections,
        symbols,
        is_dynamic: artifact.is_dynamic,
    }
}

fn volatile_artifact(artifact: &ArtifactFileInfo) -> VolatileArtifact<'_> {
    VolatileArtifact {
        path: &artifact.path,
        raw_sections: &artifact.raw_sections,
        raw_symbols: &artifact.raw_symbols,
        raw_private_headers: &artifact.raw_private_headers,
    }
}

fn normalize_command_component(value: &str, paths: &[(&str, &str)]) -> String {
    let mut normalized = value.replace('\\', "/");
    for (path, label) in paths {
        let normalized_path = path.replace('\\', "/");
        normalized = normalized.replace(&normalized_path, label);
    }
    if normalized.to_ascii_lowercase().ends_with("/kernel32.lib") {
        return "$KERNEL32_IMPORT_LIBRARY".to_string();
    }
    normalized
}

fn normalized_program(program: &str) -> String {
    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program)
        .to_ascii_lowercase()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn hash_file(path: &Path) -> Result<FileHash, BuildError> {
    let mut file = std::fs::File::open(path).map_err(|e| {
        BuildError::Spawn(
            "hash".into(),
            format!("cannot open {}: {e}", path.display()),
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).map_err(|e| {
            BuildError::Spawn(
                "hash".into(),
                format!("cannot read {}: {e}", path.display()),
            )
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let hash = format!("{:x}", hasher.finalize());
    Ok(FileHash { sha256: hash })
}

fn file_size(path: &Path) -> Result<u64, BuildError> {
    std::fs::metadata(path).map(|m| m.len()).map_err(|e| {
        BuildError::Spawn(
            "stat".into(),
            format!("cannot stat {}: {e}", path.display()),
        )
    })
}

// ---------------------------------------------------------------------------
// Objdump parsing
// ---------------------------------------------------------------------------

fn run_objdump_sections(tool: &str, path: &Path) -> Result<(Vec<SectionInfo>, String), BuildError> {
    let spec = CommandSpec::new(tool, vec!["-h".into(), path.to_string_lossy().into_owned()]);
    let output = exec::exec(&spec)?;
    let raw = String::from_utf8_lossy(&output.stdout).into_owned();
    let sections = parse_sections(&raw);
    Ok((sections, raw))
}

fn parse_sections(text: &str) -> Vec<SectionInfo> {
    // Both llvm-objdump and GNU objdump produce lines like:
    //   Idx Name Size      VMA               Type
    //   0 .text 0000002a  0000000000401000  TEXT
    //
    // We skip the header line and parse whitespace-delimited columns.
    let mut sections = Vec::new();
    let mut in_body = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            in_body = false;
            continue;
        }
        // Detect the header line; skip it.
        if trimmed.starts_with("Idx") && trimmed.contains("Name") {
            in_body = true;
            continue;
        }
        if !in_body {
            continue;
        }
        // Parse data line: "0 .text 0000002a 0000000000401000 TEXT ..."
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 4 && parts[0].chars().all(|c| c.is_ascii_digit()) {
            // Skip continuation lines (GNU style uses separate lines for
            // flag descriptions like "CONTENTS, ALLOC, LOAD, READONLY, CODE").
            let name = parts[1].to_string();
            let size = u64::from_str_radix(parts[2], 16).unwrap_or(0);
            let vma = u64::from_str_radix(parts[3], 16).unwrap_or(0);
            let flags = parts.get(4).copied().unwrap_or("").to_string();
            sections.push(SectionInfo {
                name,
                size,
                vma,
                flags,
            });
        }
    }
    sections
}

fn run_objdump_symbols(tool: &str, path: &Path) -> Result<(Vec<SymbolInfo>, String), BuildError> {
    let spec = CommandSpec::new(tool, vec!["-t".into(), path.to_string_lossy().into_owned()]);
    let output = exec::exec(&spec)?;
    let raw = String::from_utf8_lossy(&output.stdout).into_owned();
    let symbols = parse_symbols(&raw);
    Ok((symbols, raw))
}

fn parse_symbols(text: &str) -> Vec<SymbolInfo> {
    // GNU objdump -t format:
    //   0000000000401000 g     F .text  0000000000000002 _start
    //
    // llvm-objdump -t format:
    //   0000000000000000         .text  0000000000000000  .text  _start
    //
    // We handle both by scanning for the symbol name at the end.
    let mut symbols = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("SYMBOL TABLE") {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        // Need at least: addr, section, size, name
        if parts.len() < 4 {
            continue;
        }

        // Try to find the section name (the column before the size).
        // In GNU format the section is typically at index 2 or 3.
        // In LLVM format the section is at index 1.
        // We look for a size hex field (at least 8 hex digits) and take
        // the field before it as the section.
        let mut section_idx = None;
        for i in 2..parts.len().saturating_sub(1) {
            if parts[i].len() >= 8 && parts[i].chars().all(|c| c.is_ascii_hexdigit()) {
                let prev = parts[i - 1];
                if prev.len() < 12 || !prev.chars().all(|c| c.is_ascii_hexdigit()) {
                    section_idx = Some(i - 1);
                    break;
                }
            }
        }

        let (address, section, size, name) = if let Some(sidx) = section_idx {
            let addr_str = parts[0];
            let address = u64::from_str_radix(addr_str, 16).unwrap_or(0);
            let section = parts[sidx].to_string();
            let size = u64::from_str_radix(parts[sidx + 1], 16).unwrap_or(0);
            let name = parts[parts.len() - 1].to_string();
            (address, section, size, name)
        } else {
            continue;
        };

        // Determine kind: the GNU format has a character column after
        // visibility (g/l/w/space).  Try to find it.
        let kind = if parts.len() >= 3 && parts[1].len() == 1 {
            parts[1].chars().next().unwrap_or('?')
        } else {
            '?'
        };

        symbols.push(SymbolInfo {
            name,
            address,
            size,
            section,
            kind,
        });
    }
    symbols
}

fn run_objdump_private_headers(tool: &str, path: &Path) -> Result<String, BuildError> {
    let spec = CommandSpec::new(tool, vec!["-p".into(), path.to_string_lossy().into_owned()]);
    let output = exec::exec(&spec)?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn is_dynamic(raw_private: &str) -> bool {
    raw_private.contains("DYNAMIC") || raw_private.contains("dynamic")
}

// ---------------------------------------------------------------------------
// Tool version helpers
// ---------------------------------------------------------------------------

fn probe_tool_version(tool: &str) -> Option<String> {
    let spec = CommandSpec::new(tool, vec!["--version".into()]);
    exec::exec(&spec).ok().and_then(|o| {
        if o.success() {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .map(|s| s.trim().to_string())
        } else {
            None
        }
    })
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generate a complete artifact report from a pipeline build.
///
/// # Arguments
///
/// * `pipeline` — configured pipeline whose tools are used for analysis.
/// * `source` — path to the original assembly source file.
/// * `obj_path` — path to the assembled object file (may not exist for
///   single-step formats).
/// * `exe_path` — path to the linked executable.
/// * `command_records` — list of command records from the build steps.
/// * `execution` — explicit runner evidence.
pub fn generate_report(
    pipeline: &Pipeline,
    source: &Path,
    obj_path: Option<&Path>,
    exe_path: &Path,
    command_records: Vec<CommandRecordJson>,
    execution: ExecutionInfo,
) -> Result<ArtifactReport, BuildError> {
    // 1. Source info
    let source_hash = hash_file(source)?;
    let source_info = SourceInfo {
        path: source.to_string_lossy().into_owned(),
        hash: source_hash,
    };

    // 2. Tool versions
    let mut tool_versions = Vec::new();
    let tools_to_probe = [
        &pipeline.toolchain.assembler,
        &pipeline.toolchain.linker,
        &pipeline.toolchain.disassembler,
    ];
    for tool in &tools_to_probe {
        if let Some(ver) = probe_tool_version(tool) {
            tool_versions.push(ToolVersionInfo {
                tool: (*tool).clone(),
                version: ver,
            });
        }
    }
    if let Some(ref runner) = pipeline.toolchain.runner {
        if let Some(ver) = probe_tool_version(runner) {
            tool_versions.push(ToolVersionInfo {
                tool: runner.clone(),
                version: ver,
            });
        }
    }

    // 3. Object file info (if available)
    let object = match obj_path {
        Some(obj) if obj.exists() => {
            let hash = hash_file(obj)?;
            let size = file_size(obj)?;
            let (sections, raw_sections) =
                run_objdump_sections(&pipeline.toolchain.disassembler, obj)?;
            let (symbols, raw_symbols) =
                run_objdump_symbols(&pipeline.toolchain.disassembler, obj)?;
            let raw_private = run_objdump_private_headers(&pipeline.toolchain.disassembler, obj)?;
            let is_dyn = is_dynamic(&raw_private);
            Some(ArtifactFileInfo {
                path: obj.to_string_lossy().into_owned(),
                hash,
                size,
                sections,
                symbols,
                is_dynamic: is_dyn,
                raw_sections,
                raw_symbols,
                raw_private_headers: raw_private,
            })
        }
        _ => None,
    };

    // 4. Executable info
    let exe_hash = hash_file(exe_path)?;
    let exe_size = file_size(exe_path)?;
    let (exe_sections, exe_raw_sections) =
        run_objdump_sections(&pipeline.toolchain.disassembler, exe_path)?;
    let (exe_symbols, exe_raw_symbols) =
        run_objdump_symbols(&pipeline.toolchain.disassembler, exe_path)?;
    let exe_raw_private = run_objdump_private_headers(&pipeline.toolchain.disassembler, exe_path)?;
    let exe_is_dynamic = is_dynamic(&exe_raw_private);

    let executable = ArtifactFileInfo {
        path: exe_path.to_string_lossy().into_owned(),
        hash: exe_hash,
        size: exe_size,
        sections: exe_sections,
        symbols: exe_symbols,
        is_dynamic: exe_is_dynamic,
        raw_sections: exe_raw_sections,
        raw_symbols: exe_raw_symbols,
        raw_private_headers: exe_raw_private,
    };

    let isolation = match &execution {
        ExecutionInfo::NotRequested | ExecutionInfo::Unavailable { .. } => {
            ExecutionIsolation::StaticOnly
        }
        ExecutionInfo::Failed { .. } | ExecutionInfo::Succeeded { .. } => {
            ExecutionIsolation::from_runner(pipeline.toolchain.runner.as_deref())
        }
    };

    Ok(ArtifactReport {
        schema_version: ARTIFACT_REPORT_SCHEMA_VERSION,
        target: pipeline.target.to_string(),
        source: source_info,
        tool_versions,
        command_records,
        object,
        executable,
        execution,
        isolation,
    })
}

impl ArtifactReport {
    fn canonical_evidence(&self) -> CanonicalEvidence<'_> {
        let mut tools: Vec<_> = self
            .tool_versions
            .iter()
            .map(|tool| (&tool.tool, &tool.version))
            .collect();
        tools.sort_unstable();

        let object = self.object.as_ref().map(canonical_artifact);
        let executable = canonical_artifact(&self.executable);
        let mut logical_paths = vec![
            (self.source.path.as_str(), "$SOURCE"),
            (self.executable.path.as_str(), "$EXECUTABLE"),
        ];
        if let Some(object) = &self.object {
            logical_paths.push((object.path.as_str(), "$OBJECT"));
        }
        let commands: Vec<_> = self
            .command_records
            .iter()
            .map(|record| CanonicalCommandOutcome {
                label: &record.label,
                program: normalized_program(&record.program),
                arguments: record
                    .arguments
                    .iter()
                    .map(|argument| normalize_command_component(argument, &logical_paths))
                    .collect(),
                exit_code: record.exit_code,
                timed_out: record.timed_out,
                success: record.success,
            })
            .collect();
        let execution = match &self.execution {
            ExecutionInfo::NotRequested => CanonicalExecutionOutcome::NotRequested,
            ExecutionInfo::Unavailable { .. } => CanonicalExecutionOutcome::Unavailable,
            ExecutionInfo::Failed { .. } => CanonicalExecutionOutcome::Failed,
            ExecutionInfo::Succeeded {
                exit_code,
                timed_out,
                ..
            } => CanonicalExecutionOutcome::Succeeded {
                exit_code: *exit_code,
                timed_out: *timed_out,
            },
        };
        CanonicalEvidence {
            schema_version: self.schema_version,
            target: &self.target,
            source_sha256: &self.source.hash.sha256,
            tools,
            commands,
            object,
            executable,
            execution,
        }
    }

    /// Serialize the stable evidence subset using deterministic field and item ordering.
    pub fn canonical_evidence_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.canonical_evidence())
    }

    /// SHA-256 of the canonical deterministic evidence JSON bytes.
    pub fn canonical_evidence_hash(&self) -> Result<String, serde_json::Error> {
        let json = self.canonical_evidence_json()?;
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Serialise to pretty-printed JSON.
    pub fn to_json_pretty(&self) -> Result<String, BuildError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| BuildError::Spawn("serde".into(), e.to_string()))
    }

    /// Render a human-readable summary (suitable for terminal output).
    #[must_use]
    pub fn to_terminal(&self) -> String {
        let mut out = String::new();

        let _ = writeln!(out, "=== Artifact Report ===");
        let _ = writeln!(out, "Schema:      {}", self.schema_version);
        let _ = writeln!(out, "Target:      {}", self.target);
        let _ = writeln!(out, "Isolation:  {}", self.isolation.as_str());
        let _ = writeln!(out);

        let _ = writeln!(
            out,
            "Source:  {}  (SHA-256: {})",
            self.source.path, self.source.hash.sha256,
        );

        let _ = writeln!(out);
        let _ = writeln!(out, "Tools:");
        for tv in &self.tool_versions {
            let _ = writeln!(out, "  {:<20} {}", tv.tool, tv.version);
        }

        let _ = writeln!(out);
        let _ = writeln!(out, "Commands:");
        for cmd in &self.command_records {
            let status = if cmd.success { "OK" } else { "FAIL" };
            let _ = writeln!(
                out,
                "  [{status:4}] {} ({:.1}s, exit={:?})",
                cmd.command, cmd.duration_secs, cmd.exit_code,
            );
            write_capture_notice(
                &mut out,
                "         ",
                &cmd.stdout_capture,
                &cmd.stderr_capture,
            );
            write_termination_notice(&mut out, "         ", cmd.termination.as_ref());
        }

        if let Some(ref obj) = self.object {
            let _ = writeln!(out);
            let _ = writeln!(
                out,
                "Object:  {}  ({} bytes, SHA-256: {})",
                obj.path, obj.size, obj.hash.sha256,
            );
            let _ = writeln!(out, "  sections:   {}", obj.sections.len());
            let _ = writeln!(out, "  symbols:    {}", obj.symbols.len());
            let _ = writeln!(out, "  dynamic:    {}", obj.is_dynamic);
        }

        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "Executable:  {}  ({} bytes, SHA-256: {})",
            self.executable.path, self.executable.size, self.executable.hash.sha256,
        );
        let _ = writeln!(out, "  sections:   {}", self.executable.sections.len());
        let _ = writeln!(out, "  symbols:    {}", self.executable.symbols.len());
        let _ = writeln!(out, "  dynamic:    {}", self.executable.is_dynamic);

        if !self.executable.sections.is_empty() {
            let _ = writeln!(out);
            let _ = writeln!(out, "  Section headers:");
            for s in &self.executable.sections {
                let _ = writeln!(
                    out,
                    "    {:16} size={:#08x} vma={:#010x} {}",
                    s.name, s.size, s.vma, s.flags,
                );
            }
        }

        if !self.executable.symbols.is_empty() {
            let _ = writeln!(out);
            let _ = writeln!(out, "  Symbols:");
            for s in &self.executable.symbols {
                let _ = writeln!(
                    out,
                    "    {:24} addr={:#010x} size={:#06x}  {}",
                    s.name, s.address, s.size, s.section,
                );
            }
        }

        write_execution(&mut out, &self.execution);

        out
    }
}

impl Serialize for ArtifactReport {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let report = SerializedArtifactReport {
            schema_version: self.schema_version,
            deterministic_evidence: self.canonical_evidence(),
            deterministic_evidence_sha256: self
                .canonical_evidence_hash()
                .map_err(S::Error::custom)?,
            volatile_metadata: VolatileMetadata {
                source_path: &self.source.path,
                commands: &self.command_records,
                object: self.object.as_ref().map(volatile_artifact),
                executable: volatile_artifact(&self.executable),
                execution: &self.execution,
                isolation: self.isolation,
            },
        };
        report.serialize(serializer)
    }
}

fn write_execution(output: &mut String, execution: &ExecutionInfo) {
    let _ = writeln!(output);
    match execution {
        ExecutionInfo::NotRequested => {
            let _ = writeln!(output, "Execution:  not requested");
        }
        ExecutionInfo::Unavailable { reason } => {
            let _ = writeln!(output, "Execution:  unavailable ({reason})");
        }
        ExecutionInfo::Failed { error } => {
            let _ = writeln!(output, "Execution:  failed ({error})");
        }
        ExecutionInfo::Succeeded {
            exit_code,
            timed_out,
            stdout,
            stderr,
            stdout_capture,
            stderr_capture,
            termination,
        } => {
            let timeout = if *timed_out { " (TIMEOUT)" } else { "" };
            let _ = writeln!(output, "Execution:  exit={exit_code:?}{timeout}");
            write_capture_notice(output, "  ", stdout_capture, stderr_capture);
            write_termination_notice(output, "  ", termination.as_ref());
            if !stdout.is_empty() {
                let _ = writeln!(output, "  stdout: {}", stdout.trim());
            }
            if !stderr.is_empty() {
                let _ = writeln!(output, "  stderr: {}", stderr.trim());
            }
        }
    }
}

fn write_termination_notice(
    output: &mut String,
    indent: &str,
    termination: Option<&TerminationInfo>,
) {
    if let Some(termination) = termination {
        let _ = writeln!(
            output,
            "{indent}termination: {:?} ({})",
            termination.outcome, termination.detail,
        );
    }
}

fn write_capture_notice(
    output: &mut String,
    indent: &str,
    stdout: &CaptureInfo,
    stderr: &CaptureInfo,
) {
    if stdout.truncated || stderr.truncated {
        let _ = writeln!(
            output,
            "{indent}output truncated: stdout={}/{}, stderr={}/{} bytes",
            stdout.captured_bytes, stdout.total_bytes, stderr.captured_bytes, stderr.total_bytes,
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use semasm_target::TargetIdentity;

    fn capture_info(bytes: usize) -> CaptureInfo {
        CaptureInfo {
            captured_bytes: bytes,
            total_bytes: bytes,
            truncated: false,
            read_error: None,
        }
    }

    fn truncated_capture(captured: usize, total: usize) -> CaptureInfo {
        CaptureInfo {
            captured_bytes: captured,
            total_bytes: total,
            truncated: true,
            read_error: None,
        }
    }

    #[test]
    fn hash_empty_string() {
        let mut hasher = Sha256::new();
        hasher.update(b"");
        let hash = format!("{:x}", hasher.finalize());
        assert_eq!(hash.len(), 64);
        // Known SHA-256 of empty string.
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn hash_known_string() {
        let mut hasher = Sha256::new();
        hasher.update(b"hello");
        let hash = format!("{:x}", hasher.finalize());
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn parse_sections_llvm_style() {
        let text = "\
Sections:
Idx Name          Size      Address          Type
  0 .text         0000002a  0000000000401000  TEXT
  1 .data         00000010  0000000000402000  DATA
";
        let sections = parse_sections(text);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].name, ".text");
        assert_eq!(sections[0].size, 0x2a);
        assert_eq!(sections[0].vma, 0x0040_1000);
        assert_eq!(sections[1].name, ".data");
        assert_eq!(sections[1].size, 0x10);
    }

    #[test]
    fn parse_sections_gnu_style() {
        let text = "\
Sections:
Idx Name          Size      VMA               LMA               File off  Algn
  0 .text         0000002a  0000000000401000  0000000000401000  00001000  2**4
                  CONTENTS, ALLOC, LOAD, READONLY, CODE
  1 .data         00000010  0000000000402000  0000000000402000  00002000  2**2
                  CONTENTS, ALLOC, LOAD, DATA
";
        let sections = parse_sections(text);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].name, ".text");
        assert_eq!(sections[0].size, 0x2a);
        // With extra columns, flags captures the first extra field.
        // In GNU style the 5th column is LMA.  Our parser takes
        // column 4 as the "flags" field, which may be the LMA here.
        // That's fine — the important fields are name, size, vma.
        assert_eq!(sections[0].vma, 0x0040_1000);
    }

    #[test]
    fn parse_symbols_gnu_style() {
        let text = "\
SYMBOL TABLE:
0000000000401000 g     F .text  0000000000000002 _start
0000000000402000 g     O .data  0000000000000004 my_var
";
        let symbols = parse_symbols(text);
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "_start");
        assert_eq!(symbols[0].section, ".text");
        assert_eq!(symbols[0].size, 2);
        assert_eq!(symbols[0].address, 0x0040_1000);
    }

    #[test]
    fn report_terminal_includes_header() {
        let report = ArtifactReport {
            schema_version: ARTIFACT_REPORT_SCHEMA_VERSION,
            target: "x86_64-pc-windows-msvc".into(),
            source: SourceInfo {
                path: "test.asm".into(),
                hash: FileHash {
                    sha256: "aa".repeat(32),
                },
            },
            tool_versions: vec![ToolVersionInfo {
                tool: "nasm".into(),
                version: "NASM 2.16".into(),
            }],
            command_records: vec![],
            object: None,
            executable: ArtifactFileInfo {
                path: "test".into(),
                hash: FileHash {
                    sha256: "bb".repeat(32),
                },
                size: 1234,
                sections: vec![],
                symbols: vec![],
                is_dynamic: false,
                raw_sections: String::new(),
                raw_symbols: String::new(),
                raw_private_headers: String::new(),
            },
            execution: ExecutionInfo::NotRequested,
            isolation: ExecutionIsolation::StaticOnly,
        };
        let terminal = report.to_terminal();
        assert!(terminal.contains("Artifact Report"));
        assert!(terminal.contains("test.asm"));
        assert!(terminal.contains("NASM 2.16"));
        assert!(terminal.contains("test"));
        assert!(terminal.contains("1234"));
        assert!(terminal.contains("Execution:  not requested"));
    }

    #[test]
    fn hash_real_file() {
        let tmp = std::env::temp_dir().join("__semasm_hash_test__");
        std::fs::write(&tmp, b"hello world").unwrap();
        let hash = hash_file(&tmp).unwrap();
        // SHA-256 of "hello world\n" (without newline it's different).
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert_eq!(hash.sha256, expected);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn hash_file_not_found() {
        let err = hash_file(Path::new("__nonexistent__")).unwrap_err();
        assert!(err.to_string().contains("cannot open"));
    }

    #[test]
    fn report_to_json_roundtrip() {
        let report = ArtifactReport {
            schema_version: ARTIFACT_REPORT_SCHEMA_VERSION,
            target: "x86_64-unknown-linux-gnu".into(),
            source: SourceInfo {
                path: "exit.asm".into(),
                hash: FileHash {
                    sha256: "aa".repeat(32),
                },
            },
            tool_versions: vec![ToolVersionInfo {
                tool: "nasm".into(),
                version: "NASM 2.16".into(),
            }],
            command_records: vec![CommandRecordJson {
                label: "assemble".into(),
                command: "nasm -f elf64 exit.asm -o exit.o".into(),
                program: "nasm".into(),
                arguments: vec![
                    "-f".into(),
                    "elf64".into(),
                    "exit.asm".into(),
                    "-o".into(),
                    "exit.o".into(),
                ],
                exit_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
                duration_secs: 0.123,
                timed_out: false,
                success: true,
                stdout_capture: truncated_capture(0, 2048),
                stderr_capture: truncated_capture(0, 4096),
                termination: None,
            }],
            object: None,
            executable: ArtifactFileInfo {
                path: "exit".into(),
                hash: FileHash {
                    sha256: "bb".repeat(32),
                },
                size: 16384,
                sections: vec![SectionInfo {
                    name: ".text".into(),
                    size: 42,
                    vma: 0x0040_1000,
                    flags: "TEXT".into(),
                }],
                symbols: vec![SymbolInfo {
                    name: "_start".into(),
                    address: 0x0040_1000,
                    size: 2,
                    section: ".text".into(),
                    kind: 'F',
                }],
                is_dynamic: false,
                raw_sections: String::new(),
                raw_symbols: String::new(),
                raw_private_headers: String::new(),
            },
            execution: ExecutionInfo::Succeeded {
                exit_code: Some(42),
                timed_out: false,
                stdout: String::new(),
                stderr: String::new(),
                stdout_capture: capture_info(0),
                stderr_capture: capture_info(0),
                termination: None,
            },
            isolation: ExecutionIsolation::QemuUser,
        };

        // JSON round-trip
        let json = report.to_json_pretty().unwrap();
        assert!(json.contains("exit.asm"));
        assert!(json.contains("nasm"));
        assert!(json.contains("42"));
        assert!(json.contains("_start"));
        assert!(json.contains("\"truncated\": true"));
        assert!(json.contains("\"status\": \"succeeded\""));

        // Terminal output
        let term = report.to_terminal();
        assert!(term.contains("_start"));
        assert!(term.contains("42"));
        assert!(term.contains("SHA-256"));
        assert!(term.contains("output truncated"));
    }

    #[test]
    fn report_schema_compatibility_accepts_current_and_older_minor() {
        let current = format!(r#"{{"schema_version":"{ARTIFACT_REPORT_SCHEMA_VERSION}"}}"#);
        assert_eq!(
            check_report_schema_compatibility(&current),
            Ok(ReportSchemaCompatibility::Current)
        );
        assert_eq!(
            check_report_schema_compatibility(r#"{"schema_version":"0.0"}"#),
            Ok(ReportSchemaCompatibility::CompatibleOlder)
        );
        assert_eq!(
            check_report_schema_compatibility(r#"{"schema_version":"0.1"}"#),
            Ok(ReportSchemaCompatibility::CompatibleOlder)
        );
        assert_eq!(
            check_report_schema_compatibility(r#"{"schema_version":"0.2"}"#),
            Ok(ReportSchemaCompatibility::CompatibleOlder)
        );
        assert_eq!(
            check_report_schema_compatibility(r#"{"schema_version":"0.3"}"#),
            Ok(ReportSchemaCompatibility::CompatibleOlder)
        );
    }

    #[test]
    fn report_schema_compatibility_rejects_unsupported_versions() {
        for json in [r#"{"schema_version":"0.5"}"#, r#"{"schema_version":"1.0"}"#] {
            let error = check_report_schema_compatibility(json).unwrap_err();
            assert!(error.contains("unsupported artifact report schema"));
        }
    }

    #[test]
    fn report_schema_forward_minor_requires_explicit_opt_in() {
        let options = ReportReadOptions {
            allow_newer_minor: true,
        };
        assert_eq!(
            check_report_schema_compatibility_with_options(r#"{"schema_version":"0.5"}"#, options,),
            Ok(ReportSchemaCompatibility::ForwardOptIn)
        );
        let major_error =
            check_report_schema_compatibility_with_options(r#"{"schema_version":"1.0"}"#, options)
                .unwrap_err();
        assert!(major_error.contains("unsupported artifact report schema"));
    }

    #[test]
    fn report_schema_compatibility_rejects_missing_or_malformed_versions() {
        let missing = check_report_schema_compatibility("{}").unwrap_err();
        assert!(missing.contains("missing string field 'schema_version'"));

        for version in ["0", "0.1.0", "latest"] {
            let json = format!(r#"{{"schema_version":"{version}"}}"#);
            let error = check_report_schema_compatibility(&json).unwrap_err();
            assert!(error.contains("expected MAJOR.MINOR"));
        }
    }

    fn canonical_test_report(root: &str) -> ArtifactReport {
        ArtifactReport {
            schema_version: ARTIFACT_REPORT_SCHEMA_VERSION,
            target: "x86_64-unknown-linux-gnu".into(),
            source: SourceInfo {
                path: format!("{root}/src/fixture.asm"),
                hash: FileHash {
                    sha256: "source-hash".into(),
                },
            },
            tool_versions: vec![
                ToolVersionInfo {
                    tool: "linker".into(),
                    version: "1.0".into(),
                },
                ToolVersionInfo {
                    tool: "assembler".into(),
                    version: "2.0".into(),
                },
            ],
            command_records: vec![CommandRecordJson {
                label: "link".into(),
                command: format!("linker /OUT:{root}/out/fixture"),
                program: "linker".into(),
                arguments: vec![format!("/OUT:{root}/out/fixture")],
                exit_code: Some(0),
                stdout: format!("volatile stdout from {root}"),
                stderr: String::new(),
                duration_secs: 1.25,
                timed_out: false,
                success: true,
                stdout_capture: capture_info(12),
                stderr_capture: capture_info(0),
                termination: None,
            }],
            object: None,
            executable: ArtifactFileInfo {
                path: format!("{root}/out/fixture"),
                hash: FileHash {
                    sha256: "artifact-hash".into(),
                },
                size: 42,
                sections: vec![
                    SectionInfo {
                        name: ".data".into(),
                        size: 2,
                        vma: 2,
                        flags: "DATA".into(),
                    },
                    SectionInfo {
                        name: ".text".into(),
                        size: 4,
                        vma: 0,
                        flags: "TEXT".into(),
                    },
                ],
                symbols: vec![
                    SymbolInfo {
                        name: "z_symbol".into(),
                        address: 2,
                        size: 1,
                        section: ".data".into(),
                        kind: 'O',
                    },
                    SymbolInfo {
                        name: "entry".into(),
                        address: 0,
                        size: 4,
                        section: ".text".into(),
                        kind: 'F',
                    },
                ],
                is_dynamic: false,
                raw_sections: format!("volatile sections from {root}"),
                raw_symbols: format!("volatile symbols from {root}"),
                raw_private_headers: format!("volatile headers from {root}"),
            },
            execution: ExecutionInfo::NotRequested,
            isolation: ExecutionIsolation::StaticOnly,
        }
    }

    #[test]
    fn canonical_evidence_hash_ignores_host_paths_raw_output_and_item_order() {
        let first = canonical_test_report("/tmp/build-a");
        let mut second = canonical_test_report("D:/temp/build-b");
        second.tool_versions.reverse();
        second.executable.sections.reverse();
        second.executable.symbols.reverse();
        second.command_records[0].duration_secs = 9.75;

        assert_eq!(
            first.canonical_evidence_hash().unwrap(),
            second.canonical_evidence_hash().unwrap()
        );
    }

    #[test]
    fn serialized_report_separates_deterministic_and_volatile_fields() {
        let report = canonical_test_report("/tmp/build");
        let json = serde_json::to_value(&report).unwrap();
        let deterministic = &json["deterministic_evidence"];
        let volatile = &json["volatile_metadata"];

        assert_eq!(deterministic["target"], report.target);
        assert_eq!(deterministic["commands"][0]["program"], "linker");
        assert_eq!(
            deterministic["commands"][0]["arguments"][0],
            "/OUT:$EXECUTABLE"
        );
        assert!(deterministic.get("source_path").is_none());
        assert!(deterministic["executable"].get("raw_sections").is_none());
        assert_eq!(volatile["source_path"], "/tmp/build/src/fixture.asm");
        assert_eq!(volatile["commands"][0]["duration_secs"], 1.25);
        assert!(volatile["executable"]["raw_sections"]
            .as_str()
            .unwrap()
            .contains("/tmp/build"));
        assert_eq!(
            json["deterministic_evidence_sha256"],
            report.canonical_evidence_hash().unwrap()
        );
    }

    #[test]
    fn canonical_evidence_hash_changes_with_target_or_content() {
        let report = canonical_test_report("/tmp/build");
        let original = report.canonical_evidence_hash().unwrap();

        let mut changed_target = report.clone();
        changed_target.target = "x86_64-pc-windows-msvc".into();
        assert_ne!(original, changed_target.canonical_evidence_hash().unwrap());

        let mut changed_content = report;
        changed_content.executable.hash.sha256 = "different-hash".into();
        assert_ne!(original, changed_content.canonical_evidence_hash().unwrap());
    }

    #[test]
    fn canonical_command_normalizes_windows_sdk_import_library() {
        assert_eq!(
            normalize_command_component(
                r"C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64\kernel32.lib",
                &[],
            ),
            "$KERNEL32_IMPORT_LIBRARY"
        );
    }

    // ------------------------------------------------------------------
    // Integration tests (gated: require nasm + linker on PATH)
    // ------------------------------------------------------------------

    fn assemble_and_link(
        pipe: &Pipeline,
        source: &Path,
        obj: &Path,
        exe: &Path,
    ) -> Result<(CommandOutput, CommandOutput), BuildError> {
        let ao = pipe.assemble_reproducible(source, obj, "elf64")?;
        if !ao.success() {
            return Err(BuildError::Spawn(
                "assemble".into(),
                format!("exit {:?}", ao.exit_code),
            ));
        }
        let lo = pipe.link_reproducible(&[obj], exe)?;
        if !lo.success() {
            return Err(BuildError::Spawn(
                "link".into(),
                format!("exit {:?}", lo.exit_code),
            ));
        }
        Ok((ao, lo))
    }

    #[test]
    #[ignore = "requires nasm + linker on PATH"]
    fn full_report_from_build() {
        let target = TargetIdentity::x86_64_linux_gnu();
        let pipe = Pipeline::discover(&target);

        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures/asm/exit.asm");
        let out_dir = std::env::temp_dir().join("semasm-report-test-e2e");
        let _ = std::fs::create_dir_all(&out_dir);
        let obj = out_dir.join("exit.o");
        let exe = out_dir.join("exit");

        let (ao, lo) = assemble_and_link(&pipe, &source, &obj, &exe).expect("assemble+link");

        // Run (if QEMU available)
        let run_out = pipe.run(&exe).ok();

        // Build command records
        let records = vec![
            CommandRecordJson {
                label: "assemble".into(),
                command: format!("nasm -f elf64 {} -o {}", source.display(), obj.display()),
                program: "nasm".into(),
                arguments: vec![
                    "-f".into(),
                    "elf64".into(),
                    source.to_string_lossy().into_owned(),
                    "-o".into(),
                    obj.to_string_lossy().into_owned(),
                ],
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
                command: format!("ld {} -o {}", obj.display(), exe.display()),
                program: "ld".into(),
                arguments: vec![
                    obj.to_string_lossy().into_owned(),
                    "-o".into(),
                    exe.to_string_lossy().into_owned(),
                ],
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

        let execution = run_out.as_ref().map_or_else(
            || ExecutionInfo::Unavailable {
                reason: "runner unavailable".into(),
            },
            ExecutionInfo::succeeded,
        );
        let report = generate_report(&pipe, &source, Some(&obj), &exe, records, execution)
            .expect("generate_report");

        // Verify report structure
        assert_eq!(report.source.hash.sha256.len(), 64);
        assert!(report.tool_versions.len() >= 2);
        assert!(report.object.is_some());
        assert!(!report.executable.sections.is_empty());

        let obj_info = report.object.as_ref().unwrap();
        assert!(obj_info.sections.iter().any(|s| s.name == ".text"));
        assert!(obj_info.symbols.iter().any(|s| s.name == "_start"));

        assert!(matches!(
            &report.execution,
            ExecutionInfo::Succeeded {
                exit_code: Some(42),
                ..
            }
        ));

        // JSON serialisation
        let json = report.to_json_pretty().unwrap();
        assert!(json.contains("_start"));
        assert!(json.contains("elf64"));

        // Clean up
        let _ = std::fs::remove_dir_all(&out_dir);
    }
}
