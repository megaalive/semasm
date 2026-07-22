//! Safe process execution with timeout, output capture, and environment control.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

mod platform;

const DEFAULT_CAPTURE_BYTES: usize = 16 * 1024 * 1024;

/// Maximum number of bytes retained from one output stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureLimit {
    /// Maximum retained byte count. All additional bytes are drained and counted.
    pub max_bytes: usize,
}

impl CaptureLimit {
    /// Construct a stream capture limit.
    #[must_use]
    pub const fn new(max_bytes: usize) -> Self {
        Self { max_bytes }
    }
}

impl Default for CaptureLimit {
    fn default() -> Self {
        Self::new(DEFAULT_CAPTURE_BYTES)
    }
}

/// Metadata describing bounded capture of one output stream.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CaptureInfo {
    /// Bytes retained in memory.
    pub captured_bytes: usize,
    /// Bytes observed while draining the stream.
    pub total_bytes: usize,
    /// Whether observed output exceeded the configured limit.
    pub truncated: bool,
    /// Read failure after any successfully observed bytes.
    pub read_error: Option<String>,
}

/// Why SemASM initiated process-tree termination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminationReason {
    /// The configured wall-clock timeout elapsed.
    Timeout,
}

/// Result of attempting to terminate the complete child process tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminationOutcome {
    /// The tree exited after a graceful termination request.
    Graceful,
    /// The tree required forced termination.
    Forced,
    /// Complete descendant termination could not be established.
    Incomplete,
}

/// Diagnostics for a process-tree termination attempt.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TerminationInfo {
    /// Reason termination was initiated.
    pub reason: TerminationReason,
    /// Whether termination was graceful, forced, or incomplete.
    pub outcome: TerminationOutcome,
    /// Platform-specific diagnostic without child output or environment data.
    pub detail: String,
}

impl CaptureInfo {
    fn from_capture(captured_bytes: usize, total_bytes: usize, read_error: Option<String>) -> Self {
        Self {
            captured_bytes,
            total_bytes,
            truncated: total_bytes > captured_bytes,
            read_error,
        }
    }
}

/// Policy for data connected to the child process's standard input.
#[derive(Clone, Default)]
pub enum StdinPolicy {
    /// Connect stdin to the null device so reads immediately return EOF.
    #[default]
    Null,
    /// Write the provided bytes, then close stdin.
    Bytes(Vec<u8>),
    /// Explicitly inherit the parent's interactive stdin.
    Inherit,
}

impl fmt::Debug for StdinPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => formatter.write_str("Null"),
            Self::Bytes(bytes) => formatter
                .debug_struct("Bytes")
                .field("length", &bytes.len())
                .finish(),
            Self::Inherit => formatter.write_str("Inherit"),
        }
    }
}

/// Policy controlling which environment variables a child receives.
#[derive(Clone, Default)]
pub enum EnvironmentPolicy {
    /// Inherit only the cross-platform baseline required to locate and run tools.
    #[default]
    Sanitized,
    /// Inherit only the named variables.
    Allowlist(Vec<String>),
    /// Use only the provided names and values.
    Explicit(BTreeMap<String, String>),
    /// Explicit debugging opt-in to the complete parent environment.
    InheritAll,
}

impl fmt::Debug for EnvironmentPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.report_label())
    }
}

impl EnvironmentPolicy {
    /// Redacted policy description suitable for reports.
    #[must_use]
    pub fn report_label(&self) -> String {
        match self {
            Self::Sanitized => "sanitized".to_string(),
            Self::Allowlist(names) => format!("allowlist:{}", redacted_names(names)),
            Self::Explicit(values) => format!(
                "explicit:{}",
                redacted_names(&values.keys().cloned().collect::<Vec<_>>())
            ),
            Self::InheritAll => "inherit-all".to_string(),
        }
    }

