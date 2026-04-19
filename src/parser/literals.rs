// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::token::TokenType;

use super::Parser;

impl Parser {
    // === Primary expressions ===

    /// §3.7 v0.10: true when the current token starts a primary expression
    /// that can follow a bare variant_name as the inner of a variant_shorthand.
    /// Excludes LParen to avoid conflict with the function-call postfix.
    fn starts_variant_shorthand_inner(&self) -> bool {
        matches!(
            self.peek_type(),
            TokenType::Integer
                | TokenType::Float
                | TokenType::String
                | TokenType::InterpStart
                | TokenType::True
                | TokenType::False
                | TokenType::Null
                | TokenType::Undefined
                | TokenType::Inf
                | TokenType::Nan
                | TokenType::Env
                | TokenType::LBrace
                | TokenType::LBracket
                | TokenType::If
                | TokenType::Case
                | TokenType::Function
                | TokenType::Struct
                | TokenType::Identifier
        )
    }

    pub(crate) fn parse_primary(&mut self) -> Result<Node> {
        let tok = self.peek().clone();

        match tok.token_type {
            TokenType::Integer => {
                self.advance();
                Ok(Node::new(
                    NodeKind::IntegerLiteral { value: tok.value },
                    tok.line,
                    tok.col,
                ))
            }
            TokenType::Float => {
                self.advance();
                Ok(Node::new(
                    NodeKind::FloatLiteral { value: tok.value },
                    tok.line,
                    tok.col,
                ))
            }
            TokenType::String => self.parse_string_literal(),
            TokenType::True => {
                self.advance();
                Ok(Node::new(
                    NodeKind::BoolLiteral { value: true },
                    tok.line,
                    tok.col,
                ))
            }
            TokenType::False => {
                self.advance();
                Ok(Node::new(
                    NodeKind::BoolLiteral { value: false },
                    tok.line,
                    tok.col,
                ))
            }
            TokenType::Null => {
                self.advance();
                Ok(Node::new(NodeKind::NullLiteral, tok.line, tok.col))
            }
            TokenType::Undefined => {
                self.advance();
                Ok(Node::new(NodeKind::UndefinedLiteral, tok.line, tok.col))
            }
            TokenType::Inf => {
                self.advance();
                Ok(Node::new(
                    NodeKind::InfLiteral { negative: false },
                    tok.line,
                    tok.col,
                ))
            }
            TokenType::Nan => {
                self.advance();
                Ok(Node::new(NodeKind::NanLiteral, tok.line, tok.col))
            }
            TokenType::Env => {
                self.advance();
                Ok(Node::new(NodeKind::EnvRef, tok.line, tok.col))
            }
            TokenType::Identifier => {
                self.advance();
                let ident = Node::new(
                    NodeKind::Identifier { name: tok.value.clone() },
                    tok.line,
                    tok.col,
                );
                // §3.7 v0.10: variant_shorthand — `variant_name primary`. When
                // the next token begins a primary (and is not a function-call
                // paren or a postfix operator), parse it as the inner value.
                // Final disambiguation happens in the evaluator via type
                // context; without context the AST node yields a type error.
                if self.starts_variant_shorthand_inner() {
                    let inner = self.parse_primary()?;
                    return Ok(Node::new(
                        NodeKind::VariantShorthand {
                            variant_name: tok.value,
                            inner: Box::new(inner),
                        },
                        tok.line,
                        tok.col,
                    ));
                }
                Ok(ident)
            }
            TokenType::LBrace => self.parse_struct_literal(),
            TokenType::LBracket => self.parse_list_literal(),
            TokenType::LParen => self.parse_tuple_or_group(),
            TokenType::If => self.parse_if_expr(),
            TokenType::Case => self.parse_case_expr(),
            TokenType::Struct => self.parse_struct_import(),
            TokenType::Function => self.parse_function_expr(),
            _ => Err(UzonError::syntax(
                format!("unexpected token: {:?} '{}'", tok.token_type, tok.value),
                tok.line,
                tok.col,
            )),
        }
    }

    /// Parse a string literal with multiline continuation (§4.4.2).
    ///
    /// Adjacent string tokens on immediately following lines are joined with `\n`.
    /// Blank lines or comment lines between string parts break the continuation.
    pub(crate) fn parse_string_literal(&mut self) -> Result<Node> {
        let first = self.peek().clone();
        let span = Span {
            line: first.line,
            col: first.col,
        };
        let mut parts = Vec::new();

        self.parse_string_segment(&mut parts)?;

        // Multiline string continuation
        loop {
            if self.suppress_multiline_string {
                break;
            }
            let saved = self.pos;

            if !self.at(TokenType::Newline) {
                break;
            }
            self.advance(); // consume the one newline

            // If there's another newline (blank line) or EOF, check for comment rejection
            if self.at(TokenType::Newline) || self.at(TokenType::Eof) {
                let string_end_line = self.tokens[saved].line;
                let mut peek_pos = self.pos;
                while peek_pos < self.tokens.len()
                    && self.tokens[peek_pos].token_type == TokenType::Newline
                {
                    peek_pos += 1;
                }
                if peek_pos < self.tokens.len() {
                    let next_tok = &self.tokens[peek_pos];
                    if next_tok.token_type == TokenType::String
                        || next_tok.token_type == TokenType::InterpStart
                    {
                        let next_string_line = next_tok.line;
                        for &cl in &self.comment_lines {
                            if cl > string_end_line && cl < next_string_line {
                                return Err(UzonError::syntax(
                                    "comment between multiline string parts is not allowed"
                                        .to_string(),
                                    next_tok.line,
                                    next_tok.col,
                                ));
                            }
                        }
                    }
                }
                self.pos = saved;
                break;
            }

            if self.at(TokenType::String) || self.at(TokenType::InterpStart) {
                parts.push(StringPart::Literal("\n".to_string()));
                self.parse_string_segment(&mut parts)?;
            } else {
                self.pos = saved;
                break;
            }
        }

        Ok(Node::new(
            NodeKind::StringLiteral { parts },
            span.line,
            span.col,
        ))
    }

