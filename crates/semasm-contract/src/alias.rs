//! Region/Alias Evidence v1.1 — fail-closed relation engine (ADR 0006 + 0010).
//!
//! Proves selected affine identity/constant relations only. Distinct pointer
//! parameter names are **not** a proof of disjointness (ADR 0010). Does **not**
//! claim general alias analysis, provenance, or formal memory safety.

use serde::{Deserialize, Serialize};

use crate::validate::{
    CheckedMemory, CheckedRegion, LengthSpec, RegionAccess, RelationBasisDecl, RelationRequire,
};

/// Aggregate status for the alias evidence slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AliasStatus {
    /// Every required relation was statically proven and no unknown accesses.
    Passed,
    /// Required relations are met only via declared caller preconditions
    /// (and/or mixed with static proofs); no unknowns / contradictions.
    PassedUnderPreconditions,
    /// Required relation unproven and/or unknown memory accesses present.
    Incomplete,
    /// Observed relation contradicts a required one.
    Failed,
}

impl AliasStatus {
    /// Stable snake_case label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::PassedUnderPreconditions => "passed_under_preconditions",
            Self::Incomplete => "incomplete",
            Self::Failed => "failed",
        }
    }
}

/// Why a relation observation is believed (ADR 0010).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RelationEvidenceBasis {
    /// Static affine/identity reasoning over the candidate.
    ProvenStatic,
    /// Explicit contract precondition; caller obligation remains.
    DeclaredPrecondition,
    /// Reserved: runtime observation.
    ObservedRuntime,
    /// Reserved: behavioral oracle / test vector.
    BehavioralTest,
    /// Reserved: environment assumption.
    AssumedEnvironment,
    /// No usable basis.
    #[default]
    Unknown,
}

impl RelationEvidenceBasis {
    /// Stable snake_case label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProvenStatic => "proven_static",
            Self::DeclaredPrecondition => "declared_precondition",
            Self::ObservedRuntime => "observed_runtime",
            Self::BehavioralTest => "behavioral_test",
            Self::AssumedEnvironment => "assumed_environment",
            Self::Unknown => "unknown",
        }
    }
}

/// Observed status for one declared relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RelationObserved {
    /// Same-base constant affine spans do not overlap (static).
    ProvenDisjoint,
    /// Identical base (and matching const affine span when comparable).
    ProvenEqual,
    /// Constant affine containment.
    ProvenContains,
    /// Constant affine partial overlap.
    ProvenPartialOverlap,
    /// Possible overlap; not proven either way (includes distinct param names).
    MayOverlap,
    /// Region declaration could not be interpreted.
    InvalidRegion,
    /// Engine did not evaluate this relation.
    NotEvaluated,
}

impl RelationObserved {
    /// Stable snake_case label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProvenDisjoint => "proven_disjoint",
            Self::ProvenEqual => "proven_equal",
            Self::ProvenContains => "proven_contains",
            Self::ProvenPartialOverlap => "proven_partial_overlap",
            Self::MayOverlap => "may_overlap",
            Self::InvalidRegion => "invalid_region",
            Self::NotEvaluated => "not_evaluated",
        }
    }

    /// Whether this observation is a positive static proof of a relation fact.
    #[must_use]
    pub const fn is_proven(self) -> bool {
        matches!(
            self,
            Self::ProvenDisjoint
                | Self::ProvenEqual
                | Self::ProvenContains
                | Self::ProvenPartialOverlap
        )
    }
}

/// How an observed memory access addresses memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AccessAddr {
    /// Affine form relative to a named pointer parameter.
    Affine {
        /// Parameter name used as base.
        base_param: String,
        /// Constant byte offset from that parameter.
        offset: i64,
    },
    /// Frame / stack spill (ignored for region/alias evidence).
    StackFrame,
    /// Address expression was not modeled.
    Unknown,
}

