//! Build pipeline: assemble, link, verify, and run for a target.
//!
//! Each step constructs an explicit [`CommandSpec`] and delegates to
//! [`crate::exec::exec`].  Tools are resolved via
//! [`semasm_target::tools::doctor`] by default, or can be overridden.

use std::path::Path;

use semasm_target::tools;
use semasm_target::TargetIdentity;

use crate::exec::{self, BuildError, CommandOutput, CommandSpec};

// ---------------------------------------------------------------------------
// Tool paths
// ---------------------------------------------------------------------------

/// Resolved tool binaries for a target pipeline.
///
/// Created via [`Pipeline::discover`] or built manually.
#[derive(Debug, Clone)]
pub struct Toolchain {
    /// Assembler binary (e.g. `nasm`).
    pub assembler: String,
    /// Linker binary (e.g. `ld.lld` or `ld.bfd`).
    pub linker: String,
    /// Disassembler / object dumper (e.g. `llvm-objdump` or `objdump`).
    pub disassembler: String,
    /// User-mode runner (e.g. `qemu-x86_64`), if available.
    pub runner: Option<String>,
}

// ---------------------------------------------------------------------------
// Architecture info
// ---------------------------------------------------------------------------

/// Result of verifying a built executable.
#[derive(Debug, Clone)]
pub struct ArchitectureInfo {
    /// Object format string (e.g. `"elf64-x86-64"`).
    pub format: String,
    /// Architecture string (e.g. `"x86-64"`, `"aarch64"`).
    pub arch: String,
    /// Whether the file is an executable (not a relocatable object).
    pub is_executable: bool,
}

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

/// A configured build pipeline for a specific target.
///
/// ```no_run
/// use semasm_build::pipeline::Pipeline;
/// use semasm_target::TargetIdentity;
///
/// let target = TargetIdentity::x86_64_linux_gnu();
/// let pipe = Pipeline::discover(&target);
/// ```
#[derive(Debug, Clone)]
pub struct Pipeline {
    /// Target identity.
    pub target: TargetIdentity,
    /// Resolved tool chain.
    pub toolchain: Toolchain,
}