    /// Whether the policy prevents full parent-environment inheritance.
    #[must_use]
    pub const fn is_restricted(&self) -> bool {
        !matches!(self, Self::InheritAll)
    }
}

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
    /// Standard-input policy. Defaults to [`StdinPolicy::Null`].
    pub stdin: StdinPolicy,
    /// Environment policy. Defaults to [`EnvironmentPolicy::Sanitized`].
    pub environment: EnvironmentPolicy,
    /// Maximum wall-clock time before the process is killed.
    pub timeout: Duration,
    /// Maximum stdout bytes retained in memory.
    pub stdout_limit: CaptureLimit,
    /// Maximum stderr bytes retained in memory.
    pub stderr_limit: CaptureLimit,
}

impl CommandSpec {
    /// Create a command with null stdin, a sanitized environment, and a
    /// default 30-second timeout.
    #[must_use]
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
            working_dir: None,
            stdin: StdinPolicy::Null,
            environment: EnvironmentPolicy::Sanitized,
            timeout: Duration::from_secs(30),
            stdout_limit: CaptureLimit::default(),
            stderr_limit: CaptureLimit::default(),
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
        self.environment = EnvironmentPolicy::Allowlist(vars);
        self
    }

    /// Use an explicit environment without inheriting parent values.
    #[must_use]
    pub fn with_environment(mut self, values: BTreeMap<String, String>) -> Self {
        self.environment = EnvironmentPolicy::Explicit(values);
        self
    }

    /// Deliberately inherit the complete parent environment for debugging.
    #[must_use]
    pub fn inherit_full_environment(mut self) -> Self {
        self.environment = EnvironmentPolicy::InheritAll;
        self
    }

    /// Provide controlled bytes on stdin, then close the stream.
    #[must_use]
    pub fn with_stdin_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.stdin = StdinPolicy::Bytes(bytes);
        self
    }

    /// Deliberately inherit interactive stdin.
    #[must_use]
    pub fn inherit_stdin(mut self) -> Self {
        self.stdin = StdinPolicy::Inherit;
        self
    }

    /// Override the default 30-second timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set independent stdout and stderr capture limits.
    #[must_use]
    pub fn with_capture_limits(mut self, stdout: CaptureLimit, stderr: CaptureLimit) -> Self {
        self.stdout_limit = stdout;
        self.stderr_limit = stderr;
        self
    }

    /// Apply the same capture limit to stdout and stderr.
    #[must_use]
    pub fn with_capture_limit(self, limit: CaptureLimit) -> Self {
        self.with_capture_limits(limit, limit)
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
    /// Bounded stdout capture metadata.
    pub stdout_capture: CaptureInfo,
    /// Bounded stderr capture metadata.
    pub stderr_capture: CaptureInfo,
    /// Elapsed wall-clock time.
    pub duration: Duration,
    /// Whether the process was killed due to timeout.
    pub timed_out: bool,
    /// Process-tree termination diagnostics, when SemASM initiated termination.
    pub termination: Option<TerminationInfo>,
}

impl CommandOutput {
    /// True when the process exited successfully (exit code 0) or was
    /// terminated by timeout (checked separately).
    #[must_use]
    pub fn success(&self) -> bool {
        !self.timed_out && self.exit_code == Some(0)
    }

    /// Human-readable summary for logs and reports.
    #[must_use]
    pub fn summary(&self) -> String {
        if self.timed_out {
            format!(
                "TIMEOUT after {:.1}s (stdout={}, stderr={})",
                self.duration.as_secs_f64(),
                capture_summary(&self.stdout_capture),
                capture_summary(&self.stderr_capture),
            )
        } else if let Some(code) = self.exit_code {
            format!(
                "exit={code} in {:.1}s (stdout={}, stderr={})",
                self.duration.as_secs_f64(),
                capture_summary(&self.stdout_capture),
                capture_summary(&self.stderr_capture),
            )
        } else {
            format!(
                "KILLED in {:.1}s (stdout={}, stderr={})",
                self.duration.as_secs_f64(),
                capture_summary(&self.stdout_capture),
                capture_summary(&self.stderr_capture),
            )
        }
    }
}

