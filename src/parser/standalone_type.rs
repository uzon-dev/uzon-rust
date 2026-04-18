// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! v0.9 standalone type declarations (§6.2).
//!
//! Desugars into existing AST nodes:
//!   X is enum v1, v2, ...                   → X = (v1 from v1, v2, ...), called X
//!   X is union T1, T2, ...                  → X = (DefaultForType(T1) from union T1, T2, ...), called X
//!   X is tagged union v1 as T1, v2 as T2, … → X = (DefaultForType(T1) named v1 from v1 as T1, …), called X
//!   X is struct { ... }                     → X = { ... }, called X
//! The binding's `standalone_type_kind` is set so the stringifier can round-trip
//! the declaration in its original form.

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::token::TokenType;

use super::Parser;

impl Parser {
    /// `X is enum v1, v2, ...` (§3.5).
    pub(crate) fn parse_standalone_enum(&mut self, name: String, span: Span) -> Result<Binding> {
        let kw = self.expect(TokenType::Enum)?;
        self.skip_newlines();

        let variants = self.parse_enum_variants_for_standalone()?;

        if variants.len() < 2 {
            return Err(UzonError::syntax(
                "enum must have at least 2 variants".to_string(),
                kw.line, kw.col,
            ));
        }
        // §3.5: duplicate variants are a syntax error
        for (i, v) in variants.iter().enumerate() {
            for prev in &variants[..i] {
                if v == prev {
                    return Err(UzonError::syntax(
                        format!("duplicate enum variant '{v}'"),
                        kw.line, kw.col,
                    ));
                }
            }
        }

        let first = variants[0].clone();
        let value_node = Node::new(
            NodeKind::Identifier { name: first },
            kw.line, kw.col,
        );
        let value = Node::new(
            NodeKind::FromEnum { value: Box::new(value_node), variants },
            kw.line, kw.col,
        );

        self.reject_explicit_called("enum")?;

        Ok(Binding {
            name: name.clone(),
            value,
            called: Some(name),
            is_are: false,
            list_type_annotation: None,
            standalone_type_kind: Some(StandaloneTypeKind::Enum),
            span,
        })
    }

    /// `X is union T1, T2, ...` (§3.6).
    pub(crate) fn parse_standalone_union(&mut self, name: String, span: Span) -> Result<Binding> {
        let kw = self.expect(TokenType::Union)?;
        self.skip_newlines();

        let types = self.parse_type_list_for_standalone()?;

        if types.len() < 2 {
            return Err(UzonError::syntax(
                "union must have at least 2 member types".to_string(),
                kw.line, kw.col,
            ));
        }
        // §3.6: duplicate member types are a syntax error
        for (i, t) in types.iter().enumerate() {
            for prev in &types[..i] {
                if type_expr_path_eq(t, prev) {
                    return Err(UzonError::syntax(
                        format!(
                            "duplicate union member type '{}'",
                            t.path.last().cloned().unwrap_or_default()
                        ),
                        kw.line, kw.col,
                    ));
                }
            }
        }

        let first = types[0].clone();
        let value_node = Node::new(
            NodeKind::DefaultForType { type_expr: first },
            kw.line, kw.col,
        );
        let value = Node::new(
            NodeKind::FromUnion { value: Box::new(value_node), types },
            kw.line, kw.col,
        );

        self.reject_explicit_called("union")?;

        Ok(Binding {
            name: name.clone(),
            value,
            called: Some(name),
            is_are: false,
            list_type_annotation: None,
            standalone_type_kind: Some(StandaloneTypeKind::Union),
            span,
        })
    }

    /// `X is tagged union v1 as T1, v2 as T2, ...` (§3.7).
    pub(crate) fn parse_standalone_tagged_union(
        &mut self,
        name: String,
        span: Span,
    ) -> Result<Binding> {
        let kw = self.expect(TokenType::Tagged)?;
        self.skip_newlines();
        self.expect(TokenType::Union)?;
        self.skip_newlines();

        let variants = self.parse_tagged_union_variants_for_standalone()?;

        if variants.len() < 2 {
            return Err(UzonError::syntax(
                "tagged union must have at least 2 variants".to_string(),
                kw.line, kw.col,
            ));
        }
        // §3.7: duplicate variants are a syntax error
        for (i, (v, _)) in variants.iter().enumerate() {
            for (prev, _) in &variants[..i] {
                if v == prev {
                    return Err(UzonError::syntax(
                        format!("duplicate tagged union variant '{v}'"),
                        kw.line, kw.col,
                    ));
                }
            }
        }

        let (first_name, first_type) = variants[0].clone();
        let value_node = Node::new(
            NodeKind::DefaultForType { type_expr: first_type },
            kw.line, kw.col,
        );
        let value = Node::new(
            NodeKind::NamedVariant {
                value: Box::new(value_node),
                tag: first_name,
                variants,
            },
            kw.line, kw.col,
        );

        self.reject_explicit_called("tagged union")?;

        Ok(Binding {
            name: name.clone(),
            value,
            called: Some(name),
            is_are: false,
            list_type_annotation: None,
            standalone_type_kind: Some(StandaloneTypeKind::TaggedUnion),
            span,
        })
    }

