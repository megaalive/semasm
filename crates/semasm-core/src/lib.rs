//! Shared foundation types for SemASM.
//!
//! This crate must remain architecture-neutral and free of optional heavy
//! integrations (Capstone, LLVM, QEMU, AI SDKs).

#![forbid(unsafe_code)]

mod diagnostic;
mod error;
mod id;
mod span;
mod version;

pub use diagnostic::{Diagnostic, DiagnosticLevel, Diagnostics};
pub use error::{Error, Result};
pub use id::{FunctionId, SymbolId};
pub use span::{ByteOffset, SourceSpan};
pub use version::{
    SEMASM_VERSION, SEMASM_VERSION_MAJOR, SEMASM_VERSION_MINOR, SEMASM_VERSION_PATCH,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver() {
        let parts: Vec<_> = SEMASM_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].parse::<u32>().unwrap(), SEMASM_VERSION_MAJOR);
        assert_eq!(parts[1].parse::<u32>().unwrap(), SEMASM_VERSION_MINOR);
        assert_eq!(parts[2].parse::<u32>().unwrap(), SEMASM_VERSION_PATCH);
    }

    #[test]
    fn diagnostics_collect_errors() {
        let mut diags = Diagnostics::default();
        diags.push(Diagnostic::error("example failure"));
        assert!(diags.has_errors());
        assert_eq!(diags.len(), 1);
    }
}