fn capture_summary(info: &CaptureInfo) -> String {
    if info.truncated {
        format!("{}/{}B truncated", info.captured_bytes, info.total_bytes)
    } else {
        format!("{}B", info.captured_bytes)
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

    cmd.stdin(match &spec.stdin {
        // On Windows, `Stdio::null()` has been observed to leave some readers
        // (notably CPython `stdin.buffer.read`) blocked under CI load. An empty
        // pipe that we close immediately still yields EOF and is more reliable.
        StdinPolicy::Null => {
            if cfg!(windows) {
                Stdio::piped()
            } else {
                Stdio::null()
            }
        }
        StdinPolicy::Bytes(_) => Stdio::piped(),
        StdinPolicy::Inherit => Stdio::inherit(),
    });

    if let Some(dir) = &spec.working_dir {
        cmd.current_dir(dir);
    }

    match &spec.environment {
        EnvironmentPolicy::Sanitized => apply_sanitized_environment(&mut cmd),
        EnvironmentPolicy::Allowlist(allowlist) => {
            cmd.env_clear();
            for var in allowlist {
                if let Some(value) = std::env::var_os(var) {
                    cmd.env(var, value);
                }
            }
        }
        EnvironmentPolicy::Explicit(values) => {
            cmd.env_clear();
            cmd.envs(values);
        }
        EnvironmentPolicy::InheritAll => {}
    }

    cmd
}

fn apply_sanitized_environment(cmd: &mut Command) {
    cmd.env_clear();
    for name in [
        "PATH",
        "SYSTEMROOT",
        "WINDIR",
        "SYSTEMDRIVE",
        "PROGRAMDATA",
        "ALLUSERSPROFILE",
        "PATHEXT",
        "COMSPEC",
    ] {
        if let Some(value) = std::env::var_os(name) {
            cmd.env(name, value);
        }
    }

    let temporary = std::env::temp_dir();
    cmd.env("TMP", &temporary).env("TEMP", &temporary);
    #[cfg(not(windows))]
    cmd.env("TMPDIR", &temporary)
        .env("LC_ALL", "C")
        .env("LANG", "C");
}

fn redacted_names(names: &[String]) -> String {
    names
        .iter()
        .map(|name| {
            if is_secret_name(name) {
                "<redacted>"
            } else {
                name.as_str()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn is_secret_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    [
        "TOKEN",
        "SECRET",
        "PASSWORD",
        "PASSWD",
        "CREDENTIAL",
        "AUTH",
        "COOKIE",
        "API_KEY",
        "PRIVATE_KEY",
    ]
    .iter()
    .any(|marker| upper.contains(marker))
}

// ---------------------------------------------------------------------------
// Execution
// ---------------------------------------------------------------------------

fn spawn_stdin_worker(
    child: &mut std::process::Child,
    policy: &StdinPolicy,
) -> Option<thread::JoinHandle<()>> {
    match policy {
        StdinPolicy::Bytes(bytes) => {
            let bytes = bytes.clone();
            child.stdin.take().map(|mut stdin| {
                thread::spawn(move || {
                    let _ = stdin.write_all(&bytes);
                })
            })
        }
        StdinPolicy::Null => {
            // Close the optional piped stdin promptly so readers see EOF.
            child
                .stdin
                .take()
                .map(|stdin| thread::spawn(move || drop(stdin)))
        }
        StdinPolicy::Inherit => None,
    }
}

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

    let mut command = build_command(spec);
    platform::configure(&mut command);
    let mut child = command.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            BuildError::ProgramNotFound(spec.program.clone())
        } else {
            BuildError::Spawn(spec.program.clone(), e.to_string())
        }
    })?;
    let mut process_tree = platform::ProcessTree::attach(&child).map_err(|detail| {
        let _ = child.kill();
        let _ = child.wait();
        BuildError::Spawn(
            spec.program.clone(),
            format!("failed to establish process-tree ownership: {detail}"),
        )
    })?;

    let stdin_handle = spawn_stdin_worker(&mut child, &spec.stdin);

    // Drain pipes in background threads to prevent deadlocks when the
    // child fills a pipe buffer.
    let stdout_pipe = child.stdout.take().expect("stdout piped");
    let stderr_pipe = child.stderr.take().expect("stderr piped");

    let stdout_limit = spec.stdout_limit;
    let stderr_limit = spec.stderr_limit;
    let (stdout_sender, stdout_receiver) = mpsc::channel();
    let (stderr_sender, stderr_receiver) = mpsc::channel();
    let stdout_handle = thread::spawn(move || {
        let _ = stdout_sender.send(drain(stdout_pipe, stdout_limit));
    });
    let stderr_handle = thread::spawn(move || {
        let _ = stderr_sender.send(drain(stderr_pipe, stderr_limit));
    });

    // Poll both the direct child and pipe drains. A launcher can exit while a
    // descendant keeps inherited pipe handles open, so neither condition alone
    // establishes completion of the owned process tree.
    let deadline = start + spec.timeout;
    let mut direct_status = None;
    let mut stdout_result = None;
    let mut stderr_result = None;
    let (exit_status, exit_code, timed_out, termination) = loop {
        if direct_status.is_none() {
            match child.try_wait() {
                Ok(Some(status)) => direct_status = Some(status),
                Ok(None) => {}
                Err(error) => {
                    let _ = process_tree.terminate(&mut child);
                    let _ = child.wait();
                    return Err(BuildError::Poll(spec.program.clone(), error.to_string()));
                }
            }
        }
        receive_capture(&stdout_receiver, &mut stdout_result);
        receive_capture(&stderr_receiver, &mut stderr_result);

        if stdout_result.is_some() && stderr_result.is_some() {
            if let Some(status) = direct_status {
                break (Some(status), status.code(), false, None);
            }
        }
        if Instant::now() >= deadline {
            let termination = process_tree.terminate(&mut child);
            let status = direct_status.or_else(|| child.wait().ok());
            break (
                status,
                status.and_then(|value| value.code()),
                true,
                Some(termination),
            );
        }
        thread::sleep(Duration::from_millis(5));
    };

    let stdout = receive_capture_blocking(stdout_result, &stdout_receiver, "stdout");
    let stderr = receive_capture_blocking(stderr_result, &stderr_receiver, "stderr");
    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    let duration = start.elapsed();
    let (stdout, stdout_capture) = stdout;
    let (stderr, stderr_capture) = stderr;
    if let Some(handle) = stdin_handle {
        let _ = handle.join();
    }

    Ok(CommandOutput {
        exit_status,
        exit_code,
        stdout,
        stderr,
        stdout_capture,
        stderr_capture,
        duration,
        timed_out,
        termination,
    })
}

