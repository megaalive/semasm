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
    /// Control-flow leaf policy (direct edges only on golden path).
    ///
    /// Absent in older reports; deserializes as [`GateStatus::Passed`].
    #[serde(default = "gate_status_passed")]
    pub control: GateStatus,
    /// Memory effects leaf policy (read-only buffer leaves must not store).
    ///
    /// Absent in older reports; deserializes as [`GateStatus::Passed`].
    #[serde(default = "gate_status_passed")]
    pub memory: GateStatus,
}

fn gate_status_passed() -> GateStatus {
    GateStatus::Passed
}

impl SemanticGates {
    /// True when every required semantic sub-gate passed with full coverage.
    ///
    /// [`GateStatus::Skipped`] is allowed for `control` / `memory` when those
    /// leaf policies are not implemented for the target (AArch64 / RV64 today).
    #[must_use]
    pub fn all_passed(&self) -> bool {
        fn required(status: GateStatus) -> bool {
            matches!(status, GateStatus::Passed)
        }
        fn leaf_ok(status: GateStatus) -> bool {
            matches!(status, GateStatus::Passed | GateStatus::Skipped)
        }
        required(self.object_policy)
            && required(self.abi)
            && required(self.capability)
            && leaf_ok(self.control)
            && leaf_ok(self.memory)
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
                control: GateStatus::Skipped,
                memory: GateStatus::Skipped,
            },
            "decode" => Self {
                object_policy: GateStatus::Passed,
                executable_bytes,
                decode: error.decode.unwrap_or_else(Coverage::unknown_incomplete),
                lowering: Coverage::not_started(),
                abi: GateStatus::Skipped,
                capability: GateStatus::Skipped,
                control: GateStatus::Skipped,
                memory: GateStatus::Skipped,
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
                control: GateStatus::Skipped,
                memory: GateStatus::Skipped,
            },
            "abi" => Self {
                object_policy: GateStatus::Passed,
                executable_bytes,
                decode,
                lowering,
                abi: GateStatus::Failed,
                capability: GateStatus::Skipped,
                control: GateStatus::Skipped,
                memory: GateStatus::Skipped,
            },
            "capability" => Self {
                object_policy: GateStatus::Passed,
                executable_bytes,
                decode,
                lowering,
                abi: GateStatus::Passed,
                capability: GateStatus::Failed,
                control: GateStatus::Skipped,
                memory: GateStatus::Skipped,
            },
            "cfg" | "control" => Self {
                object_policy: GateStatus::Passed,
                executable_bytes,
                decode,
                lowering,
                abi: GateStatus::Passed,
                capability: GateStatus::Passed,
                control: GateStatus::Failed,
                memory: GateStatus::Skipped,
            },
            "memory" => Self {
                object_policy: GateStatus::Passed,
                executable_bytes,
                decode,
                lowering,
                abi: GateStatus::Passed,
                capability: GateStatus::Passed,
                control: GateStatus::Passed,
                memory: GateStatus::Failed,
            },
            _ => Self {
                object_policy: GateStatus::Failed,
                executable_bytes,
                decode: error.decode.unwrap_or_else(Coverage::unknown_incomplete),
                lowering: error.lowering.unwrap_or_else(Coverage::not_started),
                abi: GateStatus::Skipped,
                capability: GateStatus::Skipped,
                control: GateStatus::Skipped,
                memory: GateStatus::Skipped,
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
    /// Tool identity string (`semasm MAJOR.MINOR.PATCH`).
    pub tool_version: String,
    /// SHA-256 digest of the contract file bytes (`sha256:` + hex).
    pub contract_digest: String,
    /// SHA-256 digest of the candidate source bytes (`sha256:` + hex).
    pub source_digest: String,
    /// Static semantic gate results.
    pub semantic: SemanticGates,
    /// Post-link executable gate result.
    pub executable: ExecutableGate,
    /// Behavioral harness results when execution was allowed.
    pub behavior: Option<HarnessReport>,
    /// Named, versioned behavioral oracle for the recognized routine shape.
    ///
    /// Present when the contract matches a builtin harness profile. Equality
    /// behavior for golden shapes (e.g. count-equal-byte) is proven by this
    /// oracle plus vectors — not by weak contract `ensures` alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behavior_oracle: Option<BehaviorOracle>,
}

/// Named deterministic behavioral oracle attached to a verification report.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct BehaviorOracle {
    /// Stable oracle id (`builtin.buffer.count_equal_u8`, …).
    pub id: String,
    /// Integer profile version for this oracle id.
    pub version: u32,
    /// Contract path or basename supplied to verify.
    pub contract: String,
    /// Short SHA-256 of contract bytes.
    pub contract_hash: String,
    /// Raw `ensures` expressions from the contract (may be weaker than the claim).
    pub contract_ensures: Vec<String>,
    /// How equality/behavior was proven for this shape.
    pub proof_basis: ProofBasis,
    /// Human-readable claim checked by the oracle (not a formal ensures AST).
    pub claim: String,
    /// Vectors that passed (0 when execution was not run).
    pub vectors_passed: usize,
    /// Vectors that failed (0 when execution was not run).
    pub vectors_failed: usize,
    /// Total vectors planned or evaluated.
    pub vectors_total: usize,
    /// Deterministic hash over oracle identity and vector evidence.
    pub evidence_hash: String,
}

