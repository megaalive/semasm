//! Contract Expression Semantics v1 — fail-closed subset evaluator (ADR 0007).
//!
//! See `docs/CONTRACT_EXPR_V1_SUBSET.md`. Not a theorem prover / SMT.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::alias::{AliasAnalysisReport, RelationEvidenceBasis, RelationObserved};
use crate::expr::{BinOp, Expr, UnaryOp};
use crate::validate::{CheckedCondition, CheckedContract, RelationRequire};

/// Model string embedded in reports.
pub const CONTRACT_EXPR_V1: &str = "contract-expr-v1";

/// Aggregate status for the contract-expression slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ContractExprStatus {
    /// Every attempted expression was proven true.
    Passed,
    /// Attempted expressions hold only under declared caller preconditions.
    PassedUnderPreconditions,
    /// At least one attempted expression could not be decided.
    Incomplete,
    /// At least one attempted expression was proven false.
    Failed,
}

impl ContractExprStatus {
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

/// Per-expression judgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ExprJudgement {
    /// Expression evaluated to true under the subset rules.
    ProvenTrue,
    /// Expression evaluated to false under the subset rules.
    ProvenFalse,
    /// True only under an explicit declared precondition (ADR 0010).
    TrueUnderPrecondition,
    /// Subset rules apply but a fact was missing / unknown op.
    Incomplete,
    /// Out of static subset scope (e.g. unbound postcondition).
    NotEvaluated,
}

impl ExprJudgement {
    /// Stable snake_case label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProvenTrue => "proven_true",
            Self::ProvenFalse => "proven_false",
            Self::TrueUnderPrecondition => "true_under_precondition",
            Self::Incomplete => "incomplete",
            Self::NotEvaluated => "not_evaluated",
        }
    }
}

/// Where the expression appeared in the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ExprRole {
    /// `[[function.requires]]`
    Requires,
    /// `[[function.ensures]]`
    Ensures,
}

impl ExprRole {
    /// Stable snake_case label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Requires => "requires",
            Self::Ensures => "ensures",
        }
    }
}

/// One evaluated (or skipped) expression row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ContractExprEvidence {
    /// requires / ensures.
    pub role: ExprRole,
    /// Original source text.
    pub source: String,
    /// Judgement.
    pub judgement: ExprJudgement,
    /// Short basis string.
    pub basis: String,
}

/// Full contract-expression block for VerificationReport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ContractExprReport {
    /// Evidence model id.
    pub model: String,
    /// Aggregate slice status.
    pub status: ContractExprStatus,
    /// Per-expression rows (including not_evaluated).
    pub expressions: Vec<ContractExprEvidence>,
    /// Explicit honesty assumptions.
    pub assumptions: Vec<String>,
}

/// Optional concrete integer/bool bindings for closed comparisons.
#[derive(Debug, Clone, Default)]
pub struct ExprBindings {
    /// Name → integer value.
    pub ints: BTreeMap<String, i64>,
    /// Name → boolean value.
    pub bools: BTreeMap<String, bool>,
}

/// Evaluate the documented subset for a checked contract.
///
/// Returns [`None`] when every expression is [`ExprJudgement::NotEvaluated`]
/// (nothing for the slice to claim).
#[must_use]
pub fn evaluate_contract_expressions(
    contract: &CheckedContract,
    alias: Option<&AliasAnalysisReport>,
    bindings: &ExprBindings,
) -> Option<ContractExprReport> {
    let mut expressions = Vec::new();
    for cond in &contract.requires {
        expressions.push(eval_condition(ExprRole::Requires, cond, alias, bindings));
    }
    for cond in &contract.ensures {
        expressions.push(eval_condition(ExprRole::Ensures, cond, alias, bindings));
    }

    if expressions
        .iter()
        .all(|row| row.judgement == ExprJudgement::NotEvaluated)
    {
        return None;
    }

    let mut any_false = false;
    let mut any_incomplete = false;
    let mut any_true = false;
    let mut any_under_precondition = false;
    for row in &expressions {
        match row.judgement {
            ExprJudgement::ProvenFalse => any_false = true,
            ExprJudgement::Incomplete => any_incomplete = true,
            ExprJudgement::ProvenTrue => any_true = true,
            ExprJudgement::TrueUnderPrecondition => {
                any_true = true;
                any_under_precondition = true;
            }
            ExprJudgement::NotEvaluated => {}
        }
    }

    let status = if any_false {
        ContractExprStatus::Failed
    } else if any_incomplete {
        ContractExprStatus::Incomplete
    } else if any_under_precondition {
        ContractExprStatus::PassedUnderPreconditions
    } else if any_true {
        ContractExprStatus::Passed
    } else {
        // Only incompletes were filtered above; defensive.
        ContractExprStatus::Incomplete
    };

    Some(ContractExprReport {
        model: CONTRACT_EXPR_V1.to_string(),
        status,
        expressions,
        assumptions: vec![
            "subset_documented_in_CONTRACT_EXPR_V1_SUBSET".to_string(),
            "regions_atoms_require_alias_evidence_when_present".to_string(),
        ],
    })
}

