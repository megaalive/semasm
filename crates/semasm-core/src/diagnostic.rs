//! Collectable diagnostics with teaching-oriented messages.

use crate::span::SourceSpan;

/// Severity of a diagnostic message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticLevel {
    /// Informational note; does not fail validation.
    Note,
    /// Non-fatal warning.
    Warning,
    /// Error that fails validation.
    Error,
}

/// A single diagnostic message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Severity.
    pub level: DiagnosticLevel,
    /// Human-readable message explaining the violated rule when possible.
    pub message: String,
    /// Optional source location.
    pub span: Option<SourceSpan>,
    /// Optional machine-stable code (for example `E0001`).
    pub code: Option<String>,
}

impl Diagnostic {
    /// Create an error diagnostic without a span.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Error,
            message: message.into(),
            span: None,
            code: None,
        }
    }

    /// Create a warning diagnostic without a span.
    #[must_use]
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Warning,
            message: message.into(),
            span: None,
            code: None,
        }
    }

    /// Create a note diagnostic without a span.
    #[must_use]
    pub fn note(message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Note,
            message: message.into(),
            span: None,
            code: None,
        }
    }

    /// Attach a source span.
    #[must_use]
    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    /// Attach a stable diagnostic code.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

/// Ordered collection of diagnostics.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
}

impl Diagnostics {
    /// Create an empty collection.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a diagnostic.
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.items.push(diagnostic);
    }

    /// Number of diagnostics.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there are no diagnostics.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Whether any error-level diagnostic is present.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|d| d.level == DiagnosticLevel::Error)
    }

    /// Iterate diagnostics in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.items.iter()
    }

    /// Consume and return the underlying vector.
    #[must_use]
    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.items
    }
}

impl IntoIterator for Diagnostics {
    type Item = Diagnostic;
    type IntoIter = std::vec::IntoIter<Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}