type CaptureResult = (Vec<u8>, CaptureInfo);

fn receive_capture(
    receiver: &mpsc::Receiver<CaptureResult>,
    destination: &mut Option<CaptureResult>,
) {
    if destination.is_some() {
        return;
    }
    match receiver.try_recv() {
        Ok(result) => *destination = Some(result),
        Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
    }
}

fn receive_capture_blocking(
    available: Option<CaptureResult>,
    receiver: &mpsc::Receiver<CaptureResult>,
    stream: &str,
) -> CaptureResult {
    available.unwrap_or_else(|| {
        receiver.recv().unwrap_or_else(|_| {
            (
                Vec::new(),
                CaptureInfo::from_capture(
                    0,
                    0,
                    Some(format!("{stream} drain thread stopped without a result")),
                ),
            )
        })
    })
}

fn drain(mut reader: impl Read, limit: CaptureLimit) -> (Vec<u8>, CaptureInfo) {
    let mut captured = Vec::with_capacity(limit.max_bytes.min(8192));
    let mut total_bytes = 0usize;
    let mut chunk = [0u8; 8192];
    let read_error = loop {
        match reader.read(&mut chunk) {
            Ok(0) => break None,
            Ok(count) => {
                total_bytes = total_bytes.saturating_add(count);
                let remaining = limit.max_bytes.saturating_sub(captured.len());
                captured.extend_from_slice(&chunk[..count.min(remaining)]);
            }
            Err(error) => break Some(error.to_string()),
        }
    };
    let info = CaptureInfo::from_capture(captured.len(), total_bytes, read_error);
    (captured, info)
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
    /// A produced artifact was corrupt or unsupported.
    ObjectParse(String),
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
            Self::ObjectParse(detail) => write!(f, "object parsing failed: {detail}"),
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
        assert!(
            !output.success(),
            "a timed-out command cannot be successful"
        );
        assert!(output.termination.is_some());
        // On Unix the process is killed by signal → exit_code is None.
        // On Windows TerminateProcess sets an exit code (typically 1).
        assert!(output.duration.as_millis() < 5000);
    }

    #[test]
    fn timeout_terminates_spawned_grandchild() {
        let marker =
            std::env::temp_dir().join(format!("semasm-grandchild-{}.pid", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let code = "import subprocess,sys,time; child=subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)']); open(sys.argv[1],'w').write(str(child.pid)); time.sleep(60)";
        let python = if cfg!(windows) { "python" } else { "python3" };
        let spec = CommandSpec::new(
            python,
            vec![
                "-c".into(),
                code.into(),
                marker.to_string_lossy().into_owned(),
            ],
        )
        .with_timeout(Duration::from_secs(10));

        let output = exec(&spec).unwrap();
        assert!(output.timed_out);
        let termination = output.termination.expect("termination diagnostics");
        assert_ne!(termination.outcome, TerminationOutcome::Incomplete);

        let grandchild_pid: u32 = std::fs::read_to_string(&marker)
            .expect("parent should record grandchild PID before timeout")
            .parse()
            .expect("grandchild PID should be numeric");
        let deadline = Instant::now() + Duration::from_secs(2);
        while process_exists(grandchild_pid) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            !process_exists(grandchild_pid),
            "grandchild process {grandchild_pid} survived timeout"
        );
        let _ = std::fs::remove_file(marker);
    }

    #[cfg(unix)]
    fn process_exists(pid: u32) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(windows)]
    fn process_exists(pid: u32) -> bool {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .is_ok_and(|output| {
                String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\""))
            })
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
    fn default_stdin_is_immediate_eof() {
        let spec = CommandSpec::new(
            "python",
            vec![
                "-c".into(),
                "import sys; data=sys.stdin.buffer.read(); print(len(data))".into(),
            ],
        )
        .with_timeout(Duration::from_secs(10));
        let output = exec(&spec).unwrap();
        assert!(output.success(), "exec failed: {}", output.summary());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "0");
    }

    #[test]
    fn controlled_stdin_bytes_are_delivered() {
        let spec = CommandSpec::new(
            "python",
            vec![
                "-c".into(),
                "import sys; sys.stdout.buffer.write(sys.stdin.buffer.read())".into(),
            ],
        )
        .with_stdin_bytes(b"controlled input".to_vec());
        let output = exec(&spec).unwrap();
        assert!(output.success());
        assert_eq!(output.stdout, b"controlled input");
    }

    #[test]
    fn sanitized_environment_excludes_parent_secrets() {
        let variable = "SEMASM_TEST_SECRET_TOKEN";
        std::env::set_var(variable, "must-not-leak");
        let spec = CommandSpec::new(
            "python",
            vec![
                "-c".into(),
                format!("import os; print(os.environ.get('{variable}', 'absent'))"),
            ],
        );
        let output = exec(&spec).unwrap();
        std::env::remove_var(variable);
        assert!(output.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "absent");
    }

    #[test]
    fn full_environment_inheritance_is_explicit() {
        let variable = "SEMASM_TEST_INHERITED_VALUE";
        std::env::set_var(variable, "visible-by-opt-in");
        let spec = CommandSpec::new(
            "python",
            vec![
                "-c".into(),
                format!("import os; print(os.environ.get('{variable}', 'absent'))"),
            ],
        )
        .inherit_full_environment();
        let output = exec(&spec).unwrap();
        std::env::remove_var(variable);
        assert!(output.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "visible-by-opt-in"
        );
    }

    #[test]
    fn explicit_environment_does_not_inherit_parent() {
        let values = BTreeMap::from([("SEMASM_EXPLICIT".into(), "yes".into())]);
        let spec = CommandSpec::new("tool", vec![]).with_environment(values);
        let command = build_command(&spec);
        let environment: BTreeMap<_, _> = command
            .get_envs()
            .map(|(name, value)| (name.to_owned(), value.map(ToOwned::to_owned)))
            .collect();
        assert_eq!(
            environment.get(std::ffi::OsStr::new("SEMASM_EXPLICIT")),
            Some(&Some(std::ffi::OsString::from("yes")))
        );
        assert!(!environment.contains_key(std::ffi::OsStr::new("PATH")));
    }

    #[test]
    fn debug_output_redacts_environment_values_and_stdin() {
        let spec = CommandSpec::new("tool", vec![])
            .with_environment(BTreeMap::from([(
                "SERVICE_API_TOKEN".into(),
                "super-secret-value".into(),
            )]))
            .with_stdin_bytes(b"private stdin".to_vec());
        let debug = format!("{spec:?}");
        assert!(!debug.contains("super-secret-value"));
        assert!(!debug.contains("SERVICE_API_TOKEN"));
        assert!(!debug.contains("private stdin"));
        assert!(debug.contains("<redacted>"));
        assert!(debug.contains("length: 13"));
    }

    #[test]
    fn captures_output_under_limit() {
        let spec = CommandSpec::new(
            "python",
            vec![
                "-c".into(),
                "import sys; sys.stdout.buffer.write(b'x' * 1000)".into(),
            ],
        )
        .with_capture_limit(CaptureLimit::new(1024));
        let output = exec(&spec).unwrap();
        assert_eq!(output.stdout.len(), 1000);
        assert_eq!(output.stdout_capture.captured_bytes, 1000);
        assert_eq!(output.stdout_capture.total_bytes, 1000);
        assert!(!output.stdout_capture.truncated);
        assert!(output.stdout_capture.read_error.is_none());
    }

    #[test]
    fn exact_capture_limit_is_not_truncated() {
        let spec = CommandSpec::new(
            "python",
            vec![
                "-c".into(),
                "import sys; sys.stdout.buffer.write(b'x' * 1024)".into(),
            ],
        )
        .with_capture_limit(CaptureLimit::new(1024));
        let output = exec(&spec).unwrap();
        assert_eq!(output.stdout.len(), 1024);
        assert_eq!(output.stdout_capture.total_bytes, 1024);
        assert!(!output.stdout_capture.truncated);
    }

    #[test]
    fn large_flood_is_drained_but_bounded() {
        let spec = CommandSpec::new(
            "python",
            vec![
                "-c".into(),
                "import sys; sys.stdout.buffer.write(b'x' * (1024 * 1024))".into(),
            ],
        )
        .with_capture_limit(CaptureLimit::new(1024));
        let output = exec(&spec).unwrap();
        assert!(output.success());
        assert_eq!(output.stdout.len(), 1024);
        assert_eq!(output.stdout_capture.captured_bytes, 1024);
        assert_eq!(output.stdout_capture.total_bytes, 1024 * 1024);
        assert!(output.stdout_capture.truncated);
        assert!(output.summary().contains("truncated"));
    }

    #[test]
    fn simultaneous_stream_floods_are_independently_bounded() {
        let spec = CommandSpec::new(
            "python",
            vec![
                "-c".into(),
                "import sys; sys.stdout.buffer.write(b'o' * 524288); sys.stderr.buffer.write(b'e' * 524288)".into(),
            ],
        )
        .with_capture_limits(CaptureLimit::new(2048), CaptureLimit::new(4096));
        let output = exec(&spec).unwrap();
        assert!(output.success());
        assert_eq!(output.stdout.len(), 2048);
        assert_eq!(output.stderr.len(), 4096);
        assert_eq!(output.stdout_capture.total_bytes, 524_288);
        assert_eq!(output.stderr_capture.total_bytes, 524_288);
        assert!(output.stdout_capture.truncated);
        assert!(output.stderr_capture.truncated);
    }

    #[test]
    fn invalid_utf8_capture_remains_binary_safe() {
        let spec = CommandSpec::new(
            "python",
            vec![
                "-c".into(),
                "import sys; sys.stdout.buffer.write(bytes([255, 0, 254]))".into(),
            ],
        );
        let output = exec(&spec).unwrap();
        assert_eq!(output.stdout, [255, 0, 254]);
        assert_eq!(output.stdout_capture.total_bytes, 3);
        assert!(!output.stdout_capture.truncated);
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
