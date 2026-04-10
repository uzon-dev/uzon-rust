// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::scope::Scope;
use crate::value::*;

use super::Evaluator;

impl Evaluator {
    pub(crate) fn eval_node(&mut self, node: &Node, scope: &mut Scope, exclude: Option<&str>) -> Result<Value> {
        match &node.kind {
            NodeKind::IntegerLiteral { value } => self.eval_integer(value, node),
            NodeKind::FloatLiteral { value } => self.eval_float(value, node),
            NodeKind::StringLiteral { parts } => self.eval_string(parts, scope, exclude, node),
            NodeKind::BoolLiteral { value } => Ok(Value::Bool(*value)),
            NodeKind::NullLiteral => Ok(Value::Null),
            NodeKind::UndefinedLiteral => Ok(Value::Undefined),
            NodeKind::InfLiteral { negative } => {
                Ok(Value::float(if *negative { f64::NEG_INFINITY } else { f64::INFINITY }))
            }
            NodeKind::NanLiteral => Ok(Value::float(f64::NAN)),

            NodeKind::Identifier { name } => {
                if let Some(val) = scope.get(name, exclude) {
                    return Ok(val.clone());
                }
                // Bare identifier not in scope — treat as enum variant placeholder
                Ok(Value::String(name.clone()))
            }
            NodeKind::SelfRef => {
                Err(UzonError::runtime("standalone 'self' is not valid; use self.name", node.span.line, node.span.col))
            }
            NodeKind::EnvRef => {
                Err(UzonError::runtime("standalone 'env' is not valid; use env.NAME", node.span.line, node.span.col))
            }

            NodeKind::MemberAccess { object, member } => {
                self.eval_member_access(object, member, scope, exclude, node)
            }
            NodeKind::BinaryOp { op, left, right } => {
                self.eval_binary_op(*op, left, right, scope, exclude, node)
            }
            NodeKind::UnaryOp { op, operand } => {
                self.eval_unary_op(*op, operand, scope, exclude, node)
            }
            NodeKind::OrElse { left, right } => {
                let lv = self.eval_node(left, scope, exclude)?;
                if lv.is_undefined() {
                    self.eval_node(right, scope, exclude)
                } else {
                    // §5.7: both operands MUST be the same type.
                    // §3.5 rule 4: enum type-context inference for or else right operand.
                    let rv_result = if let Value::Enum(ref e) = lv {
                        self.resolve_enum_context(right, e, scope, exclude)
                    } else {
                        self.eval_node(right, scope, exclude)
                    };
                    if let Ok(rv) = rv_result {
                        if !matches!(lv, Value::Null) && !matches!(rv, Value::Null)
                            && !rv.is_undefined()
                        {
                            if lv.type_name() != rv.type_name() {
                                return Err(UzonError::type_error(
                                    format!(
                                        "'or else' operands must be the same type, got {} and {}",
                                        lv.type_name(), rv.type_name()
                                    ),
                                    node.span.line, node.span.col,
                                ));
                            }
                            // D.3: exact numeric type_ann must be compatible
                            match (&lv, &rv) {
                                (Value::Integer(li), Value::Integer(ri)) => {
                                    if let Err(msg) = UzonInteger::adopt_type(&li.type_ann, &ri.type_ann) {
                                        return Err(UzonError::type_error(
                                            format!("'or else' operands must be the same type: {msg}"),
                                            node.span.line, node.span.col,
                                        ));
                                    }
                                }
                                (Value::Float(lf), Value::Float(rf)) => {
                                    if let Err(msg) = UzonFloat::adopt_type(&lf.type_ann, &rf.type_ann) {
                                        return Err(UzonError::type_error(
                                            format!("'or else' operands must be the same type: {msg}"),
                                            node.span.line, node.span.col,
                                        ));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok(lv)
                }
            }
            NodeKind::IfExpr { condition, then_branch, else_branch } => {
                let cond = self.eval_node(condition, scope, exclude)?;
                let cond = Self::unwrap_union_owned(cond);
                match cond {
                    Value::Bool(true) => {
                        let then_val = self.eval_node(then_branch, scope, exclude)?;
                        // §5.9: speculative type check of else branch
                        let else_result = if let Value::Enum(ref e) = then_val {
                            self.resolve_enum_context(else_branch, e, scope, exclude)
                        } else {
                            self.eval_node(else_branch, scope, exclude)
                        };
                        if let Ok(else_val) = else_result {
                            Self::check_branch_types(&then_val, &else_val, node)?;
                        }
                        Ok(then_val)
                    }
                    Value::Bool(false) => {
                        let else_val = self.eval_node(else_branch, scope, exclude)?;
                        // §5.9: speculative type check of then branch
                        let then_result = if let Value::Enum(ref e) = else_val {
                            self.resolve_enum_context(then_branch, e, scope, exclude)
                        } else {
                            self.eval_node(then_branch, scope, exclude)
                        };
                        if let Ok(then_val) = then_result {
                            Self::check_branch_types(&then_val, &else_val, node)?;
                        }
                        Ok(else_val)
                    }
                    _ => Err(UzonError::type_error(
                        format!("if condition must be bool, got {}", cond.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            NodeKind::CaseExpr { scrutinee, when_clauses, else_branch } => {
                self.eval_case(scrutinee, when_clauses, else_branch, scope, exclude, node)
            }

            NodeKind::TypeAnnotation { expr, type_expr } => {
                self.eval_type_annotation(expr, type_expr, scope, exclude, node)
            }
            NodeKind::Conversion { expr, type_expr } => {
                self.eval_conversion(expr, type_expr, scope, exclude, node)
            }
            NodeKind::FromEnum { value, variants } => {
                self.eval_from_enum(value, variants, scope, exclude, node)
            }
            NodeKind::FromUnion { value, types } => {
                self.eval_from_union(value, types, scope, exclude, node)
            }
            NodeKind::NamedVariant { value, tag, variants } => {
                self.eval_named_variant(value, tag, variants, scope, exclude, node)
            }

            NodeKind::StructLiteral { fields } => {
                self.eval_struct_literal(fields, scope)
            }
            NodeKind::ListLiteral { elements } => {
                self.eval_list_literal(elements, scope, exclude, node)
            }
            NodeKind::TupleLiteral { elements } => {
                let mut vals = Vec::with_capacity(elements.len());
                for elem in elements {
                    vals.push(self.eval_node(elem, scope, exclude)?);
                }
                Ok(Value::Tuple(UzonTuple::new(vals)))
            }
            NodeKind::Grouping { expr } => self.eval_node(expr, scope, exclude),
            NodeKind::StructOverride { base, overrides } => {
                self.eval_struct_override(base, overrides, scope, exclude, node)
            }
            NodeKind::StructExtension { base, extension } => {
                self.eval_struct_extension(base, extension, scope, exclude, node)
            }

            NodeKind::FunctionExpr { params, return_type, body_bindings, body_expr } => {
                let captured = scope.to_map();
                Ok(Value::Function(UzonFunction {
                    params: params.clone(),
                    return_type: return_type.clone(),
                    body_bindings: body_bindings.clone(),
                    body_expr: (**body_expr).clone(),
                    captured_bindings: captured,
                    captured_types: scope.all_types(),
                    type_name: None,
                }))
            }
            NodeKind::FunctionCall { callee, args } => {
                if let NodeKind::MemberAccess { object, member } = &callee.kind {
                    if let NodeKind::Identifier { name } = &object.kind {
                        if name == "std" {
                            return self.eval_std_call(member, args, scope, exclude, node);
                        }
                    }
                }
                self.eval_function_call(callee, args, scope, exclude, node)
            }

            NodeKind::StructImport { path } => {
                let map = self.eval_struct_import(path, node)?;
                Ok(Value::Struct(map))
            }
            NodeKind::FieldExtraction { .. } => {
                Err(UzonError::runtime("'of' can only be used directly after 'is' in a binding", node.span.line, node.span.col))
            }
        }
    }

    // === Literal evaluation ===

    pub(crate) fn eval_integer(&self, value: &str, node: &Node) -> Result<Value> {
        let s = value.replace('_', "");
        let negative = s.starts_with('-');
        let abs = if negative { &s[1..] } else { &s };

        let n: BigInt = if abs.starts_with("0x") || abs.starts_with("0X") {
            BigInt::parse_bytes(abs[2..].as_bytes(), 16)
        } else if abs.starts_with("0o") || abs.starts_with("0O") {
            BigInt::parse_bytes(abs[2..].as_bytes(), 8)
        } else if abs.starts_with("0b") || abs.starts_with("0B") {
            BigInt::parse_bytes(abs[2..].as_bytes(), 2)
        } else {
            BigInt::parse_bytes(abs.as_bytes(), 10)
        }
        .ok_or_else(|| {
            UzonError::runtime(
                format!("invalid integer literal: {value}"),
                node.span.line,
                node.span.col,
            )
        })?;

        let n = if negative { -n } else { n };

        if let Some(i) = n.to_i128() {
            // §4.2: integer literal without `as` defaults to i64 — enforce range
            if !self.in_type_annotation {
                let result = UzonInteger::new(i);
                result.validate_range().map_err(|msg| UzonError::runtime(
                    format!("{msg}; use 'as i128' for larger values"),
                    node.span.line, node.span.col,
                ))?;
            }
            Ok(Value::int(i))
        } else {
            if !self.in_type_annotation {
                return Err(UzonError::runtime(
                    format!("integer literal {value} exceeds i64 range; use an explicit type annotation"),
                    node.span.line, node.span.col,
                ));
            }
            Ok(Value::BigInteger(n))
        }
    }

    pub(crate) fn eval_float(&self, value: &str, node: &Node) -> Result<Value> {
        let s = value.replace('_', "");
        match s.as_str() {
            "inf" => Ok(Value::float(f64::INFINITY)),
            "-inf" => Ok(Value::float(f64::NEG_INFINITY)),
            "nan" => Ok(Value::float(f64::NAN)),
            "-nan" => Ok(Value::float(f64::NAN)), // §5.2: -nan is semantically identical to nan
            _ => {
                let f: f64 = s.parse().map_err(|_| {
                    UzonError::runtime(
                        format!("invalid float literal: {value}"),
                        node.span.line,
                        node.span.col,
                    )
                })?;
                Ok(Value::float(f))
            }
        }
    }

    pub(crate) fn eval_string(
        &mut self,
        parts: &[StringPart],
        scope: &mut Scope,
        exclude: Option<&str>,
        _node: &Node,
    ) -> Result<Value> {
        let mut result = String::new();
        for part in parts {
            match part {
                StringPart::Literal(s) => result.push_str(s),
                StringPart::Interpolation(expr) => {
                    let val = self.eval_node(expr, scope, exclude)?;
                    if val.is_undefined() {
                        return Err(UzonError::runtime(
                            "undefined value in string interpolation; use 'or else' to provide a fallback",
                            expr.span.line,
                            expr.span.col,
                        ));
                    }
                    // §3.6/§3.7.1: unions are transparent in string interpolation
                    let val = Self::unwrap_union_owned(val);
                    match &val {
                        Value::Struct(_) | Value::List(_) | Value::Tuple(_) => {
                            return Err(UzonError::type_error(
                                format!("{} cannot be converted to string", val.type_name()),
                                expr.span.line,
                                expr.span.col,
                            ));
                        }
                        _ => result.push_str(&val.to_string()),
                    }
                }
            }
        }
        Ok(Value::String(result))
    }
}
