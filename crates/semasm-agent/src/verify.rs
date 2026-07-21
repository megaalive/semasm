//! Immutable verification-plane reports for agent candidate checks.
//!
//! Stages are recorded as separate results and composed once into a
//! [`VerificationReport`]. Nothing is mutated after construction.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::harness::HarnessReport;

pub use semasm_target::ExecutionIsolation;

/// Outcome of a single verification gate.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum GateStatus {
    /// Gate ran and passed.
    Passed,
    /// Gate ran and failed.
    Failed,
    /// Gate was not applicable or intentionally not run.
    Skipped,
}

impl GateStatus {
    /// Stable snake_case label for terminal printers.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

/// Instruction-level coverage counts (unknown ≠ verified).
///
/// Decode and lowering gates always use instruction counts here. Byte-level
/// detail belongs in error messages, not in these fields.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Coverage {
    /// Total instructions presented to the gate.
    pub total: usize,
    /// Instructions with modeled / accepted semantics.
    pub modeled: usize,
    /// Instructions left unknown or unsupported.
    pub unknown: usize,
}

impl Coverage {
    /// Full coverage: every unit modeled, none unknown.
    #[must_use]
    pub fn complete(total: usize) -> Self {
        Self {
            total,
            modeled: total,
            unknown: 0,
        }
    }

    /// Fail-closed placeholder when instruction coverage is unknown.
    #[must_use]
    pub fn unknown_incomplete() -> Self {
        Self {
            total: 0,
            modeled: 0,
            unknown: 1,
        }
    }

    /// Empty coverage for gates that have not run yet.
    #[must_use]
    pub fn not_started() -> Self {
        Self {
            total: 0,
            modeled: 0,
            unknown: 0,
        }
    }

    /// Percent of units modeled, floored to `u8` (100 when `total == 0`).
    #[must_use]
    pub fn percent_modeled(self) -> u8 {
        let Some(percent) = self
            .modeled
            .checked_mul(100)
            .and_then(|n| n.checked_div(self.total))
        else {
            return 100;
        };
        u8::try_from(percent.min(100)).unwrap_or(100)
    }
}

/// Static semantic gates applied to the candidate relocatable object.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct SemanticGates {
    /// Relocatable object, exports, and import policy.
    pub object_policy: GateStatus,
    /// Total executable bytes examined (metadata; not coverage units).
    pub executable_bytes: usize,
    /// Decode coverage as instruction counts.
    pub decode: Coverage,
    /// Lowering coverage over decoded instructions.
    pub lowering: Coverage,
    /// ABI cleanliness gate.
    pub abi: GateStatus,
    /// Capability policy (e.g. no syscall in the candidate).
    pub capability: GateStatus,
}

impl SemanticGates {
    /// True when every semantic sub-gate passed with full coverage.
    #[must_use]
    pub fn all_passed(&self) -> bool {
        self.object_policy == GateStatus::Passed
            && self.abi == GateStatus::Passed
            && self.capability == GateStatus::Passed
            && self.decode.unknown == 0
            && self.lowering.unknown == 0
            && self.decode.modeled == self.decode.total
            && self.lowering.modeled == self.lowering.total
    }

