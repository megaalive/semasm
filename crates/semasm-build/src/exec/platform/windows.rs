//! Windows process-tree ownership using a Job Object.

use std::os::windows::io::AsRawHandle;
use std::process::{Child, Command, Stdio};

use win32job::{ExtendedLimitInfo, Job};

use super::super::{TerminationInfo, TerminationOutcome, TerminationReason};

pub(crate) fn configure(_command: &mut Command) {}

pub(crate) struct ProcessTree {
    job: Option<Job>,
}

impl ProcessTree {
    pub(crate) fn attach(child: &Child) -> Result<Self, String> {
        let mut limits = ExtendedLimitInfo::new();
        limits.limit_kill_on_job_close();
        let job = Job::create_with_limit_info(&limits).map_err(|error| error.to_string())?;
        if let Err(error) = job.assign_process(child.as_raw_handle() as isize) {
            // Assignment failure leaves the process outside the Job Object.
            // Best-effort taskkill is the only safe tree fallback exposed by
            // Windows without adding unsafe process enumeration here.
            let _ = Command::new("taskkill")
                .args(["/PID", &child.id().to_string(), "/T", "/F"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            return Err(error.to_string());
        }
        Ok(Self { job: Some(job) })
    }

    pub(crate) fn terminate(&mut self, child: &mut Child) -> TerminationInfo {
        if self.job.take().is_some() {
            let _ = child.wait();
            TerminationInfo {
                reason: TerminationReason::Timeout,
                outcome: TerminationOutcome::Forced,
                detail: "process tree terminated by closing its Windows Job Object".into(),
            }
        } else {
            let _ = child.kill();
            TerminationInfo {
                reason: TerminationReason::Timeout,
                outcome: TerminationOutcome::Incomplete,
                detail: "Windows Job Object was unavailable; direct-child fallback used".into(),
            }
        }
    }
}
