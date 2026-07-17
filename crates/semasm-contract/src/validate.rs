//! Load and validate contract documents into a checked form.

use std::collections::BTreeSet;
use std::path::Path;

use semasm_core::{Diagnostic, DiagnosticLevel, Diagnostics, SourceSpan};
use semasm_target::TargetIdentity;
use serde::{Deserialize, Serialize};

use crate::codes::ContractCode;
use crate::expr::Expr;
use crate::schema::{ContractDocument, EffectSchema, FunctionSchema};
use crate::sem_type::SemType;

/// Validated contract ready for later analysis stages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedContract {
    /// Schema version major/minor.
    pub version_major: u32,
    /// Schema version minor.
    pub version_minor: u32,
    /// Function name.
    pub name: String,
    /// Optional summary.
    pub summary: Option<String>,
    /// Parameters with parsed types.
    pub parameters: Vec<CheckedParam>,
    /// Returns with parsed types.
    pub returns: Vec<CheckedReturn>,
    /// Parsed requires.
    pub requires: Vec<CheckedCondition>,
    /// Parsed ensures.
    pub ensures: Vec<CheckedCondition>,
    /// Checked effects.
    pub effects: Vec<EffectSchema>,
    /// Constraints passthrough.
    pub constraints: Option<crate::schema::ConstraintsSchema>,
    /// Validated target override IDs.
    pub target_overrides: Vec<String>,
}

/// Checked parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedParam {
    /// Name.
    pub name: String,
    /// Parsed type.
    pub ty: SemType,
    /// Role.
    pub role: Option<String>,
}

/// Checked return.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedReturn {
    /// Name.
    pub name: String,
    /// Parsed type.
    pub ty: SemType,
}

/// Checked condition with AST.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedCondition {
    /// Expression AST.
    pub expr: Expr,
    /// Original source.
    pub source: String,
    /// Optional reason.
    pub reason: Option<String>,
}

/// Result of checking a contract file or string.
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// Checked contract when no errors.
    pub contract: Option<CheckedContract>,
    /// Diagnostics (errors and notes).
    pub diagnostics: Diagnostics,
}

impl CheckResult {
    /// Whether validation succeeded.
    #[must_use]
    pub fn ok(&self) -> bool {
        !self.diagnostics.has_errors() && self.contract.is_some()
    }
}

/// Structured diagnostic for JSON output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticJson {
    /// Level string.
    pub level: String,
    /// Message.
    pub message: String,
    /// Optional code.
    pub code: Option<String>,
    /// Optional span.
    pub span: Option<SpanJson>,
}

/// JSON span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanJson {
    /// Start byte.
    pub start: u32,
    /// End byte.
    pub end: u32,
}

/// Full check report for CLI JSON mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckReportJson {
    /// Path checked.
    pub path: String,
    /// Success flag.
    pub ok: bool,
    /// Diagnostics.
    pub diagnostics: Vec<DiagnosticJson>,
    /// Checked contract when ok.
    pub contract: Option<CheckedContract>,
}

impl CheckReportJson {
    /// Build from a path and result.
    #[must_use]
    pub fn from_result(path: &str, result: &CheckResult) -> Self {
        Self {
            path: path.to_string(),
            ok: result.ok(),
            diagnostics: result
                .diagnostics
                .iter()
                .map(|d| DiagnosticJson {
                    level: match d.level {
                        DiagnosticLevel::Note => "note".into(),
                        DiagnosticLevel::Warning => "warning".into(),
                        DiagnosticLevel::Error => "error".into(),
                    },
                    message: d.message.clone(),
                    code: d.code.clone(),
                    span: d.span.map(|s| SpanJson {
                        start: s.start.get(),
                        end: s.end.get(),
                    }),
                })
                .collect(),
            contract: result.contract.clone(),
        }
    }
}

/// Load and validate a contract TOML file.
pub fn check_file(path: &Path) -> std::io::Result<CheckResult> {
    let text = std::fs::read_to_string(path)?;
    Ok(check_str(&text))
}

/// Validate contract TOML source.
#[must_use]
pub fn check_str(text: &str) -> CheckResult {
    let mut diagnostics = Diagnostics::new();

    let doc: ContractDocument = match toml::from_str(text) {
        Ok(d) => d,
        Err(e) => {
            diagnostics.push(
                Diagnostic::error(format!("failed to parse contract TOML: {e}"))
                    .with_code("CTR000"),
            );
            return CheckResult {
                contract: None,
                diagnostics,
            };
        }
    };

    validate_document(&doc, &mut diagnostics)
}

