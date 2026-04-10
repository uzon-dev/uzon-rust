// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::scope::{Scope, TypeDefKind};
use crate::value::*;

use super::Evaluator;

impl Evaluator {
    pub(crate) fn eval_type_annotation(
        &mut self,
        expr: &Node,
        type_expr: &TypeExpr,
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        // §6.3: `as TaggedUnionType` without `named` is a type error.
        // NamedVariant expressions already carry a tag, so they are exempt.
        if !matches!(expr.kind, NodeKind::NamedVariant { .. }) {
            if let Some(typedef) = scope.resolve_type_path(&type_expr.path) {
                if let TypeDefKind::TaggedUnion { .. } = &typedef.kind {
                    return Err(UzonError::type_error(
                        format!("'as {}' requires 'named' for tagged union types; use 'value named tag as {}'",
                            typedef.name, typedef.name),
                        node.span.line, node.span.col,
                    ));
                }
            }
        }

        // §3.5 rule 4: enum variant resolution — `red as RGB`
        if let NodeKind::Identifier { name } = &expr.kind {
            if let Some(typedef) = scope.resolve_type_path(&type_expr.path) {
                if let TypeDefKind::Enum { variants } = &typedef.kind {
                    if variants.contains(name) {
                        return Ok(Value::Enum(UzonEnum::new(
                            name.clone(),
                            variants.clone(),
                            Some(typedef.name.clone()),
                        )));
                    } else {
                        return Err(UzonError::type_error(
                            format!("'{name}' is not a variant of {}; valid variants: {}",
                                typedef.name, variants.join(", ")),
                            node.span.line, node.span.col,
                        ));
                    }
                }
            }
        }

        // §4.2: suppress default i64 range check for integer literals inside `as`
        let prev_in_ta = self.in_type_annotation;
        self.in_type_annotation = true;
        let val = self.eval_node(expr, scope, exclude);
        self.in_type_annotation = prev_in_ta;
        let mut val = val?;

        if val.is_undefined() {
            // §6.1: undefined propagates through `as`, but the type name MUST still be validated.
            if let Some(type_name) = type_expr.path.last() {
                self.validate_type_exists(type_name, type_expr, scope, node)?;
            }
            return Ok(Value::Undefined);
        }

        // §6.1: list type annotation `as [Type]`
        // §3.5 rule 4: enum type-context inference via `as [EnumType]`
        if type_expr.is_list {
            return self.eval_type_annotation_list(&mut val, expr, type_expr, scope, exclude, node);
        }

        // §6.3: named struct type conformance checking
        if let Some(typedef) = scope.resolve_type_path(&type_expr.path) {
            if let TypeDefKind::Struct { .. } = typedef.kind {
                return self.eval_type_annotation_struct(val, type_expr, scope, node);
            }
        }

        // §3.8: named function type conformance checking
        if let Some(typedef) = scope.resolve_type_path(&type_expr.path) {
            if let TypeDefKind::Function { .. } = typedef.kind {
                return self.eval_type_annotation_function(val, type_expr, scope, node);
            }
        }

        // Basic range checking for numeric types
        if let Some(type_name) = type_expr.path.last() {
            self.check_type_assertion(&val, type_name, node)?;
        }

        // Set type annotation on numeric values after validation
        if let Some(type_name) = type_expr.path.last() {
            if let Some(int_type) = IntegerType::from_type_name(type_name) {
                if let Value::Integer(ref mut n) = val {
                    n.type_ann = int_type;
                    n.explicit = true;
                }
            } else if let Some(float_type) = FloatType::from_type_name(type_name) {
                if let Value::Float(ref mut f) = val {
                    f.type_ann = float_type;
                    f.explicit = true;
                }
            }
        }

        Ok(val)
    }

