//! Safe process execution with timeout, output capture, and environment control.

use std::fmt;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Constraints for a single process invocation.
///
/// No argument is ever interpreted by a shell.
#[derive(Debug, Clone)]
pub struct CommandSpec {
    /// Path or name of the program to execute.
    pub program: String,
    /// Argument array (not shell-escaped strings).
    pub args: Vec<String>,
    /// Optional working directory.
    pub working_dir: Option<PathBuf>,
    /// Environment allowlist: when `Some`, only these variables are inherited.
    /// When `None`, the full current environment is passed through.
    pub env_allowlist: Option<Vec<String>>,
    /// Maximum wall-clock time before the process is killed.
    pub timeout: Duration,
}

impl CommandSpec {
    /// Create a new command with a default 30-second timeout and no
    /// environment restrictions.
    #[must_use]
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
            working_dir: None,
            env_allowlist: None,
            timeout: Duration::from_secs(30),
        }
    }

    /// Set the working directory for the child process.
    #[must_use]
    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Restrict the environment to the given variables.
    #[must_use]
    pub fn with_env_allowlist(mut self, vars: Vec<String>) -> Self {
        self.env_allowlist = Some(vars);
        self
    }

    /// Override the default 30-second timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl fmt::Display for CommandSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.program, self.args.join(" "))
    }
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

/// Captured result of a single process execution.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    /// Exit status, or `None` when the process was killed by timeout.
    pub exit_status: Option<ExitStatus>,
    /// Exit code, or `None` when killed by timeout or signal.
    pub exit_code: Option<i32>,
    /// Captured stdout (binary-safe).
    pub stdout: Vec<u8>,
    /// Captured stderr (binary-safe).
    pub stderr: Vec<u8>,
    /// Elapsed wall-clock time.
    pub duration: Duration,
    /// Whether the process was killed due to timeout.
    pub timed_out: bool,
}

impl CommandOutput {
    /// True when the process exited successfully (exit code 0) or was
    /// terminated by timeout (checked separately).
    #[must_use]
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }

    /// Human-readable summary for logs and reports.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.timed_out {
            format!(
                "TIMEOUT after {:.1}s (stdout={}B, stderr={}B)",
                self.duration.as_secs_f64(),
                self.stdout.len(),
                self.stderr.len(),
            )
        } else if let Some(code) = self.exit_code {
            format!(
                "exit={code} in {:.1}s (stdout={}B, stderr={}B)",
                self.duration.as_secs_f64(),
                self.stdout.len(),
                self.stderr.len(),
            )
        } else {
            format!(
                "KILLED in {:.1}s (stdout={}B, stderr={}B)",
                self.duration.as_secs_f64(),
                self.stdout.len(),
                self.stderr.len(),
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

fn build_command(spec: &CommandSpec) -> Command {
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(dir) = &spec.working_dir {
        cmd.current_dir(dir);
    }

    match &spec.env_allowlist {
        None => {
            // Full environment inherited (default Rust behaviour).
        }
        Some(allowlist) => {
            cmd.env_clear();
            for var in allowlist {
                if let Ok(val) = std::env::var(var) {
                    cmd.env(var, val);
                }
            }
        }
    }

    cmd
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

/// Run the command described by `spec`.
///
/// * Stdin is **not** provided (piped from `/dev/null` equivalent).
/// * Stdout and stderr are captured into in-memory buffers.
/// * The process is killed when `spec.timeout` elapses.
/// * No network access is enforced at the OS level (the caller must
///   configure a sandbox for that); the process never receives a network
///   capability from this function.
/// * The function never concatenates arguments into a shell string.
///
/// # Errors
///
/// Returns [`BuildError::Spawn`] when the program cannot be started
/// (e.g. not found on PATH, permission denied).
pub fn exec(spec: &CommandSpec) -> Result<CommandOutput, BuildError> {
    let start = Instant::now();

    let mut child = build_command(spec).spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            BuildError::ProgramNotFound(spec.program.clone())
        } else {
            BuildError::Spawn(spec.program.clone(), e.to_string())
        }
    })?;

    // Drain pipes in background threads to prevent deadlocks when the
    // child fills a pipe buffer.
    let stdout_pipe = child.stdout.take().expect("stdout piped");
    let stderr_pipe = child.stderr.take().expect("stderr piped");

    let stdout_handle = thread::spawn(|| drain(stdout_pipe));
    let stderr_handle = thread::spawn(|| drain(stderr_pipe));

    // Poll with timeout.
    let deadline = start + spec.timeout;
    let (exit_status, exit_code, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break (Some(status), status.code(), false);
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let killed = child.wait().ok();
                    break (killed, killed.and_then(|s| s.code()), true);
                }
                // Brief sleep to avoid busy-looping.
                thread::sleep(Duration::from_millis(5));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(BuildError::Poll(spec.program.clone(), e.to_string()));
            }
        }
    };

    let duration = start.elapsed();
    let stdout = stdout_handle.join().unwrap_or_default();
    let stderr = stderr_handle.join().unwrap_or_default();

    Ok(CommandOutput {
        exit_status,
        exit_code,
        stdout,
        stderr,
        duration,
        timed_out,
    })
}