fn validate_document(doc: &ContractDocument, diagnostics: &mut Diagnostics) -> CheckResult {
    if doc.contract_version != "0.1" {
        push_code(
            diagnostics,
            ContractCode::Ctr001,
            format!(
                "unsupported contract version `{}` (only \"0.1\" is accepted)",
                doc.contract_version
            ),
            None,
        );
        return CheckResult {
            contract: None,
            diagnostics: diagnostics.clone(),
        };
    }

    let f = &doc.function;
    let mut parameters = Vec::new();
    let mut returns = Vec::new();
    let mut names = BTreeSet::new();

    for p in &f.parameters {
        if !names.insert(p.name.clone()) {
            push_code(
                diagnostics,
                ContractCode::Ctr002,
                format!("duplicate parameter `{}`", p.name),
                None,
            );
        }
        match SemType::parse(&p.ty) {
            Ok((ty, _)) => parameters.push(CheckedParam {
                name: p.name.clone(),
                ty,
                role: p.role.clone(),
            }),
            Err(msg) => push_code(
                diagnostics,
                ContractCode::Ctr003,
                format!("parameter `{}`: {msg}", p.name),
                None,
            ),
        }
    }

    for r in &f.returns {
        names.insert(r.name.clone());
        match SemType::parse(&r.ty) {
            Ok((ty, _)) => returns.push(CheckedReturn {
                name: r.name.clone(),
                ty,
            }),
            Err(msg) => push_code(
                diagnostics,
                ContractCode::Ctr003,
                format!("return `{}`: {msg}", r.name),
                None,
            ),
        }
    }

    let mut requires = Vec::new();
    for c in &f.requires {
        if let Some(cond) =
            check_condition(c.expression.as_str(), c.reason.clone(), &names, diagnostics)
        {
            requires.push(cond);
        }
    }
    let mut ensures = Vec::new();
    for c in &f.ensures {
        if let Some(cond) =
            check_condition(c.expression.as_str(), c.reason.clone(), &names, diagnostics)
        {
            ensures.push(cond);
        }
    }

    check_effects(&f.effects, &names, diagnostics);

    let mut target_overrides = Vec::new();
    for ov in &f.target_overrides {
        match TargetIdentity::parse_known(&ov.target) {
            Ok(t) => target_overrides.push(t.name),
            Err(e) => push_code(
                diagnostics,
                ContractCode::Ctr007,
                format!("invalid target override: {e}"),
                None,
            ),
        }
    }

    if diagnostics.has_errors() {
        return CheckResult {
            contract: None,
            diagnostics: diagnostics.clone(),
        };
    }

    CheckResult {
        contract: Some(CheckedContract {
            version_major: 0,
            version_minor: 1,
            name: f.name.clone(),
            summary: f.summary.clone(),
            parameters,
            returns,
            requires,
            ensures,
            effects: f.effects.clone(),
            constraints: f.constraints.clone(),
            target_overrides,
        }),
        diagnostics: diagnostics.clone(),
    }
}

fn check_condition(
    source: &str,
    reason: Option<String>,
    names: &BTreeSet<String>,
    diagnostics: &mut Diagnostics,
) -> Option<CheckedCondition> {
    match Expr::parse(source) {
        Ok(expr) => {
            for id in expr.free_idents() {
                if !names.contains(&id) {
                    push_code(
                        diagnostics,
                        ContractCode::Ctr005,
                        format!("unknown identifier `{id}` in expression `{source}`"),
                        None,
                    );
                }
            }
            Some(CheckedCondition {
                expr,
                source: source.to_string(),
                reason,
            })
        }
        Err(msg) => {
            push_code(
                diagnostics,
                ContractCode::Ctr004,
                format!("invalid expression `{source}`: {msg}"),
                None,
            );
            None
        }
    }
}

fn check_effects(
    effects: &[EffectSchema],
    names: &BTreeSet<String>,
    diagnostics: &mut Diagnostics,
) {
    let mut kinds = BTreeSet::new();
    for e in effects {
        kinds.insert(e.kind.as_str());
        if let Some(region) = &e.region {
            // Region is an expression-like fragment; parse as index expression when possible.
            let wrapped = if region.contains('[') {
                region.clone()
            } else {
                // bare name ok
                region.clone()
            };
            if wrapped.contains('[') {
                if let Err(msg) = Expr::parse(&wrapped) {
                    push_code(
                        diagnostics,
                        ContractCode::Ctr004,
                        format!("invalid effect region `{region}`: {msg}"),
                        None,
                    );
                } else if let Ok(expr) = Expr::parse(&wrapped) {
                    for id in expr.free_idents() {
                        if !names.contains(&id) {
                            push_code(
                                diagnostics,
                                ContractCode::Ctr005,
                                format!("unknown identifier `{id}` in effect region `{region}`"),
                                None,
                            );
                        }
                    }
                }
            } else if !names.contains(region.as_str())
                && !region
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                // leave free-form resources alone
            }
        }
    }

    let no_mem = kinds.contains("no_memory");
    let reads = kinds.contains("memory_read");
    let writes = kinds.contains("memory_write");
    if no_mem && (reads || writes) {
        push_code(
            diagnostics,
            ContractCode::Ctr006,
            "contradictory memory effects: `no_memory` combined with memory_read/memory_write"
                .into(),
            None,
        );
    }
}