    /// Parse one string segment: a sequence of String tokens and InterpStart/InterpEnd pairs.
    fn parse_string_segment(&mut self, parts: &mut Vec<StringPart>) -> Result<()> {
        loop {
            match self.peek_type() {
                TokenType::String => {
                    let tok = self.advance().clone();
                    if !tok.value.is_empty() {
                        parts.push(StringPart::Literal(tok.value));
                    }
                }
                TokenType::InterpStart => {
                    self.advance();
                    self.skip_newlines();
                    let expr = self.parse_expression()?;
                    self.skip_newlines();
                    self.expect(TokenType::InterpEnd)?;
                    parts.push(StringPart::Interpolation(expr));
                }
                _ => break,
            }
        }
        Ok(())
    }

    /// Parse `{ bindings }` — struct literal (§3.1).
    pub(crate) fn parse_struct_literal(&mut self) -> Result<Node> {
        let tok = self.expect(TokenType::LBrace)?;
        let span = Span {
            line: tok.line,
            col: tok.col,
        };
        // Restore `as` parsing inside struct literals
        let saved_suppress_as = self.suppress_as;
        self.suppress_as = false;
        let fields = self.parse_bindings(TokenType::RBrace)?;
        self.expect(TokenType::RBrace)?;
        self.suppress_as = saved_suppress_as;
        Ok(Node::new(
            NodeKind::StructLiteral { fields },
            span.line,
            span.col,
        ))
    }

    /// Parse `[elements]` — list literal (§3.4).
    pub(crate) fn parse_list_literal(&mut self) -> Result<Node> {
        let tok = self.expect(TokenType::LBracket)?;
        let span = Span {
            line: tok.line,
            col: tok.col,
        };
        let saved_suppress_as = self.suppress_as;
        self.suppress_as = false;
        self.skip_newlines();

        let mut elements = Vec::new();
        if !self.at(TokenType::RBracket) {
            elements.push(self.parse_expression()?);
            loop {
                self.skip_newlines();
                if !self.eat(TokenType::Comma) {
                    break;
                }
                self.skip_newlines();
                if self.at(TokenType::RBracket) {
                    break; // trailing comma allowed in list literals
                }
                elements.push(self.parse_expression()?);
            }
        }

        self.skip_newlines();
        self.expect(TokenType::RBracket)?;
        self.suppress_as = saved_suppress_as;
        Ok(Node::new(
            NodeKind::ListLiteral { elements },
            span.line,
            span.col,
        ))
    }

    /// Parse `(expr)` (grouping) or `(e1, e2, ...)` (tuple) (§3.3).
    ///
    /// Distinguishes by presence of comma after first expression.
    pub(crate) fn parse_tuple_or_group(&mut self) -> Result<Node> {
        let tok = self.expect(TokenType::LParen)?;
        let span = Span {
            line: tok.line,
            col: tok.col,
        };
        let saved_suppress_as = self.suppress_as;
        self.suppress_as = false;
        let result = self.parse_tuple_or_group_inner(span);
        self.suppress_as = saved_suppress_as;
        result
    }

    fn parse_tuple_or_group_inner(&mut self, span: Span) -> Result<Node> {
        self.skip_newlines();

        // Empty tuple
        if self.at(TokenType::RParen) {
            self.advance();
            return Ok(Node::new(
                NodeKind::TupleLiteral {
                    elements: Vec::new(),
                },
                span.line,
                span.col,
            ));
        }

        let first = self.parse_expression()?;
        self.skip_newlines();

        // Check for comma -> tuple
        if self.eat(TokenType::Comma) {
            self.skip_newlines();
            let mut elements = vec![first];

            // Trailing comma after single element -> 1-tuple
            if self.at(TokenType::RParen) {
                self.advance();
                return Ok(Node::new(
                    NodeKind::TupleLiteral { elements },
                    span.line,
                    span.col,
                ));
            }

            // Multi-element tuple
            elements.push(self.parse_expression()?);
            loop {
                self.skip_newlines();
                if !self.eat(TokenType::Comma) {
                    break;
                }
                self.skip_newlines();
                if self.at(TokenType::RParen) {
                    break; // trailing comma
                }
                elements.push(self.parse_expression()?);
            }

            self.skip_newlines();
            self.expect(TokenType::RParen)?;
            Ok(Node::new(
                NodeKind::TupleLiteral { elements },
                span.line,
                span.col,
            ))
        } else {
            // No comma -> grouping
            self.expect(TokenType::RParen)?;
            Ok(Node::new(
                NodeKind::Grouping {
                    expr: Box::new(first),
                },
                span.line,
                span.col,
            ))
        }
    }
}
