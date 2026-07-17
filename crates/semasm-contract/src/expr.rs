//! Bounded contract expression language (parse only; no machine evaluation).

use semasm_core::SourceSpan;
use serde::{Deserialize, Serialize};

/// Binary operators with fixed precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinOp {
    /// Logical implication.
    Implies,
    /// Boolean or.
    Or,
    /// Boolean and.
    And,
    /// Equality.
    Eq,
    /// Inequality.
    Ne,
    /// Less-than.
    Lt,
    /// Less-or-equal.
    Le,
    /// Greater-than.
    Gt,
    /// Greater-or-equal.
    Ge,
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Division.
    Div,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnaryOp {
    /// Logical not.
    Not,
    /// Numeric negation.
    Neg,
}

/// Expression AST node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Expr {
    /// Integer literal.
    Int {
        /// Value.
        value: i64,
        /// Source span in the expression string.
        span: SourceSpan,
    },
    /// Boolean literal.
    Bool {
        /// Value.
        value: bool,
        /// Source span.
        span: SourceSpan,
    },
    /// Identifier.
    Ident {
        /// Name.
        name: String,
        /// Source span.
        span: SourceSpan,
    },
    /// Field or member access (`status.ok`).
    Member {
        /// Base expression.
        base: Box<Expr>,
        /// Field name.
        field: String,
        /// Span covering `.field`.
        span: SourceSpan,
    },
    /// Call or method-style call after member resolution (`valid_for_read(length)`).
    Call {
        /// Callee expression (often a member).
        callee: Box<Expr>,
        /// Arguments.
        args: Vec<Expr>,
        /// Full call span.
        span: SourceSpan,
    },
    /// Inclusive-start exclusive-end style range `lo..hi` (bounds only; not evaluated).
    Range {
        /// Lower bound.
        start: Box<Expr>,
        /// Upper bound.
        end: Box<Expr>,
        /// Full range span.
        span: SourceSpan,
    },
    /// Indexing `base[index_or_range]`.
    Index {
        /// Base.
        base: Box<Expr>,
        /// Index expression (may be a range).
        index: Box<Expr>,
        /// Full index span.
        span: SourceSpan,
    },
    /// Unary operation.
    Unary {
        /// Operator.
        op: UnaryOp,
        /// Operand.
        expr: Box<Expr>,
        /// Span.
        span: SourceSpan,
    },
    /// Binary operation.
    Binary {
        /// Operator.
        op: BinOp,
        /// Left.
        left: Box<Expr>,
        /// Right.
        right: Box<Expr>,
        /// Span.
        span: SourceSpan,
    },
}

impl Expr {
    /// Parse an expression from source text.
    pub fn parse(input: &str) -> Result<Self, String> {
        let mut p = ExprParser::new(input);
        let expr = p.parse_implies()?;
        p.skip_ws();
        if p.pos < p.bytes.len() {
            return Err(format!(
                "trailing junk in expression at byte {} near `{}`",
                p.pos,
                p.snippet()
            ));
        }
        Ok(expr)
    }

    /// Collect free identifier names (not field names of members).
    #[must_use]
    pub fn free_idents(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.collect_idents(&mut out, false);
        out.sort();
        out.dedup();
        out
    }

    fn collect_idents(&self, out: &mut Vec<String>, in_member_field: bool) {
        match self {
            Self::Ident { name, .. } if !in_member_field => out.push(name.clone()),
            Self::Ident { .. } | Self::Int { .. } | Self::Bool { .. } => {}
            Self::Member { base, .. } => base.collect_idents(out, false),
            Self::Call { callee, args, .. } => {
                callee.collect_idents(out, false);
                for a in args {
                    a.collect_idents(out, false);
                }
            }
            Self::Range { start, end, .. } => {
                start.collect_idents(out, false);
                end.collect_idents(out, false);
            }
            Self::Index { base, index, .. } => {
                base.collect_idents(out, false);
                index.collect_idents(out, false);
            }
            Self::Unary { expr, .. } => expr.collect_idents(out, false),
            Self::Binary { left, right, .. } => {
                left.collect_idents(out, false);
                right.collect_idents(out, false);
            }
        }
    }
}