fn push_code(
    diagnostics: &mut Diagnostics,
    code: ContractCode,
    message: String,
    span: Option<SourceSpan>,
) {
    let mut d = Diagnostic::error(message).with_code(code.as_str());
    if let Some(span) = span {
        d = d.with_span(span);
    }
    diagnostics.push(d);
}

/// Format diagnostics for a terminal.
#[must_use]
pub fn format_diagnostics_terminal(path: &str, diagnostics: &Diagnostics) -> String {
    let mut out = String::new();
    for d in diagnostics.iter() {
        let level = match d.level {
            DiagnosticLevel::Note => "note",
            DiagnosticLevel::Warning => "warning",
            DiagnosticLevel::Error => "error",
        };
        let code = d
            .code
            .as_ref()
            .map_or_else(String::new, |c| format!("[{c}] "));
        out.push_str(&format!("{path}: {level}: {code}{}\n", d.message));
    }
    out
}

/// Re-export for tests that build documents.
pub use crate::schema::ContractDocument as Document;

#[cfg(test)]
mod tests {
    use super::*;

    const WRITE_ALL: &str = r#"
contract_version = "0.1"

[function]
name = "write_all"
summary = "Write all requested bytes or return an explicit failure status."
visibility = "internal"

[[function.parameters]]
name = "buffer"
type = "ptr<const u8>"
role = "input"

[[function.parameters]]
name = "length"
type = "usize"
role = "input"

[[function.returns]]
name = "written"
type = "usize"

[[function.returns]]
name = "status"
type = "status"

[[function.requires]]
expression = "buffer.valid_for_read(length)"
reason = "The function reads exactly length bytes from buffer."

[[function.ensures]]
expression = "status.ok implies written == length"

[[function.effects]]
kind = "memory_read"
region = "buffer[0..length]"

[[function.effects]]
kind = "platform_io"
resource = "stdout"

[function.constraints]
no_heap = true
no_recursion = true
bounded_stack_bytes = 128
"#;

    #[test]
    fn accepts_write_all() {
        let r = check_str(WRITE_ALL);
        assert!(r.ok(), "{:?}", r.diagnostics.into_vec());
        let c = r.contract.unwrap();
        assert_eq!(c.name, "write_all");
        assert_eq!(c.parameters.len(), 2);
    }

    #[test]
    fn rejects_bad_version() {
        let r = check_str(
            r#"
contract_version = "9.9"
[function]
name = "x"
"#,
        );
        assert!(!r.ok());
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.code.as_deref() == Some("CTR001")));
    }

    #[test]
    fn rejects_duplicate_param() {
        let r = check_str(
            r#"
contract_version = "0.1"
[function]
name = "x"
[[function.parameters]]
name = "a"
type = "u8"
[[function.parameters]]
name = "a"
type = "u8"
"#,
        );
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.code.as_deref() == Some("CTR002")));
    }

    #[test]
    fn rejects_unknown_type() {
        let r = check_str(
            r#"
contract_version = "0.1"
[function]
name = "x"
[[function.parameters]]
name = "a"
type = "&str"
"#,
        );
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.code.as_deref() == Some("CTR003")));
    }

    #[test]
    fn rejects_unknown_ident() {
        let r = check_str(
            r#"
contract_version = "0.1"
[function]
name = "x"
[[function.requires]]
expression = "no_such_name == 1"
"#,
        );
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.code.as_deref() == Some("CTR005")));
    }

    #[test]
    fn rejects_contradictory_effects() {
        let r = check_str(
            r#"
contract_version = "0.1"
[function]
name = "x"
[[function.parameters]]
name = "buffer"
type = "ptr<u8>"
[[function.effects]]
kind = "no_memory"
[[function.effects]]
kind = "memory_write"
region = "buffer[0..1]"
"#,
        );
        // length missing may also flag CTR005; ensure CTR006 present
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.code.as_deref() == Some("CTR006")));
    }

    #[test]
    fn json_report_serializes() {
        let r = check_str(WRITE_ALL);
        let report = CheckReportJson::from_result("contracts/write_all.sem.toml", &r);
        let s = serde_json::to_string_pretty(&report).unwrap();
        assert!(s.contains("write_all"));
    }
}