    /// Build a fail-closed gate snapshot from a structured stage error.
    ///
    /// Stages that did not run are marked [`GateStatus::Skipped`]. Coverage
    /// fields use instruction counts from the error when present; otherwise a
    /// fail-closed unknown placeholder is used for the failing coverage stage.
    #[must_use]
    pub fn from_error(error: &SemanticGateError, executable_bytes: usize) -> Self {
        let decode = error.decode.unwrap_or_else(Coverage::not_started);
        let lowering = error.lowering.unwrap_or_else(Coverage::not_started);

        match error.stage {
            "target" | "object" => Self {
                object_policy: GateStatus::Failed,
                executable_bytes,
                decode: Coverage::not_started(),
                lowering: Coverage::not_started(),
                abi: GateStatus::Skipped,
                capability: GateStatus::Skipped,
            },
            "decode" => Self {
                object_policy: GateStatus::Passed,
                executable_bytes,
                decode: error.decode.unwrap_or_else(Coverage::unknown_incomplete),
                lowering: Coverage::not_started(),
                abi: GateStatus::Skipped,
                capability: GateStatus::Skipped,
            },
            "lowering" => Self {
                object_policy: GateStatus::Passed,
                executable_bytes,
                decode: if error.decode.is_some() {
                    decode
                } else {
                    Coverage::not_started()
                },
                lowering: error.lowering.unwrap_or_else(Coverage::unknown_incomplete),
                abi: GateStatus::Skipped,
                capability: GateStatus::Skipped,
            },
            "abi" => Self {
                object_policy: GateStatus::Passed,
                executable_bytes,
                decode,
                lowering,
                abi: GateStatus::Failed,
                capability: GateStatus::Skipped,
            },
            "capability" => Self {
                object_policy: GateStatus::Passed,
                executable_bytes,
                decode,
                lowering,
                abi: GateStatus::Passed,
                capability: GateStatus::Failed,
            },
            _ => Self {
                object_policy: GateStatus::Failed,
                executable_bytes,
                decode: error.decode.unwrap_or_else(Coverage::unknown_incomplete),
                lowering: error.lowering.unwrap_or_else(Coverage::not_started),
                abi: GateStatus::Skipped,
                capability: GateStatus::Skipped,
            },
        }
    }
}

/// Linked-image object-format gate (run only after link).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ExecutableGate {
    /// Whether the linked image is an executable container.
    pub status: GateStatus,
}

impl ExecutableGate {
    /// Construct a passed executable gate.
    #[must_use]
    pub fn passed() -> Self {
        Self {
            status: GateStatus::Passed,
        }
    }

    /// Construct a failed executable gate.
    #[must_use]
    pub fn failed() -> Self {
        Self {
            status: GateStatus::Failed,
        }
    }

    /// Construct a skipped executable gate (semantic failed before link).
    #[must_use]
    pub fn skipped() -> Self {
        Self {
            status: GateStatus::Skipped,
        }
    }
}

/// Overall verification outcome after composing immutable stage results.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum VerificationStatus {
    /// Static gates and all behavioral vectors passed.
    Verified,
    /// A static semantic gate failed.
    SemanticFailed,
    /// Linked image failed the executable-container policy.
    ExecutableFailed,
    /// Execution ran but one or more vectors failed.
    BehaviorFailed,
    /// Static gates passed; execution was not opted in.
    ExecutionDenied,
}

impl VerificationStatus {
    /// Stable snake_case label for terminal printers.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::SemanticFailed => "semantic_failed",
            Self::ExecutableFailed => "executable_failed",
            Self::BehaviorFailed => "behavior_failed",
            Self::ExecutionDenied => "execution_denied",
        }
    }
}

/// Full agent verification report: one compose, no pending mutation.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct VerificationReport {
    /// Agent verification-report schema version (`MAJOR.MINOR`).
    pub schema_version: String,
    /// Aggregate status derived from stage results.
    pub status: VerificationStatus,
    /// Target identity name.
    pub target: String,
    /// Routine symbol under verification.
    pub routine_symbol: String,
    /// How execution was isolated (or that only static gates ran).
    pub isolation: ExecutionIsolation,
    /// Static semantic gate results.
    pub semantic: SemanticGates,
    /// Post-link executable gate result.
    pub executable: ExecutableGate,
    /// Behavioral harness results when execution was allowed.
    pub behavior: Option<HarnessReport>,
}

/// Current experimental schema version for [`VerificationReport`] JSON.
pub const VERIFICATION_REPORT_SCHEMA_VERSION: &str = "0.1";

