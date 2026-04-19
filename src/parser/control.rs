// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::token::TokenType;

use super::Parser;

/// Convert a TypeExpr to its string representation for `case type` matching.
pub(super) fn type_expr_to_string(te: &TypeExpr) -> String {
    if te.is_null {
        return "null".to_string();
    }
    if te.is_list {
        if let Some(ref inner) = te.inner {
            return format!("[{}]", type_expr_to_string(inner));
        }
        return "list".to_string();
    }
    if let Some(ref types) = te.tuple_types {
        let parts: Vec<String> = types.iter().map(|t| type_expr_to_string(t)).collect();
        return format!("({})", parts.join(", "));
    }
    te.path.join(".")
}

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

    /// Parse `case [type|named] expr when v then r ... else default` (§5.10).
    ///
    /// Three forms:
    /// - `case expr` — value matching
    /// - `case type expr` — type dispatch (untagged unions)
    /// - `case named expr` — variant dispatch (tagged unions)
    ///
    /// At least one `when` clause is required.
    pub(crate) fn parse_case_expr(&mut self) -> Result<Node> {
        let tok = self.expect(TokenType::Case)?;
        let span = Span {
            line: tok.line,
            col: tok.col,
        };
        self.skip_newlines();

        // Determine case mode: `case type`, `case named`, or plain `case`
        let mode = if self.at(TokenType::Type) {
            self.advance();
            self.skip_newlines();
            CaseMode::Type
        } else if self.at(TokenType::Named) {
            self.advance();
            self.skip_newlines();
            CaseMode::Named
        } else {
            CaseMode::Value
        };

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

            // For `case named` and `case type`, when values are bare identifiers (tags/types)
            let value = match mode {
                CaseMode::Named => {
                    let name = self.parse_variant_name()?;
                    Node::new(
                        NodeKind::Identifier { name },
                        when_span.line,
                        when_span.col,
                    )
                }
                CaseMode::Type => {
                    // §5.10 v0.8: compound type expressions allowed: [i32], (i32, string), etc.
                    let type_expr = self.parse_type_expr()?;
                    let name = type_expr_to_string(&type_expr);
                    Node::new(
                        NodeKind::Identifier { name },
                        when_span.line,
                        when_span.col,
                    )
                }
                CaseMode::Value => self.parse_expression()?,
            };

            self.skip_newlines();
            self.expect(TokenType::Then)?;
            self.skip_newlines();
            let result = self.parse_expression()?;

            when_clauses.push(WhenClause {
                value,
                result,
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
                mode,
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

        // §3.8: reject duplicate parameter names
        for (i, p) in params.iter().enumerate() {
            for prev in &params[..i] {
                if p.name == prev.name {
                    return Err(UzonError::syntax(
                        format!("duplicate parameter name '{}'", p.name),
                        p.span.line, p.span.col,
                    ));
                }
            }
        }

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

        // §3.8: default expressions MUST NOT reference any parameter of the same function
        // (parameters are not in scope until the function body begins) — syntax error.
        // Also: `undefined` is not permitted as a default value.
        let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        for param in &params {
            if let Some(ref default_expr) = param.default {
                if let Some(name) = find_param_ref_in_node(default_expr, &param_names) {
                    return Err(UzonError::syntax(
                        format!(
                            "default expression for parameter '{}' references parameter '{}'; \
                             parameters are not in scope until the function body begins",
                            param.name, name
                        ),
                        default_expr.span.line, default_expr.span.col,
                    ));
                }
                if contains_undefined_literal(default_expr) {
                    return Err(UzonError::syntax(
                        format!(
                            "'undefined' is not permitted as a default value for parameter '{}'",
                            param.name
                        ),
                        default_expr.span.line, default_expr.span.col,
                    ));
                }
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
            // §9 func_binding: only `name "is" expression` — `are` is not
            // allowed inside function bodies.
            if self.is_func_binding_start_at(self.pos) {
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

/// Walk a default-expression AST looking for an identifier that refers to one
/// of the function's parameter names (§3.8: parameters are not in scope inside
/// their own defaults). Returns the name on the first hit.
fn find_param_ref_in_node(node: &Node, params: &[String]) -> Option<String> {
    match &node.kind {
        NodeKind::Identifier { name } => {
            if params.iter().any(|p| p == name) {
                Some(name.clone())
            } else {
                None
            }
        }
        NodeKind::MemberAccess { object, .. } => find_param_ref_in_node(object, params),
        NodeKind::BinaryOp { left, right, .. } => find_param_ref_in_node(left, params)
            .or_else(|| find_param_ref_in_node(right, params)),
        NodeKind::UnaryOp { operand, .. } => find_param_ref_in_node(operand, params),
        NodeKind::OrElse { left, right } => find_param_ref_in_node(left, params)
            .or_else(|| find_param_ref_in_node(right, params)),
        NodeKind::IfExpr { condition, then_branch, else_branch } => {
            find_param_ref_in_node(condition, params)
                .or_else(|| find_param_ref_in_node(then_branch, params))
                .or_else(|| find_param_ref_in_node(else_branch, params))
        }
        NodeKind::CaseExpr { scrutinee, when_clauses, else_branch, .. } => {
            if let Some(n) = find_param_ref_in_node(scrutinee, params) { return Some(n); }
            for w in when_clauses {
                if let Some(n) = find_param_ref_in_node(&w.value, params) { return Some(n); }
                if let Some(n) = find_param_ref_in_node(&w.result, params) { return Some(n); }
            }
            find_param_ref_in_node(else_branch, params)
        }
        NodeKind::TypeAnnotation { expr, .. } => find_param_ref_in_node(expr, params),
        NodeKind::Conversion { expr, .. } => find_param_ref_in_node(expr, params),
        NodeKind::FromEnum { value, .. } => find_param_ref_in_node(value, params),
        NodeKind::FromUnion { value, .. } => find_param_ref_in_node(value, params),
        NodeKind::NamedVariant { value, .. } => find_param_ref_in_node(value, params),
        NodeKind::VariantShorthand { inner, .. } => find_param_ref_in_node(inner, params),
        NodeKind::StructLiteral { fields } => {
            for b in fields {
                if let Some(n) = find_param_ref_in_node(&b.value, params) { return Some(n); }
            }
            None
        }
        NodeKind::ListLiteral { elements } | NodeKind::TupleLiteral { elements } => {
            for e in elements {
                if let Some(n) = find_param_ref_in_node(e, params) { return Some(n); }
            }
            None
        }
        NodeKind::Grouping { expr } => find_param_ref_in_node(expr, params),
        NodeKind::StructOverride { base, overrides } => {
            find_param_ref_in_node(base, params)
                .or_else(|| find_param_ref_in_node(overrides, params))
        }
        NodeKind::StructExtension { base, extension } => {
            find_param_ref_in_node(base, params)
                .or_else(|| find_param_ref_in_node(extension, params))
        }
        NodeKind::FunctionCall { callee, args } => {
            if let Some(n) = find_param_ref_in_node(callee, params) { return Some(n); }
            for a in args {
                if let Some(n) = find_param_ref_in_node(a, params) { return Some(n); }
            }
            None
        }
        NodeKind::StringLiteral { parts } => {
            for p in parts {
                if let StringPart::Interpolation(e) = p {
                    if let Some(n) = find_param_ref_in_node(e, params) { return Some(n); }
                }
            }
            None
        }
        NodeKind::FieldExtraction { source } => find_param_ref_in_node(source, params),
        // Leaf kinds with no subexpressions that could reference a param.
        NodeKind::IntegerLiteral { .. }
        | NodeKind::FloatLiteral { .. }
        | NodeKind::BoolLiteral { .. }
        | NodeKind::NullLiteral
        | NodeKind::UndefinedLiteral
        | NodeKind::InfLiteral { .. }
        | NodeKind::NanLiteral
        | NodeKind::EnvRef
        | NodeKind::StructImport { .. }
        | NodeKind::DefaultForType { .. }
        | NodeKind::FunctionExpr { .. } => None,
    }
}

/// Detect an unconditional `undefined` literal in a default expression.
/// A literal `undefined` (or anything that trivially yields it, like a
/// type-annotation wrapper around `undefined`) is rejected statically.
fn contains_undefined_literal(node: &Node) -> bool {
    match &node.kind {
        NodeKind::UndefinedLiteral => true,
        NodeKind::Grouping { expr } => contains_undefined_literal(expr),
        NodeKind::TypeAnnotation { expr, .. } => contains_undefined_literal(expr),
        _ => false,
    }
}
