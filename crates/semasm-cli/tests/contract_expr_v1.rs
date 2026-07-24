//! Contract Expression Semantics v1 corpus (ADR 0007).

use semasm_contract::{
    check_file, evaluate_alias, evaluate_contract_expressions, ContractExprStatus, ExprBindings,
    ExprJudgement,
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
fn memcpy_region_atom_passes_with_alias() {
    let checked = load("memcpy.sem.toml");
    let alias = evaluate_alias(checked.memory.as_ref().unwrap(), &[]);
    let report =
        evaluate_contract_expressions(&checked, Some(&alias), &ExprBindings::default()).unwrap();
    assert_eq!(report.status, ContractExprStatus::Passed);
    assert!(report.expressions.iter().any(|e| {
        e.source.contains("regions.disjoint") && e.judgement == ExprJudgement::ProvenTrue
    }));
}

#[test]
fn unknown_predicate_incomplete() {
    let checked = load("memcpy_expr_unknown.sem.toml");
    let alias = evaluate_alias(checked.memory.as_ref().unwrap(), &[]);
    let report =
        evaluate_contract_expressions(&checked, Some(&alias), &ExprBindings::default()).unwrap();
    assert_eq!(report.status, ContractExprStatus::Incomplete);
}

#[test]
fn contradicting_equal_atom_fails() {
    let checked = load("memcpy_expr_contradict.sem.toml");
    let alias = evaluate_alias(checked.memory.as_ref().unwrap(), &[]);
    let report =
        evaluate_contract_expressions(&checked, Some(&alias), &ExprBindings::default()).unwrap();
    assert_eq!(report.status, ContractExprStatus::Failed);
    assert_eq!(report.expressions[0].judgement, ExprJudgement::ProvenFalse);
}

#[test]
fn without_alias_slice_region_atoms_omit_report() {
    let checked = load("memcpy.sem.toml");
    assert!(evaluate_contract_expressions(&checked, None, &ExprBindings::default()).is_none());
}