fn eval_condition(
    role: ExprRole,
    cond: &CheckedCondition,
    alias: Option<&AliasAnalysisReport>,
    bindings: &ExprBindings,
) -> ContractExprEvidence {
    let (judgement, basis) = eval_expr(&cond.expr, alias, bindings, false);
    ContractExprEvidence {
        role,
        source: cond.source.clone(),
        judgement,
        basis,
    }
}

/// `force_attempt` is true when a parent already saw a region atom.
#[allow(clippy::too_many_lines)]
fn eval_expr(
    expr: &Expr,
    alias: Option<&AliasAnalysisReport>,
    bindings: &ExprBindings,
    force_attempt: bool,
) -> (ExprJudgement, String) {
    match expr {
        Expr::Bool { value, .. } => (
            if *value {
                ExprJudgement::ProvenTrue
            } else {
                ExprJudgement::ProvenFalse
            },
            format!("bool_literal:{value}"),
        ),
        Expr::Int { .. } => (
            ExprJudgement::Incomplete,
            "bare_integer_not_a_proposition".to_string(),
        ),
        Expr::Ident { name, .. } => {
            if let Some(b) = bindings.bools.get(name) {
                (
                    if *b {
                        ExprJudgement::ProvenTrue
                    } else {
                        ExprJudgement::ProvenFalse
                    },
                    format!("bound_bool:{name}"),
                )
            } else {
                skip_or_incomplete(
                    force_attempt,
                    format!("unbound_ident:{name}"),
                    "unbound_ident_not_evaluated",
                )
            }
        }
        Expr::Unary {
            op: UnaryOp::Not,
            expr: inner,
            ..
        } => {
            let (j, basis) = eval_expr(inner, alias, bindings, force_attempt);
            match j {
                ExprJudgement::ProvenTrue => (ExprJudgement::ProvenFalse, format!("not({basis})")),
                ExprJudgement::ProvenFalse => (ExprJudgement::ProvenTrue, format!("not({basis})")),
                ExprJudgement::TrueUnderPrecondition => (
                    ExprJudgement::Incomplete,
                    format!("not_under_precondition({basis})"),
                ),
                other => (other, format!("not({basis})")),
            }
        }
        Expr::Unary {
            op: UnaryOp::Neg, ..
        } => skip_or_incomplete(
            force_attempt,
            "unary_neg_out_of_subset".to_string(),
            "unary_neg_not_evaluated",
        ),
        Expr::Binary {
            op: BinOp::And,
            left,
            right,
            ..
        } => eval_and(left, right, alias, bindings, force_attempt),
        Expr::Binary {
            op: BinOp::Or,
            left,
            right,
            ..
        } => eval_or(left, right, alias, bindings, force_attempt),
        Expr::Binary {
            op: BinOp::Implies,
            left,
            right,
            ..
        } => {
            // A implies B  ≡  (not A) or B
            let (jl, bl) = eval_expr(left, alias, bindings, force_attempt);
            match jl {
                ExprJudgement::ProvenFalse => {
                    (ExprJudgement::ProvenTrue, format!("implies_vacuous:{bl}"))
                }
                ExprJudgement::ProvenTrue | ExprJudgement::TrueUnderPrecondition => {
                    let under = matches!(jl, ExprJudgement::TrueUnderPrecondition);
                    let (jr, br) = eval_expr(right, alias, bindings, true);
                    let j = match (under, jr) {
                        (_, ExprJudgement::ProvenFalse) => ExprJudgement::ProvenFalse,
                        (_, ExprJudgement::Incomplete) => ExprJudgement::Incomplete,
                        (_, ExprJudgement::NotEvaluated) => ExprJudgement::NotEvaluated,
                        (true, _) | (_, ExprJudgement::TrueUnderPrecondition) => {
                            ExprJudgement::TrueUnderPrecondition
                        }
                        (false, ExprJudgement::ProvenTrue) => ExprJudgement::ProvenTrue,
                    };
                    (j, format!("implies:{bl}=>{br}"))
                }
                ExprJudgement::Incomplete => (
                    ExprJudgement::Incomplete,
                    format!("implies_antecedent:{bl}"),
                ),
                ExprJudgement::NotEvaluated => {
                    let force =
                        force_attempt || contains_region_atom(left) || contains_region_atom(right);
                    if force {
                        let (jr, br) = eval_expr(right, alias, bindings, true);
                        match jr {
                            ExprJudgement::NotEvaluated => {
                                (ExprJudgement::NotEvaluated, format!("implies_skipped:{bl}"))
                            }
                            other => (other, format!("implies_partial:{bl}=>{br}")),
                        }
                    } else {
                        (
                            ExprJudgement::NotEvaluated,
                            format!("implies_not_evaluated:{bl}"),
                        )
                    }
                }
            }
        }
        Expr::Binary {
            op: BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge,
            left,
            right,
            ..
        } => eval_cmp(expr, left, right, bindings, force_attempt),
        Expr::Binary {
            op: BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div,
            ..
        } => skip_or_incomplete(
            force_attempt,
            "arithmetic_out_of_subset".to_string(),
            "arithmetic_not_evaluated",
        ),
        Expr::Call { .. } => eval_call(expr, alias, force_attempt),
        Expr::Member { .. } | Expr::Index { .. } | Expr::Range { .. } => {
            if contains_region_atom(expr) || force_attempt {
                (
                    ExprJudgement::Incomplete,
                    "unsupported_construct_in_attempted_expr".to_string(),
                )
            } else {
                (
                    ExprJudgement::NotEvaluated,
                    "unsupported_construct_not_evaluated".to_string(),
                )
            }
        }
    }
}

