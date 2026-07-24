//! Load and validate contract documents into a checked form.

use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::Path;

use semasm_core::{Diagnostic, DiagnosticLevel, Diagnostics, SourceSpan};
use semasm_target::TargetIdentity;
use serde::{Deserialize, Serialize};

use crate::codes::ContractCode;
use crate::expr::Expr;
use crate::schema::{ContractDocument, EffectSchema};
use crate::sem_type::SemType;

/// Validated contract ready for later analysis stages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
    /// Optional Region/Alias Evidence v1 block.
    pub memory: Option<CheckedMemory>,
    /// Constraints passthrough.
    pub constraints: Option<crate::schema::ConstraintsSchema>,
    /// Validated target override IDs.
    pub target_overrides: Vec<String>,
}

/// Declared access mode for a memory region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RegionAccess {
    /// Read-only.
    Read,
    /// Write-only.
    Write,
    /// Read and write.
    ReadWrite,
}

impl RegionAccess {
    /// Parse from contract string.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "read" => Some(Self::Read),
            "write" => Some(Self::Write),
            "read_write" => Some(Self::ReadWrite),
            _ => None,
        }
    }

    /// Canonical string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::ReadWrite => "read_write",
        }
    }
}

/// Required relation kind (narrow v1 subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RelationRequire {
    /// Regions must not overlap.
    Disjoint,
    /// Regions must be identical.
    Equal,
    /// Left must contain right.
    Contains,
}

impl RelationRequire {
    /// Parse from contract string.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "disjoint" => Some(Self::Disjoint),
            "equal" => Some(Self::Equal),
            "contains" => Some(Self::Contains),
            _ => None,
        }
    }

    /// Canonical string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disjoint => "disjoint",
            Self::Equal => "equal",
            Self::Contains => "contains",
        }
    }
}

/// Length of a declared region.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum LengthSpec {
    /// Named integer parameter.
    Param(String),
    /// Decimal literal byte count.
    Literal(u64),
}

/// Checked affine memory region.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CheckedRegion {
    /// Region name.
    pub name: String,
    /// Pointer parameter used as base.
    pub base_param: String,
    /// Constant byte offset from base.
    pub offset: i64,
    /// Region length.
    pub length: LengthSpec,
    /// Declared access.
    pub access: RegionAccess,
}

/// Checked required relation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CheckedRelation {
    /// Left region name.
    pub left: String,
    /// Right region name.
    pub right: String,
    /// Required relation.
    pub require: RelationRequire,
}

/// Checked `[function.memory]` block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CheckedMemory {
    /// Declared regions.
    pub regions: Vec<CheckedRegion>,
    /// Required relations.
    pub relations: Vec<CheckedRelation>,
}

/// Checked parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CheckedParam {
    /// Name.
    pub name: String,
    /// Parsed type.
    pub ty: SemType,
    /// Original type string as written in the contract (e.g. `ptr<const u8>`).
    pub type_source: String,
    /// Role.
    pub role: Option<String>,
}

/// Checked return.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CheckedReturn {
    /// Name.
    pub name: String,
    /// Parsed type.
    pub ty: SemType,
    /// Original type string as written in the contract.
    pub type_source: String,
}

/// Checked condition with AST.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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

#[allow(clippy::too_many_lines)]
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
                type_source: p.ty.clone(),
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
                type_source: r.ty.clone(),
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
    let mut expr_names = names.clone();
    // Reserved receiver for contract-expr-v1 region atoms (ADR 0007).
    expr_names.insert("regions".to_string());
    if let Some(mem) = &f.memory {
        for region in &mem.regions {
            expr_names.insert(region.name.clone());
        }
    }
    for c in &f.requires {
        if let Some(cond) = check_condition(
            c.expression.as_str(),
            c.reason.clone(),
            &expr_names,
            diagnostics,
        ) {
            requires.push(cond);
        }
    }
    let mut ensures = Vec::new();
    for c in &f.ensures {
        if let Some(cond) = check_condition(
            c.expression.as_str(),
            c.reason.clone(),
            &expr_names,
            diagnostics,
        ) {
            ensures.push(cond);
        }
    }

    check_effects(&f.effects, &names, diagnostics);
    let memory = check_memory(f.memory.as_ref(), &parameters, diagnostics);

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
            memory,
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

