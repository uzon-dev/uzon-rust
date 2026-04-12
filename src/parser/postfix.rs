// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::token::TokenType;

use super::Parser;

impl Parser {
    /// Level 4: `as` — type annotation/assertion (§6.1).
    ///
    /// Allows chained `as Type to Type` (e.g., `100 as u8 to u16`).
    pub(crate) fn parse_type_annotation(&mut self) -> Result<Node> {
        let expr = self.parse_struct_override()?;
        if self.suppress_as {
            return Ok(expr);
        }
        self.skip_newlines();
        if self.at(TokenType::As) {
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let type_expr = self.parse_type_expr()?;
            let mut result = Node::new(
                NodeKind::TypeAnnotation {
                    expr: Box::new(expr),
                    type_expr,
                },
                span.line,
                span.col,
            );
            // Allow chained `to` after `as type`
            self.skip_newlines();
            if self.at(TokenType::To) {
                let to_span = self.current_span();
                self.advance();
                self.skip_newlines();
                let to_type = self.parse_type_expr()?;
                result = Node::new(
                    NodeKind::Conversion {
                        expr: Box::new(result),
                        type_expr: to_type,
                    },
                    to_span.line,
                    to_span.col,
                );
            }
            Ok(result)
        } else {
            Ok(expr)
        }
    }

    /// Level 3: `with` / `plus` — struct override/extension (§3.2.1, §3.2.2).
    ///
    /// No chaining: `base with {...} with {...}` is a syntax error.
    fn parse_struct_override(&mut self) -> Result<Node> {
        let expr = self.parse_conversion()?;
        self.skip_newlines();
        if self.at(TokenType::With) {
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let overrides = self.parse_struct_literal()?;
            // Reject chaining
            self.skip_newlines();
            if self.at(TokenType::With) || self.at(TokenType::PlusKw) {
                let tok = self.peek();
                return Err(UzonError::syntax(
                    "cannot chain 'with'/'plus'; use a single operator per expression",
                    tok.line,
                    tok.col,
                ));
            }
            Ok(Node::new(
                NodeKind::StructOverride {
                    base: Box::new(expr),
                    overrides: Box::new(overrides),
                },
                span.line,
                span.col,
            ))
        } else if self.at(TokenType::PlusKw) {
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let extension = self.parse_struct_literal()?;
            // Reject chaining
            self.skip_newlines();
            if self.at(TokenType::With) || self.at(TokenType::PlusKw) {
                let tok = self.peek();
                return Err(UzonError::syntax(
                    "cannot chain 'with'/'plus'; use a single operator per expression",
                    tok.line,
                    tok.col,
                ));
            }
            Ok(Node::new(
                NodeKind::StructExtension {
                    base: Box::new(expr),
                    extension: Box::new(extension),
                },
                span.line,
                span.col,
            ))
        } else {
            Ok(expr)
        }
    }

    /// Level 2: `to` — type conversion (§5.11).
    fn parse_conversion(&mut self) -> Result<Node> {
        let expr = self.parse_member_access()?;
        self.skip_newlines();
        if self.at(TokenType::To) {
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let type_expr = self.parse_type_expr()?;
            Ok(Node::new(
                NodeKind::Conversion {
                    expr: Box::new(expr),
                    type_expr,
                },
                span.line,
                span.col,
            ))
        } else {
            Ok(expr)
        }
    }

    /// Level 1: `.` and `()` — member access and function call (§5.12, §3.8), left-associative.
    pub(crate) fn parse_member_access(&mut self) -> Result<Node> {
        let mut expr = self.parse_primary()?;
        loop {
            self.skip_newlines();
            if self.at(TokenType::Dot) {
                self.advance();
                let member_tok = self.advance().clone();
                let member = member_tok.value;
                expr = Node::new(
                    NodeKind::MemberAccess {
                        object: Box::new(expr),
                        member,
                    },
                    member_tok.line,
                    member_tok.col,
                );
            } else if self.at(TokenType::LParen) {
                let span = self.current_span();
                self.advance();
                self.skip_newlines();
                let mut args = Vec::new();
                if !self.at(TokenType::RParen) {
                    args.push(self.parse_expression()?);
                    loop {
                        self.skip_newlines();
                        if !self.eat(TokenType::Comma) {
                            break;
                        }
                        self.skip_newlines();
                        if self.at(TokenType::RParen) {
                            break;
                        }
                        args.push(self.parse_expression()?);
                    }
                }
                self.skip_newlines();
                self.expect(TokenType::RParen)?;
                expr = Node::new(
                    NodeKind::FunctionCall {
                        callee: Box::new(expr),
                        args,
                    },
                    span.line,
                    span.col,
                );
            } else {
                break;
            }
        }
        Ok(expr)
    }
}