fn skip_or_incomplete(
    force_attempt: bool,
    incomplete_basis: String,
    skip_basis: &str,
) -> (ExprJudgement, String) {
    if force_attempt {
        (ExprJudgement::Incomplete, incomplete_basis)
    } else {
        (ExprJudgement::NotEvaluated, skip_basis.to_string())
    }
}

fn eval_and(
    left: &Expr,
    right: &Expr,
    alias: Option<&AliasAnalysisReport>,
    bindings: &ExprBindings,
    force_attempt: bool,
) -> (ExprJudgement, String) {
    let force = force_attempt || contains_region_atom(left) || contains_region_atom(right);
    let (jl, bl) = eval_expr(left, alias, bindings, force);
    match jl {
        ExprJudgement::ProvenFalse => (ExprJudgement::ProvenFalse, format!("and_short:{bl}")),
        ExprJudgement::ProvenTrue | ExprJudgement::TrueUnderPrecondition => {
            let left_under = matches!(jl, ExprJudgement::TrueUnderPrecondition);
            let (jr, br) = eval_expr(right, alias, bindings, force);
            let j = match (left_under, jr) {
                (_, ExprJudgement::ProvenFalse) => ExprJudgement::ProvenFalse,
                (_, ExprJudgement::Incomplete) => ExprJudgement::Incomplete,
                (_, ExprJudgement::NotEvaluated) if force => ExprJudgement::Incomplete,
                (_, ExprJudgement::NotEvaluated) => ExprJudgement::NotEvaluated,
                (true, _) | (_, ExprJudgement::TrueUnderPrecondition) => {
                    ExprJudgement::TrueUnderPrecondition
                }
                (false, ExprJudgement::ProvenTrue) => ExprJudgement::ProvenTrue,
            };
            (j, format!("and:{bl}&{br}"))
        }
        ExprJudgement::Incomplete => (ExprJudgement::Incomplete, format!("and_left:{bl}")),
        ExprJudgement::NotEvaluated => {
            let (jr, br) = eval_expr(right, alias, bindings, force);
            match jr {
                ExprJudgement::NotEvaluated => {
                    (ExprJudgement::NotEvaluated, format!("and_skip:{bl}"))
                }
                ExprJudgement::ProvenFalse => {
                    (ExprJudgement::ProvenFalse, format!("and:{bl}&{br}"))
                }
                other => {
                    // Left unknown, right true/incomplete → incomplete if forced else not_evaluated
                    if force {
                        (ExprJudgement::Incomplete, format!("and_partial:{bl}&{br}"))
                    } else {
                        (other, format!("and_right_only:{br}"))
                    }
                }
            }
        }
    }
}

