//! Unix process-group setup and termination.

use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use std::os::unix::process::CommandExt;

use super::super::{TerminationInfo, TerminationOutcome, TerminationReason};

pub(crate) struct ProcessTree {
    process_group: String,
}

pub(crate) fn configure(command: &mut Command) {
    command.process_group(0);
}

impl ProcessTree {
    pub(crate) fn attach(child: &Child) -> Result<Self, String> {
        Ok(Self {
            process_group: format!("-{}", child.id()),
        })
    }

    pub(crate) fn terminate(&mut self, child: &mut Child) -> TerminationInfo {
        let term_sent = signal_group("-TERM", &self.process_group);
        let deadline = Instant::now() + Duration::from_millis(200);
        while Instant::now() < deadline && group_exists(&self.process_group) {
            thread::sleep(Duration::from_millis(10));
        }

        if !group_exists(&self.process_group) {
            return TerminationInfo {
                reason: TerminationReason::Timeout,
                outcome: TerminationOutcome::Graceful,
                detail: "process group exited after SIGTERM".into(),
            };
        }

        if signal_group("-KILL", &self.process_group) {
            return TerminationInfo {
                reason: TerminationReason::Timeout,
                outcome: TerminationOutcome::Forced,
                detail: "process group required SIGKILL".into(),
            };
        }

        let _ = child.kill();
        TerminationInfo {
            reason: TerminationReason::Timeout,
            outcome: TerminationOutcome::Incomplete,
            detail: if term_sent {
                "SIGTERM did not stop the group and SIGKILL failed"
            } else {
                "failed to signal the process group"
            }
            .into(),
        }
    }
}

fn signal_group(signal: &str, process_group: &str) -> bool {
    Command::new("kill")
        .args([signal, "--", process_group])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn group_exists(process_group: &str) -> bool {
    signal_group("-0", process_group)
}
