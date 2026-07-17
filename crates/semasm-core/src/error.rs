//! Explicit error model for SemASM crates.

use thiserror::Error;

/// Result alias using [`enum@Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// Workspace-wide error type for recoverable failures.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Error {
    /// A semantic or validation rule was violated.
    #[error("validation error: {0}")]
    Validation(String),

    /// Input could not be parsed.
    #[error("parse error: {0}")]
    Parse(String),

    /// An I/O failure occurred.
    #[error("I/O error: {0}")]
    Io(String),

    /// A requested target, tool, or resource was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// Internal invariant broken; indicates a bug if reached in production.
    #[error("internal error: {0}")]
    Internal(String),
}

impl Error {
    /// Create a validation error from a displayable message.
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    /// Create a parse error from a displayable message.
    pub fn parse(msg: impl Into<String>) -> Self {
        Self::Parse(msg.into())
    }

    /// Create an I/O error from a displayable message.
    pub fn io(msg: impl Into<String>) -> Self {
        Self::Io(msg.into())
    }

    /// Create a not-found error from a displayable message.
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    /// Create an internal error from a displayable message.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}
