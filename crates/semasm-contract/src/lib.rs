//! Portable semantic contracts for assembly programs.
//!
//! Contracts describe intent, ABI-facing parameters, effects, and constraints.
//! They are not an implementation language. Parsing and schema validation land
//! in later vertical slices (VS-01).

#![forbid(unsafe_code)]

use semasm_core::FunctionId;

/// Schema version for portable contract documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContractVersion {
    /// Major component.
    pub major: u32,
    /// Minor component.
    pub minor: u32,
}

impl ContractVersion {
    /// Initial contract schema version used by SemASM 0.1.
    pub const V0_1: Self = Self { major: 0, minor: 1 };

    /// Create a version pair.
    #[must_use]
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }
}

/// A named function contract (placeholder until VS-01 fills the schema).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionContract {
    /// Stable function identifier within a project.
    pub id: FunctionId,
    /// Human-readable symbol name (for example `write_all`).
    pub name: String,
}

impl FunctionContract {
    /// Create a minimal function contract shell.
    #[must_use]
    pub fn new(id: FunctionId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
        }
    }
}

/// A portable semantic contract document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contract {
    /// Schema version.
    pub version: ContractVersion,
    /// Functions described by this contract.
    pub functions: Vec<FunctionContract>,
}

impl Contract {
    /// Create an empty contract at the current schema version.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            version: ContractVersion::V0_1,
            functions: Vec::new(),
        }
    }

    /// Number of function contracts.
    #[must_use]
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semasm_core::FunctionId;

    #[test]
    fn empty_contract_has_schema_version() {
        let c = Contract::empty();
        assert_eq!(c.version, ContractVersion::V0_1);
        assert_eq!(c.function_count(), 0);
    }

    #[test]
    fn function_contract_stores_name() {
        let f = FunctionContract::new(FunctionId::new(1), "main");
        assert_eq!(f.name, "main");
        assert_eq!(f.id.get(), 1);
    }
}
