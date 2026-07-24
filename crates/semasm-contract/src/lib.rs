//! Portable semantic contracts for assembly programs.
//!
//! Contracts describe intent, ABI-facing parameters, effects, and constraints.
//! They are not an implementation language.
//!
//! Compatibility policy for unknown fields and versions lives in
//! `COMPATIBILITY.md` at the crate root.

#![forbid(unsafe_code)]

pub mod alias;
pub mod codes;
pub mod eval;
pub mod expr;
pub mod schema;
pub mod sem_type;
pub mod validate;

pub use alias::{
    evaluate_alias, AccessAddr, AccessMode, AliasAnalysisReport, AliasRelationEvidence,
    AliasStatus, ObservedMemoryAccess, RelationEvidenceBasis, RelationObserved, REGION_AFFINE_V1,
};
pub use codes::ContractCode;
pub use eval::{
    evaluate_contract_expressions, ContractExprEvidence, ContractExprReport, ContractExprStatus,
    ExprBindings, ExprJudgement, ExprRole, CONTRACT_EXPR_V1,
};
pub use expr::{BinOp, Expr, UnaryOp};
pub use schema::{
    ConditionSchema, ConstraintsSchema, ContractDocument, EffectSchema, FunctionSchema,
    MemoryBlockSchema, MemoryRegionSchema, MemoryRelationSchema, ParameterSchema, ReturnSchema,
    TargetOverrideSchema,
};
pub use sem_type::SemType;
pub use validate::{
    check_file, check_str, format_diagnostics_terminal, CheckReportJson, CheckResult,
    CheckedCondition, CheckedContract, CheckedMemory, CheckedParam, CheckedRegion, CheckedRelation,
    CheckedReturn, DiagnosticJson, LengthSpec, RegionAccess, RelationBasisDecl, RelationRequire,
};

/// Explain a stable contract diagnostic code.
#[must_use]
pub fn explain_code(code: &str) -> Option<&'static str> {
    ContractCode::parse(code).map(ContractCode::explain)
}
