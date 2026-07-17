//! Source locations for diagnostics and ASIR provenance.

use serde::{Deserialize, Serialize};

/// Byte offset into a source buffer (UTF-8 bytes, not characters).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct ByteOffset(pub u32);

impl ByteOffset {
    /// Create a new byte offset.
    #[must_use]
    pub const fn new(offset: u32) -> Self {
        Self(offset)
    }

    /// Return the raw offset value.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Inclusive-start exclusive-end span over a source buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct SourceSpan {
    /// Start byte offset (inclusive).
    pub start: ByteOffset,
    /// End byte offset (exclusive).
    pub end: ByteOffset,
}

impl SourceSpan {
    /// Create a span from start and end offsets.
    #[must_use]
    pub const fn new(start: ByteOffset, end: ByteOffset) -> Self {
        Self { start, end }
    }

    /// Create a span from raw `u32` offsets.
    #[must_use]
    pub const fn from_offsets(start: u32, end: u32) -> Self {
        Self {
            start: ByteOffset::new(start),
            end: ByteOffset::new(end),
        }
    }

    /// Length in bytes, or zero if inverted.
    #[must_use]
    pub fn len(self) -> u32 {
        self.end.get().saturating_sub(self.start.get())
    }

    /// Whether the span covers no bytes.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
}
