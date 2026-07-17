//! Stable diagnostic codes for contract validation (`CTR###`).

use serde::{Deserialize, Serialize};

/// Machine-stable contract diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ContractCode {
    /// Unsupported or missing contract schema version.
    Ctr001,
    /// Duplicate parameter name in a function contract.
    Ctr002,
    /// Unknown or invalid semantic type.
    Ctr003,
    /// Invalid expression syntax.
    Ctr004,
    /// Unknown identifier in an expression.
    Ctr005,
    /// Contradictory memory effects.
    Ctr006,
    /// Invalid target override.
    Ctr007,
}

impl ContractCode {
    /// Canonical code string (for example `CTR003`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ctr001 => "CTR001",
            Self::Ctr002 => "CTR002",
            Self::Ctr003 => "CTR003",
            Self::Ctr004 => "CTR004",
            Self::Ctr005 => "CTR005",
            Self::Ctr006 => "CTR006",
            Self::Ctr007 => "CTR007",
        }
    }

    /// Human explanation suitable for `semasm --explain`.
    #[must_use]
    pub const fn explain(self) -> &'static str {
        match self {
            Self::Ctr001 => {
                "CTR001: unsupported contract version. \
                 Only contract_version = \"0.1\" is accepted in this release. \
                 Bump the file version only when the schema is intentionally upgraded."
            }
            Self::Ctr002 => {
                "CTR002: duplicate parameter. \
                 Each parameter name in a function contract must be unique."
            }
            Self::Ctr003 => {
                "CTR003: unknown semantic type. \
                 Allowed base types: bool, status, u8/u16/u32/u64/u128, i8/i16/i32/i64/i128, \
                 usize, isize. Composites: ptr<T>, ptr<const T>, slice<T>, array<T, N>, opaque<Name>. \
                 Arbitrary Rust, C, or LLVM type syntax is rejected."
            }
            Self::Ctr004 => {
                "CTR004: invalid expression. \
                 The contract expression language is a small closed subset \
                 (identifiers, literals, comparisons, boolean/arithmetic operators, \
                 ranges, approved predicates, implication). No loops or definitions."
            }
            Self::Ctr005 => {
                "CTR005: unknown identifier. \
                 Names in expressions must refer to parameters, returns, or other \
                 names introduced by the contract. Typos fail validation."
            }
            Self::Ctr006 => {
                "CTR006: contradictory memory effect. \
                 A function cannot both forbid memory access and declare a read/write \
                 of the same conceptual region, or stack contradictory effect kinds."
            }
            Self::Ctr007 => {
                "CTR007: invalid target override. \
                 Target override IDs must parse as known SemASM target identities \
                 and must not mix incompatible ABI/format/profile combinations \
                 once those checks are enabled."
            }
        }
    }

    /// Parse a code string such as `CTR003` or `ctr003`.
    #[must_use]
    pub fn parse(input: &str) -> Option<Self> {
        match input.trim().to_ascii_uppercase().as_str() {
            "CTR001" => Some(Self::Ctr001),
            "CTR002" => Some(Self::Ctr002),
            "CTR003" => Some(Self::Ctr003),
            "CTR004" => Some(Self::Ctr004),
            "CTR005" => Some(Self::Ctr005),
            "CTR006" => Some(Self::Ctr006),
            "CTR007" => Some(Self::Ctr007),
            _ => None,
        }
    }

    /// All codes in numeric order.
    #[must_use]
    pub const fn all() -> [Self; 7] {
        [
            Self::Ctr001,
            Self::Ctr002,
            Self::Ctr003,
            Self::Ctr004,
            Self::Ctr005,
            Self::Ctr006,
            Self::Ctr007,
        ]
    }
}

impl std::fmt::Display for ContractCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_roundtrip() {
        for code in ContractCode::all() {
            assert_eq!(ContractCode::parse(code.as_str()), Some(code));
            assert!(!code.explain().is_empty());
        }
    }
}