    /// Handles `as [Type]` list type annotation.
    /// Includes both enum list resolution and general list type annotation.
    fn eval_type_annotation_list(
        &mut self,
        val: &mut Value,
        expr: &Node,
        type_expr: &TypeExpr,
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        if let Some(ref inner) = type_expr.inner {
            // Check if inner type is a named enum — resolve list elements as variants
            let enum_info = scope.resolve_type_path(&inner.path).and_then(|td| {
                if let TypeDefKind::Enum { variants } = td.kind {
                    Some((td.name, variants))
                } else { None }
            });
            if let Some((enum_name, variants)) = enum_info {
                // Re-evaluate list elements from AST with enum context
                if let NodeKind::ListLiteral { elements } = &expr.kind {
                    let mut resolved = Vec::with_capacity(elements.len());
                    for elem in elements {
                        let v = if let NodeKind::Identifier { ref name } = elem.kind {
                            if variants.contains(name) {
                                Value::Enum(UzonEnum::new(
                                    name.clone(), variants.clone(), Some(enum_name.clone()),
                                ))
                            } else {
                                self.eval_node(elem, scope, exclude)?
                            }
                        } else {
                            self.eval_node(elem, scope, exclude)?
                        };
                        if !v.is_null() {
                            if let Some(inner_type_name) = inner.path.last() {
                                self.check_type_assertion(&v, inner_type_name, node)?;
                            }
                        }
                        resolved.push(v);
                    }
                    return Ok(Value::List(resolved));
                }
            }
        }
        // Non-enum list type annotation — validate already-evaluated elements
        if let Value::List(items) = val {
            if let Some(ref inner) = type_expr.inner {
                if let Some(inner_type_name) = inner.path.last() {
                    let int_type = IntegerType::from_type_name(inner_type_name);
                    let float_type = FloatType::from_type_name(inner_type_name);
                    let named_func_type = scope.resolve_type_path(&inner.path).and_then(|td| {
                        if let TypeDefKind::Function { .. } = &td.kind { Some(td) } else { None }
                    });
                    let named_struct_type = scope.resolve_type_path(&inner.path).and_then(|td| {
                        if let TypeDefKind::Struct { .. } = &td.kind { Some(td) } else { None }
                    });
                    for item in items.iter_mut() {
                        if !item.is_null() {
                            self.check_type_assertion(item, inner_type_name, node)?;
                            // §6.3: named function type — nominal check
                            if let Some(ref func_td) = named_func_type {
                                if let Value::Function(f) = &*item {
                                    if f.type_name.as_deref() != Some(&func_td.name) {
                                        let got = f.type_name.as_deref().unwrap_or("anonymous");
                                        return Err(UzonError::type_error(
                                            format!("function type mismatch: expected {}, got {got}",
                                                func_td.name),
                                            node.span.line, node.span.col,
                                        ));
                                    }
                                } else {
                                    return Err(UzonError::type_error(
                                        format!("cannot annotate {} as function type {}", item.type_name(), func_td.name),
                                        node.span.line, node.span.col,
                                    ));
                                }
                            }
                            // §6.3: named struct type conformance in list
                            if let Some(ref struct_td) = named_struct_type {
                                if let TypeDefKind::Struct { ref fields } = struct_td.kind {
                                    if let Value::Struct(val_fields) = &*item {
                                        for key in val_fields.keys() {
                                            if !fields.contains_key(key) {
                                                return Err(UzonError::type_error(
                                                    format!("field '{key}' does not exist in type {}", struct_td.name),
                                                    node.span.line, node.span.col,
                                                ));
                                            }
                                        }
                                        for key in fields.keys() {
                                            if !val_fields.contains_key(key) {
                                                return Err(UzonError::type_error(
                                                    format!("missing field '{key}' required by type {}", struct_td.name),
                                                    node.span.line, node.span.col,
                                                ));
                                            }
                                        }
                                    } else {
                                        return Err(UzonError::type_error(
                                            format!("cannot annotate {} as struct type {}", item.type_name(), struct_td.name),
                                            node.span.line, node.span.col,
                                        ));
                                    }
                                }
                            }
                            // Set type_ann on each element after validation
                            if let Some(it) = int_type {
                                if let Value::Integer(n) = item {
                                    n.type_ann = it;
                                    n.explicit = true;
                                }
                            } else if let Some(ft) = float_type {
                                if let Value::Float(f) = item {
                                    f.type_ann = ft;
                                    f.explicit = true;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            return Err(UzonError::type_error(
                format!("cannot annotate {} as list type", val.type_name()),
                node.span.line, node.span.col,
            ));
        }
        Ok(val.clone())
    }

    /// Handles `as StructType` conformance checking (§6.3).
    fn eval_type_annotation_struct(
        &mut self,
        mut val: Value,
        type_expr: &TypeExpr,
        scope: &mut Scope,
        node: &Node,
    ) -> Result<Value> {
        let typedef = scope.resolve_type_path(&type_expr.path).unwrap();
        if let TypeDefKind::Struct { ref fields } = typedef.kind {
            let type_name_str = typedef.name.clone();
            if let Value::Struct(ref val_fields) = val {
                // Check no extra fields
                for key in val_fields.keys() {
                    if !fields.contains_key(key) {
                        return Err(UzonError::type_error(
                            format!("field '{key}' does not exist in type {type_name_str}"),
                            node.span.line, node.span.col,
                        ));
                    }
                }
                // Check no missing fields
                for key in fields.keys() {
                    if !val_fields.contains_key(key) {
                        return Err(UzonError::type_error(
                            format!("missing field '{key}' required by type {type_name_str}"),
                            node.span.line, node.span.col,
                        ));
                    }
                }
                // Check field type compatibility and annotations
                for (key, field_info) in fields {
                    let val_field = &val_fields[key];
                    if !val_field.is_null() {
                        if val_field.type_name() != field_info.type_category {
                            return Err(UzonError::type_error(
                                format!(
                                    "field '{key}' type mismatch: expected {}, got {}",
                                    field_info.type_category, val_field.type_name()
                                ),
                                node.span.line, node.span.col,
                            ));
                        }
                        if let Some(ref ann) = field_info.type_annotation {
                            self.check_type_assertion(val_field, ann, node)?;
                        }
                    }
                }
                // Propagate type annotations from type definition to struct fields
                if let Value::Struct(ref mut val_fields) = val {
                    for (key, field_info) in fields {
                        if let Some(ref ann) = field_info.type_annotation {
                            if let Some(val_field) = val_fields.get_mut(key) {
                                if let Some(int_type) = IntegerType::from_type_name(ann) {
                                    if let Value::Integer(n) = val_field {
                                        n.type_ann = int_type;
                                        n.explicit = true;
                                    }
                                } else if let Some(float_type) = FloatType::from_type_name(ann) {
                                    if let Value::Float(f) = val_field {
                                        f.type_ann = float_type;
                                        f.explicit = true;
                                    }
                                }
                            }
                        }
                    }
                }
                return Ok(val);
            } else {
                return Err(UzonError::type_error(
                    format!("cannot annotate {} as struct type {type_name_str}", val.type_name()),
                    node.span.line, node.span.col,
                ));
            }
        }
        Ok(val)
    }

    /// Handles `as FunctionType` conformance checking (§3.8).
    fn eval_type_annotation_function(
        &mut self,
        mut val: Value,
        type_expr: &TypeExpr,
        scope: &mut Scope,
        node: &Node,
    ) -> Result<Value> {
        let typedef = scope.resolve_type_path(&type_expr.path).unwrap();
        if let TypeDefKind::Function { ref param_types, ref return_type } = typedef.kind {
            let type_name_str = typedef.name.clone();
            if let Value::Function(ref f) = val {
                let val_param_types: Vec<String> = f.params.iter()
                    .map(|p| p.type_expr.path.last().cloned().unwrap_or_default())
                    .collect();
                if val_param_types.len() != param_types.len() {
                    return Err(UzonError::type_error(
                        format!("function has {} parameters, but type {type_name_str} requires {}",
                            val_param_types.len(), param_types.len()),
                        node.span.line, node.span.col,
                    ));
                }
                for (i, (expected, actual)) in param_types.iter().zip(val_param_types.iter()).enumerate() {
                    if expected != actual {
                        return Err(UzonError::type_error(
                            format!("parameter {} type mismatch: type {type_name_str} expects {expected}, got {actual}",
                                i + 1),
                            node.span.line, node.span.col,
                        ));
                    }
                }
                let val_return = f.return_type.path.last().cloned().unwrap_or_default();
                if &val_return != return_type {
                    return Err(UzonError::type_error(
                        format!("return type mismatch: type {type_name_str} expects {return_type}, got {val_return}"),
                        node.span.line, node.span.col,
                    ));
                }
                if let Value::Function(ref mut f) = val {
                    f.type_name = Some(type_name_str);
                }
                return Ok(val);
            } else {
                return Err(UzonError::type_error(
                    format!("cannot annotate {} as function type {type_name_str}", val.type_name()),
                    node.span.line, node.span.col,
                ));
            }
        }
        Ok(val)
    }

    pub(crate) fn check_type_assertion(&self, val: &Value, type_name: &str, node: &Node) -> Result<()> {
        // null is compatible with any type in annotation context
        if matches!(val, Value::Null) {
            return Ok(());
        }

        if let Some(int_type) = IntegerType::from_type_name(type_name) {
            match val {
                Value::Integer(v) => {
                    let check = UzonInteger::with_type(v.value, int_type);
                    check.validate_range().map_err(|msg| UzonError::type_error(
                        msg,
                        node.span.line, node.span.col,
                    ))?;
                }
                _ => {
                    return Err(UzonError::type_error(
                        format!("cannot annotate {} as {type_name}; use 'to' for conversion", val.type_name()),
                        node.span.line, node.span.col,
                    ));
                }
            }
        } else if let Some(_float_type) = FloatType::from_type_name(type_name) {
            if !matches!(val, Value::Float(_)) {
                return Err(UzonError::type_error(
                    format!("cannot annotate {} as {type_name}; use 'to' for conversion", val.type_name()),
                    node.span.line, node.span.col,
                ));
            }
        } else if type_name.starts_with('f') && type_name[1..].parse::<u32>().is_ok() {
            // Reject arbitrary fN like f7 — only f16/f32/f64/f80/f128 are valid
            return Err(UzonError::type_error(
                format!("invalid float type '{type_name}'; only f16, f32, f64, f80, f128 are supported"),
                node.span.line, node.span.col,
            ));
        } else if type_name == "string" {
            if !matches!(val, Value::String(_)) {
                return Err(UzonError::type_error(
                    format!("cannot annotate {} as string", val.type_name()),
                    node.span.line, node.span.col,
                ));
            }
        } else if type_name == "bool" {
            if !matches!(val, Value::Bool(_)) {
                return Err(UzonError::type_error(
                    format!("cannot annotate {} as bool", val.type_name()),
                    node.span.line, node.span.col,
                ));
            }
        }
        Ok(())
    }

    /// Coerce a value to a function parameter's declared type (§3.8).
    /// Applies the type annotation (e.g., i32, f64) so it carries the correct type
    /// within the function body.
    pub(crate) fn coerce_to_param_type(val: Value, type_name: &str) -> Value {
        match val {
            Value::Integer(mut i) => {
                if let Some(int_type) = IntegerType::from_type_name(type_name) {
                    i.type_ann = int_type;
                    i.explicit = true;
                }
                Value::Integer(i)
            }
            Value::Float(mut f) => {
                if let Some(float_type) = FloatType::from_type_name(type_name) {
                    f.type_ann = float_type;
                    f.explicit = true;
                }
                Value::Float(f)
            }
            other => other,
        }
    }

    // === Type conversion (`to`) ===

    pub(crate) fn eval_conversion(
        &mut self,
        expr: &Node,
        type_expr: &TypeExpr,
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        let val = self.eval_node(expr, scope, exclude)?;

        // §5.11.0: handle `to null` — null type has empty path, is_null flag
        let type_name = if type_expr.is_null {
            "null"
        } else {
            type_expr.path.last().map(|s| s.as_str()).unwrap_or("")
        };

        if val.is_undefined() {
            // §5.11 + §5.13: env refs are always string when defined.
            // Validate the conversion path even when the env var is missing.
            if Self::expr_is_env_ref(expr) {
                Self::check_string_conversion(type_name, node)?;
            }
            return Ok(Value::Undefined);
        }

        // §3.7.1 / §5.11.0: tagged union `to` only allows `to string`
        if let Value::TaggedUnion(ref tu) = val {
            if type_name == "string" {
                let inner = &tu.value;
                match inner.as_ref() {
                    Value::Struct(_) | Value::List(_) | Value::Tuple(_) => {
                        return Err(UzonError::type_error(
                            format!("cannot convert tagged union (inner type {}) to string", inner.type_name()),
                            node.span.line, node.span.col,
                        ));
                    }
                    _ => return Ok(Value::String(inner.to_string())),
                }
            } else {
                return Err(UzonError::type_error(
                    format!("tagged union can only be converted to string, not {type_name}"),
                    node.span.line, node.span.col,
                ));
            }
        }

        // §5.11.0: untagged union `to` only allows `to string`
        if let Value::Union(ref u) = val {
            if type_name == "string" {
                let inner = &u.value;
                match inner.as_ref() {
                    Value::Struct(_) | Value::List(_) | Value::Tuple(_) => {
                        return Err(UzonError::type_error(
                            format!("cannot convert union (inner type {}) to string", inner.type_name()),
                            node.span.line, node.span.col,
                        ));
                    }
                    _ => return Ok(Value::String(inner.to_string())),
                }
            } else {
                return Err(UzonError::type_error(
                    format!("untagged union can only be converted to string, not {type_name}"),
                    node.span.line, node.span.col,
                ));
            }
        }

        // String to enum conversion
        if let Some(typedef) = scope.resolve_type_path(&type_expr.path) {
            if let TypeDefKind::Enum { variants } = &typedef.kind {
                if let Value::String(s) = &val {
                    if variants.contains(s) {
                        return Ok(Value::Enum(UzonEnum::new(
                            s.clone(),
                            variants.clone(),
                            Some(typedef.name.clone()),
                        )));
                    } else {
                        return Err(UzonError::runtime(
                            format!("'{s}' is not a variant of {}", typedef.name),
                            node.span.line, node.span.col,
                        ));
                    }
                }
            }
        }

        self.convert_value(val, type_name, node)
    }

    pub(crate) fn convert_value(&self, val: Value, type_name: &str, node: &Node) -> Result<Value> {
        // §5.11.0: null conversions
        if val.is_null() {
            if type_name == "string" || type_name == "null" {
                // identity or to-string conversions are permitted
            } else {
                return Err(UzonError::runtime(
                    format!("cannot convert null to {type_name}"),
                    node.span.line, node.span.col,
                ));
            }
        }

        match type_name {
            "string" => {
                // §5.11.2: compound types cannot be converted to string
                match &val {
                    Value::Struct(_) | Value::List(_) | Value::Tuple(_) => {
                        Err(UzonError::type_error(
                            format!("cannot convert {} to string", val.type_name()),
                            node.span.line, node.span.col,
                        ))
                    }
                    _ => Ok(Value::String(val.to_string())),
                }
            }
            // §5.11.0: to bool — only bool → bool (identity) is permitted
            "bool" => match &val {
                Value::Bool(_) => Ok(val),
                _ => Err(UzonError::type_error(
                    format!("cannot convert {} to bool", val.type_name()),
                    node.span.line, node.span.col,
                )),
            },
            // §5.11.0: to null — only null → null (identity) is permitted
            "null" => Ok(val),
            _ if type_name.starts_with('i') || type_name.starts_with('u') || type_name.starts_with('f') => {
                self.convert_numeric(val, type_name, node)
            }
            _ => Err(UzonError::type_error(
                format!("cannot convert {} to {type_name}", val.type_name()),
                node.span.line, node.span.col,
            )),
        }
    }

    pub(crate) fn convert_numeric(&self, val: Value, type_name: &str, node: &Node) -> Result<Value> {
        // String → numeric
        if let Value::String(s) = &val {
            // §5.11.1: leading/trailing whitespace is rejected
            if s != s.trim() {
                return Err(UzonError::runtime(
                    format!("cannot convert string with leading/trailing whitespace to {type_name}"),
                    node.span.line, node.span.col,
                ));
            }
            // §5.11.1: empty string is rejected
            if s.is_empty() {
                return Err(UzonError::runtime(
                    format!("cannot convert empty string to {type_name}"),
                    node.span.line, node.span.col,
                ));
            }
            let s = s.replace('_', "");
            if type_name.starts_with('f') {
                // §5.11.1: only "inf", "-inf", "nan" are recognized; reject "infinity"
                let lower = s.to_lowercase();
                if lower == "infinity" || lower == "-infinity" {
                    return Err(UzonError::runtime(
                        format!("cannot convert '{s}' to {type_name}; only \"inf\" is recognized, not \"infinity\""),
                        node.span.line, node.span.col,
                    ));
                }
                let f: f64 = s.parse().map_err(|_| {
                    UzonError::runtime(format!("cannot convert '{s}' to {type_name}"), node.span.line, node.span.col)
                })?;
                let parsed_float_type = FloatType::from_type_name(type_name).unwrap_or_default();
                return Ok(Value::Float(UzonFloat::with_type(f, parsed_float_type)));
            } else {
                let n = self.eval_integer(&s, node)?;
                if let Value::Integer(ref i) = n {
                    self.check_type_assertion(&n, type_name, node)?;
                    let parsed_int_type = IntegerType::from_type_name(type_name).unwrap_or_default();
                    return Ok(Value::Integer(UzonInteger::with_type(i.value, parsed_int_type)));
                }
                return Ok(n);
            }
        }

        // Float → integer (truncation)
        if let Value::Float(f) = val {
            if type_name.starts_with('i') || type_name.starts_with('u') {
                // §5.11: NaN and infinity cannot be represented as integers
                if !f.value.is_finite() {
                    return Err(UzonError::runtime(
                        format!("cannot convert {} to {type_name}", if f.value.is_nan() { "nan" } else { "infinity" }),
                        node.span.line, node.span.col,
                    ));
                }
                let truncated = f.value as i128;
                let parsed_int_type = IntegerType::from_type_name(type_name).unwrap_or_default();
                let result = Value::Integer(UzonInteger::with_type(truncated, parsed_int_type.clone()));
                self.check_type_assertion(&result, type_name, node)?;
                return Ok(result);
            }
            // float → float
            let parsed_float_type = FloatType::from_type_name(type_name).unwrap_or_default();
            return Ok(Value::Float(UzonFloat::with_type(f.value, parsed_float_type)));
        }

        // Integer → float
        if let Value::Integer(i) = val {
            if type_name.starts_with('f') {
                let parsed_float_type = FloatType::from_type_name(type_name).unwrap_or_default();
                return Ok(Value::Float(UzonFloat::with_type(i.value as f64, parsed_float_type)));
            }
            let parsed_int_type = IntegerType::from_type_name(type_name).unwrap_or_default();
            self.check_type_assertion(&Value::Integer(UzonInteger::with_type(i.value, parsed_int_type.clone())), type_name, node)?;
            return Ok(Value::Integer(UzonInteger::with_type(i.value, parsed_int_type)));
        }

        Err(UzonError::type_error(
            format!("cannot convert {} to {type_name}", val.type_name()),
            node.span.line, node.span.col,
        ))
    }
}
