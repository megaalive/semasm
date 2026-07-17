//! Package version constants shared across crates.

/// Full SemASM version string (`MAJOR.MINOR.PATCH`).
pub const SEMASM_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Major version component.
pub const SEMASM_VERSION_MAJOR: u32 = 0;

/// Minor version component.
pub const SEMASM_VERSION_MINOR: u32 = 1;

/// Patch version component.
pub const SEMASM_VERSION_PATCH: u32 = 0;
