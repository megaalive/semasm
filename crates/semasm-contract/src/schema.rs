//! Serde schema for portable TOML function contracts.

use serde::{Deserialize, Serialize};

/// Top-level contract document (one primary function per file in v0.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractDocument {
    /// Schema version string, must be `"0.1"`.
    #[serde(default = "default_contract_version")]
    pub contract_version: String,
    /// Function contract body.
    pub function: FunctionSchema,
}

fn default_contract_version() -> String {
    "0.1".to_string()
}

/// Function-level contract fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunctionSchema {
    /// Symbol name.
    pub name: String,
    /// Short human summary.
    #[serde(default)]
    pub summary: Option<String>,
    /// Visibility hint (`internal`, `public`, ...).
    #[serde(default)]
    pub visibility: Option<String>,
    /// Parameters.
    #[serde(default)]
    pub parameters: Vec<ParameterSchema>,
    /// Return values (multiple allowed for multi-result ABIs later).
    #[serde(default)]
    pub returns: Vec<ReturnSchema>,
    /// Preconditions.
    #[serde(default)]
    pub requires: Vec<ConditionSchema>,
    /// Postconditions.
    #[serde(default)]
    pub ensures: Vec<ConditionSchema>,
    /// Declared effects.
    #[serde(default)]
    pub effects: Vec<EffectSchema>,
    /// Resource / complexity constraints.
    #[serde(default)]
    pub constraints: Option<ConstraintsSchema>,
    /// Per-target overrides.
    #[serde(default)]
    pub target_overrides: Vec<TargetOverrideSchema>,
}

/// Parameter declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterSchema {
    /// Parameter name.
    pub name: String,
    /// Semantic type string.
    #[serde(rename = "type")]
    pub ty: String,
    /// Role such as `input` or `output_buffer`.
    #[serde(default)]
    pub role: Option<String>,
}

/// Return value declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReturnSchema {
    /// Return slot name.
    pub name: String,
    /// Semantic type string.
    #[serde(rename = "type")]
    pub ty: String,
}

/// Require/ensure condition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConditionSchema {
    /// Expression source.
    pub expression: String,
    /// Optional human reason.
    #[serde(default)]
    pub reason: Option<String>,
}

/// Side-effect declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct EffectSchema {
    /// Effect kind (`memory_read`, `memory_write`, `platform_io`, `no_memory`, ...).
    pub kind: String,
    /// Optional region expression (for example `buffer[0..length]`).
    #[serde(default)]
    pub region: Option<String>,
    /// Optional platform resource (`stdout`, ...).
    #[serde(default)]
    pub resource: Option<String>,
}

/// Constraints block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ConstraintsSchema {
    /// Disallow heap allocation.
    #[serde(default)]
    pub no_heap: Option<bool>,
    /// Disallow recursion.
    #[serde(default)]
    pub no_recursion: Option<bool>,
    /// Maximum stack bytes if known.
    #[serde(default)]
    pub bounded_stack_bytes: Option<u64>,
}

/// Target-specific override table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetOverrideSchema {
    /// Target identity string.
    pub target: String,
    /// Optional extra notes for agents.
    #[serde(default)]
    pub notes: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_toml() {
        let raw = r#"
contract_version = "0.1"

[function]
name = "demo"
summary = "demo"
visibility = "internal"

[[function.parameters]]
name = "x"
type = "u32"
role = "input"

[[function.returns]]
name = "y"
type = "u32"

[[function.requires]]
expression = "true"

[[function.ensures]]
expression = "y == x"

[[function.effects]]
kind = "no_memory"

[function.constraints]
no_heap = true
"#;
        let doc: ContractDocument = toml::from_str(raw).unwrap();
        let encoded = toml::to_string(&doc).unwrap();
        let doc2: ContractDocument = toml::from_str(&encoded).unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn rejects_unknown_field() {
        let raw = r#"
contract_version = "0.1"
[function]
name = "demo"
loop_body = "for i in ..."
"#;
        let err = toml::from_str::<ContractDocument>(raw).unwrap_err();
        assert!(err.to_string().contains("loop_body") || err.to_string().contains("unknown"));
    }
}