struct ExprParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> ExprParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            bytes: input.as_bytes(),
            pos: 0,
        }
    }

    fn snippet(&self) -> String {
        let end = (self.pos + 16).min(self.bytes.len());
        String::from_utf8_lossy(&self.bytes[self.pos..end]).into_owned()
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

    fn span_from(&self, start: usize) -> SourceSpan {
        SourceSpan::from_offsets(
            u32::try_from(start).unwrap_or(u32::MAX),
            u32::try_from(self.pos).unwrap_or(u32::MAX),
        )
    }

    fn eat_kw(&mut self, kw: &str) -> bool {
        self.skip_ws();
        let end = self.pos + kw.len();
        if end <= self.bytes.len() && &self.bytes[self.pos..end] == kw.as_bytes() {
            let boundary_ok = self
                .bytes
                .get(end)
                .is_none_or(|b| !b.is_ascii_alphanumeric() && *b != b'_');
            if boundary_ok {
                self.pos = end;
                return true;
            }
        }
        false
    }

    fn eat_op(&mut self, op: &str) -> bool {
        self.skip_ws();
        let end = self.pos + op.len();
        if end <= self.bytes.len() && &self.bytes[self.pos..end] == op.as_bytes() {
            self.pos = end;
            true
        } else {
            false
        }
    }

    fn parse_implies(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_or()?;
        while self.eat_kw("implies") {
            let start = match &left {
                Expr::Binary { span, .. }
                | Expr::Unary { span, .. }
                | Expr::Ident { span, .. }
                | Expr::Int { span, .. }
                | Expr::Bool { span, .. }
                | Expr::Member { span, .. }
                | Expr::Call { span, .. }
                | Expr::Range { span, .. }
                | Expr::Index { span, .. } => span.start.get() as usize,
            };
            let right = self.parse_or()?;
            let span = self.span_from(start);
            left = Expr::Binary {
                op: BinOp::Implies,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Ok(left)
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        loop {
            let op = if self.eat_op("||") || self.eat_kw("or") {
                BinOp::Or
            } else {
                break;
            };
            let start = left_start(&left);
            let right = self.parse_and()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span: self.span_from(start),
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_cmp()?;
        loop {
            let op = if self.eat_op("&&") || self.eat_kw("and") {
                BinOp::And
            } else {
                break;
            };
            let start = left_start(&left);
            let right = self.parse_cmp()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span: self.span_from(start),
            };
        }
        Ok(left)
    }

    fn parse_cmp(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_add()?;
        loop {
            let op = if self.eat_op("==") {
                BinOp::Eq
            } else if self.eat_op("!=") {
                BinOp::Ne
            } else if self.eat_op("<=") {
                BinOp::Le
            } else if self.eat_op(">=") {
                BinOp::Ge
            } else if self.eat_op("<") {
                BinOp::Lt
            } else if self.eat_op(">") {
                BinOp::Gt
            } else {
                break;
            };
            let start = left_start(&left);
            let right = self.parse_add()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span: self.span_from(start),
            };
        }
        Ok(left)
    }

    fn parse_add(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_mul()?;
        loop {
            let op = if self.eat_op("+") {
                BinOp::Add
            } else if self.eat_op("-") {
                // Ambiguous with unary; only if binary context
                BinOp::Sub
            } else {
                break;
            };
            let start = left_start(&left);
            let right = self.parse_mul()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span: self.span_from(start),
            };
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        loop {
            let op = if self.eat_op("*") {
                BinOp::Mul
            } else if self.eat_op("/") {
                BinOp::Div
            } else {
                break;
            };
            let start = left_start(&left);
            let right = self.parse_unary()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span: self.span_from(start),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        self.skip_ws();
        let start = self.pos;
        if self.eat_op("!") || self.eat_kw("not") {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
                span: self.span_from(start),
            });
        }
        if self.eat_op("-") {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(expr),
                span: self.span_from(start),
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;
        loop {
            self.skip_ws();
            if self.eat_op(".") {
                // Don't consume `.` if it's part of `..` (range operator).
                if self.bytes.get(self.pos) == Some(&b'.') {
                    self.pos -= 1;
                    break;
                }
                let start = left_start(&expr);
                let field_start = self.pos;
                let field = self.parse_ident_raw()?;
                let field_span = self.span_from(field_start);
                expr = Expr::Member {
                    base: Box::new(expr),
                    field,
                    span: field_span,
                };
                // Recompute outer span for chained ops via index/call
                let _ = start;
                continue;
            }
            if self.eat_op("(") {
                let start = left_start(&expr);
                let mut args = Vec::new();
                self.skip_ws();
                if !self.eat_op(")") {
                    loop {
                        args.push(self.parse_implies()?);
                        self.skip_ws();
                        if self.eat_op(")") {
                            break;
                        }
                        if !self.eat_op(",") {
                            return Err(format!(
                                "expected `,` or `)` in call at byte {}",
                                self.pos
                            ));
                        }
                    }
                }
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                    span: self.span_from(start),
                };
                continue;
            }
            if self.eat_op("[") {
                let start = left_start(&expr);
                let index = self.parse_range_or_expr()?;
                if !self.eat_op("]") {
                    return Err(format!("expected `]` at byte {}", self.pos));
                }
                expr = Expr::Index {
                    base: Box::new(expr),
                    index: Box::new(index),
                    span: self.span_from(start),
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_range_or_expr(&mut self) -> Result<Expr, String> {
        let start_pos = {
            self.skip_ws();
            self.pos
        };
        let start = self.parse_implies()?;
        self.skip_ws();
        if self.eat_op("..") {
            let end = self.parse_implies()?;
            Ok(Expr::Range {
                start: Box::new(start),
                end: Box::new(end),
                span: self.span_from(start_pos),
            })
        } else {
            Ok(start)
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        self.skip_ws();
        let start = self.pos;
        if self.eat_op("(") {
            let expr = self.parse_implies()?;
            if !self.eat_op(")") {
                return Err(format!("expected `)` at byte {}", self.pos));
            }
            return Ok(expr);
        }
        if self.eat_kw("true") {
            return Ok(Expr::Bool {
                value: true,
                span: self.span_from(start),
            });
        }
        if self.eat_kw("false") {
            return Ok(Expr::Bool {
                value: false,
                span: self.span_from(start),
            });
        }
        if self.peek().is_some_and(|b| b.is_ascii_digit()) {
            let value = self.parse_int_literal()?;
            return Ok(Expr::Int {
                value,
                span: self.span_from(start),
            });
        }
        if self
            .peek()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        {
            let name = self.parse_ident_raw()?;
            return Ok(Expr::Ident {
                name,
                span: self.span_from(start),
            });
        }
        Err(format!(
            "invalid expression at byte {start} near `{}`",
            self.snippet()
        ))
    }

    fn parse_int_literal(&mut self) -> Result<i64, String> {
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        let s = std::str::from_utf8(&self.bytes[start..self.pos]).unwrap_or("");
        s.parse::<i64>()
            .map_err(|_| format!("invalid integer at byte {start}"))
    }

    fn parse_ident_raw(&mut self) -> Result<String, String> {
        self.skip_ws();
        let start = self.pos;
        if !self
            .peek()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        {
            return Err(format!("expected identifier at byte {start}"));
        }
        self.bump();
        while self
            .peek()
            .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            self.bump();
        }
        Ok(std::str::from_utf8(&self.bytes[start..self.pos])
            .unwrap_or("")
            .to_string())
    }
}

fn left_start(expr: &Expr) -> usize {
    match expr {
        Expr::Int { span, .. }
        | Expr::Bool { span, .. }
        | Expr::Ident { span, .. }
        | Expr::Member { span, .. }
        | Expr::Call { span, .. }
        | Expr::Range { span, .. }
        | Expr::Index { span, .. }
        | Expr::Unary { span, .. }
        | Expr::Binary { span, .. } => span.start.get() as usize,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_and_implies() {
        let e = Expr::parse("status.ok implies written == length").unwrap();
        assert!(matches!(
            e,
            Expr::Binary {
                op: BinOp::Implies,
                ..
            }
        ));
    }

    #[test]
    fn method_call() {
        let e = Expr::parse("buffer.valid_for_read(length)").unwrap();
        assert!(matches!(e, Expr::Call { .. }));
    }

    #[test]
    fn malformed_range_in_index() {
        let err = Expr::parse("buffer[0..]").unwrap_err();
        assert!(err.contains("invalid") || err.contains("expected") || err.contains("byte"));
    }

    #[test]
    fn free_idents() {
        let e = Expr::parse("status.ok implies written == length").unwrap();
        let ids = e.free_idents();
        assert!(ids.contains(&"status".to_string()));
        assert!(ids.contains(&"written".to_string()));
        assert!(ids.contains(&"length".to_string()));
        assert!(!ids.contains(&"ok".to_string()));
    }
}
