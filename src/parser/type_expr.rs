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

    /// Parse parenthesized type: grouping `(Type)` or tuple `(Type, Type, ...)` / `(Type,)`.
    ///
    /// §3.4.2: `(T)` is grouping (returns `T`), `(T,)` is a 1-tuple type.
    fn parse_tuple_type(&mut self, span: Span) -> Result<TypeExpr> {
        self.advance(); // consume `(`
        self.skip_newlines();

        // () — empty tuple type
        if self.at(TokenType::RParen) {
            self.advance();
            return Ok(TypeExpr {
                path: Vec::new(),
                is_list: false,
                inner: None,
                is_null: false,
                tuple_types: Some(Vec::new()),
                span,
            });
        }

        let first = self.parse_type_expr()?;
        self.skip_newlines();

        // (Type) — grouping, not a tuple
        if self.at(TokenType::RParen) {
            self.advance();
            return Ok(first);
        }

        // Comma required for tuple
        self.expect(TokenType::Comma)?;
        self.skip_newlines();
        let mut types = vec![first];

        // (Type,) — 1-tuple (trailing comma after single type)
        if self.at(TokenType::RParen) {
            self.advance();
            return Ok(TypeExpr {
                path: Vec::new(),
                is_list: false,
                inner: None,
                is_null: false,
                tuple_types: Some(types),
                span,
            });
        }

        // 2+ element tuple: (Type, Type, ...)
        types.push(self.parse_type_expr()?);
        loop {
            self.skip_newlines();
            if !self.eat(TokenType::Comma) {
                break;
            }
            self.skip_newlines();
            if self.at(TokenType::RParen) {
                break; // trailing comma
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