#[allow(clippy::too_many_lines)]
fn check_memory(
    memory: Option<&crate::schema::MemoryBlockSchema>,
    parameters: &[CheckedParam],
    diagnostics: &mut Diagnostics,
) -> Option<CheckedMemory> {
    let block = memory?;

    let mut region_names = BTreeSet::new();
    let mut regions = Vec::new();

    for region in &block.regions {
        if !region_names.insert(region.name.clone()) {
            push_code(
                diagnostics,
                ContractCode::Ctr008,
                format!("duplicate memory region `{}`", region.name),
                None,
            );
            continue;
        }

        let Some(base_param) = parameters.iter().find(|p| p.name == region.base) else {
            push_code(
                diagnostics,
                ContractCode::Ctr008,
                format!(
                    "memory region `{}`: base `{}` is not a parameter",
                    region.name, region.base
                ),
                None,
            );
            continue;
        };
        if !matches!(base_param.ty, SemType::Ptr { .. }) {
            push_code(
                diagnostics,
                ContractCode::Ctr008,
                format!(
                    "memory region `{}`: base `{}` must be a pointer parameter",
                    region.name, region.base
                ),
                None,
            );
            continue;
        }

        let offset = match region.offset.as_deref() {
            None | Some("" | "0") => 0_i64,
            Some(raw) => {
                if let Some(v) = parse_i64_literal(raw) {
                    v
                } else {
                    push_code(
                        diagnostics,
                        ContractCode::Ctr008,
                        format!(
                            "memory region `{}`: offset `{raw}` must be a decimal literal",
                            region.name
                        ),
                        None,
                    );
                    continue;
                }
            }
        };

        let length = match parse_length_spec(&region.length, parameters) {
            Ok(spec) => spec,
            Err(msg) => {
                push_code(
                    diagnostics,
                    ContractCode::Ctr008,
                    format!("memory region `{}`: {msg}", region.name),
                    None,
                );
                continue;
            }
        };

        let Some(access) = RegionAccess::parse(&region.access) else {
            push_code(
                diagnostics,
                ContractCode::Ctr008,
                format!(
                    "memory region `{}`: access `{}` must be read|write|read_write",
                    region.name, region.access
                ),
                None,
            );
            continue;
        };

        regions.push(CheckedRegion {
            name: region.name.clone(),
            base_param: region.base.clone(),
            offset,
            length,
            access,
        });
    }

    let mut relations = Vec::new();
    for rel in &block.relations {
        if !region_names.contains(&rel.left) || !region_names.contains(&rel.right) {
            push_code(
                diagnostics,
                ContractCode::Ctr008,
                format!(
                    "memory relation `{}`/`{}`: endpoints must name declared regions",
                    rel.left, rel.right
                ),
                None,
            );
            continue;
        }
        let Some(require) = RelationRequire::parse(&rel.require) else {
            push_code(
                diagnostics,
                ContractCode::Ctr008,
                format!(
                    "memory relation `{}`/`{}`: require `{}` must be disjoint|equal|contains",
                    rel.left, rel.right, rel.require
                ),
                None,
            );
            continue;
        };
        relations.push(CheckedRelation {
            left: rel.left.clone(),
            right: rel.right.clone(),
            require,
        });
    }

    Some(CheckedMemory { regions, relations })
}

fn parse_length_spec(raw: &str, parameters: &[CheckedParam]) -> Result<LengthSpec, String> {
    if let Some(lit) = parse_u64_literal(raw) {
        return Ok(LengthSpec::Literal(lit));
    }
    let Some(param) = parameters.iter().find(|p| p.name == raw) else {
        return Err(format!(
            "length `{raw}` must be an integer parameter or decimal literal"
        ));
    };
    if !is_integer_sem_type(&param.ty) {
        return Err(format!("length parameter `{raw}` must be an integer type"));
    }
    Ok(LengthSpec::Param(raw.to_string()))
}

fn is_integer_sem_type(ty: &SemType) -> bool {
    matches!(
        ty,
        SemType::UInt { .. } | SemType::Int { .. } | SemType::Usize | SemType::Isize
    )
}

fn parse_u64_literal(raw: &str) -> Option<u64> {
    let s = raw.trim();
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn parse_i64_literal(raw: &str) -> Option<i64> {
    let s = raw.trim();
    if let Some(rest) = s.strip_prefix('-') {
        let v = parse_u64_literal(rest)?;
        return i64::try_from(v).ok().map(|n| -n);
    }
    parse_u64_literal(s).and_then(|v| i64::try_from(v).ok())
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
        let _ = writeln!(out, "{path}: {level}: {code}{}", d.message);
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
    fn accepts_memory_regions_and_relations() {
        let r = check_str(
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
[[function.memory.regions]]
name = "src"
base = "src"
length = "length"
access = "read"
[[function.memory.regions]]
name = "dst"
base = "dst"
offset = "0"
length = "length"
access = "write"
[[function.memory.relations]]
left = "src"
right = "dst"
require = "disjoint"
"#,
        );
        assert!(r.ok(), "{:?}", r.diagnostics.into_vec());
        let mem = r.contract.unwrap().memory.unwrap();
        assert_eq!(mem.regions.len(), 2);
        assert_eq!(mem.relations.len(), 1);
    }

    #[test]
    fn rejects_invalid_memory_base() {
        let r = check_str(
            r#"
contract_version = "0.1"
[function]
name = "f"
[[function.parameters]]
name = "length"
type = "usize"
[[function.memory.regions]]
name = "buf"
base = "length"
length = "1"
access = "read"
"#,
        );
        assert!(r
            .diagnostics
            .iter()
            .any(|d| d.code.as_deref() == Some("CTR008")));
    }

    #[test]
    fn json_report_serializes() {
        let r = check_str(WRITE_ALL);
        let report = CheckReportJson::from_result("contracts/write_all.sem.toml", &r);
        let s = serde_json::to_string_pretty(&report).unwrap();
        assert!(s.contains("write_all"));
    }
}
