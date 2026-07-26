//! Machine-readable target capability registry.

use std::collections::HashSet;
use std::fmt::Write as _;

use semasm_core::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Supported capability maturity levels, ordered from weakest to strongest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityLevel {
    /// No implementation or evidence is available.
    Unavailable,
    /// The target or capability is named, but not implemented.
    Declared,
    /// An implementation exists without sufficient verification evidence.
    Experimental,
    /// Only a documented subset is implemented.
    Partial,
    /// Unit tests provide executable evidence.
    VerifiedInUnitTests,
    /// A named CI job provides executable evidence.
    VerifiedInCi,
    /// Release gates qualify this capability for supported use.
    ReleaseQualified,
}

impl CapabilityLevel {
    /// Stable display spelling for this level.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Declared => "declared",
            Self::Experimental => "experimental",
            Self::Partial => "partial",
            Self::VerifiedInUnitTests => "unit-tested",
            Self::VerifiedInCi => "CI-verified",
            Self::ReleaseQualified => "release-qualified",
        }
    }
}

/// Capability levels for one target.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TargetCapabilities {
    /// Target identity maturity.
    pub declared: CapabilityLevel,
    /// Decoder maturity.
    pub decode: CapabilityLevel,
    /// Semantic lowering maturity.
    pub lower: CapabilityLevel,
    /// ABI analysis maturity.
    pub abi_analysis: CapabilityLevel,
    /// Assembly maturity.
    pub assemble: CapabilityLevel,
    /// Link maturity.
    pub link: CapabilityLevel,
    /// Execution maturity.
    pub execute: CapabilityLevel,
    /// Pipeline assemble/link/run maturity (alias of [`Self::pipeline_verify`]).
    ///
    /// Kept for schema `0.1` TOML compatibility. Prefer `pipeline_verify` /
    /// `agent_verify` when reading claims programmatically.
    pub verify: CapabilityLevel,
    /// Build-pipeline verification (fixture assemble → link → run).
    ///
    /// When omitted, mirrors [`Self::verify`].
    #[serde(default)]
    pub pipeline_verify: Option<CapabilityLevel>,
    /// `semasm agent verify` maturity (semantic gates + optional harness).
    ///
    /// When omitted, defaults to [`CapabilityLevel::Experimental`].
    #[serde(default)]
    pub agent_verify: Option<CapabilityLevel>,
}

impl TargetCapabilities {
    /// Effective pipeline verification level (`pipeline_verify` or `verify`).
    #[must_use]
    pub fn pipeline_verify_level(&self) -> CapabilityLevel {
        self.pipeline_verify.unwrap_or(self.verify)
    }

    /// Effective agent verification level (explicit or experimental default).
    #[must_use]
    pub fn agent_verify_level(&self) -> CapabilityLevel {
        self.agent_verify.unwrap_or(CapabilityLevel::Experimental)
    }
}

/// Executable evidence attached to a target capability claim.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityEvidence {
    /// Unit-test names or test modules that exercise this target.
    #[serde(default)]
    pub unit_tests: Vec<String>,
    /// Repository fixtures used as evidence.
    #[serde(default)]
    pub fixtures: Vec<String>,
    /// CI job names that execute target-specific evidence.
    #[serde(default)]
    pub ci_jobs: Vec<String>,
}

/// One target entry in the capability manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TargetCapability {
    /// Canonical target identity.
    pub id: String,
    /// Short, factual target description.
    pub description: String,
    /// Capability maturity by pipeline stage.
    pub capabilities: TargetCapabilities,
    /// Evidence supporting the maturity claims.
    pub evidence: CapabilityEvidence,
}

/// Workspace metadata recorded alongside target capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceCapabilities {
    /// Current workspace crate names.
    pub crates: Vec<String>,
}

/// How far agents may author for an admitted capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoringLevel {
    /// Full candidate authoring is admitted.
    Full,
    /// Assemble-only drafts (no behavioral acceptance path).
    AssembleOnly,
    /// Authoring is not admitted.
    None,
}

impl AuthoringLevel {
    /// Stable snake_case spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::AssembleOnly => "assemble_only",
            Self::None => "none",
        }
    }
}

/// Highest acceptance claim SemASM may return for an admitted capability.
///
/// Controllers must not promote a weaker observed status to a stronger claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AcceptanceLevel {
    /// Unconditional `verified` is reachable.
    Verified,
    /// At most `verified_under_preconditions` (≠ plain verified).
    VerifiedUnderPreconditions,
    /// Static analysis / gates only; no behavioral seal claim.
    Analyzed,
    /// Draft / authoring paths only; not an acceptance authority.
    AuthoringOnly,
    /// No acceptance path.
    None,
}