fn eval_or(
    left: &Expr,
    right: &Expr,
    alias: Option<&AliasAnalysisReport>,
    bindings: &ExprBindings,
    force_attempt: bool,
) -> (ExprJudgement, String) {
    let force = force_attempt || contains_region_atom(left) || contains_region_atom(right);
    let (jl, bl) = eval_expr(left, alias, bindings, force);
    match jl {
        ExprJudgement::ProvenTrue => (ExprJudgement::ProvenTrue, format!("or_short:{bl}")),
        ExprJudgement::TrueUnderPrecondition => (
            ExprJudgement::TrueUnderPrecondition,
            format!("or_short:{bl}"),
        ),
        ExprJudgement::ProvenFalse => {
            let (jr, br) = eval_expr(right, alias, bindings, force);
            (jr, format!("or:{bl}|{br}"))
        }
        ExprJudgement::Incomplete => (ExprJudgement::Incomplete, format!("or_left:{bl}")),
        ExprJudgement::NotEvaluated => {
            let (jr, br) = eval_expr(right, alias, bindings, force);
            match jr {
                ExprJudgement::NotEvaluated => {
                    (ExprJudgement::NotEvaluated, format!("or_skip:{bl}"))
                }
                ExprJudgement::ProvenTrue => (ExprJudgement::ProvenTrue, format!("or:{bl}|{br}")),
                other => {
                    if force {
                        (ExprJudgement::Incomplete, format!("or_partial:{bl}|{br}"))
                    } else {
                        (other, format!("or_right_only:{br}"))
                    }
                }
            }
        }
    }
}

fn eval_cmp(
    full: &Expr,
    left: &Expr,
    right: &Expr,
    bindings: &ExprBindings,
    force_attempt: bool,
) -> (ExprJudgement, String) {
    let Expr::Binary { op, .. } = full else {
        return (ExprJudgement::Incomplete, "internal_cmp_shape".to_string());
    };
    let Some(l) = int_value(left, bindings) else {
        return skip_or_incomplete(
            force_attempt || contains_region_atom(full),
            "cmp_left_unbound".to_string(),
            "cmp_unbound_not_evaluated",
        );
    };
    let Some(r) = int_value(right, bindings) else {
        return skip_or_incomplete(
            force_attempt || contains_region_atom(full),
            "cmp_right_unbound".to_string(),
            "cmp_unbound_not_evaluated",
        );
    };
    let ok = match op {
        BinOp::Eq => l == r,
        BinOp::Ne => l != r,
        BinOp::Lt => l < r,
        BinOp::Le => l <= r,
        BinOp::Gt => l > r,
        BinOp::Ge => l >= r,
        _ => return (ExprJudgement::Incomplete, "cmp_bad_op".to_string()),
    };
    (
        if ok {
            ExprJudgement::ProvenTrue
        } else {
            ExprJudgement::ProvenFalse
        },
        format!("cmp:{l}?{r}"),
    )
}