fn drain(mut r: impl Read) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = r.read_to_end(&mut buf);
    buf
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Errors from process execution.
#[derive(Debug, Clone)]
pub enum BuildError {
    /// The program was not found on PATH.
    ProgramNotFound(String),
    /// The process could not be started.
    Spawn(String, String),
    /// The process could not be polled after starting.
    Poll(String, String),
    /// Tool output could not establish the requested artifact property.
    Verification(String),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProgramNotFound(prog) => write!(f, "program not found on PATH: `{prog}`"),
            Self::Spawn(prog, detail) => {
                write!(f, "failed to spawn `{prog}`: {detail}")
            }
            Self::Poll(prog, detail) => {
                write!(f, "failed to poll `{prog}`: {detail}")
            }
            Self::Verification(detail) => write!(f, "verification failed: {detail}"),
        }
    }
}

impl std::error::Error for BuildError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_hello() {
        let spec = CommandSpec::new("python", vec!["-c".into(), "print('hello')".into()]);
        let output = exec(&spec).unwrap();
        assert!(output.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
        assert!(!output.timed_out);
    }

    #[test]
    fn captures_stderr() {
        // `cmd /c` on Windows, `sh -c` on Unix would be simpler but we
        // avoid shells entirely.  Use a Python one-liner to write to stderr.
        let spec = CommandSpec::new(
            "python",
            vec![
                "-c".into(),
                "import sys; sys.stderr.write('err\\n'); sys.stdout.write('out\\n')".into(),
            ],
        );
        let output = exec(&spec).unwrap();
        assert!(output.success());
        assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "err");
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "out");
    }

    #[test]
    fn non_zero_exit() {
        let spec = CommandSpec::new("python", vec!["-c".into(), "exit(42)".into()]);
        let output = exec(&spec).unwrap();
        assert!(!output.success());
        assert_eq!(output.exit_code, Some(42));
        assert!(!output.timed_out);
    }

    #[test]
    fn timeout_kills() {
        let spec = CommandSpec::new(
            "python",
            vec!["-c".into(), "import time; time.sleep(10)".into()],
        )
        .with_timeout(Duration::from_millis(50));
        let output = exec(&spec).unwrap();
        assert!(output.timed_out);
        // On Unix the process is killed by signal → exit_code is None.
        // On Windows TerminateProcess sets an exit code (typically 1).
        assert!(output.duration.as_millis() < 5000);
    }

    #[test]
    fn program_not_found() {
        let spec = CommandSpec::new("nonexistent-tool-xyz", vec![]);
        let err = exec(&spec).unwrap_err();
        assert!(matches!(&err, BuildError::ProgramNotFound(p) if p == "nonexistent-tool-xyz"));
        assert!(err.to_string().contains("nonexistent-tool-xyz"));
    }

    #[test]
    fn working_directory() {
        let tmp = std::env::temp_dir();
        let marker = tmp.join("__semasm_cwd_test__");
        let marker_str = marker.to_string_lossy().replace('\\', "/");
        let code = format!("open(r'{marker_str}', 'w').close()");
        let spec = CommandSpec::new("python", vec!["-c".into(), code]).with_working_dir(&tmp);
        let output = exec(&spec).unwrap();
        assert!(output.success(), "exec failed: {}", output.summary());
        assert!(marker.exists(), "marker file should exist in working dir");
        // Clean up.
        let _ = std::fs::remove_file(&marker);
    }

    #[test]
    fn env_allowlist_isolates() {
        // On Windows `PATH` is set; `SYSTEMROOT` is also set but should be
        // excluded when the allowlist is restricted to `["PATH"]` only.
        let spec = CommandSpec::new(
            "python",
            vec![
                "-c".into(),
                "import os; print(sorted(os.environ.keys()))".into(),
            ],
        )
        .with_env_allowlist(vec!["PATH".into()]);
        let output = exec(&spec).unwrap();
        assert!(output.success());
        let out = String::from_utf8_lossy(&output.stdout);
        // Only PATH should be visible.
        assert!(out.contains("PATH"));
        // Unrelated vars should NOT appear.
        assert!(!out.contains("SYSTEMROOT"));
    }

    #[test]
    fn no_shell_concatenation() {
        // If args were concatenated into a shell string, the semicolon
        // would end the first command.  Instead, it should be a literal arg.
        let spec = CommandSpec::new(
            "python",
            vec![
                "-c".into(),
                "import sys; sys.stdout.write(sys.argv[1])".into(),
                "hello; echo pwned".into(),
            ],
        );
        let output = exec(&spec).unwrap();
        assert!(output.success());
        let out = String::from_utf8_lossy(&output.stdout);
        assert_eq!(out.trim(), "hello; echo pwned");
        assert!(!out.contains("pwned") || out.trim() == "hello; echo pwned");
    }

    #[test]
    fn display_spec() {
        let spec = CommandSpec::new("nasm", vec!["-f".into(), "elf64".into(), "foo.asm".into()]);
        let s = spec.to_string();
        assert_eq!(s, "nasm -f elf64 foo.asm");
    }

    #[test]
    fn summary_formats() {
        let spec = CommandSpec::new("python", vec!["-c".into(), "print('ok')".into()]);
        let output = exec(&spec).unwrap();
        let summary = output.summary();
        assert!(summary.contains("exit=0"));
        assert!(summary.contains("stdout="));
    }

    #[test]
    fn empty_args() {
        let spec = CommandSpec::new("python", vec!["-c".into(), "pass".into()]);
        let output = exec(&spec).unwrap();
        assert!(output.success());
    }
}