/// Load vs store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AccessMode {
    /// Memory load.
    Load,
    /// Memory store.
    Store,
}

/// One observed memory access from lowering / ASIR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ObservedMemoryAccess {
    /// Load or store.
    pub mode: AccessMode,
    /// Access width in bytes.
    pub width_bytes: u32,
    /// Address model.
    pub addr: AccessAddr,
    /// Original mnemonic (reporting).
    pub mnemonic: String,
    /// Instruction offset in the candidate when known (ADR 0011).
    #[serde(default)]
    pub instruction_offset: u64,
}

/// Per-relation evidence row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct AliasRelationEvidence {
    /// Left region name.
    pub left: String,
    /// Right region name.
    pub right: String,
    /// Required relation from the contract.
    pub required: String,
    /// Observed relation status.
    pub observed: RelationObserved,
    /// Typed evidence basis (ADR 0010).
    #[serde(default)]
    pub evidence_basis: RelationEvidenceBasis,
    /// Short proof basis or reason string.
    pub basis: String,
}

/// One unresolved verification obligation (typically a caller precondition).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct VerificationObligation {
    /// Obligation kind (`regions_disjoint`, `regions_equal`, `regions_contains`).
    pub kind: String,
    /// Left region name.
    pub left: String,
    /// Right region name.
    pub right: String,
    /// Who must discharge the obligation (`caller`).
    pub owner: String,
}

/// Full alias-analysis block for [`crate` consumers / VerificationReport].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct AliasAnalysisReport {
    /// Evidence model id.
    pub model: String,
    /// Aggregate slice status.
    pub status: AliasStatus,
    /// Per-relation rows.
    pub relations: Vec<AliasRelationEvidence>,
    /// Count of unmodeled (non-stack) memory accesses.
    pub unknown_memory_accesses: usize,
    /// Explicit honesty assumptions.
    pub assumptions: Vec<String>,
    /// Caller obligations not discharged by static proof (ADR 0010).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unresolved_obligations: Vec<VerificationObligation>,
}

/// Model string embedded in reports.
pub const REGION_AFFINE_V1: &str = "region-affine-v1";

const ASSUMPTION_DISTINCT_NOT_PROOF: &str =
    "distinct_param_names_do_not_prove_runtime_disjointness";

/// Evaluate Region/Alias Evidence v1 for a checked memory block.
#[must_use]
pub fn evaluate_alias(
    memory: &CheckedMemory,
    accesses: &[ObservedMemoryAccess],
) -> AliasAnalysisReport {
    let unknown_memory_accesses = accesses
        .iter()
        .filter(|a| matches!(a.addr, AccessAddr::Unknown))
        .count();

    let mut relations = Vec::with_capacity(memory.relations.len());
    for rel in &memory.relations {
        let left = memory.regions.iter().find(|r| r.name == rel.left);
        let right = memory.regions.iter().find(|r| r.name == rel.right);
        let (observed, detail, evidence_basis) = match (left, right) {
            (Some(l), Some(r)) => {
                let (obs, detail, mut basis) = observe_pair(l, r);
                // Explicit contract precondition only applies when static proof
                // did not already decide the required fact.
                if matches!(rel.basis, Some(RelationBasisDecl::Precondition))
                    && !relation_satisfies(rel.require, obs)
                    && !contradicts(rel.require, obs)
                {
                    basis = RelationEvidenceBasis::DeclaredPrecondition;
                }
                (obs, detail, basis)
            }
            _ => (
                RelationObserved::InvalidRegion,
                "relation endpoint missing from regions".to_string(),
                RelationEvidenceBasis::Unknown,
            ),
        };

        relations.push(AliasRelationEvidence {
            left: rel.left.clone(),
            right: rel.right.clone(),
            required: rel.require.as_str().to_string(),
            observed,
            evidence_basis,
            basis: detail,
        });
    }

    let mut any_failed = false;
    let mut any_unproven = false;
    let mut any_caller_obligation = false;
    let mut unresolved_obligations = Vec::new();
    for (rel, row) in memory.relations.iter().zip(relations.iter()) {
        if contradicts(rel.require, row.observed) {
            any_failed = true;
        } else if relation_satisfies(rel.require, row.observed) {
            // statically proven
        } else if row.evidence_basis == RelationEvidenceBasis::DeclaredPrecondition
            && matches!(rel.basis, Some(RelationBasisDecl::Precondition))
        {
            any_caller_obligation = true;
            unresolved_obligations.push(VerificationObligation {
                kind: format!("regions_{}", rel.require.as_str()),
                left: rel.left.clone(),
                right: rel.right.clone(),
                owner: "caller".to_string(),
            });
        } else {
            any_unproven = true;
        }
    }

    let status = if any_failed {
        AliasStatus::Failed
    } else if any_unproven || unknown_memory_accesses > 0 {
        AliasStatus::Incomplete
    } else if any_caller_obligation {
        AliasStatus::PassedUnderPreconditions
    } else {
        AliasStatus::Passed
    };

    AliasAnalysisReport {
        model: REGION_AFFINE_V1.to_string(),
        status,
        relations,
        unknown_memory_accesses,
        assumptions: vec![ASSUMPTION_DISTINCT_NOT_PROOF.to_string()],
        unresolved_obligations,
    }
}

