//! Region Access Evidence v1 — affine access → contract region matching (ADR 0011).
//!
//! Narrow fail-closed slice. Not general memory safety.

use serde::{Deserialize, Serialize};

use crate::alias::{
    access_allowed, AccessAddr, AccessMode, ObservedMemoryAccess, RelationEvidenceBasis,
};
use crate::validate::{CheckedMemory, CheckedRegion, LengthSpec};

/// Model string embedded in reports.
pub const REGION_ACCESS_AFFINE_V1: &str = "region-access-affine-v1";

/// Aggregate status for the region-access evidence slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RegionAccessStatus {
    /// Every non-stack access proven inside an allowed region; no unknowns.
    Passed,
    /// Affine accesses match declared regions, but bounds rely on symbolic
    /// region length (caller obligation); no unknown addresses / denials.
    PassedUnderPreconditions,
    /// Unknown / may-escape accesses present (or unmatched).
    Incomplete,
    /// Proven out-of-bounds or permission denied.
    Failed,
}

impl RegionAccessStatus {
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

/// Bounds judgement for one access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BoundsStatus {
    /// Access fully inside a matched region (static affine).
    ProvenInside,
    /// Access fully outside all candidate regions (static affine).
    ProvenOutside,
    /// Might escape; not proven either way.
    MayEscape,
    /// Address not modeled enough to judge.
    Unknown,
}

impl BoundsStatus {
    /// Stable snake_case label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProvenInside => "proven_inside",
            Self::ProvenOutside => "proven_outside",
            Self::MayEscape => "may_escape",
            Self::Unknown => "unknown",
        }
    }
}

/// Permission judgement for one access against a matched region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PermissionStatus {
    /// Mode allowed by region access declaration.
    Allowed,
    /// Mode denied by region access declaration.
    Denied,
    /// No matched region / unknown.
    Unknown,
}

impl PermissionStatus {
    /// Stable snake_case label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Denied => "denied",
            Self::Unknown => "unknown",
        }
    }
}

/// One memory-access evidence row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct MemoryAccessEvidence {
    /// Instruction offset when known (0 if unavailable).
    #[serde(default)]
    pub instruction_offset: u64,
    /// Load or store.
    pub operation: AccessMode,
    /// Access width in bytes.
    pub width: u32,
    /// Short address expression for reporting.
    pub address: String,
    /// Matched contract region name, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// Bounds status.
    pub bounds: BoundsStatus,
    /// Permission status.
    pub permission: PermissionStatus,
    /// Evidence basis for this row.
    #[serde(default)]
    pub evidence_basis: RelationEvidenceBasis,
}

/// Full region-access block for VerificationReport (wired in Ra4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RegionAccessReport {
    /// Evidence model id.
    pub model: String,
    /// Aggregate slice status.
    pub status: RegionAccessStatus,
    /// Total non-stack accesses considered.
    pub accesses_total: usize,
    /// Accesses with [`BoundsStatus::ProvenInside`].
    pub accesses_proven_inside: usize,
    /// Accesses with unknown address / bounds.
    pub accesses_unknown: usize,
    /// Per-access rows.
    pub accesses: Vec<MemoryAccessEvidence>,
}

/// Evaluate Region Access Evidence v1 against a checked memory block.
#[must_use]
pub fn evaluate_region_access(
    memory: &CheckedMemory,
    accesses: &[ObservedMemoryAccess],
) -> RegionAccessReport {
    let mut rows = Vec::new();
    for access in accesses {
        if matches!(access.addr, AccessAddr::StackFrame) {
            continue;
        }
        rows.push(judge_access(memory, access));
    }

    let accesses_total = rows.len();
    let accesses_proven_inside = rows
        .iter()
        .filter(|r| r.bounds == BoundsStatus::ProvenInside)
        .count();
    let accesses_unknown = rows
        .iter()
        .filter(|r| r.bounds == BoundsStatus::Unknown || r.permission == PermissionStatus::Unknown)
        .count();

    let mut any_failed = false;
    let mut any_incomplete = false;
    let mut any_under_preconditions = false;
    for row in &rows {
        if row.bounds == BoundsStatus::ProvenOutside || row.permission == PermissionStatus::Denied {
            any_failed = true;
        } else if row.bounds == BoundsStatus::Unknown
            || row.permission == PermissionStatus::Unknown
        {
            any_incomplete = true;
        } else if row.bounds == BoundsStatus::MayEscape {
            // Symbolic-length regions: matched + allowed is a caller-length
            // obligation, not a static inside proof (ADR 0011 honesty).
            if row.evidence_basis == RelationEvidenceBasis::DeclaredPrecondition
                && row.permission == PermissionStatus::Allowed
            {
                any_under_preconditions = true;
            } else {
                any_incomplete = true;
            }
        } else if row.bounds != BoundsStatus::ProvenInside
            || row.permission != PermissionStatus::Allowed
        {
            any_incomplete = true;
        }
    }

    let status = if any_failed {
        RegionAccessStatus::Failed
    } else if any_incomplete || accesses_unknown > 0 {
        RegionAccessStatus::Incomplete
    } else if any_under_preconditions {
        RegionAccessStatus::PassedUnderPreconditions
    } else {
        // Empty access list → passed (nothing unknown); all proven inside+allowed.
        RegionAccessStatus::Passed
    };

    RegionAccessReport {
        model: REGION_ACCESS_AFFINE_V1.to_string(),
        status,
        accesses_total,
        accesses_proven_inside,
        accesses_unknown,
        accesses: rows,
    }
}