impl Pipeline {
    /// Locate `kernel32.lib` for the x64 Windows Kit so PE linking can
    /// resolve the Win32 base imports without pulling in the C runtime.
    ///
    /// Returns the first existing path among the common Windows Kit / SDK
    /// lib directories. When nothing is found `None` is returned and the
    /// caller falls back to a `/DEFAULTLIB` hint.
    #[cfg(windows)]
    #[must_use]
    fn find_kernel32_lib() -> Option<std::path::PathBuf> {
        let program_files = std::env::var_os("ProgramFiles(x86)")
            .or_else(|| std::env::var_os("ProgramFiles"))
            .unwrap_or_else(|| "C:\\Program Files (x86)".into());
        let base = std::path::Path::new(&program_files);
        let candidates = [
            "Windows Kits\\10\\Lib\\10.0.26100.0\\um\\x64\\kernel32.lib",
            "Windows Kits\\10\\Lib\\10.0.22000.0\\um\\x64\\kernel32.lib",
            "Windows Kits\\10\\Lib\\10.0.19041.0\\um\\x64\\kernel32.lib",
        ];
        for rel in candidates {
            let p = base.join(rel);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    #[cfg(not(windows))]
    #[must_use]
    fn find_kernel32_lib() -> Option<std::path::PathBuf> {
        None
    }

    /// Auto-discover tools for the given target.
    ///
    /// Uses [`tools::doctor`] to probe `PATH`.  The first available
    /// candidate (preferred → fallback) is selected for each role.
    /// When a role has no resolved tool the binary name is still set
    /// to the preferred candidate — callers should check
    /// [`Self::all_tools_found`] before building.
    #[must_use]
    pub fn discover(target: &TargetIdentity) -> Self {
        let report = tools::doctor(target);
        let mut assembler = "nasm".to_string();
        let mut linker = "ld.lld".to_string();
        let mut disassembler = "llvm-objdump".to_string();
        let mut runner: Option<String> = None;

        for slot in &report.slots {
            let effective = slot.effective();
            let name = effective.map_or_else(
                || {
                    // No tool found — use the preferred candidate name
                    // so error messages are meaningful.
                    slot.candidates
                        .first()
                        .map_or("?", |k| k.binary())
                        .to_string()
                },
                |p| p.kind.binary().to_string(),
            );
            match slot.role {
                "assembler" => assembler = name,
                "linker" => linker = name,
                "disassembler" => disassembler = name,
                "runner" => {
                    // Native host execution is represented internally as a
                    // sentinel so the run step knows to exec the binary itself.
                    if slot
                        .candidates
                        .first()
                        .is_some_and(|k| matches!(k, semasm_target::tools::ToolKind::NativeHost))
                    {
                        runner = Some("__native__".to_string());
                    } else {
                        runner = Some(name);
                    }
                }
                _ => {}
            }
        }

        Self {
            target: target.clone(),
            toolchain: Toolchain {
                assembler,
                linker,
                disassembler,
                runner,
            },
        }
    }

    /// True when every required tool role has a resolved candidate.
    #[must_use]
    pub fn all_tools_found(&self) -> bool {
        let report = tools::doctor(&self.target);
        report.all_found()
    }

    // -- Assemble -------------------------------------------------------

    /// Assemble a NASM source file into an object file.
    ///
    /// `format` is the NASM output format (e.g. `"elf64"`, `"win64"`,
    /// `"bin"`).  For a given target this is typically:
    ///
    /// | Target | Format |
    /// |---|---|
    /// | `x86_64-unknown-linux-gnu` | `elf64` |
    /// | `x86_64-pc-windows-msvc` | `win64` |
    ///
    /// # Errors
    ///
    /// Delegates to [`exec`] — returns [`BuildError`] on spawn / poll failure.
    pub fn assemble(
        &self,
        source: &Path,
        output: &Path,
        format: &str,
    ) -> Result<CommandOutput, BuildError> {
        let spec = CommandSpec::new(
            &self.toolchain.assembler,
            vec![
                "-f".into(),
                format.to_string(),
                source.to_string_lossy().into_owned(),
                "-o".into(),
                output.to_string_lossy().into_owned(),
            ],
        );
        exec::exec(&spec)
    }

    /// Assemble with deterministic flags for reproducible builds.
    ///
    /// Adds `-O0` (no optimisation) and `-w+error` (warnings as errors).
    pub fn assemble_reproducible(
        &self,
        source: &Path,
        output: &Path,
        format: &str,
    ) -> Result<CommandOutput, BuildError> {
        let spec = CommandSpec::new(
            &self.toolchain.assembler,
            vec![
                "-O0".into(),
                "-w+error".into(),
                "-f".into(),
                format.to_string(),
                source.to_string_lossy().into_owned(),
                "-o".into(),
                output.to_string_lossy().into_owned(),
            ],
        );
        exec::exec(&spec)
    }

    // -- Link -----------------------------------------------------------

    /// Link object files into an executable.
    ///
    /// Uses the resolved linker with default flags.  For reproducibility
    /// consider [`Self::link_reproducible`].
    pub fn link(&self, objects: &[&Path], output: &Path) -> Result<CommandOutput, BuildError> {
        let mut args = vec!["-o".into(), output.to_string_lossy().into_owned()];
        for obj in objects {
            args.push(obj.to_string_lossy().into_owned());
        }
        let spec = CommandSpec::new(&self.toolchain.linker, args);
        exec::exec(&spec)
    }

    /// Link with reproducibility flags: `--build-id=none`,
    /// `--hash-style=sysv` (deterministic section ordering).
    pub fn link_reproducible(
        &self,
        objects: &[&Path],
        output: &Path,
    ) -> Result<CommandOutput, BuildError> {
        let mut args = vec![
            "--build-id=none".into(),
            "--hash-style=sysv".into(),
            "-o".into(),
            output.to_string_lossy().into_owned(),
        ];
        for obj in objects {
            args.push(obj.to_string_lossy().into_owned());
        }
        let spec = CommandSpec::new(&self.toolchain.linker, args);
        exec::exec(&spec)
    }

    // -- Verify ---------------------------------------------------------

    /// Verify the architecture of a built file using the configured
    /// disassembler's `-f` (file-header summary) flag.
    ///
    /// Both `llvm-objdump` and GNU `objdump` support `-f`.
    pub fn verify_architecture(&self, path: &Path) -> Result<ArchitectureInfo, BuildError> {
        let spec = CommandSpec::new(
            &self.toolchain.disassembler,
            vec!["-f".into(), path.to_string_lossy().into_owned()],
        );
        let output = exec::exec(&spec)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let header = stdout.trim();

        // Parse typical output:
        //   llvm-objdump:   "exit:   file format elf64-x86-64"
        //   GNU objdump:    "exit:     file format elf64-x86-64"
        //   llvm-objdump:   "architecture: x86_64"
        //   GNU objdump:    "architecture: i386:x86-64"
        //
        // Also detect executable vs relocatable:
        //   "EXECUTABLE" or "executable" in flags → executable
        //   "RELOCATABLE" → object file

        let format = header
            .split("file format")
            .nth(1)
            .unwrap_or("?")
            .trim()
            .to_string();

        let arch = if header.contains("architecture:") {
            header
                .lines()
                .find_map(|l| l.strip_prefix("architecture:"))
                .unwrap_or("?")
                .trim()
                .to_string()
        } else {
            // Fallback: extract from format string
            format.split('-').nth(1).unwrap_or("?").to_string()
        };

        let is_executable = header.contains("EXECUTABLE") || header.contains("executable");

        Ok(ArchitectureInfo {
            format,
            arch,
            is_executable,
        })
    }

    // -- Run ------------------------------------------------------------

    /// Run the executable under the configured user-mode runner (QEMU).
    ///
    /// Returns the captured output including exit code.
    /// When no runner is configured an error is returned.
    ///
    /// A runner value of `"__native__"` denotes direct host execution
    /// (e.g. running a Windows PE on a Windows host); on a non-matching
    /// host this is an error.
    pub fn run(&self, executable: &Path) -> Result<CommandOutput, BuildError> {
        match &self.toolchain.runner {
            Some(runner) if runner == "__native__" => {
                // Native execution: invoke the binary directly. Only valid on
                // the matching host OS.
                #[cfg(windows)]
                {
                    let spec =
                        CommandSpec::new(executable.to_string_lossy().into_owned(), Vec::new());
                    exec::exec(&spec)
                }
                #[cfg(not(windows))]
                {
                    Err(BuildError::ProgramNotFound(
                        "native execution requires the matching host OS".into(),
                    ))
                }
            }
            Some(runner) => {
                let spec =
                    CommandSpec::new(runner, vec![executable.to_string_lossy().into_owned()]);
                exec::exec(&spec)
            }
            None => Err(BuildError::ProgramNotFound(
                "no runner configured for this target".into(),
            )),
        }
    }

    /// Link object files into a PE/COFF executable using `lld-link`.
    ///
    /// `entry` names the program entry symbol (e.g. `"mainCRTStartup"` or
    /// a custom `_start`). `subsystem` selects the PE subsystem
    /// (`"console"` or `"windows"`).
    ///
    /// No C runtime is linked by default: `kernel32.lib` is discovered and
    /// passed explicitly so console programs can call the Win32 API
    /// (`GetStdHandle`, `WriteFile`, `ExitProcess`, …) without pulling in
    /// MSVCRT. Callers that want the C runtime should pass their own
    /// import libraries.
    pub fn link_pe(
        &self,
        objects: &[&Path],
        output: &Path,
        entry: &str,
        subsystem: &str,
    ) -> Result<CommandOutput, BuildError> {
        let mut args = vec![
            format!("/ENTRY:{}", entry),
            format!("/SUBSYSTEM:{}", subsystem),
        ];
        // Link only the Win32 base import library (no MSVCRT).
        if let Some(lib) = Self::find_kernel32_lib() {
            args.push(lib.to_string_lossy().into_owned());
        } else {
            // Fall back to letting the linker resolve the default lib by name.
            args.push("/DEFAULTLIB:kernel32.lib".into());
        }
        args.push("/OUT:".to_string() + &output.to_string_lossy());
        for obj in objects {
            args.push(obj.to_string_lossy().into_owned());
        }
        let spec = CommandSpec::new(&self.toolchain.linker, args);
        exec::exec(&spec)
    }

    /// Build the fixture end-to-end and return the assembly, link, verify,
    /// and run records.
    ///
    /// This is a convenience that calls `assemble_reproducible`,
    /// `link_reproducible`, `verify_architecture`, and `run` in sequence.
    /// On failure the error includes the step that failed.
    pub fn build_fixture(
        &self,
        source: &Path,
        obj_path: &Path,
        exe_path: &Path,
        format: &str,
        expected_exit: Option<i32>,
    ) -> Result<BuildReport, BuildError> {
        let assemble_out = self.assemble_reproducible(source, obj_path, format)?;
        if !assemble_out.success() {
            return Err(BuildError::Spawn(
                "assemble".into(),
                format!("exit code {:?}", assemble_out.exit_code),
            ));
        }

        let link_out = self.link_reproducible(&[obj_path], exe_path)?;
        if !link_out.success() {
            return Err(BuildError::Spawn(
                "link".into(),
                format!("exit code {:?}", link_out.exit_code),
            ));
        }

        let arch = self.verify_architecture(exe_path)?;

        let run_out = self.run(exe_path).ok();

        if let (Some(expected), Some(ref run)) = (expected_exit, &run_out) {
            if run.exit_code != Some(expected) {
                return Err(BuildError::Spawn(
                    "run".into(),
                    format!("expected exit code {expected}, got {:?}", run.exit_code),
                ));
            }
        }

        Ok(BuildReport {
            assemble: assemble_out,
            link: link_out,
            architecture: arch,
            run: run_out,
        })
    }
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

/// Complete result of a fixture build.
#[derive(Debug)]
pub struct BuildReport {
    /// Assembly step output.
    pub assemble: CommandOutput,
    /// Link step output.
    pub link: CommandOutput,
    /// Architecture verification info.
    pub architecture: ArchitectureInfo,
    /// Run step output (if runner was available).
    pub run: Option<CommandOutput>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_target() -> TargetIdentity {
        TargetIdentity::x86_64_linux_gnu()
    }

    #[test]
    fn discover_creates_pipeline() {
        let target = test_target();
        let pipe = Pipeline::discover(&target);
        assert_eq!(pipe.target.name, "x86_64-unknown-linux-gnu");
        // Tool names are set even if tools are not on PATH.
        assert_eq!(pipe.toolchain.assembler, "nasm");
        assert!(!pipe.toolchain.linker.is_empty());
        assert!(!pipe.toolchain.disassembler.is_empty());
    }

    // ------------------------------------------------------------------
    // Integration tests (gated: require nasm on PATH)
    // ------------------------------------------------------------------

    #[allow(dead_code)]
    fn tool_available(name: &str) -> bool {
        let spec = CommandSpec::new(name, vec!["--version".into()]);
        exec::exec(&spec).is_ok_and(|o| o.success())
    }

    #[test]
    #[ignore = "requires nasm on PATH"]
    fn assemble_exit_fixture() {
        let target = test_target();
        let pipe = Pipeline::discover(&target);

        let source = Path::new("fixtures/asm/exit.asm");
        let out_dir = std::env::temp_dir().join("semasm-build-test");
        let _ = std::fs::create_dir_all(&out_dir);
        let obj = out_dir.join("exit.o");

        let result = pipe.assemble(source, &obj, "elf64");
        assert!(result.is_ok(), "assemble failed: {:?}", result.err());
        let output = result.unwrap();
        assert!(
            output.success(),
            "nasm exited {:?}: {}",
            output.exit_code,
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(obj.exists(), "object file was not created");

        let _ = std::fs::remove_dir_all(&out_dir);
    }

    #[test]
    #[ignore = "requires nasm + linker on PATH"]
    fn build_exit_fixture_end_to_end() {
        let target = test_target();
        let pipe = Pipeline::discover(&target);

        let source = Path::new("fixtures/asm/exit.asm");
        let out_dir = std::env::temp_dir().join("semasm-build-test-e2e");
        let _ = std::fs::create_dir_all(&out_dir);
        let obj = out_dir.join("exit.o");
        let exe = out_dir.join("exit");

        // Assemble
        let ao = pipe
            .assemble_reproducible(source, &obj, "elf64")
            .expect("assemble");
        assert!(ao.success(), "assemble failed");
        assert!(obj.exists());

        // Link
        let lo = pipe.link_reproducible(&[&obj], &exe).expect("link");
        assert!(lo.success(), "link failed");
        assert!(exe.exists());

        // Verify architecture
        let arch = pipe.verify_architecture(&exe).expect("verify");
        assert!(
            arch.format.contains("x86-64") || arch.format.contains("x86_64"),
            "unexpected format: {}",
            arch.format
        );
        assert!(
            arch.is_executable,
            "linked file should be executable, got format={}, arch={}",
            arch.format, arch.arch
        );

        // Run (only if QEMU available)
        if pipe.toolchain.runner.is_some() {
            let ro = pipe.run(&exe).expect("run");
            assert_eq!(
                ro.exit_code,
                Some(42),
                "expected exit code 42, got {:?}",
                ro.exit_code
            );
        }

        // Clean up
        let _ = std::fs::remove_dir_all(&out_dir);
    }

    #[test]
    #[ignore = "requires nasm on PATH"]
    fn verify_detects_object_not_executable() {
        let target = test_target();
        let pipe = Pipeline::discover(&target);

        let source = Path::new("fixtures/asm/exit.asm");
        let out_dir = std::env::temp_dir().join("semasm-build-test-obj");
        let _ = std::fs::create_dir_all(&out_dir);
        let obj = out_dir.join("exit.o");

        pipe.assemble(source, &obj, "elf64").expect("assemble");

        let arch = pipe.verify_architecture(&obj).expect("verify");
        assert!(!arch.is_executable, "object file should NOT be executable");

        let _ = std::fs::remove_dir_all(&out_dir);
    }

    #[test]
    #[ignore = "requires nasm + lld-link on PATH (Windows host)"]
    fn build_windows_pe_and_run() {
        use semasm_obj::ContainerFormat;
        use semasm_target::TargetIdentity;

        let target = TargetIdentity::x86_64_windows_msvc();
        let pipe = Pipeline::discover(&target);
        if pipe.toolchain.runner.is_none() {
            // Native execution only works on a Windows host.
            return;
        }

        // Resolve the fixture relative to the workspace root so the test works
        // regardless of the crate's build directory.
        let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
        // crates/<crate> -> workspace root is two parents up.
        let workspace = std::path::Path::new(&manifest)
            .parent()
            .and_then(std::path::Path::parent)
            .unwrap_or_else(|| std::path::Path::new("."));
        let source = workspace.join("fixtures/asm/hello_win64.asm");
        assert!(source.exists(), "missing fixture {source:?}");
        let out_dir = std::env::temp_dir().join("semasm-build-win64");
        let _ = std::fs::create_dir_all(&out_dir);
        let obj = out_dir.join("hello.obj");
        let exe = out_dir.join("hello.exe");

        // Assemble (win64 format).
        let ao = pipe
            .assemble_reproducible(&source, &obj, target.nasm_format())
            .expect("assemble");
        assert!(
            ao.success(),
            "assemble failed: {}",
            String::from_utf8_lossy(&ao.stderr)
        );

        // Link a PE (kernel32.lib, no C runtime). Discover the entry
        // symbol from the object's exported globals.
        let entry = {
            let info = semasm_obj::read(&obj).expect("inspect obj");
            info.exports
                .iter()
                .find(|s| *s == "main" || *s == "_start" || *s == "mainCRTStartup")
                .or_else(|| info.exports.first())
                .cloned()
                .unwrap_or_else(|| "main".into())
        };
        let lo = pipe
            .link_pe(&[&obj], &exe, &entry, "console")
            .expect("link");
        assert!(
            lo.success(),
            "link failed: {}",
            String::from_utf8_lossy(&lo.stderr)
        );
        assert!(exe.exists(), "executable was not created");

        // Inspect the PE container.
        let info = semasm_obj::read(&exe).expect("inspect");
        assert_eq!(info.format, ContainerFormat::Pe);
        assert!(info.entry != 0, "PE should have an entry point");

        // The object must import only the Win32 base API (no C runtime) — i.e.
        // only `kernel32` style symbols, no `msvcrt`/`__acrt`/`printf` etc.
        let obj_info = semasm_obj::read(&obj).expect("inspect obj");
        assert!(
            obj_info.imports.iter().any(|s| s == "GetStdHandle"),
            "expected Win32 import GetStdHandle, got {:?}",
            obj_info.imports
        );
        assert!(
            !obj_info.imports.iter().any(|s| {
                s.contains("msvcrt") || s.contains("acrt") || s == "printf" || s == "memset"
            }),
            "C runtime symbols must NOT be linked, got {:?}",
            obj_info.imports
        );

        // Run it natively; expect the fixture string on stdout.
        let ro = pipe.run(&exe).expect("run");
        assert_eq!(ro.exit_code, Some(0), "expected exit code 0");
        let out = String::from_utf8_lossy(&ro.stdout);
        assert!(
            out.contains("SemASM Windows x64"),
            "unexpected program output: {out:?}"
        );

        let _ = std::fs::remove_dir_all(&out_dir);
    }
}
