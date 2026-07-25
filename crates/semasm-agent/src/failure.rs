//! Structured early-failure envelope for agent controllers.
//!
//! When `agent verify` cannot reach a full [`crate::verify::VerificationReport`]
//! (unsupported shape, assemble/link, toolchain, I/O), SemASM still emits one
//! JSON document on stdout so VAA never scrapes stderr for truth.

use serde::{Deserialize, Serialize};

/// Schema version for [`AgentFailureEnvelope`].
pub const AGENT_FAILURE_SCHEMA_VERSION: &str = "0.1";

/// Machine-readable stage that failed before a full verification report.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum FailureStage {
    /// CLI usage / target identity.
    Usage,
    /// Target / toolchain discovery.
    Toolchain,
    /// Contract parse / check.
    Contract,
    /// Source / I/O staging.
    Io,
    /// No builtin harness vectors for the routine shape.
    UnsupportedShape,
    /// Assemble routine or harness.
    Assemble,
    /// Link executable.
    Link,
    /// Runner spawn / execute.
    Execute,
    /// Scratch / timeout / other pipeline.
    Pipeline,
}

impl FailureStage {
    /// Stable snake_case label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::Toolchain => "toolchain",
            Self::Contract => "contract",
            Self::Io => "io",
            Self::UnsupportedShape => "unsupported_shape",
            Self::Assemble => "assemble",
            Self::Link => "link",
            Self::Execute => "execute",
            Self::Pipeline => "pipeline",
        }
    }
}

/// Whether a controller may safely retry the same inputs.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Retryability {
    /// Do not retry (logic / shape / contract).
    Never,
    /// Safe to retry after tooling fix (missing NASM, transient I/O).
    Tooling,
    /// Safe to retry after timeout / resource pressure.
    Transient,
}

impl Retryability {
    /// Stable snake_case label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::Tooling => "tooling",
            Self::Transient => "transient",
        }
    }
}

/// Optional source location for assembler / linker diagnostics.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FailureLocation {
    /// Path when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// 1-based line when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// 1-based column when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
}

/// Stdout JSON envelope when verify cannot emit a full VerificationReport.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct AgentFailureEnvelope {
    /// Envelope schema version (`0.1`).
    pub schema_version: String,
    /// Discriminator for controllers (`agent_failure`).
    pub kind: String,
    /// Stable reason code (`UNSUPPORTED_SHAPE`, `ASSEMBLE_FAILED`, …).
    pub code: String,
    /// Pipeline stage.
    pub stage: FailureStage,
    /// Human message (also duplicated on stderr).
    pub message: String,
    /// Retry guidance for harnesses.
    pub retryability: Retryability,
    /// Tool identity when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_version: Option<String>,
    /// Target triple when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Routine symbol when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routine_symbol: Option<String>,
    /// Contract digest when bytes were read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_digest: Option<String>,
    /// Source digest when bytes were read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_digest: Option<String>,
    /// Optional file/line/column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<FailureLocation>,
    /// Tool stderr excerpt (truncated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl AgentFailureEnvelope {
    /// Build a failure envelope with required fields.
    #[must_use]
    pub fn new(
        code: impl Into<String>,
        stage: FailureStage,
        message: impl Into<String>,
        retryability: Retryability,
    ) -> Self {
        Self {
            schema_version: AGENT_FAILURE_SCHEMA_VERSION.to_owned(),
            kind: "agent_failure".to_owned(),
            code: code.into(),
            stage,
            message: message.into(),
            retryability,
            tool_version: Some(format!("semasm {}", semasm_core::SEMASM_VERSION)),
            target: None,
            routine_symbol: None,
            contract_digest: None,
            source_digest: None,
            location: None,
            detail: None,
        }
    }

    /// Attach target triple.
    #[must_use]
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Attach routine symbol.
    #[must_use]
    pub fn with_routine(mut self, symbol: impl Into<String>) -> Self {
        self.routine_symbol = Some(symbol.into());
        self
    }

    /// Attach digests when available.
    #[must_use]
    pub fn with_digests(
        mut self,
        contract_digest: Option<String>,
        source_digest: Option<String>,
    ) -> Self {
        self.contract_digest = contract_digest;
        self.source_digest = source_digest;
        self
    }

    /// Attach truncated tool detail (stderr / reason).
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        let mut text = detail.into();
        const MAX: usize = 4_096;
        if text.len() > MAX {
            text.truncate(MAX);
            text.push_str("…");
        }
        self.detail = Some(text);
        self
    }

    /// Attach source location.
    #[must_use]
    pub fn with_location(mut self, location: FailureLocation) -> Self {
        self.location = Some(location);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_round_trips_json() {
        let env = AgentFailureEnvelope::new(
            "UNSUPPORTED_SHAPE",
            FailureStage::UnsupportedShape,
            "no vectors",
            Retryability::Never,
        )
        .with_target("x86_64-unknown-linux-gnu")
        .with_routine("foo");
        let json = serde_json::to_string(&env).expect("ser");
        let back: AgentFailureEnvelope = serde_json::from_str(&json).expect("de");
        assert_eq!(back.code, "UNSUPPORTED_SHAPE");
        assert_eq!(back.kind, "agent_failure");
        assert_eq!(back.schema_version, AGENT_FAILURE_SCHEMA_VERSION);
    }

    #[test]
    fn golden_unsupported_shape_fixture() {
        let raw = include_str!("../schemas/fixtures/agent-failure.unsupported_shape.json");
        let env: AgentFailureEnvelope = serde_json::from_str(raw).expect("fixture");
        assert_eq!(env.kind, "agent_failure");
        assert_eq!(env.code, "UNSUPPORTED_SHAPE");
        assert_eq!(env.stage, FailureStage::UnsupportedShape);
        assert_eq!(env.retryability, Retryability::Never);
    }
}