/// How a recognized shape's behavioral claim was established.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ProofBasis {
    /// Named builtin oracle plus synthesized/evaluated vectors — not contract alone.
    OracleAndVectors,
}

impl ProofBasis {
    /// Stable snake_case label for printers.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OracleAndVectors => "oracle_and_vectors",
        }
    }
}

/// Current experimental schema version for [`VerificationReport`] JSON.
pub const VERIFICATION_REPORT_SCHEMA_VERSION: &str = "0.4";

/// Prefixed SHA-256 digest for controller provenance (`sha256:` + lowercase hex).
#[must_use]
pub fn sha256_digest_prefixed(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(7 + 64);
    out.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

impl VerificationReport {
    /// Compose an immutable report from completed stage results.
    ///
    /// Callers must not pass a half-filled semantic record and mutate it
    /// later; build `semantic` and `executable` fully before calling.
    ///
    /// Digests default to empty; CLI callers should attach them with
    /// [`Self::with_digests`].
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
            tool_version: format!("semasm {}", semasm_core::SEMASM_VERSION),
            contract_digest: String::new(),
            source_digest: String::new(),
            semantic,
            executable,
            behavior,
            behavior_oracle: None,
        }
    }

    /// Attach contract and source content digests for controller consumers.
    #[must_use]
    pub fn with_digests(mut self, contract_digest: String, source_digest: String) -> Self {
        self.contract_digest = contract_digest;
        self.source_digest = source_digest;
        self
    }

    /// Attach a named behavioral oracle (fluent builder for callers).
    #[must_use]
    pub fn with_behavior_oracle(mut self, oracle: BehaviorOracle) -> Self {
        self.behavior_oracle = Some(oracle);
        self
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
            control: GateStatus::Passed,
            memory: GateStatus::Passed,
        }
    }

    #[test]
    fn all_passed_allows_skipped_control_and_memory_leaves() {
        let mut gates = passed_semantic();
        gates.control = GateStatus::Skipped;
        gates.memory = GateStatus::Skipped;
        assert!(gates.all_passed());
        gates.control = GateStatus::Failed;
        assert!(!gates.all_passed());
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
        )
        .with_digests(
            sha256_digest_prefixed(b"contract"),
            sha256_digest_prefixed(b"source"),
        );
        let value = serde_json::to_value(&report).unwrap();
        assert!(value.get("semantic").is_some());
        assert!(value.get("executable").is_some());
        assert!(value.get("behavior").is_some());
        assert_eq!(value["schema_version"], VERIFICATION_REPORT_SCHEMA_VERSION);
        assert!(value["tool_version"]
            .as_str()
            .is_some_and(|v| v.starts_with("semasm ")));
        assert!(value["contract_digest"]
            .as_str()
            .is_some_and(|v| v.starts_with("sha256:")));
        assert!(value["source_digest"]
            .as_str()
            .is_some_and(|v| v.starts_with("sha256:")));
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

    #[test]
    fn golden_execution_denied_report_deserializes() {
        let json = include_str!("../fixtures/verification-report-count_byte.execution_denied.json");
        let report: VerificationReport =
            serde_json::from_str(json).expect("golden VerificationReport must deserialize");
        assert_eq!(report.schema_version, VERIFICATION_REPORT_SCHEMA_VERSION);
        assert_eq!(report.status, VerificationStatus::ExecutionDenied);
        assert_eq!(report.target, "x86_64-unknown-linux-gnu");
        assert_eq!(report.routine_symbol, "count_byte");
        assert_eq!(report.isolation, ExecutionIsolation::StaticOnly);
        assert!(report.tool_version.starts_with("semasm "));
        assert!(report.contract_digest.starts_with("sha256:"));
        assert_eq!(report.contract_digest.len(), 7 + 64);
        assert!(report.source_digest.starts_with("sha256:"));
        assert_eq!(report.source_digest.len(), 7 + 64);
        assert!(report.behavior.is_none());
        let oracle = report.behavior_oracle.expect("golden includes oracle");
        assert_eq!(oracle.id, "builtin.buffer.count_equal_u8");
        assert_eq!(oracle.version, 2);
        assert_eq!(oracle.proof_basis, ProofBasis::OracleAndVectors);
    }

    #[test]
    fn golden_verified_report_deserializes() {
        let json = include_str!("../fixtures/verification-report-count_byte.verified.json");
        let report: VerificationReport =
            serde_json::from_str(json).expect("verified golden must deserialize");
        assert_eq!(report.schema_version, VERIFICATION_REPORT_SCHEMA_VERSION);
        assert_eq!(report.status, VerificationStatus::Verified);
        assert_eq!(report.isolation, ExecutionIsolation::NativeHost);
        let behavior = report.behavior.expect("verified golden includes behavior");
        assert!(behavior.all_passed);
        assert!(!behavior.cases.is_empty());
        assert!(behavior.cases.iter().all(|c| c.passed));
        let oracle = report.behavior_oracle.expect("oracle present");
        assert!(oracle.vectors_passed > 0);
        assert_eq!(oracle.vectors_failed, 0);
    }

    #[test]
    fn golden_sum_i64_execution_denied_report_deserializes() {
        let json = include_str!("../fixtures/verification-report-sum_i64.execution_denied.json");
        let report: VerificationReport =
            serde_json::from_str(json).expect("sum_i64 golden must deserialize");
        assert_eq!(report.schema_version, VERIFICATION_REPORT_SCHEMA_VERSION);
        assert_eq!(report.status, VerificationStatus::ExecutionDenied);
        assert_eq!(report.routine_symbol, "sum_i64");
        assert_eq!(report.isolation, ExecutionIsolation::StaticOnly);
        assert!(report.behavior.is_none());
        let oracle = report.behavior_oracle.expect("golden includes oracle");
        assert_eq!(oracle.id, "builtin.buffer.wrapping_sum_i64");
        assert_eq!(oracle.version, 2);
        assert_eq!(oracle.proof_basis, ProofBasis::OracleAndVectors);
        assert!(oracle.contract_ensures.iter().any(|e| e == "true"));
        assert!(oracle.claim.contains("wrapping sum"));
    }

    #[test]
    fn golden_sum_i64_verified_report_deserializes() {
        let json = include_str!("../fixtures/verification-report-sum_i64.verified.json");
        let report: VerificationReport =
            serde_json::from_str(json).expect("sum_i64 verified golden must deserialize");
        assert_eq!(report.schema_version, VERIFICATION_REPORT_SCHEMA_VERSION);
        assert_eq!(report.status, VerificationStatus::Verified);
        assert_eq!(report.routine_symbol, "sum_i64");
        assert_eq!(report.isolation, ExecutionIsolation::NativeHost);
        let behavior = report.behavior.expect("verified golden includes behavior");
        assert!(behavior.all_passed);
        assert!(!behavior.cases.is_empty());
        assert!(behavior.cases.iter().all(|c| c.passed));
        let oracle = report.behavior_oracle.expect("oracle present");
        assert_eq!(oracle.id, "builtin.buffer.wrapping_sum_i64");
        assert_eq!(oracle.version, 2);
        assert_eq!(oracle.proof_basis, ProofBasis::OracleAndVectors);
        assert!(oracle.vectors_passed > 0);
        assert_eq!(oracle.vectors_failed, 0);
    }

    #[test]
    fn golden_min_usize_execution_denied_report_deserializes() {
        let json = include_str!("../fixtures/verification-report-min_usize.execution_denied.json");
        let report: VerificationReport =
            serde_json::from_str(json).expect("min_usize golden must deserialize");
        assert_eq!(report.schema_version, VERIFICATION_REPORT_SCHEMA_VERSION);
        assert_eq!(report.status, VerificationStatus::ExecutionDenied);
        assert_eq!(report.routine_symbol, "min_usize");
        assert_eq!(report.isolation, ExecutionIsolation::StaticOnly);
        assert!(report.behavior.is_none());
        let oracle = report.behavior_oracle.expect("golden includes oracle");
        assert_eq!(oracle.id, "builtin.pure_int.binary_usize");
        assert_eq!(oracle.version, 2);
        assert_eq!(oracle.proof_basis, ProofBasis::OracleAndVectors);
        assert!(oracle.claim.contains("min"));
    }

    #[test]
    fn golden_min_usize_verified_report_deserializes() {
        let json = include_str!("../fixtures/verification-report-min_usize.verified.json");
        let report: VerificationReport =
            serde_json::from_str(json).expect("min_usize verified golden must deserialize");
        assert_eq!(report.schema_version, VERIFICATION_REPORT_SCHEMA_VERSION);
        assert_eq!(report.status, VerificationStatus::Verified);
        assert_eq!(report.routine_symbol, "min_usize");
        assert_eq!(report.isolation, ExecutionIsolation::NativeHost);
        let behavior = report.behavior.expect("verified golden includes behavior");
        assert!(behavior.all_passed);
        assert!(!behavior.cases.is_empty());
        assert!(behavior.cases.iter().all(|c| c.passed));
        let oracle = report.behavior_oracle.expect("oracle present");
        assert_eq!(oracle.id, "builtin.pure_int.binary_usize");
        assert_eq!(oracle.version, 2);
        assert!(oracle.claim.contains("min"));
        assert!(oracle.vectors_passed > 0);
        assert_eq!(oracle.vectors_failed, 0);
    }

    #[test]
    fn golden_max_usize_execution_denied_report_deserializes() {
        let json = include_str!("../fixtures/verification-report-max_usize.execution_denied.json");
        let report: VerificationReport =
            serde_json::from_str(json).expect("max_usize golden must deserialize");
        assert_eq!(report.schema_version, VERIFICATION_REPORT_SCHEMA_VERSION);
        assert_eq!(report.status, VerificationStatus::ExecutionDenied);
        assert_eq!(report.routine_symbol, "max_usize");
        assert_eq!(report.isolation, ExecutionIsolation::StaticOnly);
        assert!(report.behavior.is_none());
        let oracle = report.behavior_oracle.expect("golden includes oracle");
        assert_eq!(oracle.id, "builtin.pure_int.binary_usize");
        assert_eq!(oracle.version, 2);
        assert_eq!(oracle.proof_basis, ProofBasis::OracleAndVectors);
        assert!(oracle.claim.contains("max(a, b)"));
    }

    #[test]
    fn golden_max_usize_verified_report_deserializes() {
        let json = include_str!("../fixtures/verification-report-max_usize.verified.json");
        let report: VerificationReport =
            serde_json::from_str(json).expect("max_usize verified golden must deserialize");
        assert_eq!(report.schema_version, VERIFICATION_REPORT_SCHEMA_VERSION);
        assert_eq!(report.status, VerificationStatus::Verified);
        assert_eq!(report.routine_symbol, "max_usize");
        assert_eq!(report.isolation, ExecutionIsolation::NativeHost);
        let behavior = report.behavior.expect("verified golden includes behavior");
        assert!(behavior.all_passed);
        assert!(!behavior.cases.is_empty());
        assert!(behavior.cases.iter().all(|c| c.passed));
        let oracle = report.behavior_oracle.expect("oracle present");
        assert_eq!(oracle.id, "builtin.pure_int.binary_usize");
        assert_eq!(oracle.version, 2);
        assert!(oracle.claim.contains("max(a, b)"));
        assert!(oracle.vectors_passed > 0);
        assert_eq!(oracle.vectors_failed, 0);
    }

    #[test]
    fn golden_find_first_byte_execution_denied_report_deserializes() {
        let json =
            include_str!("../fixtures/verification-report-find_first_byte.execution_denied.json");
        let report: VerificationReport =
            serde_json::from_str(json).expect("find_first_byte golden must deserialize");
        assert_eq!(report.schema_version, VERIFICATION_REPORT_SCHEMA_VERSION);
        assert_eq!(report.status, VerificationStatus::ExecutionDenied);
        assert_eq!(report.routine_symbol, "find_first_byte");
        assert!(report.behavior.is_none());
        let oracle = report.behavior_oracle.expect("golden includes oracle");
        assert_eq!(oracle.id, "builtin.buffer.find_first_u8");
        assert_eq!(oracle.version, 1);
        assert!(oracle.claim.contains("first index"));
    }

    #[test]
    fn golden_find_first_byte_verified_report_deserializes() {
        let json = include_str!("../fixtures/verification-report-find_first_byte.verified.json");
        let report: VerificationReport =
            serde_json::from_str(json).expect("find_first_byte verified golden must deserialize");
        assert_eq!(report.status, VerificationStatus::Verified);
        assert_eq!(report.routine_symbol, "find_first_byte");
        let behavior = report.behavior.expect("verified golden includes behavior");
        assert!(behavior.all_passed);
        let oracle = report.behavior_oracle.expect("oracle present");
        assert_eq!(oracle.id, "builtin.buffer.find_first_u8");
        assert!(oracle.claim.contains("length when absent"));
    }
}
