//! Portable semantic contracts for assembly programs.
//!
//! Contracts describe intent, ABI-facing parameters, effects, and constraints.
//! They are not an implementation language.
//!
//! Compatibility policy for unknown fields and versions lives in
//! `COMPATIBILITY.md` at the crate root.

#![forbid(unsafe_code)]

pub mod codes;
pub mod expr;
pub mod schema;
pub mod sem_type;
pub mod validate;

pub use codes::ContractCode;
pub use expr::{BinOp, Expr, UnaryOp};
pub use schema::{
    ConditionSchema, ConstraintsSchema, ContractDocument, EffectSchema, FunctionSchema,
    ParameterSchema, ReturnSchema, TargetOverrideSchema,
};
pub use sem_type::SemType;
pub use validate::{
    check_file, check_str, format_diagnostics_terminal, CheckReportJson, CheckResult,
    CheckedCondition, CheckedContract, CheckedParam, CheckedReturn, DiagnosticJson,
};

/// Explain a stable contract diagnostic code.
#[must_use]
pub fn explain_code(code: &str) -> Option<&'static str> {
    ContractCode::parse(code).map(ContractCode::explain)
}