fn int_value(expr: &Expr, bindings: &ExprBindings) -> Option<i64> {
    match expr {
        Expr::Int { value, .. } => Some(*value),
        Expr::Ident { name, .. } => bindings.ints.get(name).copied(),
        _ => None,
    }
}

fn eval_call(
    expr: &Expr,
    alias: Option<&AliasAnalysisReport>,
    force_attempt: bool,
) -> (ExprJudgement, String) {
    let Expr::Call { callee, args, .. } = expr else {
        return (ExprJudgement::Incomplete, "internal_call_shape".to_string());
    };
    let Some((pred, left, right)) = match_regions_atom(callee, args) else {
        if force_attempt || looks_like_regions_receiver(callee) {
            return (
                ExprJudgement::Incomplete,
                "unknown_regions_predicate".to_string(),
            );
        }
        return (
            ExprJudgement::NotEvaluated,
            "non_subset_call_not_evaluated".to_string(),
        );
    };

    let Some(alias) = alias else {
        // No alias slice on this target (e.g. A64/RV today): do not fail-closed
        // the expression wave — omit via not_evaluated aggregation.
        return (
            ExprJudgement::NotEvaluated,
            format!("regions_{pred}_no_alias_slice"),
        );
    };

    match lookup_relation_row(alias, &left, &right) {
        None => (
            ExprJudgement::Incomplete,
            format!("no_alias_row:{left}/{right}"),
        ),
        Some((row, observed)) => match pred {
            "disjoint" => judge_require(RelationRequire::Disjoint, row, observed, &left, &right),
            "equal" => judge_require(RelationRequire::Equal, row, observed, &left, &right),
            "contains" => judge_require(RelationRequire::Contains, row, observed, &left, &right),
            _ => (ExprJudgement::Incomplete, format!("unknown_pred:{pred}")),
        },
    }
}

fn judge_require(
    require: RelationRequire,
    row: &crate::alias::AliasRelationEvidence,
    observed: RelationObserved,
    left: &str,
    right: &str,
) -> (ExprJudgement, String) {
    let ok = match require {
        RelationRequire::Disjoint => matches!(observed, RelationObserved::ProvenDisjoint),
        RelationRequire::Equal => matches!(observed, RelationObserved::ProvenEqual),
        RelationRequire::Contains => matches!(observed, RelationObserved::ProvenContains),
    };
    if ok {
        (
            ExprJudgement::ProvenTrue,
            format!(
                "regions_{}({left},{right})<= {}",
                require.as_str(),
                observed.as_str()
            ),
        )
    } else if row.evidence_basis == RelationEvidenceBasis::DeclaredPrecondition
        && row.required == require.as_str()
    {
        // Declared precondition for the same require — not a static proof.
        (
            ExprJudgement::TrueUnderPrecondition,
            format!(
                "regions_{}({left},{right})<=declared_precondition",
                require.as_str()
            ),
        )
    } else if matches!(
        observed,
        RelationObserved::MayOverlap
            | RelationObserved::NotEvaluated
            | RelationObserved::InvalidRegion
    ) {
        (
            ExprJudgement::Incomplete,
            format!(
                "regions_{}({left},{right})<= {}",
                require.as_str(),
                observed.as_str()
            ),
        )
    } else {
        // Proven fact that does not satisfy the atom → false.
        (
            ExprJudgement::ProvenFalse,
            format!(
                "regions_{}({left},{right})<= {}",
                require.as_str(),
                observed.as_str()
            ),
        )
    }
}

fn match_regions_atom<'a>(
    callee: &'a Expr,
    args: &'a [Expr],
) -> Option<(&'static str, String, String)> {
    let Expr::Member { base, field, .. } = callee else {
        return None;
    };
    let Expr::Ident { name, .. } = base.as_ref() else {
        return None;
    };
    if name != "regions" {
        return None;
    }
    let pred = match field.as_str() {
        "disjoint" => "disjoint",
        "equal" => "equal",
        "contains" => "contains",
        _ => return None,
    };
    if args.len() != 2 {
        return None;
    }
    let left = match &args[0] {
        Expr::Ident { name, .. } => name.clone(),
        _ => return None,
    };
    let right = match &args[1] {
        Expr::Ident { name, .. } => name.clone(),
        _ => return None,
    };
    Some((pred, left, right))
}