impl VerificationReport {
    /// Compose an immutable report from completed stage results.
    ///
    /// Callers must not pass a half-filled semantic record and mutate it
    /// later; build `semantic` and `executable` fully before calling.
    #[must_use]
    pub fn from_parts(
        target: String,
        routine_symbol: String,
        semantic: SemanticGates,
        executable: ExecutableGate,
        behavior: Option<HarnessReport>,
        isolation: ExecutionIsolation,
    ) -> Self {
        let status = if !semantic.all_passed() {
            VerificationStatus::SemanticFailed
        } else if executable.status == GateStatus::Failed {
            VerificationStatus::ExecutableFailed
        } else if behavior.is_none() {
            VerificationStatus::ExecutionDenied
        } else if behavior.as_ref().is_some_and(|report| report.all_passed) {
            VerificationStatus::Verified
        } else {
            VerificationStatus::BehaviorFailed
        };

        Self {
            schema_version: VERIFICATION_REPORT_SCHEMA_VERSION.to_string(),
            status,
            target,
            routine_symbol,
            isolation,
            semantic,
            executable,
            behavior,
        }
    }
}

/// Structured failure from a static semantic gate stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticGateError {
    /// Gate stage that failed (`target`, `object`, `decode`, `lowering`, `abi`, `capability`, …).
    pub stage: &'static str,
    /// Human-readable explanation.
    pub message: String,
    /// Partial decode coverage when available at the failure point.
    pub decode: Option<Coverage>,
    /// Partial lowering coverage when available at the failure point.
    pub lowering: Option<Coverage>,
}

impl SemanticGateError {
    /// Build an error for a named stage without partial coverage.
    #[must_use]
    pub fn new(stage: &'static str, message: impl Into<String>) -> Self {
        Self {
            stage,
            message: message.into(),
            decode: None,
            lowering: None,
        }
    }
}

impl fmt::Display for SemanticGateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.stage, self.message)
    }
}

