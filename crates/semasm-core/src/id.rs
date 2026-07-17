//! Stable identifiers used across crates.

/// Opaque identifier for a function symbol in a project or contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunctionId(u32);

impl FunctionId {
    /// Create a function id from a raw value.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Return the raw id value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Opaque identifier for a general symbol (label, data, or external).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolId(u32);

impl SymbolId {
    /// Create a symbol id from a raw value.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Return the raw id value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}
