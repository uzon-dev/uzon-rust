// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::token::{TokenType, is_keyword, is_reserved_keyword};

use super::Parser;

impl Parser {
    /// Continue parsing from type-declaration level with an already-parsed left
    /// expression. Used by binding decomposition when `IsNamed`/`IsType` are
    /// decomposed into an identifier + type-declaration suffixes (§9).
    pub(crate) fn continue_from_type_decl(&mut self, expr: Node) -> Result<Node> {
        self.skip_newlines();
        if self.at(TokenType::From) {
            return self.parse_from_clause(expr);
        }
        if self.at(TokenType::Named) {
            return self.parse_named_clause(expr);
        }
        Ok(expr)
    }

    /// Level 5: `from`, `named` — type declaration (§3.5, §3.6, §3.7).
    ///
    /// Respects NEWLINE_SEP: if a newline precedes `from`/`named` and the next
    /// non-newline token starts a new binding, the expression ends here.
    pub(crate) fn parse_type_decl(&mut self) -> Result<Node> {
        let expr = self.parse_type_annotation()?;

        // Only look for `from`/`named` continuation if the next non-newline token
        // does not start a new binding (NEWLINE_SEP rule).
        if self.at(TokenType::Newline) {
            let mut look = self.pos;
            while look < self.tokens.len()
                && self.tokens[look].token_type == TokenType::Newline
            {
                look += 1;
            }
            if look < self.tokens.len() && self.is_binding_start_at(look) {
                return Ok(expr);
            }
        }

        self.skip_newlines();

        if self.at(TokenType::From) {
            return self.parse_from_clause(expr);
        }

        if self.at(TokenType::Named) {
            return self.parse_named_clause(expr);
        }

        Ok(expr)
    }

    /// Parse `from` clause: enum (`from v1, v2, ...`) or union (`from union T1, T2, ...`) (§3.5, §3.6).
    fn parse_from_clause(&mut self, value: Node) -> Result<Node> {
        let span = self.current_span();
        self.advance(); // consume `from`
        self.skip_newlines();

        // Check for `union` -> untagged union (§3.6)
        if self.at(TokenType::Union) {
            self.advance();
            self.skip_newlines();
            let mut types = vec![self.parse_type_expr()?];
            loop {
                self.skip_newlines();
                if !self.at(TokenType::Comma) {
                    break;
                }
                // Comma lookahead: if next is a new binding or a container closer,
                // leave comma for outer construct (§11.4 trailing comma in struct/list/tuple).
                let mut look = self.pos + 1;
                while look < self.tokens.len()
                    && self.tokens[look].token_type == TokenType::Newline
                {
                    look += 1;
                }
                if self.is_binding_start_at(look)
                    || matches!(
                        self.tokens[look.min(self.tokens.len() - 1)].token_type,
                        TokenType::RBrace | TokenType::RBracket | TokenType::RParen
                    )
                {
                    break;
                }

                self.advance(); // consume comma
                self.skip_newlines();

                // Trailing comma not permitted
                if self.at(TokenType::Called)
                    || self.at(TokenType::Eof)
                {
                    let tok = self.peek();
                    return Err(UzonError::syntax(
                        "trailing comma is not permitted in 'from union' types".to_string(),
                        tok.line,
                        tok.col,
                    ));
                }

                types.push(self.parse_type_expr()?);
            }
            return Ok(Node::new(
                NodeKind::FromUnion {
                    value: Box::new(value),
                    types,
                },
                span.line,
                span.col,
            ));
        }

        // Enum: `value from variant1, variant2, ...` (§3.5)
        let mut variants = vec![self.parse_variant_name()?];

        loop {
            self.skip_newlines();
            if !self.at(TokenType::Comma) {
                break;
            }

            let mut look = self.pos + 1;
            while look < self.tokens.len()
                && self.tokens[look].token_type == TokenType::Newline
            {
                look += 1;
            }
            if self.is_binding_start_at(look)
                || matches!(
                    self.tokens[look.min(self.tokens.len() - 1)].token_type,
                    TokenType::RBrace | TokenType::RBracket | TokenType::RParen
                )
            {
                break;
            }

            self.advance(); // consume comma
            self.skip_newlines();

            // Trailing comma not permitted in enum variants
            if self.at(TokenType::Called)
                || self.at(TokenType::Eof)
            {
                let tok = self.peek();
                return Err(UzonError::syntax(
                    "trailing comma is not permitted in enum variants".to_string(),
                    tok.line,
                    tok.col,
                ));
            }

            variants.push(self.parse_variant_name()?);
        }

        Ok(Node::new(
            NodeKind::FromEnum {
                value: Box::new(value),
                variants,
            },
            span.line,
            span.col,
        ))
    }