impl std::error::Error for SemanticGateError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::{HarnessReport, VectorResult};

    fn passed_semantic() -> SemanticGates {
        SemanticGates {
            object_policy: GateStatus::Passed,
            executable_bytes: 32,
            decode: Coverage::complete(4),
            lowering: Coverage::complete(4),
            abi: GateStatus::Passed,
            capability: GateStatus::Passed,
        }
    }

    #[test]
    fn compose_execution_denied_when_behavior_absent() {
        let report = VerificationReport::from_parts(
            "x86_64-unknown-linux-gnu".into(),
            "count_byte".into(),
            passed_semantic(),
            ExecutableGate::passed(),
            None,
            ExecutionIsolation::StaticOnly,
        );
        assert_eq!(report.status, VerificationStatus::ExecutionDenied);
        assert!(report.behavior.is_none());
    }

    #[test]
    fn compose_verified_when_all_vectors_pass() {
        let behavior = HarnessReport {
            cases: vec![VectorResult {
                name: "empty".into(),
                passed: true,
                expected: "0".into(),
                observed: "0".into(),
            }],
            all_passed: true,
        };
        let report = VerificationReport::from_parts(
            "x86_64-unknown-linux-gnu".into(),
            "count_byte".into(),
            passed_semantic(),
            ExecutableGate::passed(),
            Some(behavior),
            ExecutionIsolation::QemuUser,
        );
        assert_eq!(report.status, VerificationStatus::Verified);
    }

    #[test]
    fn compose_behavior_failed_when_vector_fails() {
        let behavior = HarnessReport {
            cases: vec![VectorResult {
                name: "empty".into(),
                passed: false,
                expected: "0".into(),
                observed: "1".into(),
            }],
            all_passed: false,
        };
        let report = VerificationReport::from_parts(
            "x86_64-unknown-linux-gnu".into(),
            "count_byte".into(),
            passed_semantic(),
            ExecutableGate::passed(),
            Some(behavior),
            ExecutionIsolation::QemuUser,
        );
        assert_eq!(report.status, VerificationStatus::BehaviorFailed);
    }

    #[test]
    fn compose_executable_failed() {
        let report = VerificationReport::from_parts(
            "x86_64-unknown-linux-gnu".into(),
            "count_byte".into(),
            passed_semantic(),
            ExecutableGate::failed(),
            None,
            ExecutionIsolation::StaticOnly,
        );
        assert_eq!(report.status, VerificationStatus::ExecutableFailed);
    }

    #[test]
    fn compose_semantic_failed_when_coverage_incomplete() {
        let mut semantic = passed_semantic();
        semantic.lowering = Coverage {
            total: 4,
            modeled: 3,
            unknown: 1,
        };
        let report = VerificationReport::from_parts(
            "x86_64-unknown-linux-gnu".into(),
            "count_byte".into(),
            semantic,
            ExecutableGate::passed(),
            None,
            ExecutionIsolation::StaticOnly,
        );
        assert_eq!(report.status, VerificationStatus::SemanticFailed);
        assert_eq!(report.semantic.lowering.percent_modeled(), 75);
    }

    #[test]
    fn json_keys_are_stable() {
        let report = VerificationReport::from_parts(
            "x86_64-unknown-linux-gnu".into(),
            "count_byte".into(),
            passed_semantic(),
            ExecutableGate::passed(),
            None,
            ExecutionIsolation::StaticOnly,
        );
        let value = serde_json::to_value(&report).unwrap();
        assert!(value.get("semantic").is_some());
        assert!(value.get("executable").is_some());
        assert!(value.get("behavior").is_some());
        assert_eq!(value["schema_version"], VERIFICATION_REPORT_SCHEMA_VERSION);
        assert_eq!(value["status"], "execution_denied");
        assert_eq!(value["semantic"]["abi"], "passed");
        assert_eq!(value["semantic"]["lowering"]["unknown"], 0);
    }

    #[test]
    fn from_error_object_stage_skips_later_gates() {
        let error = SemanticGateError::new("object", "not relocatable");
        let gates = SemanticGates::from_error(&error, 0);
        assert_eq!(gates.object_policy, GateStatus::Failed);
        assert_eq!(gates.abi, GateStatus::Skipped);
        assert_eq!(gates.capability, GateStatus::Skipped);
        assert!(!gates.all_passed());
        let report = VerificationReport::from_parts(
            "x86_64-unknown-linux-gnu".into(),
            "count_byte".into(),
            gates,
            ExecutableGate::skipped(),
            None,
            ExecutionIsolation::StaticOnly,
        );
        assert_eq!(report.status, VerificationStatus::SemanticFailed);
    }

    #[test]
    fn from_error_lowering_keeps_partial_coverage() {
        let error = SemanticGateError {
            stage: "lowering",
            message: "unsupported `foo`".into(),
            decode: Some(Coverage::complete(4)),
            lowering: Some(Coverage {
                total: 4,
                modeled: 2,
                unknown: 2,
            }),
        };
        let gates = SemanticGates::from_error(&error, 64);
        assert_eq!(gates.object_policy, GateStatus::Passed);
        assert_eq!(gates.decode, Coverage::complete(4));
        assert_eq!(gates.lowering.modeled, 2);
        assert_eq!(gates.lowering.unknown, 2);
        assert_eq!(gates.abi, GateStatus::Skipped);
        assert_eq!(gates.executable_bytes, 64);
        assert!(!gates.all_passed());
    }

    #[test]
    fn from_error_abi_marks_abi_failed() {
        let error = SemanticGateError {
            stage: "abi",
            message: "stack misaligned".into(),
            decode: Some(Coverage::complete(3)),
            lowering: Some(Coverage::complete(3)),
        };
        let gates = SemanticGates::from_error(&error, 16);
        assert_eq!(gates.abi, GateStatus::Failed);
        assert_eq!(gates.capability, GateStatus::Skipped);
        assert_eq!(gates.decode.total, 3);
        assert!(!gates.all_passed());
    }

    #[test]
    fn from_error_decode_without_coverage_is_fail_closed() {
        let error = SemanticGateError::new(
            "decode",
            "decode coverage incomplete: decoded 3 of 8 executable bytes",
        );
        let gates = SemanticGates::from_error(&error, 8);
        assert_eq!(gates.decode, Coverage::unknown_incomplete());
        assert_eq!(gates.executable_bytes, 8);
        assert!(!gates.all_passed());
    }

    #[test]
    fn compose_executable_failed_json_status() {
        let report = VerificationReport::from_parts(
            "x86_64-unknown-linux-gnu".into(),
            "count_byte".into(),
            passed_semantic(),
            ExecutableGate::failed(),
            None,
            ExecutionIsolation::StaticOnly,
        );
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["status"], "executable_failed");
    }
}
