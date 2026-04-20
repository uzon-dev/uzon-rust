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
                // Suggest correct form for case-variant typos of keywords
                let lower = name.to_ascii_lowercase();
                if lower != *name
                    && (crate::token::keyword_token_type(&lower).is_some()
                        || crate::token::is_reserved_keyword(&lower))
                {
                    return Err(UzonError::runtime(
                        format!("unknown identifier '{name}'; did you mean '{lower}'?"),
                        node.span.line, node.span.col,
                    ));
                }
                // §5.12: unresolved bare name evaluates to undefined
                Ok(Value::Undefined)
            }
            NodeKind::EnvRef => {
                // §5.13: standalone `env` without member access is a type error.
                Err(UzonError::type_error("standalone 'env' is not valid; use env.NAME", node.span.line, node.span.col))
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
                self.eval_or_else(left, right, scope, exclude, node)
            }
            NodeKind::IfExpr { condition, then_branch, else_branch } => {
                self.eval_if_expr(condition, then_branch, else_branch, scope, exclude, node)
            }
            NodeKind::CaseExpr { mode, scrutinee, when_clauses, else_branch } => {
                self.eval_case(mode, scrutinee, when_clauses, else_branch, scope, exclude, node)
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
            NodeKind::VariantShorthand { variant_name, .. } => {
                // §3.7 v0.10: variant_shorthand requires type context. When the
                // evaluator reaches this node directly (no enclosing
                // `as`/struct-field/parameter type), the tagged union type is
                // unknown — a type error.
                Err(UzonError::type_error(
                    format!("variant shorthand '{variant_name}' requires a tagged union type context; \
                             use 'value as Type named {variant_name}' or annotate with 'as TypeName'"),
                    node.span.line, node.span.col,
                ))
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
                // §4.5: literal 'undefined' as the function body's final (return)
                // expression is a type error.
                if matches!(body_expr.kind, NodeKind::UndefinedLiteral) {
                    return Err(UzonError::type_error(
                        "literal 'undefined' is not allowed as a function body's final expression; \
                         return an expression that evaluates to undefined instead",
                        body_expr.span.line, body_expr.span.col,
                    ));
                }
                // §6.2: Validate parameter and return type names at definition time
                for param in params {
                    self.validate_type_exists(&param.type_expr, scope, node)?;
                }
                self.validate_type_exists(return_type, scope, node)?;

                // §3.8: Eagerly evaluate defaults in the enclosing scope to catch
                // type mismatches and `undefined` defaults at definition time.
                // (Spec permits either once-at-definition or once-per-call; we pick
                // once-at-definition so errors surface even for uncalled functions.)
                for param in params {
                    if let Some(ref default_expr) = param.default {
                        let val = self.eval_node(default_expr, scope, None)?;
                        if val.is_undefined() {
                            return Err(UzonError::type_error(
                                format!(
                                    "default value for parameter '{}' evaluates to undefined; \
                                     'undefined' is not permitted as a default",
                                    param.name
                                ),
                                default_expr.span.line, default_expr.span.col,
                            ));
                        }
                        if let Some(type_name) = param.type_expr.path.last() {
                            self.check_type_assertion(&val, type_name, default_expr)?;
                        }
                    }
                }

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
                Ok(Value::Struct(map.into_iter().collect()))
            }
            NodeKind::FieldExtraction { .. } => {
                Err(UzonError::runtime("'of' can only be used directly after 'is' in a binding", node.span.line, node.span.col))
            }
            NodeKind::DefaultForType { type_expr } => {
                self.eval_default_for_type(type_expr, scope, node)
            }
        }
    }

    // === Or-else and if-then-else ===

    /// §5.7: `or else` — returns left if defined, else right. Both operands must be the same type.
    fn eval_or_else(
        &mut self,
        left: &Node,
        right: &Node,
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        // §4.5: literal 'undefined' is restricted to `is`/`is not` operands.
        if matches!(left.kind, NodeKind::UndefinedLiteral) {
            return Err(UzonError::type_error(
                "literal 'undefined' is not allowed as 'or else' left operand; \
                 use an expression that may produce undefined (e.g., `env.MISSING`)",
                left.span.line, left.span.col,
            ));
        }
        if matches!(right.kind, NodeKind::UndefinedLiteral) {
            return Err(UzonError::type_error(
                "literal 'undefined' is not allowed as 'or else' right operand; \
                 use an expression or omit the fallback",
                right.span.line, right.span.col,
            ));
        }
        let lv = self.eval_node(left, scope, exclude)?;
        if lv.is_undefined() {
            // §5.7: the static type guarantee — even when the left operand is
            // undefined at runtime, the static types of the two operands MUST
            // match. If the left node carries a static type (via `to T` or
            // `as T`), check the right against it.
            let rv = self.eval_node(right, scope, exclude)?;
            if let Some(static_type) = static_type_name(left) {
                if !matches!(rv, Value::Null) && !rv.is_undefined()
                    && !static_type_matches(&static_type, &rv)
                {
                    return Err(UzonError::type_error(
                        format!(
                            "'or else' operands must be the same type, expected {static_type}, got {}",
                            rv.type_name()
                        ),
                        node.span.line, node.span.col,
                    ));
                }
            }
            return Ok(rv);
        }
        // §5.7: both operands MUST be the same type.
        // §3.5 rule 4: enum type-context inference for or else right operand.
        let rv_result = if let Value::Enum(ref e) = lv {
            self.resolve_enum_context(right, e, scope, exclude)
        } else {
            self.eval_node(right, scope, exclude)
        };
        // §D.5: speculatively evaluate right operand — suppress RuntimeError, propagate TypeError.
        match rv_result {
            Ok(rv) => {
                if !matches!(lv, Value::Null) && !matches!(rv, Value::Null)
                    && !rv.is_undefined()
                {
                    if lv.type_name() != rv.type_name()
                        && !super::can_adopt_cross_category(&lv, &rv)
                    {
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
            Err(e) if e.is_runtime() => {} // §D.5: suppress RuntimeError
            Err(e) => return Err(e),        // §D.5: propagate TypeError
        }
        Ok(lv)
    }

    /// §5.9: if-then-else expression with speculative type checking of both branches.
    fn eval_if_expr(
        &mut self,
        condition: &Node,
        then_branch: &Node,
        else_branch: &Node,
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        // §4.5: literal 'undefined' is restricted to `is`/`is not` operands.
        if matches!(then_branch.kind, NodeKind::UndefinedLiteral) {
            return Err(UzonError::type_error(
                "literal 'undefined' is not allowed as 'then' branch; \
                 use an expression that may produce undefined",
                then_branch.span.line, then_branch.span.col,
            ));
        }
        if matches!(else_branch.kind, NodeKind::UndefinedLiteral) {
            return Err(UzonError::type_error(
                "literal 'undefined' is not allowed as 'else' branch; \
                 use an expression that may produce undefined",
                else_branch.span.line, else_branch.span.col,
            ));
        }
        let cond = self.eval_node(condition, scope, exclude)?;
        let cond = Self::unwrap_union_owned(cond);
        match cond {
            Value::Bool(true) => {
                let then_val = self.eval_node(then_branch, scope, exclude)?;
                // §5.9/§D.5: speculative type check of else branch
                let else_result = if let Value::Enum(ref e) = then_val {
                    self.resolve_enum_context(else_branch, e, scope, exclude)
                } else {
                    self.eval_node(else_branch, scope, exclude)
                };
                match else_result {
                    Ok(else_val) => Self::check_branch_types(&then_val, &else_val, node)?,
                    Err(e) if e.is_runtime() => {} // §D.5: suppress RuntimeError
                    Err(e) => return Err(e),        // §D.5: propagate TypeError
                }
                Ok(then_val)
            }
            Value::Bool(false) => {
                let else_val = self.eval_node(else_branch, scope, exclude)?;
                // §5.9/§D.5: speculative type check of then branch
                let then_result = if let Value::Enum(ref e) = else_val {
                    self.resolve_enum_context(then_branch, e, scope, exclude)
                } else {
                    self.eval_node(then_branch, scope, exclude)
                };
                match then_result {
                    Ok(then_val) => Self::check_branch_types(&then_val, &else_val, node)?,
                    Err(e) if e.is_runtime() => {} // §D.5: suppress RuntimeError
                    Err(e) => return Err(e),        // §D.5: propagate TypeError
                }
                Ok(else_val)
            }
            _ => Err(UzonError::type_error(
                format!("if condition must be bool, got {}", cond.type_name()),
                node.span.line, node.span.col,
            )),
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

    /// Compute the default value of a type expression per the §3.6 table.
    ///
    /// Used by v0.9 standalone `union`/`tagged union` declarations where the
    /// binding's value is implicitly the default of the first member type.
    pub(crate) fn eval_default_for_type(
        &mut self,
        type_expr: &TypeExpr,
        scope: &Scope,
        node: &Node,
    ) -> Result<Value> {
        // null type → null
        if type_expr.is_null {
            return Ok(Value::Null);
        }

        // List type [T] → empty list with element type
        if type_expr.is_list {
            let element_type = type_expr.inner.as_ref()
                .and_then(|inner| inner.path.last().cloned())
                .unwrap_or_default();
            return Ok(Value::List(UzonList::with_type(Vec::new(), element_type)));
        }

        // Tuple type (T1, T2, ...) → tuple of defaults
        if let Some(ref types) = type_expr.tuple_types {
            let mut elements = Vec::with_capacity(types.len());
            for t in types {
                elements.push(self.eval_default_for_type(t, scope, node)?);
            }
            return Ok(Value::Tuple(UzonTuple::new(elements)));
        }

        let type_name = type_expr.path.last().cloned().unwrap_or_default();

        // Primitive types
        match type_name.as_str() {
            "bool" => return Ok(Value::Bool(false)),
            "string" => return Ok(Value::String(String::new())),
            _ => {}
        }
        if let Some(int_ty) = IntegerType::from_type_name(&type_name) {
            return Ok(Value::Integer(UzonInteger::with_type(0, int_ty)));
        }
        if let Some(float_ty) = FloatType::from_type_name(&type_name) {
            return Ok(Value::Float(UzonFloat::with_type(0.0, float_ty)));
        }

        // Named types via scope lookup
        if let Some(td) = scope.resolve_type_path(&type_expr.path) {
            match td.kind {
                crate::scope::TypeDefKind::Enum { variants } => {
                    let first = variants.first().cloned().ok_or_else(|| {
                        UzonError::type_error(
                            format!("named enum '{}' has no variants", td.name),
                            node.span.line, node.span.col,
                        )
                    })?;
                    return Ok(Value::Enum(UzonEnum::new(
                        first, variants, Some(td.name),
                    )));
                }
                crate::scope::TypeDefKind::Function { .. } => {
                    return Err(UzonError::type_error(
                        format!("'function' type has no default value; use inline declaration with an explicit value"),
                        node.span.line, node.span.col,
                    ));
                }
                crate::scope::TypeDefKind::TaggedUnion { variants } => {
                    // §3.7 v0.10: the default of a tagged union is the default of
                    // its first variant's inner type, wrapped in the variant tag.
                    // "First" here is the iteration order of the variant map —
                    // deterministic and matches declaration for most cases.
                    let (first_name, first_inner) = variants.iter().next().ok_or_else(|| {
                        UzonError::type_error(
                            format!("tagged union '{}' has no variants", td.name),
                            node.span.line, node.span.col,
                        )
                    })?;
                    let inner_val = match first_inner {
                        Some(t) if t == "null" => Value::Null,
                        Some(t) => {
                            let synthetic = TypeExpr {
                                path: vec![t.clone()],
                                is_list: false,
                                inner: None,
                                is_null: false,
                                tuple_types: None,
                                span: type_expr.span,
                            };
                            self.eval_default_for_type(&synthetic, scope, node)?
                        }
                        None => Value::Null,
                    };
                    return Ok(Value::TaggedUnion(UzonTaggedUnion::new(
                        inner_val,
                        first_name.clone(),
                        variants.clone(),
                        Some(td.name),
                    )));
                }
                crate::scope::TypeDefKind::Struct { fields } => {
                    // §3.2 v0.10: each field carries a tracked default value.
                    // Build the struct by pulling the stored defaults in
                    // declaration order and stamp the named type.
                    let mut result = indexmap::IndexMap::with_capacity(fields.len());
                    for (k, info) in fields {
                        result.insert(k.clone(), info.default_value.clone());
                    }
                    return Ok(Value::Struct(
                        UzonStruct::with_type_name(result, td.name),
                    ));
                }
                crate::scope::TypeDefKind::Union { .. } => {
                    // §3.6: union defaults would require the original AST of
                    // the declaring binding. Reject as not computable.
                    return Err(UzonError::type_error(
                        format!("cannot compute default value for named type '{}' in this context; use inline declaration with an explicit value", td.name),
                        node.span.line, node.span.col,
                    ));
                }
            }
        }

        Err(UzonError::type_error(
            format!("unknown type '{type_name}' — no default value"),
            node.span.line, node.span.col,
        ))
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
                    // §5.11.2: compound types and functions cannot be converted to string
                    match &val {
                        Value::Struct(_) | Value::List(_) | Value::Tuple(_) | Value::Function(_) => {
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

/// If a node carries a trailing `as T` or `to T` annotation, return T.
/// Used by `or else` to enforce the static type guarantee (§5.7) even when
/// the left operand is undefined at runtime.
fn static_type_name(node: &Node) -> Option<String> {
    match &node.kind {
        NodeKind::TypeAnnotation { type_expr, .. }
        | NodeKind::Conversion { type_expr, .. } => type_expr.path.last().cloned(),
        NodeKind::Grouping { expr } => static_type_name(expr),
        _ => None,
    }
}

/// Does a runtime value match the given static type name?
fn static_type_matches(type_name: &str, val: &Value) -> bool {
    use crate::value::{IntegerType, FloatType};
    if type_name == "bool" { return matches!(val, Value::Bool(_)); }
    if type_name == "string" { return matches!(val, Value::String(_)); }
    if IntegerType::from_type_name(type_name).is_some() {
        return matches!(val, Value::Integer(_) | Value::BigInteger(_));
    }
    if FloatType::from_type_name(type_name).is_some() {
        return matches!(val, Value::Float(_));
    }
    // For named / structural types fall back to a conservative "yes" — we do
    // not have enough information here to reject cleanly.
    true
}