    /// Parse `named tag [from variant as type, ...]` or `named tag as TypeName` — tagged union (§3.7, §6.3).
    fn parse_named_clause(&mut self, value: Node) -> Result<Node> {
        let span = self.current_span();
        self.advance(); // consume `named`
        self.skip_newlines();

        let tag = self.parse_variant_name()?;

        self.skip_newlines();
        let (value, variants) = if self.at(TokenType::From) {
            self.advance();
            self.skip_newlines();
            (value, self.parse_tagged_union_variants()?)
        } else if self.at(TokenType::As) {
            // §6.3 v0.8: `as Type` MUST precede `named variant`.
            // `value named tag as Type` is a syntax error — use `value as Type named tag`.
            return Err(crate::error::UzonError::syntax(
                "'as' must precede 'named'; use 'value as Type named variant' order",
                self.current_span().line, self.current_span().col,
            ));
        } else {
            (value, Vec::new())
        };

        Ok(Node::new(
            NodeKind::NamedVariant {
                value: Box::new(value),
                tag,
                variants,
            },
            span.line,
            span.col,
        ))
    }

    /// Parse tagged union variant definitions: `name as Type, name as Type, ...` (§3.7).
    fn parse_tagged_union_variants(&mut self) -> Result<Vec<(String, TypeExpr)>> {
        let mut variants = Vec::new();

        let name = self.parse_variant_name()?;
        self.skip_newlines();
        self.expect(TokenType::As)?;
        self.skip_newlines();
        let type_expr = self.parse_type_expr()?;
        variants.push((name, type_expr));

        loop {
            self.skip_newlines();
            if !self.at(TokenType::Comma) {
                break;
            }

            let mut look = self.pos + 1;
            while look < self.tokens.len()
                && self.tokens[look].token_type == TokenType::Newline
            {
                look += 1;
            }
            if self.is_binding_start_at(look)
                || matches!(
                    self.tokens[look.min(self.tokens.len() - 1)].token_type,
                    TokenType::RBrace | TokenType::RBracket | TokenType::RParen
                )
            {
                break;
            }

            self.advance(); // consume comma
            self.skip_newlines();

            // Trailing comma not permitted in tagged union variants
            if self.at(TokenType::Called)
                || self.at(TokenType::Eof)
            {
                let tok = self.peek();
                return Err(UzonError::syntax(
                    "trailing comma is not permitted in tagged union variants".to_string(),
                    tok.line,
                    tok.col,
                ));
            }

            let name = self.parse_variant_name()?;
            self.skip_newlines();
            self.expect(TokenType::As)?;
            self.skip_newlines();
            let type_expr = self.parse_type_expr()?;
            variants.push((name, type_expr));
        }

        Ok(variants)
    }

    /// Parse a variant name — can be an identifier or a keyword (keywords are valid variant names).
    pub(crate) fn parse_variant_name(&mut self) -> Result<String> {
        let tok = self.peek().clone();
        match tok.token_type {
            TokenType::Identifier => {
                self.advance();
                Ok(tok.value)
            }
            _ if is_keyword(&tok.value) || is_reserved_keyword(&tok.value) => {
                self.advance();
                Ok(tok.value)
            }
            _ => Err(UzonError::syntax(
                format!("expected variant name, found {:?}", tok.token_type),
                tok.line,
                tok.col,
            )),
        }
    }
}