fn relation_satisfies(required: RelationRequire, observed: RelationObserved) -> bool {
    match required {
        RelationRequire::Disjoint => matches!(observed, RelationObserved::ProvenDisjoint),
        RelationRequire::Equal => matches!(observed, RelationObserved::ProvenEqual),
        RelationRequire::Contains => matches!(observed, RelationObserved::ProvenContains),
    }
}

fn contradicts(required: RelationRequire, observed: RelationObserved) -> bool {
    match required {
        RelationRequire::Disjoint => matches!(
            observed,
            RelationObserved::ProvenEqual
                | RelationObserved::ProvenContains
                | RelationObserved::ProvenPartialOverlap
        ),
        RelationRequire::Equal => matches!(
            observed,
            RelationObserved::ProvenDisjoint | RelationObserved::ProvenPartialOverlap
        ),
        RelationRequire::Contains => matches!(
            observed,
            RelationObserved::ProvenDisjoint | RelationObserved::ProvenPartialOverlap
        ),
    }
}

fn observe_pair(
    left: &CheckedRegion,
    right: &CheckedRegion,
) -> (RelationObserved, String, RelationEvidenceBasis) {
    if left.base_param != right.base_param {
        // ADR 0010: different parameter names are not a static disjoint proof.
        return (
            RelationObserved::MayOverlap,
            format!(
                "distinct_param_names_not_proof:{}!={}",
                left.base_param, right.base_param
            ),
            RelationEvidenceBasis::Unknown,
        );
    }

    // Same pointer-parameter identity.
    let (Some(l_off), Some(r_off)) = (Some(left.offset), Some(right.offset)) else {
        return (
            RelationObserved::MayOverlap,
            "same_base_missing_offset".to_string(),
            RelationEvidenceBasis::Unknown,
        );
    };

    match (&left.length, &right.length) {
        (LengthSpec::Literal(l_len), LengthSpec::Literal(r_len)) => {
            let l0 = l_off;
            let l1 = l_off.saturating_add_unsigned(*l_len);
            let r0 = r_off;
            let r1 = r_off.saturating_add_unsigned(*r_len);
            if l0 == r0 && l1 == r1 {
                return (
                    RelationObserved::ProvenEqual,
                    format!("same_base_equal_span:[{l0},{l1})"),
                    RelationEvidenceBasis::ProvenStatic,
                );
            }
            if l0 <= r0 && l1 >= r1 {
                return (
                    RelationObserved::ProvenContains,
                    format!("left_contains_right:[{l0},{l1})>=[{r0},{r1})"),
                    RelationEvidenceBasis::ProvenStatic,
                );
            }
            if r0 <= l0 && r1 >= l1 {
                // Right contains left — still "contains" for require=contains on
                // (right, left); for (left, right) this is not contains.
                return (
                    RelationObserved::ProvenPartialOverlap,
                    format!("right_contains_left:[{r0},{r1})>=[{l0},{l1})"),
                    RelationEvidenceBasis::ProvenStatic,
                );
            }
            if l1 <= r0 || r1 <= l0 {
                return (
                    RelationObserved::ProvenDisjoint,
                    format!("same_base_const_disjoint:[{l0},{l1})|[{r0},{r1})"),
                    RelationEvidenceBasis::ProvenStatic,
                );
            }
            (
                RelationObserved::ProvenPartialOverlap,
                format!("same_base_partial_overlap:[{l0},{l1})~[{r0},{r1})"),
                RelationEvidenceBasis::ProvenStatic,
            )
        }
        _ => {
            // Same base, symbolic length: equal only when offsets match and
            // length specs are identical param names.
            if l_off == r_off && left.length == right.length {
                (
                    RelationObserved::ProvenEqual,
                    "same_base_same_offset_same_length_param".to_string(),
                    RelationEvidenceBasis::ProvenStatic,
                )
            } else {
                (
                    RelationObserved::MayOverlap,
                    "same_base_symbolic_length".to_string(),
                    RelationEvidenceBasis::Unknown,
                )
            }
        }
    }
}

