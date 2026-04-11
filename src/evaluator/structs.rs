// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use indexmap::IndexMap;

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::scope::Scope;
use crate::value::*;

use super::Evaluator;

impl Evaluator {
    pub(crate) fn eval_struct_literal(
        &mut self,
        fields: &[Binding],
        parent_scope: &mut Scope,
    ) -> Result<Value> {
        let mut child_scope = Scope::with_parent(parent_scope.clone());
        self.eval_bindings(fields, &mut child_scope)?;

        // Propagate type definitions from child to parent
        for (name, td) in child_scope.local_types() {
            if parent_scope.get_type(&name).is_none() {
                parent_scope.define_type(name, td);
            }
        }

        let scope_map = child_scope.to_map();
        let mut result = IndexMap::with_capacity(fields.len());
        // Preserve declaration order
        for field in fields {
            if let Some(val) = scope_map.get(&field.name) {
                result.insert(field.name.clone(), val.clone());
            }
        }

        Ok(Value::Struct(result))
    }

    pub(crate) fn eval_list_literal(
        &mut self,
        elements: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        let mut vals = Vec::with_capacity(elements.len());
        for elem in elements {
            vals.push(self.eval_node(elem, scope, exclude)?);
        }

        // Homogeneity check (§3.4)
        if vals.len() > 1 {
            let rep_type = vals.iter().find(|v| !v.is_null()).map(|v| v.type_name());
            if let Some(expected_type) = rep_type {
                for (i, v) in vals.iter().enumerate() {
                    if !v.is_null() && v.type_name() != expected_type {
                        return Err(UzonError::type_error(
                            format!(
                                "list elements must be the same type; expected {}, element {i} is {}",
                                expected_type, v.type_name()
                            ),
                            node.span.line, node.span.col,
                        ));
                    }
                }
            }
        }

        Ok(Value::list(vals))
    }