impl AcceptanceLevel {
    /// Stable snake_case spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::VerifiedUnderPreconditions => "verified_under_preconditions",
            Self::Analyzed => "analyzed",
            Self::AuthoringOnly => "authoring_only",
            Self::None => "none",
        }
    }
}

/// One admitted contract/oracle shape for agent controllers.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityAdmission {
    /// Stable admission id (unique within the manifest).
    pub capability_id: String,
    /// Target identities this admission applies to.
    pub targets: Vec<String>,
    /// Assemblers admitted for this shape (`nasm`, `gas`, …).
    pub assemblers: Vec<String>,
    /// Parameter type spellings (e.g. `i64`, `ptr<const u8>`).
    pub parameters: Vec<String>,
    /// Return type spellings.
    pub returns: Vec<String>,
    /// Builtin oracle ids (e.g. `builtin.pure_int.binary_i64`).
    pub oracles: Vec<String>,
    /// How far agents may author for this admission.
    pub authoring_level: AuthoringLevel,
    /// Highest acceptance claim controllers may pin for this admission.
    pub acceptance_level: AcceptanceLevel,
    /// Required verification gates (object, decode, lower, abi, …).
    pub required_gates: Vec<String>,
    /// Optional VAA allowlist leaf names bridged by this admission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leaf_names: Option<Vec<String>>,
}

/// Root machine-readable capability manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityManifest {
    /// Manifest schema version.
    pub schema_version: String,
    /// Workspace-level inventory.
    pub workspace: WorkspaceCapabilities,
    /// Target capability entries.
    #[serde(rename = "target")]
    pub targets: Vec<TargetCapability>,
    /// Optional admission registry for contract/oracle shapes.
    #[serde(default, rename = "admission")]
    pub admissions: Vec<CapabilityAdmission>,
}

impl CapabilityManifest {
    /// Parse and validate a capability manifest.
    pub fn parse(source: &str) -> Result<Self> {
        let manifest: Self = toml::from_str(source)
            .map_err(|error| Error::Parse(format!("invalid capability manifest: {error}")))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate uniqueness, pipeline ordering, and evidence requirements.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != "0.1" {
            return Err(Error::Validation(format!(
                "unsupported capability schema version `{}`",
                self.schema_version
            )));
        }
        ensure_unique(
            "workspace crate",
            self.workspace.crates.iter().map(String::as_str),
        )?;
        ensure_unique(
            "target",
            self.targets.iter().map(|target| target.id.as_str()),
        )?;
        if self.workspace.crates.is_empty() {
            return Err(Error::Validation(
                "capability manifest must list workspace crates".to_string(),
            ));
        }
        if self.targets.is_empty() {
            return Err(Error::Validation(
                "capability manifest must contain at least one target".to_string(),
            ));
        }
        for target in &self.targets {
            validate_target(target)?;
        }
        ensure_unique(
            "capability_id",
            self.admissions
                .iter()
                .map(|admission| admission.capability_id.as_str()),
        )?;
        let known_targets: HashSet<&str> =
            self.targets.iter().map(|target| target.id.as_str()).collect();
        for admission in &self.admissions {
            if admission.targets.is_empty() {
                return Err(Error::Validation(format!(
                    "admission `{}` must list at least one target",
                    admission.capability_id
                )));
            }
            for target_id in &admission.targets {
                if !known_targets.contains(target_id.as_str()) {
                    return Err(Error::Validation(format!(
                        "admission `{}` references unknown target `{target_id}`",
                        admission.capability_id
                    )));
                }
            }
        }
        Ok(())
    }

    /// Render the target table used inside the marked README block.
    #[must_use]
    pub fn render_readme_table(&self) -> String {
        let mut output = String::from(
            "| Identity | Decode | Lower | ABI | Assemble | Link | Execute | Pipeline | Agent |\n\
             |---|---|---|---|---|---|---|---|---|\n",
        );
        for target in &self.targets {
            let c = &target.capabilities;
            writeln!(
                output,
                "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} |",
                target.id,
                c.decode.as_str(),
                c.lower.as_str(),
                c.abi_analysis.as_str(),
                c.assemble.as_str(),
                c.link.as_str(),
                c.execute.as_str(),
                c.pipeline_verify_level().as_str(),
                c.agent_verify_level().as_str()
            )
            .expect("writing to a String cannot fail");
        }
        output
    }

