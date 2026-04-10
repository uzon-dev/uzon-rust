// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::ast::*;
use crate::error::Result;
use crate::token::TokenType;

use super::Parser;

impl Parser {
    // === Type expressions (§6) ===

    /// Parse a type expression: simple name, dotted path, list `[T]`, tuple `(T, U)`, or `null`.
    pub(crate) fn parse_type_expr(&mut self) -> Result<TypeExpr> {
        let span = self.current_span();

        // Tuple type: `(Type, Type, ...)` — requires at least 2 types
        if self.at(TokenType::LParen) {
            return self.parse_tuple_type(span);
        }

        // List type: `[Type]`
        if self.at(TokenType::LBracket) {
            return self.parse_list_type(span);
        }

        // `null` type (for tagged union variants like `down as null`)
        if self.at(TokenType::Null) {
            self.advance();
            return Ok(TypeExpr {
                path: Vec::new(),
                is_list: false,
                inner: None,
                is_null: true,
                tuple_types: None,
                span,
            });
        }

        // Simple type or dotted path: `Type` or `outer.inner.Type`
        let mut path = Vec::new();
        let first = self.expect(TokenType::Identifier)?;
        path.push(first.value);

        while self.at(TokenType::Dot) {
            self.advance();
            let seg = self.expect(TokenType::Identifier)?;
            path.push(seg.value);
        }

        Ok(TypeExpr {
            path,
            is_list: false,
            inner: None,
            is_null: false,
            tuple_types: None,
            span,
        })
    }

    /// Parse tuple type: `(Type, Type, ...)`
    fn parse_tuple_type(&mut self, span: Span) -> Result<TypeExpr> {
        self.advance();
        self.skip_newlines();
        let first = self.parse_type_expr()?;
        self.skip_newlines();
        self.expect(TokenType::Comma)?;
        self.skip_newlines();
        let mut types = vec![first];
        types.push(self.parse_type_expr()?);
        loop {
            self.skip_newlines();
            if !self.eat(TokenType::Comma) {
                break;
            }
            self.skip_newlines();
            if self.at(TokenType::RParen) {
                break;
            }
            types.push(self.parse_type_expr()?);
        }
        self.skip_newlines();
        self.expect(TokenType::RParen)?;
        Ok(TypeExpr {
            path: Vec::new(),
            is_list: false,
            inner: None,
            is_null: false,
            tuple_types: Some(types),
            span,
        })
    }

    /// Parse list type: `[Type]`
    fn parse_list_type(&mut self, span: Span) -> Result<TypeExpr> {
        self.advance();
        self.skip_newlines();
        let inner = self.parse_type_expr()?;
        self.skip_newlines();
        self.expect(TokenType::RBracket)?;
        Ok(TypeExpr {
            path: Vec::new(),
            is_list: true,
            inner: Some(Box::new(inner)),
            is_null: false,
            tuple_types: None,
            span,
        })
    }
}
