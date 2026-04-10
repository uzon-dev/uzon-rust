// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::token::TokenType;

use super::Parser;

impl Parser {
    /// Parse `if condition then expr else expr` (§5.9).
    pub(crate) fn parse_if_expr(&mut self) -> Result<Node> {
        let tok = self.expect(TokenType::If)?;
        let span = Span {
            line: tok.line,
            col: tok.col,
        };
        self.skip_newlines();
        let condition = self.parse_expression()?;
        self.skip_newlines();
        self.expect(TokenType::Then)?;
        self.skip_newlines();
        let then_branch = self.parse_expression()?;
        self.skip_newlines();
        self.expect(TokenType::Else)?;
        self.skip_newlines();
        let else_branch = self.parse_expression()?;

        Ok(Node::new(
            NodeKind::IfExpr {
                condition: Box::new(condition),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
            },
            span.line,
            span.col,
        ))
    }

    /// Parse `case expr when v then r ... else default` (§5.10).
    ///
    /// At least one `when` clause is required.
    pub(crate) fn parse_case_expr(&mut self) -> Result<Node> {
        let tok = self.expect(TokenType::Case)?;
        let span = Span {
            line: tok.line,
            col: tok.col,
        };
        self.skip_newlines();
        let scrutinee = self.parse_expression()?;

        let mut when_clauses = Vec::new();
        loop {
            self.skip_newlines();
            if !self.at(TokenType::When) {
                break;
            }
            let when_span = self.current_span();
            self.advance();
            self.skip_newlines();

            // `when named <tag>` for tagged union matching (§3.7.2)
            let (value, is_named) = if self.at(TokenType::Named) {
                self.advance();
                self.skip_newlines();
                let name = self.parse_variant_name()?;
                let node = Node::new(
                    NodeKind::Identifier { name },
                    when_span.line,
                    when_span.col,
                );
                (node, true)
            } else {
                (self.parse_expression()?, false)
            };

            self.skip_newlines();
            self.expect(TokenType::Then)?;
            self.skip_newlines();
            let result = self.parse_expression()?;

            when_clauses.push(WhenClause {
                value,
                result,
                is_named,
                span: when_span,
            });
        }

        // §5.10: at least one `when` clause is required
        if when_clauses.is_empty() {
            return Err(UzonError::syntax(
                "case expression requires at least one when clause",
                span.line,
                span.col,
            ));
        }

        self.skip_newlines();
        self.expect(TokenType::Else)?;
        self.skip_newlines();
        let else_branch = self.parse_expression()?;

        Ok(Node::new(
            NodeKind::CaseExpr {
                scrutinee: Box::new(scrutinee),
                when_clauses,
                else_branch: Box::new(else_branch),
            },
            span.line,
            span.col,
        ))
    }

    /// Parse `function params returns Type { body }` (§3.8).
    pub(crate) fn parse_function_expr(&mut self) -> Result<Node> {
        let tok = self.expect(TokenType::Function)?;
        let span = Span {
            line: tok.line,
            col: tok.col,
        };
        self.skip_newlines();

        // Parse parameter list (no parentheses per spec §3.8)
        let mut params = Vec::new();
        if !self.at(TokenType::Returns) && !self.at(TokenType::LBrace) {
            params.push(self.parse_function_param()?);
            loop {
                self.skip_newlines();
                if !self.eat(TokenType::Comma) {
                    break;
                }
                self.skip_newlines();
                params.push(self.parse_function_param()?);
            }
        }
        self.skip_newlines();

        // §3.8: parameters with defaults must appear after all required parameters
        let mut seen_default = false;
        for param in &params {
            if param.default.is_some() {
                seen_default = true;
            } else if seen_default {
                return Err(UzonError::syntax(
                    format!(
                        "required parameter '{}' cannot appear after a parameter with a default value",
                        param.name
                    ),
                    param.span.line,
                    param.span.col,
                ));
            }
        }

        // Parse return type (mandatory per §3.8 EBNF)
        self.expect(TokenType::Returns)?;
        self.skip_newlines();
        let return_type = self.parse_type_expr()?;
        self.skip_newlines();

        // Parse body: { bindings... expr }
        self.expect(TokenType::LBrace)?;
        self.skip_newlines();

        let mut body_bindings = Vec::new();
        // Suppress multiline string continuation inside function bodies
        let prev_suppress = self.suppress_multiline_string;
        self.suppress_multiline_string = true;
        while !self.at(TokenType::RBrace) && !self.at(TokenType::Eof) {
            if self.is_binding_start_at(self.pos) {
                body_bindings.push(self.parse_binding()?);
                self.skip_separator();
            } else {
                break;
            }
        }

        // Parse body expression (the return value)
        let body_expr = self.parse_expression()?;
        self.suppress_multiline_string = prev_suppress;
        self.skip_newlines();
        self.expect(TokenType::RBrace)?;

        Ok(Node::new(
            NodeKind::FunctionExpr {
                params,
                return_type,
                body_bindings,
                body_expr: Box::new(body_expr),
            },
            span.line,
            span.col,
        ))
    }

    /// Parse a function parameter: `name as Type [default expr]` (§3.8).
    fn parse_function_param(&mut self) -> Result<FunctionParam> {
        let name_tok = self.expect(TokenType::Identifier)?;
        let span = Span {
            line: name_tok.line,
            col: name_tok.col,
        };
        self.skip_newlines();
        self.expect(TokenType::As)?;
        self.skip_newlines();
        let type_expr = self.parse_type_expr()?;
        self.skip_newlines();

        // Optional default value
        let default = if self.at(TokenType::Default) {
            self.advance();
            self.skip_newlines();
            Some(Box::new(self.parse_expression()?))
        } else {
            None
        };

        Ok(FunctionParam {
            name: name_tok.value,
            type_expr,
            default,
            span,
        })
    }

    /// Parse `struct "path"` — file import (§7).
    ///
    /// Rejects interpolated strings in the path.
    pub(crate) fn parse_struct_import(&mut self) -> Result<Node> {
        let tok = self.expect(TokenType::Struct)?;
        let span = Span {
            line: tok.line,
            col: tok.col,
        };
        self.skip_newlines();
        let path_tok = self.expect(TokenType::String)?;

        // Reject interpolated strings in struct import paths
        if self.at(TokenType::InterpStart) {
            return Err(UzonError::syntax(
                "struct import path must be a plain string literal, not an interpolated string",
                path_tok.line,
                path_tok.col,
            ));
        }

        Ok(Node::new(
            NodeKind::StructImport {
                path: path_tok.value,
            },
            span.line,
            span.col,
        ))
    }
}