fn judge_access(memory: &CheckedMemory, access: &ObservedMemoryAccess) -> MemoryAccessEvidence {
    match &access.addr {
        AccessAddr::Unknown => MemoryAccessEvidence {
            instruction_offset: access.instruction_offset,
            operation: access.mode,
            width: access.width_bytes,
            address: "unknown".to_string(),
            region: None,
            bounds: BoundsStatus::Unknown,
            permission: PermissionStatus::Unknown,
            evidence_basis: RelationEvidenceBasis::Unknown,
        },
        AccessAddr::StackFrame => unreachable!("filtered"),
        AccessAddr::Affine { base_param, offset } => {
            judge_affine_access(memory, access, base_param, *offset)
        }
    }
}

fn judge_affine_access(
    memory: &CheckedMemory,
    access: &ObservedMemoryAccess,
    base_param: &str,
    offset: i64,
) -> MemoryAccessEvidence {
    let address = if offset == 0 {
        base_param.to_string()
    } else {
        format!("{base_param}{offset:+}")
    };
    let candidates: Vec<&CheckedRegion> = memory
        .regions
        .iter()
        .filter(|r| r.base_param == base_param)
        .collect();
    if candidates.is_empty() {
        return MemoryAccessEvidence {
            instruction_offset: access.instruction_offset,
            operation: access.mode,
            width: access.width_bytes,
            address,
            region: None,
            bounds: BoundsStatus::Unknown,
            permission: PermissionStatus::Unknown,
            evidence_basis: RelationEvidenceBasis::Unknown,
        };
    }

    // Prefer a containing region that allows this access mode when
    // several affine regions overlap (e.g. memcpy_possible_overlap).
    let width = i64::from(access.width_bytes);
    let access_lo = offset;
    let access_hi = offset.saturating_add(width);

    let mut inside: Vec<&CheckedRegion> = Vec::new();
    let mut any_may = false;
    let mut all_outside = true;

    for region in &candidates {
        match classify_vs_region(region, access_lo, access_hi) {
            RegionClass::Inside => {
                all_outside = false;
                inside.push(region);
            }
            RegionClass::Outside => {}
            RegionClass::MayEscape => {
                all_outside = false;
                any_may = true;
            }
        }
    }

    if !inside.is_empty() {
        let region = inside
            .iter()
            .copied()
            .find(|r| access_allowed(r.access, access.mode))
            .or_else(|| inside.first().copied())
            .expect("inside non-empty");
        let permission = if access_allowed(region.access, access.mode) {
            PermissionStatus::Allowed
        } else {
            PermissionStatus::Denied
        };
        return MemoryAccessEvidence {
            instruction_offset: access.instruction_offset,
            operation: access.mode,
            width: access.width_bytes,
            address,
            region: Some(region.name.clone()),
            bounds: BoundsStatus::ProvenInside,
            permission,
            evidence_basis: RelationEvidenceBasis::ProvenStatic,
        };
    }

    if all_outside && !any_may {
        return MemoryAccessEvidence {
            instruction_offset: access.instruction_offset,
            operation: access.mode,
            width: access.width_bytes,
            address,
            region: None,
            bounds: BoundsStatus::ProvenOutside,
            permission: PermissionStatus::Unknown,
            evidence_basis: RelationEvidenceBasis::ProvenStatic,
        };
    }

    let region = candidates
        .iter()
        .copied()
        .find(|r| access_allowed(r.access, access.mode))
        .or_else(|| candidates.first().copied())
        .expect("candidates non-empty");
    let permission = if access_allowed(region.access, access.mode) {
        PermissionStatus::Allowed
    } else {
        PermissionStatus::Denied
    };
    // Symbolic length cannot prove static inside; treat matched+allowed as a
    // declared length/coverage precondition (not Incomplete silence).
    let under_length_precondition =
        matches!(region.length, LengthSpec::Param(_)) && permission == PermissionStatus::Allowed;
    MemoryAccessEvidence {
        instruction_offset: access.instruction_offset,
        operation: access.mode,
        width: access.width_bytes,
        address,
        region: Some(region.name.clone()),
        bounds: BoundsStatus::MayEscape,
        permission,
        evidence_basis: if under_length_precondition {
            RelationEvidenceBasis::DeclaredPrecondition
        } else {
            RelationEvidenceBasis::Unknown
        },
    }
}

enum RegionClass {
    Inside,
    Outside,
    MayEscape,
}