    /// Render truthful terminal output for `semasm status`.
    #[must_use]
    pub fn render_status(&self, version: &str) -> String {
        let mut output = format!(
            "semasm {version}\ncapability schema: {}\nworkspace crates ({}): {}\ntargets:\n",
            self.schema_version,
            self.workspace.crates.len(),
            self.workspace.crates.join(", ")
        );
        for target in &self.targets {
            let c = &target.capabilities;
            writeln!(
                output,
                "  {}: decode={}, lower={}, abi={}, assemble={}, link={}, execute={}, pipeline={}, agent={}",
                target.id,
                c.decode.as_str(),
                c.lower.as_str(),
                c.abi_analysis.as_str(),
                c.assemble.as_str(),
                c.link.as_str(),
                c.execute.as_str(),
                c.pipeline_verify_level().as_str(),
                c.agent_verify_level().as_str()
            )
            .expect("writing to a String cannot fail");
        }
        output.push_str(
            "note: pipeline = fixture assemble/link/run; agent = semasm agent verify gates\n",
        );
        output.push_str("note: generated programs do not link SemASM by default\n");
        output
    }

    /// Machine-readable status document for `semasm status --format json`.
    ///
    /// Additive fields may appear in later minors; consumers must ignore unknowns.
    #[must_use]
    pub fn status_json(&self, version: &str) -> serde_json::Value {
        let targets: Vec<serde_json::Value> = self
            .targets
            .iter()
            .map(|target| {
                let c = &target.capabilities;
                serde_json::json!({
                    "id": target.id,
                    "decode": c.decode.as_str(),
                    "lower": c.lower.as_str(),
                    "abi": c.abi_analysis.as_str(),
                    "assemble": c.assemble.as_str(),
                    "link": c.link.as_str(),
                    "execute": c.execute.as_str(),
                    "pipeline": c.pipeline_verify_level().as_str(),
                    "agent": c.agent_verify_level().as_str(),
                })
            })
            .collect();
        serde_json::json!({
            "name": "semasm",
            "version": version,
            "capability_schema": self.schema_version,
            "workspace_crates": self.workspace.crates,
            "targets": targets,
            "notes": [
                "pipeline = fixture assemble/link/run; agent = semasm agent verify gates",
                "generated programs do not link SemASM by default",
            ],
        })
    }

    /// Full capability + admission export for controllers (`semasm capabilities`).
    ///
    /// Includes a `digest` field over the stable subset (schema, crates, targets,
    /// admission — not `version` or `digest`). Consumers must tolerate additive
    /// fields.
    #[must_use]
    pub fn capabilities_json(&self, version: &str) -> serde_json::Value {
        let digest = self.digest();
        let mut body = self.capabilities_digest_body();
        if let Some(object) = body.as_object_mut() {
            object.insert(
                "version".to_string(),
                serde_json::Value::String(version.to_string()),
            );
            object.insert("digest".to_string(), serde_json::Value::String(digest));
        }
        body
    }

    /// Prefixed SHA-256 digest of the canonical capabilities export body.
    #[must_use]
    pub fn digest(&self) -> String {
        digest_prefixed_sha256(&self.capabilities_digest_body())
    }

    /// Human-readable summary for `semasm capabilities --format terminal`.
    #[must_use]
    pub fn render_capabilities(&self, version: &str) -> String {
        let digest = self.digest();
        let mut output = format!(
            "semasm {version}\ncapability schema: {}\nadmission entries: {}\ndigest: {digest}\ntargets:\n",
            self.schema_version,
            self.admissions.len(),
        );
        for target in &self.targets {
            writeln!(output, "  {}", target.id).expect("writing to a String cannot fail");
        }
        output.push_str("admission:\n");
        if self.admissions.is_empty() {
            output.push_str("  (none)\n");
        } else {
            for admission in &self.admissions {
                writeln!(
                    output,
                    "  {}: acceptance={}, authoring={}, targets=[{}]",
                    admission.capability_id,
                    admission.acceptance_level.as_str(),
                    admission.authoring_level.as_str(),
                    admission.targets.join(", ")
                )
                .expect("writing to a String cannot fail");
            }
        }
        output
    }

