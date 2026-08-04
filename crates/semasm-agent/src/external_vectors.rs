//! Untrusted, caller-supplied vector inputs for agent verification.
//!
//! External documents intentionally contain inputs only. Expected values are
//! derived by a recognized builtin oracle before candidate code is executed.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Current schema for external vector input documents.
pub const EXTERNAL_VECTOR_SCHEMA_VERSION: &str = "0.1";

/// A fail-closed external vector document.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ExternalVectorDocument {
    /// External vector document schema (`MAJOR.MINOR`).
    pub schema_version: String,
    /// Digest of the exact contract bytes used for verification.
    pub contract_digest: String,
    /// Target triple to which these cases are bound.
    pub target: String,
    /// Routine symbol to which these cases are bound.
    pub routine_symbol: String,
    /// Additive cases. Builtin vectors are never replaced.
    pub cases: Vec<ExternalVectorCase>,
}

/// One named set of inputs; expected output is deliberately absent.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ExternalVectorCase {
    /// Stable caller-provided case id, unique within the document.
    pub id: String,
    /// Inputs keyed by the contract parameter names.
    pub inputs: BTreeMap<String, serde_json::Value>,
}

/// Origin of a case in the merged vector set.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum VectorOrigin {
    /// Deterministic vector synthesized by SemASM.
    Builtin,
    /// Input loaded from an external vector document.
    External,
}

/// Per-case binding recorded in a verification report.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct VectorCaseBinding {
    /// Case name as shown in behavioral results.
    pub name: String,
    /// Trusted origin of the case.
    pub origin: VectorOrigin,
    /// Caller case id for external cases.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_case_id: Option<String>,
}

/// Integrity binding for the complete vector set used by verification.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct VectorSetEvidence {
    /// Canonical SHA-256 of the external document, when supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_document_digest: Option<String>,
    /// Number of always-retained builtin cases.
    pub builtin_case_count: usize,
    /// Number of additive external cases.
    pub external_case_count: usize,
    /// Ordered binding matching the behavioral result order.
    pub cases: Vec<VectorCaseBinding>,
}

/// Hash a document after recursively sorting all JSON object keys.
#[must_use]
pub fn canonical_document_digest(document: &ExternalVectorDocument) -> String {
    let value = serde_json::to_value(document).expect("serializable external vector document");
    let canonical = canonicalize(value);
    let bytes = serde_json::to_vec(&canonical).expect("serializable canonical JSON");
    crate::verify::sha256_digest_prefixed(&bytes)
}

fn canonicalize(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let sorted = map
                .into_iter()
                .map(|(key, value)| (key, canonicalize(value)))
                .collect();
            serde_json::Value::Object(sorted)
        }
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(canonicalize).collect())
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_digest_ignores_input_object_key_order() {
        let a: ExternalVectorDocument = serde_json::from_str(
            r#"{"schema_version":"0.1","contract_digest":"sha256:x","target":"t","routine_symbol":"f","cases":[{"id":"c","inputs":{"b":2,"a":1}}]}"#,
        )
        .unwrap();
        let b: ExternalVectorDocument = serde_json::from_str(
            r#"{"schema_version":"0.1","contract_digest":"sha256:x","target":"t","routine_symbol":"f","cases":[{"id":"c","inputs":{"a":1,"b":2}}]}"#,
        )
        .unwrap();
        assert_eq!(canonical_document_digest(&a), canonical_document_digest(&b));
    }

    #[test]
    fn expected_values_are_rejected_by_schema() {
        let error = serde_json::from_str::<ExternalVectorDocument>(
            r#"{"schema_version":"0.1","contract_digest":"sha256:x","target":"t","routine_symbol":"f","cases":[{"id":"c","inputs":{"n":4},"expected":10}]}"#,
        )
        .unwrap_err();
        assert!(error.to_string().contains("unknown field"));
    }
}
