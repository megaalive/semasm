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
            .map_or(0, |d| d.as_secs());
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
    pub fn now(label: impl Into<String>, spec: &CommandSpec, output: &CommandOutput) -> Self {
        Self::new(label, spec, output, SystemTime::now())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use std::time::Duration;

    fn sample_spec() -> CommandSpec {
        CommandSpec::new("nasm", vec!["-f".into(), "elf64".into(), "exit.asm".into()])
            .with_timeout(Duration::from_secs(15))
            .with_env_allowlist(vec!["PATH".into()])
    }

    fn sample_output() -> CommandOutput {
        CommandOutput {
            exit_status: None,
            exit_code: Some(0),
            stdout: b"nasm: warning: ...\n".to_vec(),
            stderr: Vec::new(),
            duration: Duration::from_millis(123),
            timed_out: false,
        }
    }

    #[test]
    fn record_roundtrip() {
        let spec = sample_spec();
        let output = sample_output();
        let started = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let rec = CommandRecord::new("assemble", &spec, &output, started);
        assert_eq!(rec.label, "assemble");
        assert_eq!(rec.command.program, "nasm");
        assert_eq!(rec.command.args, vec!["-f", "elf64", "exit.asm"]);
        assert!(rec.command.env_restricted);
        assert!((rec.command.timeout_secs - 15.0).abs() < 1e-9);
        assert_eq!(rec.output.exit_code, Some(0));
        assert!(rec.output.stdout.contains("nasm: warning"));
        assert!((rec.output.duration_secs - 0.123).abs() < 1e-9);
        assert!(!rec.output.timed_out);
        assert!(rec.output.success);
        assert_eq!(rec.started_at, 1_700_000_000);
    }

    #[test]
    fn record_now() {
        let spec = sample_spec();
        let output = sample_output();
        let rec = CommandRecord::now("link", &spec, &output);
        assert_eq!(rec.label, "link");
        // Timestamp should be close to "now".
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        assert!(rec.started_at <= now);
        assert!(rec.started_at > now - 2); // at most 2 seconds ago.
    }

    #[test]
    fn record_json_serializes() {
        let spec = sample_spec();
        let output = sample_output();
        let rec = CommandRecord::now("test", &spec, &output);
        let json = serde_json::to_string_pretty(&rec).unwrap();
        assert!(json.contains("\"program\": \"nasm\""));
        assert!(json.contains("\"label\": \"test\""));
        assert!(json.contains("\"exit_code\": 0"));
        assert!(json.contains("\"success\": true"));
    }

    #[test]
    fn record_non_zero_exit() {
        let output = CommandOutput {
            exit_status: None,
            exit_code: Some(1),
            stdout: Vec::new(),
            stderr: b"error: something failed\n".to_vec(),
            duration: Duration::from_millis(50),
            timed_out: false,
        };
        let rec = CommandRecord::now("fail", &sample_spec(), &output);
        assert!(!rec.output.success);
        assert_eq!(rec.output.exit_code, Some(1));
        assert!(rec.output.stderr.contains("something failed"));
    }

    #[test]
    fn record_timed_out() {
        let output = CommandOutput {
            exit_status: None,
            exit_code: None,
            stdout: b"partial output\n".to_vec(),
            stderr: Vec::new(),
            duration: Duration::from_secs(30),
            timed_out: true,
        };
        let rec = CommandRecord::now("timeout", &sample_spec(), &output);
        assert!(rec.output.timed_out);
        assert_eq!(rec.output.exit_code, None);
        assert!(!rec.output.success);
    }

    #[test]
    fn record_no_env_restriction() {
        let spec = CommandSpec::new("true", vec![]);
        let output = sample_output();
        let rec = CommandRecord::now("open", &spec, &output);
        assert!(!rec.command.env_restricted);
    }

    #[test]
    fn record_working_dir() {
        let spec = sample_spec().with_working_dir("/tmp/build");
        let output = sample_output();
        let rec = CommandRecord::now("build", &spec, &output);
        assert_eq!(rec.command.working_dir, Some(PathBuf::from("/tmp/build")),);
    }
}