    fn capabilities_digest_body(&self) -> serde_json::Value {
        let targets: Vec<serde_json::Value> = self
            .targets
            .iter()
            .map(|target| {
                let c = &target.capabilities;
                serde_json::json!({
                    "id": target.id,
                    "description": target.description,
                    "decode": c.decode.as_str(),
                    "lower": c.lower.as_str(),
                    "abi": c.abi_analysis.as_str(),
                    "assemble": c.assemble.as_str(),
                    "link": c.link.as_str(),
                    "execute": c.execute.as_str(),
                    "pipeline": c.pipeline_verify_level().as_str(),
                    "agent": c.agent_verify_level().as_str(),
                })
            })
            .collect();
        let admission: Vec<serde_json::Value> = self
            .admissions
            .iter()
            .map(|entry| {
                let mut value = serde_json::json!({
                    "capability_id": entry.capability_id,
                    "targets": entry.targets,
                    "assemblers": entry.assemblers,
                    "parameters": entry.parameters,
                    "returns": entry.returns,
                    "oracles": entry.oracles,
                    "authoring_level": entry.authoring_level.as_str(),
                    "acceptance_level": entry.acceptance_level.as_str(),
                    "required_gates": entry.required_gates,
                });
                if let Some(leaf_names) = &entry.leaf_names {
                    value.as_object_mut().expect("admission object").insert(
                        "leaf_names".to_string(),
                        serde_json::json!(leaf_names),
                    );
                }
                value
            })
            .collect();
        serde_json::json!({
            "name": "semasm",
            "capability_schema": self.schema_version,
            "workspace_crates": self.workspace.crates,
            "targets": targets,
            "admission": admission,
        })
    }
}

