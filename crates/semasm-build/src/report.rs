//! Artifact report generation for build pipelines.
//!
//! Collects source hashes, tool versions, command records, section
//! sizes, symbols, dynamic-dependency status, and execution results
//! into a single serialisable structure.

use std::fmt::Write as FmtWrite;
use std::io::Read;
use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::exec::{self, BuildError, CommandOutput, CommandSpec};
use crate::pipeline::Pipeline;

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

/// Execution result from the runner (QEMU).
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionInfo {
    /// Exit code, or `null` when killed / timed out.
    pub exit_code: Option<i32>,
    /// Whether the process was killed by timeout.
    pub timed_out: bool,
    /// Stdout as lossy UTF-8.
    pub stdout: String,
    /// Stderr as lossy UTF-8.
    pub stderr: String,
}

/// Complete build artifact report, fully serialisable as JSON.
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactReport {
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
    /// Execution result, if available.
    pub execution: Option<ExecutionInfo>,
}

/// A JSON-friendly version of [`crate::record::CommandRecord`].
#[derive(Debug, Clone, Serialize)]
pub struct CommandRecordJson {
    /// Step label.
    pub label: String,
    /// Command line string.
    pub command: String,
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
/// * `run_output` — optional output from the runner step.
pub fn generate_report(
    pipeline: &Pipeline,
    source: &Path,
    obj_path: Option<&Path>,
    exe_path: &Path,
    command_records: Vec<CommandRecordJson>,
    run_output: Option<&CommandOutput>,
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

    // 5. Execution info
    let execution = run_output.map(|o| ExecutionInfo {
        exit_code: o.exit_code,
        timed_out: o.timed_out,
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    });

    Ok(ArtifactReport {
        source: source_info,
        tool_versions,
        command_records,
        object,
        executable,
        execution,
    })
}

impl ArtifactReport {
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

        if let Some(ref exec) = self.execution {
            let _ = writeln!(out);
            let _ = writeln!(
                out,
                "Execution:  exit={:?}{}",
                exec.exit_code,
                if exec.timed_out { " (TIMEOUT)" } else { "" },
            );
            if !exec.stdout.is_empty() {
                let _ = writeln!(out, "  stdout: {}", exec.stdout.trim());
            }
            if !exec.stderr.is_empty() {
                let _ = writeln!(out, "  stderr: {}", exec.stderr.trim());
            }
        }

        out
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use semasm_target::TargetIdentity;

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
            execution: None,
        };
        let terminal = report.to_terminal();
        assert!(terminal.contains("Artifact Report"));
        assert!(terminal.contains("test.asm"));
        assert!(terminal.contains("NASM 2.16"));
        assert!(terminal.contains("test"));
        assert!(terminal.contains("1234"));
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
                exit_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
                duration_secs: 0.123,
                timed_out: false,
                success: true,
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
            execution: Some(ExecutionInfo {
                exit_code: Some(42),
                timed_out: false,
                stdout: String::new(),
                stderr: String::new(),
            }),
        };

        // JSON round-trip
        let json = report.to_json_pretty().unwrap();
        assert!(json.contains("exit.asm"));
        assert!(json.contains("nasm"));
        assert!(json.contains("42"));
        assert!(json.contains("_start"));

        // Terminal output
        let term = report.to_terminal();
        assert!(term.contains("_start"));
        assert!(term.contains("42"));
        assert!(term.contains("SHA-256"));
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

        let source = Path::new("fixtures/asm/exit.asm");
        let out_dir = std::env::temp_dir().join("semasm-report-test-e2e");
        let _ = std::fs::create_dir_all(&out_dir);
        let obj = out_dir.join("exit.o");
        let exe = out_dir.join("exit");

        let (ao, lo) = assemble_and_link(&pipe, source, &obj, &exe).expect("assemble+link");

        // Run (if QEMU available)
        let run_out = pipe.run(&exe).ok();

        // Build command records
        let records = vec![
            CommandRecordJson {
                label: "assemble".into(),
                command: format!("nasm -f elf64 {} -o {}", source.display(), obj.display()),
                exit_code: ao.exit_code,
                stdout: String::from_utf8_lossy(&ao.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&ao.stderr).into_owned(),
                duration_secs: ao.duration.as_secs_f64(),
                timed_out: ao.timed_out,
                success: ao.success(),
            },
            CommandRecordJson {
                label: "link".into(),
                command: format!("ld {} -o {}", obj.display(), exe.display()),
                exit_code: lo.exit_code,
                stdout: String::from_utf8_lossy(&lo.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&lo.stderr).into_owned(),
                duration_secs: lo.duration.as_secs_f64(),
                timed_out: lo.timed_out,
                success: lo.success(),
            },
        ];

        let report = generate_report(&pipe, source, Some(&obj), &exe, records, run_out.as_ref())
            .expect("generate_report");

        // Verify report structure
        assert_eq!(report.source.hash.sha256.len(), 64);
        assert!(report.tool_versions.len() >= 2);
        assert!(report.object.is_some());
        assert!(!report.executable.sections.is_empty());

        let obj_info = report.object.as_ref().unwrap();
        assert!(obj_info.sections.iter().any(|s| s.name == ".text"));
        assert!(obj_info.symbols.iter().any(|s| s.name == "_start"));

        assert!(report.execution.is_some());
        if let Some(ref exec) = report.execution {
            assert_eq!(exec.exit_code, Some(42));
        }

        // JSON serialisation
        let json = report.to_json_pretty().unwrap();
        assert!(json.contains("_start"));
        assert!(json.contains("elf64"));

        // Clean up
        let _ = std::fs::remove_dir_all(&out_dir);
    }
}
