//! Region/Alias Evidence v1 — fail-closed relation engine (ADR 0006).
//!
//! Proves selected affine identity/constant relations only. Does **not**
//! claim general alias analysis, provenance, or formal memory safety.

use serde::{Deserialize, Serialize};

use crate::validate::{CheckedMemory, CheckedRegion, LengthSpec, RegionAccess, RelationRequire};

/// Aggregate status for the alias evidence slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AliasStatus {
    /// Every required relation was proven and no unknown accesses were seen.
    Passed,
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
            Self::Incomplete => "incomplete",
            Self::Failed => "failed",
        }
    }
}

/// Observed status for one declared relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RelationObserved {
    /// Distinct pointer-parameter identities.
    ProvenDisjoint,
    /// Identical base (and matching const affine span when comparable).
    ProvenEqual,
    /// Constant affine containment.
    ProvenContains,
    /// Constant affine partial overlap.
    ProvenPartialOverlap,
    /// Possible overlap; not proven either way.
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

    /// Whether this observation is a positive proof of a relation fact.
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
    /// Short proof basis or reason string.
    pub basis: String,
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
}

/// Model string embedded in reports.
pub const REGION_AFFINE_V1: &str = "region-affine-v1";

const ASSUMPTION_DISTINCT: &str = "param_pointers_are_distinct_identities_when_named_differently";

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
        let (observed, basis) = match (left, right) {
            (Some(l), Some(r)) => observe_pair(l, r),
            _ => (
                RelationObserved::InvalidRegion,
                "relation endpoint missing from regions".to_string(),
            ),
        };

        relations.push(AliasRelationEvidence {
            left: rel.left.clone(),
            right: rel.right.clone(),
            required: rel.require.as_str().to_string(),
            observed,
            basis,
        });
    }

    let mut any_failed = false;
    let mut any_unproven = false;
    for (rel, row) in memory.relations.iter().zip(relations.iter()) {
        if contradicts(rel.require, row.observed) {
            any_failed = true;
        } else if !relation_satisfies(rel.require, row.observed) {
            any_unproven = true;
        }
    }

    let status = if any_failed {
        AliasStatus::Failed
    } else if any_unproven || unknown_memory_accesses > 0 {
        AliasStatus::Incomplete
    } else {
        AliasStatus::Passed
    };

    AliasAnalysisReport {
        model: REGION_AFFINE_V1.to_string(),
        status,
        relations,
        unknown_memory_accesses,
        assumptions: vec![ASSUMPTION_DISTINCT.to_string()],
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

fn observe_pair(left: &CheckedRegion, right: &CheckedRegion) -> (RelationObserved, String) {
    if left.base_param != right.base_param {
        return (
            RelationObserved::ProvenDisjoint,
            format!(
                "distinct_param_identity:{}!={}",
                left.base_param, right.base_param
            ),
        );
    }

    // Same pointer-parameter identity.
    let (Some(l_off), Some(r_off)) = (Some(left.offset), Some(right.offset)) else {
        return (
            RelationObserved::MayOverlap,
            "same_base_missing_offset".to_string(),
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
                );
            }
            if l0 <= r0 && l1 >= r1 {
                return (
                    RelationObserved::ProvenContains,
                    format!("left_contains_right:[{l0},{l1})>=[{r0},{r1})"),
                );
            }
            if r0 <= l0 && r1 >= l1 {
                // Right contains left — still "contains" for require=contains on
                // (right, left); for (left, right) this is not contains.
                return (
                    RelationObserved::ProvenPartialOverlap,
                    format!("right_contains_left:[{r0},{r1})>=[{l0},{l1})"),
                );
            }
            if l1 <= r0 || r1 <= l0 {
                return (
                    RelationObserved::ProvenDisjoint,
                    format!("same_base_const_disjoint:[{l0},{l1})|[{r0},{r1})"),
                );
            }
            (
                RelationObserved::ProvenPartialOverlap,
                format!("same_base_partial_overlap:[{l0},{l1})~[{r0},{r1})"),
            )
        }
        _ => {
            // Same base, symbolic length: equal only when offsets match and
            // length specs are identical param names.
            if l_off == r_off && left.length == right.length {
                (
                    RelationObserved::ProvenEqual,
                    "same_base_same_offset_same_length_param".to_string(),
                )
            } else {
                (
                    RelationObserved::MayOverlap,
                    "same_base_symbolic_length".to_string(),
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
    fn distinct_params_prove_disjoint() {
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
            }],
        };
        let report = evaluate_alias(&memory, &[]);
        assert_eq!(report.status, AliasStatus::Passed);
        assert_eq!(
            report.relations[0].observed,
            RelationObserved::ProvenDisjoint
        );
        assert_eq!(report.model, REGION_AFFINE_V1);
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
            }],
        };
        let report = evaluate_alias(&memory, &[]);
        assert_eq!(report.status, AliasStatus::Failed);
        assert_eq!(report.relations[0].observed, RelationObserved::ProvenEqual);
    }

    #[test]
    fn partial_overlap_conflicts_with_disjoint() {
        let memory = CheckedMemory {
            regions: vec![
                region("a", "buf", 0, LengthSpec::Literal(8), RegionAccess::Read),
                region("b", "buf", 4, LengthSpec::Literal(8), RegionAccess::Write),
            ],
            relations: vec![CheckedRelation {
                left: "a".into(),
                right: "b".into(),
                require: RelationRequire::Disjoint,
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
            }],
        };
        let accesses = vec![ObservedMemoryAccess {
            mode: AccessMode::Load,
            width_bytes: 1,
            addr: AccessAddr::Unknown,
            mnemonic: "mov".into(),
        }];
        let report = evaluate_alias(&memory, &accesses);
        assert_eq!(report.status, AliasStatus::Incomplete);
        assert_eq!(report.unknown_memory_accesses, 1);
    }

    #[test]
    fn self_equal_single_region() {
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
            }],
        };
        let report = evaluate_alias(&memory, &[]);
        assert_eq!(report.status, AliasStatus::Passed);
        assert_eq!(report.relations[0].observed, RelationObserved::ProvenEqual);
    }
}