fn classify_vs_region(region: &CheckedRegion, access_lo: i64, access_hi: i64) -> RegionClass {
    let region_lo = region.offset;
    match &region.length {
        LengthSpec::Literal(len) => {
            let region_hi = region.offset.saturating_add_unsigned(*len);
            if access_lo >= region_lo && access_hi <= region_hi {
                RegionClass::Inside
            } else if access_hi <= region_lo || access_lo >= region_hi {
                RegionClass::Outside
            } else {
                // Partial overlap / straddling end.
                RegionClass::MayEscape
            }
        }
        LengthSpec::Param(_) => {
            // Symbolic length: only treat exact base-aligned single-byte at
            // region origin as may_escape (not proven inside).
            if access_lo == region_lo {
                RegionClass::MayEscape
            } else if access_hi <= region_lo {
                RegionClass::Outside
            } else {
                RegionClass::MayEscape
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alias::{AccessAddr, AccessMode, ObservedMemoryAccess};
    use crate::validate::{CheckedMemory, CheckedRegion, LengthSpec, RegionAccess};

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
    fn load_inside_read_region() {
        let memory = CheckedMemory {
            regions: vec![region(
                "buf",
                "p",
                0,
                LengthSpec::Literal(8),
                RegionAccess::Read,
            )],
            relations: vec![],
        };
        let accesses = vec![ObservedMemoryAccess {
            mode: AccessMode::Load,
            width_bytes: 1,
            addr: AccessAddr::Affine {
                base_param: "p".into(),
                offset: 3,
            },
            mnemonic: "mov".into(),
            instruction_offset: 12,
        }];
        let report = evaluate_region_access(&memory, &accesses);
        assert_eq!(report.status, RegionAccessStatus::Passed);
        assert_eq!(report.accesses_proven_inside, 1);
        assert_eq!(report.accesses[0].bounds, BoundsStatus::ProvenInside);
        assert_eq!(report.accesses[0].permission, PermissionStatus::Allowed);
        assert_eq!(report.accesses[0].region.as_deref(), Some("buf"));
    }

    #[test]
    fn store_to_read_only_region() {
        let memory = CheckedMemory {
            regions: vec![region(
                "buf",
                "p",
                0,
                LengthSpec::Literal(8),
                RegionAccess::Read,
            )],
            relations: vec![],
        };
        let accesses = vec![ObservedMemoryAccess {
            mode: AccessMode::Store,
            width_bytes: 1,
            addr: AccessAddr::Affine {
                base_param: "p".into(),
                offset: 0,
            },
            mnemonic: "mov".into(),
            instruction_offset: 0,
        }];
        let report = evaluate_region_access(&memory, &accesses);
        assert_eq!(report.status, RegionAccessStatus::Failed);
        assert_eq!(report.accesses[0].permission, PermissionStatus::Denied);
    }

    #[test]
    fn store_after_region() {
        let memory = CheckedMemory {
            regions: vec![region(
                "dst",
                "p",
                0,
                LengthSpec::Literal(4),
                RegionAccess::Write,
            )],
            relations: vec![],
        };
        let accesses = vec![ObservedMemoryAccess {
            mode: AccessMode::Store,
            width_bytes: 1,
            addr: AccessAddr::Affine {
                base_param: "p".into(),
                offset: 4,
            },
            mnemonic: "mov".into(),
            instruction_offset: 0,
        }];
        let report = evaluate_region_access(&memory, &accesses);
        assert_eq!(report.status, RegionAccessStatus::Failed);
        assert_eq!(report.accesses[0].bounds, BoundsStatus::ProvenOutside);
    }

    #[test]
    fn unknown_base_register() {
        let memory = CheckedMemory {
            regions: vec![region(
                "buf",
                "p",
                0,
                LengthSpec::Literal(8),
                RegionAccess::ReadWrite,
            )],
            relations: vec![],
        };
        let accesses = vec![ObservedMemoryAccess {
            mode: AccessMode::Load,
            width_bytes: 1,
            addr: AccessAddr::Unknown,
            mnemonic: "mov".into(),
            instruction_offset: 0,
        }];
        let report = evaluate_region_access(&memory, &accesses);
        assert_eq!(report.status, RegionAccessStatus::Incomplete);
        assert_eq!(report.accesses_unknown, 1);
    }

    #[test]
    fn multi_byte_access_crosses_end() {
        let memory = CheckedMemory {
            regions: vec![region(
                "buf",
                "p",
                0,
                LengthSpec::Literal(4),
                RegionAccess::Read,
            )],
            relations: vec![],
        };
        let accesses = vec![ObservedMemoryAccess {
            mode: AccessMode::Load,
            width_bytes: 4,
            addr: AccessAddr::Affine {
                base_param: "p".into(),
                offset: 2,
            },
            mnemonic: "mov".into(),
            instruction_offset: 0,
        }];
        let report = evaluate_region_access(&memory, &accesses);
        assert_eq!(report.status, RegionAccessStatus::Incomplete);
        assert_eq!(report.accesses[0].bounds, BoundsStatus::MayEscape);
    }
}
