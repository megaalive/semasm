//! Semantic type grammar for portable contracts.

use semasm_core::SourceSpan;
use serde::{Deserialize, Serialize};

/// Parsed portable semantic type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SemType {
    /// Boolean.
    Bool,
    /// Portable success/failure status (not a raw integer ABI).
    Status,
    /// Unsigned integer of the given bit width.
    UInt {
        /// Bit width: 8, 16, 32, 64, or 128.
        bits: u16,
    },
    /// Signed integer of the given bit width.
    Int {
        /// Bit width: 8, 16, 32, 64, or 128.
        bits: u16,
    },
    /// Pointer-sized unsigned integer.
    Usize,
    /// Pointer-sized signed integer.
    Isize,
    /// Pointer to `inner`. When `is_const`, pointee is read-only.
    Ptr {
        /// Pointee type.
        inner: Box<SemType>,
        /// Const pointee.
        is_const: bool,
    },
    /// Contiguous slice of `inner` elements.
    Slice {
        /// Element type.
        inner: Box<SemType>,
    },
    /// Fixed-size array.
    Array {
        /// Element type.
        inner: Box<SemType>,
        /// Element count.
        len: u64,
    },
    /// Named opaque type (layout not portable).
    Opaque {
        /// Opaque type name.
        name: String,
    },
}

impl SemType {
    /// Parse a semantic type string. On success returns the type and full span.
    ///
    /// # Errors
    ///
    /// Returns a message when the type grammar is violated.
    pub fn parse(input: &str) -> Result<(Self, SourceSpan), String> {
        let mut p = TypeParser::new(input);
        let ty = p.parse_type()?;
        p.skip_ws();
        if p.pos < p.bytes.len() {
            return Err(format!("trailing junk in type starting at byte {}", p.pos));
        }
        let span = SourceSpan::from_offsets(0, u32::try_from(input.len()).unwrap_or(u32::MAX));
        Ok((ty, span))
    }
}

struct TypeParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> TypeParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            bytes: input.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }

    fn eat(&mut self, s: &str) -> bool {
        self.skip_ws();
        let end = self.pos + s.len();
        if end <= self.bytes.len() && &self.bytes[self.pos..end] == s.as_bytes() {
            self.pos = end;
            true
        } else {
            false
        }
    }

    fn parse_type(&mut self) -> Result<SemType, String> {
        self.skip_ws();
        if self.eat("ptr") {
            return self.parse_ptr();
        }
        if self.eat("slice") {
            return self.parse_slice();
        }
        if self.eat("array") {
            return self.parse_array();
        }
        if self.eat("opaque") {
            return self.parse_opaque();
        }
        if self.eat("bool") {
            return Ok(SemType::Bool);
        }
        if self.eat("status") {
            return Ok(SemType::Status);
        }
        if self.eat("usize") {
            return Ok(SemType::Usize);
        }
        if self.eat("isize") {
            return Ok(SemType::Isize);
        }
        self.parse_int_width()
    }

    fn parse_int_width(&mut self) -> Result<SemType, String> {
        let start = self.pos;
        let signed = if self.eat("u") {
            false
        } else if self.eat("i") {
            true
        } else {
            return Err(format!(
                "unknown semantic type near byte {start}: expected bool, status, integer width, ptr, slice, array, or opaque"
            ));
        };
        let bits = self.parse_digits()?;
        let ok = matches!(bits, 8 | 16 | 32 | 64 | 128);
        if !ok {
            return Err(format!(
                "invalid integer width {bits} at byte {start}: allowed 8, 16, 32, 64, 128"
            ));
        }
        let bits = u16::try_from(bits).unwrap_or(0);
        Ok(if signed {
            SemType::Int { bits }
        } else {
            SemType::UInt { bits }
        })
    }

    fn parse_digits(&mut self) -> Result<u64, String> {
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if start == self.pos {
            return Err(format!("expected digits at byte {start}"));
        }
        let s = std::str::from_utf8(&self.bytes[start..self.pos]).unwrap_or("");
        s.parse::<u64>()
            .map_err(|_| format!("invalid number at byte {start}"))
    }

    fn expect(&mut self, s: &str) -> Result<(), String> {
        if self.eat(s) {
            Ok(())
        } else {
            Err(format!("expected `{s}` at byte {}", self.pos))
        }
    }

    fn parse_ptr(&mut self) -> Result<SemType, String> {
        self.expect("<")?;
        self.skip_ws();
        let is_const = self.eat("const");
        if is_const {
            self.skip_ws();
        }
        let inner = self.parse_type()?;
        self.expect(">")?;
        Ok(SemType::Ptr {
            inner: Box::new(inner),
            is_const,
        })
    }

    fn parse_slice(&mut self) -> Result<SemType, String> {
        self.expect("<")?;
        let inner = self.parse_type()?;
        self.expect(">")?;
        Ok(SemType::Slice {
            inner: Box::new(inner),
        })
    }

    fn parse_array(&mut self) -> Result<SemType, String> {
        self.expect("<")?;
        let inner = self.parse_type()?;
        self.expect(",")?;
        self.skip_ws();
        let len = self.parse_digits()?;
        self.expect(">")?;
        Ok(SemType::Array {
            inner: Box::new(inner),
            len,
        })
    }

    fn parse_opaque(&mut self) -> Result<SemType, String> {
        self.expect("<")?;
        self.skip_ws();
        let start = self.pos;
        if !self
            .peek()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        {
            return Err(format!("expected opaque name at byte {start}"));
        }
        self.bump();
        while self
            .peek()
            .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            self.bump();
        }
        let name = std::str::from_utf8(&self.bytes[start..self.pos])
            .unwrap_or("")
            .to_string();
        self.expect(">")?;
        Ok(SemType::Opaque { name })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_ptr() {
        let (ty, _) = SemType::parse("ptr<const ptr<u8>>").unwrap();
        assert_eq!(
            ty,
            SemType::Ptr {
                is_const: true,
                inner: Box::new(SemType::Ptr {
                    is_const: false,
                    inner: Box::new(SemType::UInt { bits: 8 }),
                }),
            }
        );
    }

    #[test]
    fn rejects_rust_reference() {
        assert!(SemType::parse("&u8").is_err());
        assert!(SemType::parse("*const u8").is_err());
        assert!(SemType::parse("i256").is_err());
    }

    #[test]
    fn parses_array_and_slice() {
        let (a, _) = SemType::parse("array<u32, 4>").unwrap();
        assert_eq!(
            a,
            SemType::Array {
                inner: Box::new(SemType::UInt { bits: 32 }),
                len: 4,
            }
        );
        let (s, _) = SemType::parse("slice<i8>").unwrap();
        assert!(matches!(s, SemType::Slice { .. }));
    }
}