    /// `X is struct { ... }` (§3.2).
    pub(crate) fn parse_standalone_struct(
        &mut self,
        name: String,
        span: Span,
    ) -> Result<Binding> {
        self.expect(TokenType::Struct)?;
        self.skip_newlines();
        let value = self.parse_struct_literal()?;

        self.reject_explicit_called("struct")?;

        Ok(Binding {
            name: name.clone(),
            value,
            called: Some(name),
            is_are: false,
            list_type_annotation: None,
            standalone_type_kind: Some(StandaloneTypeKind::Struct),
            span,
        })
    }

    fn reject_explicit_called(&mut self, kind: &str) -> Result<()> {
        self.skip_newlines();
        if self.at(TokenType::Called) {
            let tok = self.peek();
            return Err(UzonError::syntax(
                format!(
                    "standalone '{kind}' declaration cannot be combined with 'called'; \
                     the binding name is already the type name"
                ),
                tok.line, tok.col,
            ));
        }
        Ok(())
    }

    /// Parse comma-separated enum variants for a standalone `enum` declaration.
    /// Same termination rules as `from` enum variants.
    fn parse_enum_variants_for_standalone(&mut self) -> Result<Vec<String>> {
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
            // 2-token lookahead: if next is `ident is|are`, stop
            if self.is_binding_start_at(look) {
                break;
            }

            self.advance(); // consume comma
            self.skip_newlines();

            // trailing comma not permitted
            if self.at(TokenType::Eof)
                || self.at(TokenType::RBrace)
                || self.at(TokenType::RBracket)
                || self.at(TokenType::RParen)
                || self.at(TokenType::Called)
            {
                let tok = self.peek();
                return Err(UzonError::syntax(
                    "trailing comma is not permitted in enum variants".to_string(),
                    tok.line, tok.col,
                ));
            }
            variants.push(self.parse_variant_name()?);
        }
        Ok(variants)
    }

    /// Parse comma-separated type expressions for a standalone `union` declaration.
    fn parse_type_list_for_standalone(&mut self) -> Result<Vec<TypeExpr>> {
        let mut types = vec![self.parse_type_expr()?];
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
            if self.is_binding_start_at(look) {
                break;
            }

            self.advance(); // consume comma
            self.skip_newlines();

            if self.at(TokenType::Eof)
                || self.at(TokenType::RBrace)
                || self.at(TokenType::RBracket)
                || self.at(TokenType::RParen)
                || self.at(TokenType::Called)
            {
                let tok = self.peek();
                return Err(UzonError::syntax(
                    "trailing comma is not permitted in union member types".to_string(),
                    tok.line, tok.col,
                ));
            }
            types.push(self.parse_type_expr()?);
        }
        Ok(types)
    }

    /// Parse `v1 as T1, v2 as T2, ...` for a standalone `tagged union` declaration.
    fn parse_tagged_union_variants_for_standalone(&mut self) -> Result<Vec<(String, TypeExpr)>> {
        let name = self.parse_variant_name()?;
        self.skip_newlines();
        self.expect(TokenType::As)?;
        self.skip_newlines();
        let type_expr = self.parse_type_expr()?;
        let mut variants = vec![(name, type_expr)];

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
            if self.is_binding_start_at(look) {
                break;
            }

            self.advance(); // consume comma
            self.skip_newlines();

            if self.at(TokenType::Eof)
                || self.at(TokenType::RBrace)
                || self.at(TokenType::RBracket)
                || self.at(TokenType::RParen)
                || self.at(TokenType::Called)
            {
                let tok = self.peek();
                return Err(UzonError::syntax(
                    "trailing comma is not permitted in tagged union variants".to_string(),
                    tok.line, tok.col,
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
}

fn type_expr_path_eq(a: &TypeExpr, b: &TypeExpr) -> bool {
    a.is_null == b.is_null
        && a.is_list == b.is_list
        && a.path == b.path
        && match (&a.inner, &b.inner) {
            (Some(ai), Some(bi)) => type_expr_path_eq(ai, bi),
            (None, None) => true,
            _ => false,
        }
        && a.tuple_types == b.tuple_types
}
