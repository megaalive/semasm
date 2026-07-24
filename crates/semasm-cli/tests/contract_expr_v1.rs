//! Contract Expression Semantics v1 corpus (ADR 0007 + 0010).

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
fn memcpy_region_atom_true_under_precondition() {
    let checked = load("memcpy.sem.toml");
    let alias = evaluate_alias(checked.memory.as_ref().unwrap(), &[]);
    let report =
        evaluate_contract_expressions(&checked, Some(&alias), &ExprBindings::default()).unwrap();
    assert_eq!(report.status, ContractExprStatus::PassedUnderPreconditions);
    assert!(report.expressions.iter().any(|e| {
        e.source.contains("regions.disjoint") && e.judgement == ExprJudgement::TrueUnderPrecondition
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
fn without_alias_slice_still_records_length_bound_obligation() {
    let checked = load("memcpy.sem.toml");
    let report = evaluate_contract_expressions(&checked, None, &ExprBindings::default()).unwrap();
    assert_eq!(report.status, ContractExprStatus::PassedUnderPreconditions);
    assert!(report.expressions.iter().any(|e| {
        e.source.contains("length") && e.judgement == ExprJudgement::TrueUnderPrecondition
    }));
    // Region atoms without alias remain not_evaluated / do not fail the slice alone.
    assert!(report.expressions.iter().any(|e| {
        e.source.contains("regions.disjoint") && e.judgement == ExprJudgement::NotEvaluated
    }));
}