/// Whether an access mode is allowed by a region's declared access.
#[must_use]
pub fn access_allowed(region: RegionAccess, mode: AccessMode) -> bool {
    match (region, mode) {
        (RegionAccess::Read, AccessMode::Load)
        | (RegionAccess::Write, AccessMode::Store)
        | (RegionAccess::ReadWrite, _) => true,
        (RegionAccess::Read, AccessMode::Store) | (RegionAccess::Write, AccessMode::Load) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate::{CheckedRelation, LengthSpec, RegionAccess, RelationRequire};

    fn region(
        name: &str,
        base: &str,
        offset: i64,
        length: LengthSpec,
        access: RegionAccess,
    ) -> CheckedRegion {
        CheckedRegion {
            name: name.into(),
            base_param: base.into(),
            offset,
            length,
            access,
        }
    }

    #[test]
    fn different_pointer_names_are_not_proven_disjoint() {
        let memory = CheckedMemory {
            regions: vec![
                region(
                    "src",
                    "src",
                    0,
                    LengthSpec::Param("length".into()),
                    RegionAccess::Read,
                ),
                region(
                    "dst",
                    "dst",
                    0,
                    LengthSpec::Param("length".into()),
                    RegionAccess::Write,
                ),
            ],
            relations: vec![CheckedRelation {
                left: "src".into(),
                right: "dst".into(),
                require: RelationRequire::Disjoint,
                basis: None,
            }],
        };
        let report = evaluate_alias(&memory, &[]);
        assert_eq!(report.status, AliasStatus::Incomplete);
        assert_eq!(report.relations[0].observed, RelationObserved::MayOverlap);
        assert_eq!(
            report.relations[0].evidence_basis,
            RelationEvidenceBasis::Unknown
        );
        assert_eq!(report.model, REGION_AFFINE_V1);
    }

    #[test]
    fn explicit_disjoint_precondition_is_caller_obligation() {
        let memory = CheckedMemory {
            regions: vec![
                region(
                    "src",
                    "src",
                    0,
                    LengthSpec::Param("length".into()),
                    RegionAccess::Read,
                ),
                region(
                    "dst",
                    "dst",
                    0,
                    LengthSpec::Param("length".into()),
                    RegionAccess::Write,
                ),
            ],
            relations: vec![CheckedRelation {
                left: "src".into(),
                right: "dst".into(),
                require: RelationRequire::Disjoint,
                basis: Some(RelationBasisDecl::Precondition),
            }],
        };
        let report = evaluate_alias(&memory, &[]);
        assert_eq!(report.status, AliasStatus::PassedUnderPreconditions);
        assert_eq!(report.relations[0].observed, RelationObserved::MayOverlap);
        assert_eq!(
            report.relations[0].evidence_basis,
            RelationEvidenceBasis::DeclaredPrecondition
        );
        assert_eq!(report.unresolved_obligations.len(), 1);
        assert_eq!(report.unresolved_obligations[0].kind, "regions_disjoint");
        assert_eq!(report.unresolved_obligations[0].owner, "caller");
    }

    #[test]
    fn same_pointer_is_proven_equal() {
        let memory = CheckedMemory {
            regions: vec![region(
                "buf",
                "buffer",
                0,
                LengthSpec::Param("length".into()),
                RegionAccess::ReadWrite,
            )],
            relations: vec![CheckedRelation {
                left: "buf".into(),
                right: "buf".into(),
                require: RelationRequire::Equal,
                basis: None,
            }],
        };
        let report = evaluate_alias(&memory, &[]);
        assert_eq!(report.status, AliasStatus::Passed);
        assert_eq!(report.relations[0].observed, RelationObserved::ProvenEqual);
        assert_eq!(
            report.relations[0].evidence_basis,
            RelationEvidenceBasis::ProvenStatic
        );
    }

    #[test]
    fn same_base_overlapping_offsets_are_partial_overlap() {
        let memory = CheckedMemory {
            regions: vec![
                region("a", "buf", 0, LengthSpec::Literal(8), RegionAccess::Read),
                region("b", "buf", 4, LengthSpec::Literal(8), RegionAccess::Write),
            ],
            relations: vec![CheckedRelation {
                left: "a".into(),
                right: "b".into(),
                require: RelationRequire::Disjoint,
                basis: None,
            }],
        };
        let report = evaluate_alias(&memory, &[]);
        assert_eq!(report.status, AliasStatus::Failed);
        assert_eq!(
            report.relations[0].observed,
            RelationObserved::ProvenPartialOverlap
        );
    }

    #[test]
    fn same_base_equal_conflicts_with_disjoint() {
        let memory = CheckedMemory {
            regions: vec![
                region("a", "p", 0, LengthSpec::Literal(8), RegionAccess::ReadWrite),
                region("b", "p", 0, LengthSpec::Literal(8), RegionAccess::ReadWrite),
            ],
            relations: vec![CheckedRelation {
                left: "a".into(),
                right: "b".into(),
                require: RelationRequire::Disjoint,
                basis: None,
            }],
        };
        let report = evaluate_alias(&memory, &[]);
        assert_eq!(report.status, AliasStatus::Failed);
        assert_eq!(report.relations[0].observed, RelationObserved::ProvenEqual);
    }

    #[test]
    fn unknown_access_makes_incomplete() {
        let memory = CheckedMemory {
            regions: vec![
                region(
                    "src",
                    "src",
                    0,
                    LengthSpec::Param("length".into()),
                    RegionAccess::Read,
                ),
                region(
                    "dst",
                    "dst",
                    0,
                    LengthSpec::Param("length".into()),
                    RegionAccess::Write,
                ),
            ],
            relations: vec![CheckedRelation {
                left: "src".into(),
                right: "dst".into(),
                require: RelationRequire::Disjoint,
                basis: Some(RelationBasisDecl::Precondition),
            }],
        };
        let accesses = vec![ObservedMemoryAccess {
            mode: AccessMode::Load,
            width_bytes: 1,
            addr: AccessAddr::Unknown,
            mnemonic: "mov".into(),
            instruction_offset: 0,
        }];
        let report = evaluate_alias(&memory, &accesses);
        assert_eq!(report.status, AliasStatus::Incomplete);
        assert_eq!(report.unknown_memory_accesses, 1);
    }
}