fn looks_like_regions_receiver(callee: &Expr) -> bool {
    matches!(
        callee,
        Expr::Member {
            base,
            ..
        } if matches!(base.as_ref(), Expr::Ident { name, .. } if name == "regions")
    )
}

fn contains_region_atom(expr: &Expr) -> bool {
    match expr {
        Expr::Call { callee, args, .. } => {
            if match_regions_atom(callee, args).is_some() || looks_like_regions_receiver(callee) {
                return true;
            }
            contains_region_atom(callee) || args.iter().any(contains_region_atom)
        }
        Expr::Unary { expr, .. } => contains_region_atom(expr),
        Expr::Binary { left, right, .. } => {
            contains_region_atom(left) || contains_region_atom(right)
        }
        Expr::Member { base, .. } => contains_region_atom(base),
        Expr::Index { base, index, .. } => {
            contains_region_atom(base) || contains_region_atom(index)
        }
        Expr::Range { start, end, .. } => contains_region_atom(start) || contains_region_atom(end),
        Expr::Ident { .. } | Expr::Int { .. } | Expr::Bool { .. } => false,
    }
}

fn lookup_relation_row<'a>(
    alias: &'a AliasAnalysisReport,
    left: &str,
    right: &str,
) -> Option<(&'a crate::alias::AliasRelationEvidence, RelationObserved)> {
    for row in &alias.relations {
        if row.left == left && row.right == right {
            return Some((row, row.observed));
        }
        // Symmetric for disjoint/equal; contains is not symmetric.
        if row.left == right && row.right == left {
            let observed = match row.observed {
                RelationObserved::ProvenContains => RelationObserved::MayOverlap,
                other => other,
            };
            return Some((row, observed));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alias::{evaluate_alias, REGION_AFFINE_V1};
    use crate::validate::{
        check_str, CheckedMemory, CheckedRegion, CheckedRelation, LengthSpec, RegionAccess,
        RelationRequire,
    };

    fn memcpy_with_region_requires() -> CheckedContract {
        check_str(
            r#"
contract_version = "0.1"
[function]
name = "memcpy"
[[function.parameters]]
name = "dst"
type = "ptr<u8>"
[[function.parameters]]
name = "src"
type = "ptr<const u8>"
[[function.parameters]]
name = "length"
type = "usize"
[[function.returns]]
name = "status"
type = "usize"
[[function.requires]]
expression = "regions.disjoint(src, dst)"
[[function.requires]]
expression = "length <= 4096"
[[function.ensures]]
expression = "status == 0"
[[function.memory.regions]]
name = "src"
base = "src"
length = "length"
access = "read"
[[function.memory.regions]]
name = "dst"
base = "dst"
length = "length"
access = "write"
[[function.memory.relations]]
left = "src"
right = "dst"
require = "disjoint"
basis = "precondition"
"#,
        )
        .contract
        .expect("contract")
    }

    #[test]
    fn disjoint_atom_true_under_precondition() {
        let contract = memcpy_with_region_requires();
        let memory = contract.memory.as_ref().unwrap();
        let alias = evaluate_alias(memory, &[]);
        assert_eq!(alias.model, REGION_AFFINE_V1);
        let report =
            evaluate_contract_expressions(&contract, Some(&alias), &ExprBindings::default())
                .expect("report");
        assert_eq!(report.status, ContractExprStatus::PassedUnderPreconditions);
        assert_eq!(report.model, CONTRACT_EXPR_V1);
        let region_row = report
            .expressions
            .iter()
            .find(|e| e.source.contains("regions.disjoint"))
            .unwrap();
        assert_eq!(region_row.judgement, ExprJudgement::TrueUnderPrecondition);
        let length_row = report
            .expressions
            .iter()
            .find(|e| e.source.contains("length"))
            .unwrap();
        assert_eq!(length_row.judgement, ExprJudgement::NotEvaluated);
    }

    #[test]
    fn unknown_predicate_incomplete() {
        let contract = check_str(
            r#"
contract_version = "0.1"
[function]
name = "f"
[[function.parameters]]
name = "dst"
type = "ptr<u8>"
[[function.parameters]]
name = "src"
type = "ptr<u8>"
[[function.returns]]
name = "status"
type = "usize"
[[function.requires]]
expression = "regions.frobnicate(src, dst)"
[[function.memory.regions]]
name = "src"
base = "src"
length = "1"
access = "read"
[[function.memory.regions]]
name = "dst"
base = "dst"
length = "1"
access = "write"
[[function.memory.relations]]
left = "src"
right = "dst"
require = "disjoint"
"#,
        )
        .contract
        .unwrap();
        let alias = evaluate_alias(contract.memory.as_ref().unwrap(), &[]);
        let report =
            evaluate_contract_expressions(&contract, Some(&alias), &ExprBindings::default())
                .unwrap();
        assert_eq!(report.status, ContractExprStatus::Incomplete);
    }

    #[test]
    fn equal_atom_false_when_proven_disjoint() {
        let memory = CheckedMemory {
            regions: vec![
                CheckedRegion {
                    name: "src".into(),
                    base_param: "buf".into(),
                    offset: 0,
                    length: LengthSpec::Literal(8),
                    access: RegionAccess::Read,
                },
                CheckedRegion {
                    name: "dst".into(),
                    base_param: "buf".into(),
                    offset: 8,
                    length: LengthSpec::Literal(8),
                    access: RegionAccess::Write,
                },
            ],
            relations: vec![CheckedRelation {
                left: "src".into(),
                right: "dst".into(),
                require: RelationRequire::Disjoint,
                basis: None,
            }],
        };
        let alias = evaluate_alias(&memory, &[]);
        let contract = check_str(
            r#"
contract_version = "0.1"
[function]
name = "f"
[[function.parameters]]
name = "buf"
type = "ptr<u8>"
[[function.returns]]
name = "status"
type = "usize"
[[function.requires]]
expression = "regions.equal(src, dst)"
[[function.memory.regions]]
name = "src"
base = "buf"
offset = "0"
length = "8"
access = "read"
[[function.memory.regions]]
name = "dst"
base = "buf"
offset = "8"
length = "8"
access = "write"
[[function.memory.relations]]
left = "src"
right = "dst"
require = "disjoint"
"#,
        )
        .contract
        .unwrap();
        let report =
            evaluate_contract_expressions(&contract, Some(&alias), &ExprBindings::default())
                .unwrap();
        assert_eq!(report.status, ContractExprStatus::Failed);
        assert_eq!(report.expressions[0].judgement, ExprJudgement::ProvenFalse);
    }

    #[test]
    fn no_subset_atoms_omits_report() {
        let contract = check_str(
            r#"
contract_version = "0.1"
[function]
name = "f"
[[function.parameters]]
name = "length"
type = "usize"
[[function.returns]]
name = "status"
type = "usize"
[[function.requires]]
expression = "length <= 4096"
[[function.ensures]]
expression = "status == 0"
"#,
        )
        .contract
        .unwrap();
        assert!(evaluate_contract_expressions(&contract, None, &ExprBindings::default()).is_none());
    }

    #[test]
    fn closed_comparison_with_binding() {
        let contract = check_str(
            r#"
contract_version = "0.1"
[function]
name = "f"
[[function.parameters]]
name = "length"
type = "usize"
[[function.returns]]
name = "status"
type = "usize"
[[function.requires]]
expression = "length <= 4096"
"#,
        )
        .contract
        .unwrap();
        let mut bindings = ExprBindings::default();
        bindings.ints.insert("length".into(), 16);
        let report = evaluate_contract_expressions(&contract, None, &bindings).unwrap();
        assert_eq!(report.status, ContractExprStatus::Passed);
        assert_eq!(report.expressions[0].judgement, ExprJudgement::ProvenTrue);
    }
}
