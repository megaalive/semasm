//! Command record for build reports.

use std::path::PathBuf;
use std::time::SystemTime;

use serde::Serialize;

use crate::exec::{CommandOutput, CommandSpec};

/// A timestamped, fully-documented command execution, suitable for
/// inclusion in artifact reports.
#[derive(Debug, Clone, Serialize)]
pub struct CommandRecord {
    /// Human-readable label (e.g. `"assemble"`, `"link"`).
    pub label: String,
    /// The command that was executed.
    pub command: RecordedCommand,
    /// Captured output.
    pub output: RecordedOutput,
    /// Unix timestamp (seconds since epoch) when execution started.
    pub started_at: u64,
}

/// Serialisable representation of a command.
#[derive(Debug, Clone, Serialize)]
pub struct RecordedCommand {
    /// Program path or name.
    pub program: String,
    /// Argument array.
    pub args: Vec<String>,
    /// Working directory, if explicitly set.
    pub working_dir: Option<PathBuf>,
    /// Whether the environment was restricted.
    pub env_restricted: bool,
    /// Timeout in seconds.
    pub timeout_secs: f64,
}

/// Serialisable representation of captured output.
#[derive(Debug, Clone, Serialize)]
pub struct RecordedOutput {
    /// Exit code, or `null` if killed / timed out.
    pub exit_code: Option<i32>,
    /// Stdout as lossy UTF-8 (for display). Binary content is preserved
    /// in raw form via the hex field when needed.
    pub stdout: String,
    /// Stderr as lossy UTF-8.
    pub stderr: String,
    /// Wall-clock duration in seconds.
    pub duration_secs: f64,
    /// Whether the process was killed by timeout.
    pub timed_out: bool,
    /// Whether the exit code indicates success (0).
    pub success: bool,
}

impl CommandRecord {
    /// Create a record from a spec and its output.
    #[must_use]
    pub fn new(
        label: impl Into<String>,
        spec: &CommandSpec,
        output: &CommandOutput,
        started_at: SystemTime,
    ) -> Self {
        let epoch = started_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self {
            label: label.into(),
            command: RecordedCommand {
                program: spec.program.clone(),
                args: spec.args.clone(),
                working_dir: spec.working_dir.clone(),
                env_restricted: spec.env_allowlist.is_some(),
                timeout_secs: spec.timeout.as_secs_f64(),
            },
            output: RecordedOutput {
                exit_code: output.exit_code,
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                duration_secs: output.duration.as_secs_f64(),
                timed_out: output.timed_out,
                success: output.success(),
            },
            started_at: epoch,
        }
    }

    /// Create a record with the current time as the start time.
    #[must_use]
    pub fn now(
        label: impl Into<String>,
        spec: &CommandSpec,
        output: &CommandOutput,
    ) -> Self {
        Self::new(label, spec, output, SystemTime::now())
    }
}