fn digest_prefixed_sha256(value: &serde_json::Value) -> String {
    // serde_json Map uses BTreeMap (sorted keys) when preserve_order is off.
    let canonical = serde_json::to_vec(value).expect("capabilities JSON is serializable");
    let hash = Sha256::digest(canonical);
    let mut out = String::from("sha256:");
    for byte in hash {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn ensure_unique<'a>(kind: &str, values: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut seen = HashSet::new();
    for value in values {
        if value.trim().is_empty() {
            return Err(Error::Validation(format!("{kind} name must not be empty")));
        }
        if !seen.insert(value) {
            return Err(Error::Validation(format!(
                "duplicate {kind} `{value}` in capability manifest"
            )));
        }
    }
    Ok(())
}

fn validate_target(target: &TargetCapability) -> Result<()> {
    let c = &target.capabilities;
    require_predecessor(target, "lower", c.lower, "decode", c.decode)?;
    require_predecessor(target, "abi_analysis", c.abi_analysis, "lower", c.lower)?;
    require_predecessor(target, "link", c.link, "assemble", c.assemble)?;
    require_predecessor(target, "execute", c.execute, "link", c.link)?;

    let levels = [
        c.declared,
        c.decode,
        c.lower,
        c.abi_analysis,
        c.assemble,
        c.link,
        c.execute,
        c.verify,
        c.pipeline_verify_level(),
        c.agent_verify_level(),
    ];
    if levels
        .iter()
        .any(|level| *level >= CapabilityLevel::VerifiedInUnitTests)
        && target.evidence.unit_tests.is_empty()
    {
        return Err(Error::Validation(format!(
            "target `{}` claims unit-tested capability without unit-test evidence",
            target.id
        )));
    }
    if levels
        .iter()
        .any(|level| *level >= CapabilityLevel::VerifiedInCi)
        && target.evidence.ci_jobs.is_empty()
    {
        return Err(Error::Validation(format!(
            "target `{}` claims CI-verified capability without a CI job",
            target.id
        )));
    }
    Ok(())
}

fn require_predecessor(
    target: &TargetCapability,
    capability_name: &str,
    capability: CapabilityLevel,
    predecessor_name: &str,
    predecessor: CapabilityLevel,
) -> Result<()> {
    if capability > CapabilityLevel::Declared && predecessor == CapabilityLevel::Unavailable {
        return Err(Error::Validation(format!(
            "target `{}` has {capability_name}={} without implemented {predecessor_name}",
            target.id,
            capability.as_str()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../../../capabilities.toml");

    #[test]
    fn repository_manifest_is_valid_and_complete() {
        let manifest = CapabilityManifest::parse(MANIFEST).unwrap();
        assert_eq!(manifest.workspace.crates.len(), 13);
        assert_eq!(manifest.targets.len(), 5);

        let workspace: toml::Value = toml::from_str(include_str!("../../../Cargo.toml")).unwrap();
        let members = workspace["workspace"]["members"].as_array().unwrap();
        let crate_names: Vec<&str> = members
            .iter()
            .map(|member| {
                member
                    .as_str()
                    .unwrap()
                    .rsplit('/')
                    .next()
                    .expect("workspace member has a crate name")
            })
            .collect();
        assert_eq!(manifest.workspace.crates, crate_names);
    }

    #[test]
    fn rejects_duplicate_targets() {
        let duplicate = MANIFEST.replace(
            "id = \"x86_64-pc-windows-msvc\"",
            "id = \"x86_64-unknown-linux-gnu\"",
        );
        let error = CapabilityManifest::parse(&duplicate).unwrap_err();
        assert!(error.to_string().contains("duplicate target"));
    }

    #[test]
    fn rejects_invalid_transition() {
        let invalid = MANIFEST.replacen("link = \"verified_in_ci\"", "link = \"unavailable\"", 1);
        let error = CapabilityManifest::parse(&invalid).unwrap_err();
        let msg = error.to_string();
        assert!(
            msg.contains("execute=") && msg.contains("without implemented link"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn rejects_missing_ci_evidence() {
        let invalid = MANIFEST.replacen("decode = \"declared\"", "decode = \"verified_in_ci\"", 1);
        let error = CapabilityManifest::parse(&invalid).unwrap_err();
        assert!(error.to_string().contains("without a CI job"));
    }

    #[test]
    fn readme_capability_block_is_current() {
        let manifest = CapabilityManifest::parse(MANIFEST).unwrap();
        let readme = include_str!("../../../README.md");
        let start_marker = "<!-- capabilities:start -->\n";
        let end_marker = "<!-- capabilities:end -->";
        let start = readme.find(start_marker).expect("README start marker") + start_marker.len();
        let end = readme.find(end_marker).expect("README end marker");
        assert_eq!(&readme[start..end], manifest.render_readme_table());
    }

    #[test]
    fn status_json_includes_version_and_targets() {
        let manifest = CapabilityManifest::parse(MANIFEST).unwrap();
        let json = manifest.status_json("0.1.0");
        assert_eq!(json["name"], "semasm");
        assert_eq!(json["version"], "0.1.0");
        assert_eq!(json["capability_schema"], "0.1");
        let targets = json["targets"].as_array().expect("targets array");
        assert!(!targets.is_empty());
        assert!(targets[0]["id"].is_string());
        assert!(targets[0]["agent"].is_string());
        assert!(targets[0]["pipeline"].is_string());
    }

    #[test]
    fn capabilities_export_has_admission_and_digest() {
        let manifest = CapabilityManifest::parse(MANIFEST).unwrap();
        assert!(
            !manifest.admissions.is_empty(),
            "repository manifest must seed VAA allowlist admissions"
        );
        let json = manifest.capabilities_json("0.2.1");
        assert_eq!(json["name"], "semasm");
        assert_eq!(json["version"], "0.2.1");
        assert_eq!(json["capability_schema"], "0.1");
        assert!(json["targets"].is_array());
        assert!(json["admission"].is_array());
        assert!(json["workspace_crates"].is_array());
        let digest = json["digest"].as_str().expect("digest string");
        assert!(digest.starts_with("sha256:"));
        assert_eq!(digest.len(), 7 + 64);
        assert_eq!(digest, manifest.digest());

        let fixture = include_str!("../fixtures/capabilities-export.min.json");
        let expected: serde_json::Value = serde_json::from_str(fixture).unwrap();
        for key in ["name", "capability_schema", "workspace_crates", "targets", "admission", "digest"]
        {
            assert!(
                expected.get(key).is_some(),
                "fixture missing required key `{key}`"
            );
            assert!(json.get(key).is_some(), "export missing required key `{key}`");
        }
        assert!(
            expected["digest"]
                .as_str()
                .is_some_and(|d| d.starts_with("sha256:") && d.len() == 7 + 64)
        );
    }

    #[test]
    fn rejects_unknown_admission_target() {
        let mut invalid = MANIFEST.to_string();
        invalid.push_str(
            r#"

[[admission]]
capability_id = "bogus"
targets = ["not-a-real-target"]
assemblers = ["nasm"]
parameters = ["i64"]
returns = ["i64"]
oracles = ["builtin.pure_int.binary_i64"]
authoring_level = "full"
acceptance_level = "verified"
required_gates = ["object", "decode", "lower", "abi", "behavior"]
"#,
        );
        let error = CapabilityManifest::parse(&invalid).unwrap_err();
        assert!(error.to_string().contains("unknown target"));
    }

    #[test]
    fn rejects_duplicate_capability_id() {
        let mut invalid = MANIFEST.to_string();
        invalid.push_str(
            r#"

[[admission]]
capability_id = "pure-int-binary"
targets = ["x86_64-pc-windows-msvc"]
assemblers = ["nasm"]
parameters = ["i64", "i64"]
returns = ["i64"]
oracles = ["builtin.pure_int.binary_i64"]
authoring_level = "full"
acceptance_level = "verified"
required_gates = ["object", "decode", "lower", "abi", "behavior"]
leaf_names = ["max_i64"]
"#,
        );
        let error = CapabilityManifest::parse(&invalid).unwrap_err();
        assert!(error.to_string().contains("duplicate capability_id"));
    }
}
