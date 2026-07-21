//! Immutable verification-plane reports for agent candidate checks.
//!
//! Stages are recorded as separate results and composed once into a
//! [`VerificationReport`]. Nothing is mutated after construction.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::harness::HarnessReport;

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

/// Instruction- or byte-level coverage counts (unknown ≠ verified).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Coverage {
    /// Total units presented to the gate.
    pub total: usize,
    /// Units with modeled / accepted semantics.
    pub modeled: usize,
    /// Units left unknown or unsupported.
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
    /// Total executable bytes examined.
    pub executable_bytes: usize,
    /// Decode coverage over executable bytes (as instruction counts).
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
    /// Aggregate status derived from stage results.
    pub status: VerificationStatus,
    /// Target identity name.
    pub target: String,
    /// Routine symbol under verification.
    pub routine_symbol: String,
    /// Static semantic gate results.
    pub semantic: SemanticGates,
    /// Post-link executable gate result.
    pub executable: ExecutableGate,
    /// Behavioral harness results when execution was allowed.
    pub behavior: Option<HarnessReport>,
}

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
            status,
            target,
            routine_symbol,
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
        );
        let value = serde_json::to_value(&report).unwrap();
        assert!(value.get("semantic").is_some());
        assert!(value.get("executable").is_some());
        assert!(value.get("behavior").is_some());
        assert_eq!(value["status"], "execution_denied");
        assert_eq!(value["semantic"]["abi"], "passed");
        assert_eq!(value["semantic"]["lowering"]["unknown"], 0);
    }
}