    /// §3.2.1: type compatibility check for struct override/extension field values.
    fn check_override_type_compat(
        old_val: &Value, new_val: &Value,
        field_name: &str, field_value_kind: &NodeKind,
        op: &str, check_struct_shape: bool,
        line: usize, col: usize,
    ) -> Result<()> {
        if old_val.is_null() || new_val.is_null() {
            return Ok(());
        }

        if old_val.type_name() != new_val.type_name() {
            return Err(UzonError::type_error(
                format!("type mismatch in '{}' override for '{}': base is {}, override is {}",
                    op, field_name, old_val.type_name(), new_val.type_name()),
                line, col,
            ));
        }

        let override_has_explicit_type = matches!(field_value_kind, NodeKind::TypeAnnotation { .. });

        match (old_val, new_val) {
            (Value::Integer(old_n), Value::Integer(new_n)) => {
                if override_has_explicit_type
                    && !old_n.type_ann.is_default()
                    && old_n.type_ann != new_n.type_ann
                {
                    return Err(UzonError::type_error(
                        format!("type mismatch in '{}' override for '{}': base is {}, override is {}",
                            op, field_name, old_n.type_ann.display_name(), new_n.type_ann.display_name()),
                        line, col,
                    ));
                }
            }
            (Value::Float(old_f), Value::Float(new_f)) => {
                if override_has_explicit_type
                    && !old_f.type_ann.is_default()
                    && old_f.type_ann != new_f.type_ann
                {
                    return Err(UzonError::type_error(
                        format!("type mismatch in '{}' override for '{}': base is {}, override is {}",
                            op, field_name, old_f.type_ann.display_name(), new_f.type_ann.display_name()),
                        line, col,
                    ));
                }
            }
            (Value::Enum(old_e), Value::Enum(new_e)) => {
                match (&old_e.type_name, &new_e.type_name) {
                    (Some(a), Some(b)) if a != b => {
                        return Err(UzonError::type_error(
                            format!("type mismatch in '{}' override for '{}': base is {}, override is {}",
                                op, field_name, a, b),
                            line, col,
                        ));
                    }
                    (Some(a), None) | (None, Some(a)) => {
                        return Err(UzonError::type_error(
                            format!("type mismatch in '{}' override for '{}': named enum {} is not compatible with unnamed enum",
                                op, field_name, a),
                            line, col,
                        ));
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        if check_struct_shape {
            if let (Value::Struct(old_fields), Value::Struct(new_fields)) = (old_val, new_val) {
                if old_fields.keys().collect::<Vec<_>>() != new_fields.keys().collect::<Vec<_>>() {
                    return Err(UzonError::type_error(
                        format!("struct shape mismatch in '{}' override for '{}': base has fields {:?}, override has fields {:?}",
                            op, field_name,
                            old_fields.keys().collect::<Vec<_>>(),
                            new_fields.keys().collect::<Vec<_>>()),
                        line, col,
                    ));
                }
                for (k, old_v) in old_fields {
                    let new_v = &new_fields[k];
                    if !old_v.is_null() && !new_v.is_null() && old_v.type_name() != new_v.type_name() {
                        return Err(UzonError::type_error(
                            format!("type mismatch in '{}' override for '{}.{}': base is {}, override is {}",
                                op, field_name, k, old_v.type_name(), new_v.type_name()),
                            line, col,
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// §3.2.1 Rule 2: adopt type_ann from base field onto untyped override value.
    fn adopt_type_from_base(
        old_val: &Value, new_val: &mut Value,
        field_name: &str, op: &str,
        line: usize, col: usize,
    ) -> Result<()> {
        match (old_val, new_val) {
            (Value::Integer(old_n), Value::Integer(new_n)) => {
                if !old_n.type_ann.is_default() && new_n.type_ann.is_default() {
                    new_n.type_ann = old_n.type_ann;
                    new_n.validate_range().map_err(|msg| {
                        UzonError::type_error(
                            format!("'{}' override for '{}': {}", op, field_name, msg),
                            line, col,
                        )
                    })?;
                }
            }
            (Value::Float(old_f), Value::Float(new_f)) => {
                if !old_f.type_ann.is_default() && new_f.type_ann.is_default() {
                    new_f.type_ann = old_f.type_ann;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(crate) fn eval_struct_override(
        &mut self,
        base: &Node, overrides: &Node,
        scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        let base_val = self.eval_node(base, scope, exclude)?;
        if base_val.is_undefined() {
            return Err(UzonError::runtime("'with' requires a struct base, got undefined", node.span.line, node.span.col));
        }
        let mut base_map = match base_val {
            Value::Struct(m) => m,
            _ => return Err(UzonError::type_error(
                format!("'with' requires a struct base, got {}", base_val.type_name()),
                node.span.line, node.span.col,
            )),
        };

        let override_fields = match &overrides.kind {
            NodeKind::StructLiteral { fields } => fields,
            _ => return Err(UzonError::syntax("'with' requires a struct literal", node.span.line, node.span.col)),
        };

        for field in override_fields {
            if !base_map.contains_key(&field.name) {
                return Err(UzonError::type_error(
                    format!("field '{}' does not exist in base struct", field.name),
                    field.span.line, field.span.col,
                ));
            }

            let new_val = self.eval_node(&field.value, scope, exclude)?;
            if new_val.is_undefined() {
                return Err(UzonError::type_error(
                    format!("cannot override field '{}' with undefined", field.name),
                    field.span.line, field.span.col,
                ));
            }

            let old_val = &base_map[&field.name];
            Self::check_override_type_compat(old_val, &new_val, &field.name, &field.value.kind, "with", true, field.span.line, field.span.col)?;

            let mut new_val = new_val;
            let old_val = &base_map[&field.name];
            Self::adopt_type_from_base(old_val, &mut new_val, &field.name, "with", field.span.line, field.span.col)?;
            base_map.insert(field.name.clone(), new_val);
        }

        Ok(Value::Struct(base_map))
    }

    pub(crate) fn eval_struct_extension(
        &mut self,
        base: &Node, extension: &Node,
        scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        let base_val = self.eval_node(base, scope, exclude)?;
        if base_val.is_undefined() {
            return Err(UzonError::runtime("'extends' requires a struct base, got undefined", node.span.line, node.span.col));
        }
        let base_map = match base_val {
            Value::Struct(m) => m,
            _ => return Err(UzonError::type_error(
                format!("'extends' requires a struct base, got {}", base_val.type_name()),
                node.span.line, node.span.col,
            )),
        };

        let ext_fields = match &extension.kind {
            NodeKind::StructLiteral { fields } => fields,
            _ => return Err(UzonError::syntax("'extends' requires a struct literal", node.span.line, node.span.col)),
        };

        let has_new_field = ext_fields.iter().any(|f| !base_map.contains_key(&f.name));
        if !has_new_field {
            return Err(UzonError::type_error(
                "'extends' must add at least one new field; use 'with' for pure overrides",
                node.span.line, node.span.col,
            ));
        }

        let mut result = base_map.clone();

        for field in ext_fields {
            let new_val = self.eval_node(&field.value, scope, exclude)?;

            if base_map.contains_key(&field.name) {
                let old_val = &base_map[&field.name];
                if new_val.is_undefined() {
                    return Err(UzonError::type_error(
                        format!("cannot override field '{}' with undefined", field.name),
                        field.span.line, field.span.col,
                    ));
                }
                Self::check_override_type_compat(old_val, &new_val, &field.name, &field.value.kind, "extends", false, field.span.line, field.span.col)?;
                let mut new_val = new_val;
                Self::adopt_type_from_base(&base_map[&field.name], &mut new_val, &field.name, "extends", field.span.line, field.span.col)?;
                result.insert(field.name.clone(), new_val);
            } else {
                result.insert(field.name.clone(), new_val);
            }
        }

        Ok(Value::Struct(result))
    }
}
