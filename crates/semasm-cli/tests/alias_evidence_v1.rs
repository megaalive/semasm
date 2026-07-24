//! Region/Alias Evidence v1.1 corpus checks (ADR 0006 + 0010).
//!
//! These unit tests drive the relation engine + contract fixtures without a
//! full `agent verify` pipeline. E2E agent filters still own live decode.

use semasm_contract::{
    check_file, evaluate_alias, AccessAddr, AccessMode, AliasStatus, ObservedMemoryAccess,
    RelationEvidenceBasis, RelationObserved,
};
use std::path::PathBuf;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn load(name: &str) -> semasm_contract::CheckedContract {
    let path = fixtures().join("contracts").join(name);
    check_file(&path)
        .unwrap_or_else(|e| panic!("read {name}: {e}"))
        .contract
        .unwrap_or_else(|| panic!("{name} must validate"))
}

#[test]
fn memcpy_contract_alias_passes_under_precondition() {
    let checked = load("memcpy.sem.toml");
    let memory = checked.memory.as_ref().expect("memory block");
    let report = evaluate_alias(memory, &[]);
    assert_eq!(report.status, AliasStatus::PassedUnderPreconditions);
    assert_eq!(report.relations[0].observed, RelationObserved::MayOverlap);
    assert_eq!(
        report.relations[0].evidence_basis,
        RelationEvidenceBasis::DeclaredPrecondition
    );
}

#[test]
fn exact_alias_contract_fails_disjoint_require() {
    let checked = load("memcpy_exact_alias.sem.toml");
    let memory = checked.memory.as_ref().expect("memory block");
    let report = evaluate_alias(memory, &[]);
    assert_eq!(report.status, AliasStatus::Failed);
    assert_eq!(report.relations[0].observed, RelationObserved::ProvenEqual);
}

#[test]
fn partial_overlap_contract_fails_disjoint_require() {
    let checked = load("memcpy_partial_overlap.sem.toml");
    let memory = checked.memory.as_ref().expect("memory block");
    let report = evaluate_alias(memory, &[]);
    assert_eq!(report.status, AliasStatus::Failed);
    assert_eq!(
        report.relations[0].observed,
        RelationObserved::ProvenPartialOverlap
    );
}

#[test]
fn unknown_access_fixture_is_incomplete() {
    let checked = load("memcpy_unknown_address.sem.toml");
    let memory = checked.memory.as_ref().expect("memory block");
    let accesses = vec![ObservedMemoryAccess {
        mode: AccessMode::Load,
        width_bytes: 1,
        addr: AccessAddr::Unknown,
        mnemonic: "mov".into(),
        instruction_offset: 0,
    }];
    let report = evaluate_alias(memory, &accesses);
    assert_eq!(report.status, AliasStatus::Incomplete);
    assert_eq!(report.unknown_memory_accesses, 1);
}

#[test]
fn memset_self_equal_passes() {
    let checked = load("memset.sem.toml");
    let memory = checked.memory.as_ref().expect("memory block");
    let report = evaluate_alias(memory, &[]);
    assert_eq!(report.status, AliasStatus::Passed);
    assert_eq!(report.relations[0].observed, RelationObserved::ProvenEqual);
}
