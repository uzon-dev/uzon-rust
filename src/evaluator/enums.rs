// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::scope::{Scope, TypeDefKind};
use crate::value::*;

impl Evaluator {
    /// Format a TypeExpr as a string for storing in tagged union variant maps.
    pub(crate) fn format_type_expr(te: &TypeExpr) -> String {
        if te.is_null {
            return "null".to_string();
        }
        if let Some(ref tuple_types) = te.tuple_types {
            let inner: Vec<String> = tuple_types.iter().map(Self::format_type_expr).collect();
            return format!("({})", inner.join(", "));
        }
        if te.is_list {
            if let Some(ref inner) = te.inner {
                return format!("[{}]", Self::format_type_expr(inner));
            }
            return "[]".to_string();
        }
        te.path.join(".")
    }
}

use super::Evaluator;

impl Evaluator {
    pub(crate) fn eval_from_enum(
        &mut self,
        value: &Node,
        variants: &[String],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        // §3.5: enum MUST have at least two variants
        if variants.len() < 2 {
            return Err(UzonError::type_error(
                "enum must have at least two variants",
                node.span.line, node.span.col,
            ));
        }

        // §3.5: duplicate variant names are a type error
        {
            let mut seen = std::collections::HashSet::new();
            for v in variants {
                if !seen.insert(v.as_str()) {
                    return Err(UzonError::type_error(
                        format!("duplicate enum variant '{v}'"),
                        node.span.line, node.span.col,
                    ));
                }
            }
        }

        let variant_name = match &value.kind {
            NodeKind::Identifier { name } => name.clone(),
            _ => {
                let val = self.eval_node(value, scope, exclude)?;
                return Err(UzonError::type_error(
                    format!("enum value must be a variant name, got {}", val.type_name()),
                    node.span.line, node.span.col,
                ));
            }
        };

        if !variants.contains(&variant_name) {
            return Err(UzonError::type_error(
                format!("'{variant_name}' is not in the variant set"),
                node.span.line, node.span.col,
            ));
        }

        Ok(Value::Enum(UzonEnum::new(variant_name, variants.to_vec(), None)))
    }

    pub(crate) fn eval_from_union(
        &mut self,
        value: &Node,
        types: &[TypeExpr],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        // §3.6: union MUST have at least two member types
        if types.len() < 2 {
            return Err(UzonError::type_error(
                "union must have at least two member types",
                node.span.line, node.span.col,
            ));
        }

        // §3.6: duplicate member types are a type error
        {
            let mut seen = std::collections::HashSet::new();
            for t in types {
                let name = Self::format_type_expr(t);
                if !seen.insert(name.clone()) {
                    return Err(UzonError::type_error(
                        format!("duplicate union member type '{name}'"),
                        node.span.line, node.span.col,
                    ));
                }
            }
        }

        let mut val = self.eval_node(value, scope, exclude)?;
        let type_names: Vec<String> = types.iter().map(|t| Self::format_type_expr(t)).collect();

        // Adopt matching member type for untyped numeric values so that
        // `42 from union i32, string` stores the inner value as i32.
        Self::adopt_union_member_type(&mut val, &type_names);

        Ok(Value::Union(UzonUnion::new(val, type_names, None)))
    }

    /// For untyped numeric values, adopt the matching union member type.
    /// Also handles compound types: lists adopt element types from `[Type]` members.
    fn adopt_union_member_type(val: &mut Value, type_names: &[String]) {
        match val {
            Value::Integer(n) if !n.explicit => {
                for tn in type_names {
                    if let Some(it) = IntegerType::from_type_name(tn) {
                        n.type_ann = it;
                        n.explicit = true;
                        return;
                    }
                }
            }
            Value::Float(f) if !f.explicit => {
                for tn in type_names {
                    if let Some(ft) = FloatType::from_type_name(tn) {
                        f.type_ann = ft;
                        f.explicit = true;
                        return;
                    }
                }
            }
            Value::List(list) if list.element_type.is_none() => {
                // Find a matching [Type] member and adopt element type
                for tn in type_names {
                    if tn.starts_with('[') && tn.ends_with(']') {
                        let inner = &tn[1..tn.len()-1];
                        // Check if elements are compatible with this inner type
                        let compatible = list.elements.iter().all(|e| {
                            if e.is_null() { return true; }
                            match e {
                                Value::Integer(n) if !n.explicit => {
                                    IntegerType::from_type_name(inner).is_some()
                                }
                                Value::Float(f) if !f.explicit => {
                                    FloatType::from_type_name(inner).is_some()
                                }
                                Value::Integer(n) => n.type_ann.display_name() == inner,
                                Value::Float(f) => f.type_ann.display_name() == inner,
                                other => other.type_name() == inner,
                            }
                        });
                        if compatible {
                            list.element_type = Some(inner.to_string());
                            // Adopt element types too
                            for elem in &mut list.elements {
                                Self::adopt_union_member_type(elem, &[inner.to_string()]);
                            }
                            return;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn eval_named_variant(
        &mut self,
        value: &Node,
        tag: &str,
        variants: &[(String, TypeExpr)],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        // §3.7: tagged union MUST have at least two variants (when defining inline)
        if !variants.is_empty() && variants.len() < 2 {
            return Err(UzonError::type_error(
                "tagged union must have at least two variants",
                node.span.line, node.span.col,
            ));
        }

        // §6.3: `value as TaggedUnionType named variant` or
        // `value named variant as TaggedUnionType` — type reuse
        if variants.is_empty() {
            if let NodeKind::TypeAnnotation { expr, type_expr } = &value.kind {
                let tu_info = scope.resolve_type_path(&type_expr.path)
                    .and_then(|td| {
                        if let TypeDefKind::TaggedUnion { variants: tv } = &td.kind {
                            Some((tv.clone(), td.name.clone()))
                        } else {
                            None
                        }
                    });
                if let Some((tv, type_name)) = tu_info {
                    if !tv.contains_key(tag) {
                        return Err(UzonError::type_error(
                            format!("'{}' is not a variant of tagged union '{}'", tag, type_name),
                            node.span.line, node.span.col,
                        ));
                    }
                    let mut val = self.eval_node(expr, scope, exclude)?;
                    // Adopt the variant's declared type for untyped numeric values.
                    if let Some(Some(variant_type)) = tv.get(tag) {
                        Self::adopt_variant_type(&mut val, variant_type);
                    }
                    return Ok(Value::TaggedUnion(UzonTaggedUnion::new(
                        val, tag, tv, Some(type_name),
                    )));
                }
            }
        }

        // §3.7: duplicate variant names are a type error
        {
            let mut seen = std::collections::HashSet::new();
            for (name, _) in variants {
                if !seen.insert(name.as_str()) {
                    return Err(UzonError::type_error(
                        format!("duplicate tagged union variant '{name}'"),
                        node.span.line, node.span.col,
                    ));
                }
            }
        }

        let mut val = self.eval_node(value, scope, exclude)?;
        let variant_map: BTreeMap<String, Option<String>> = variants
            .iter()
            .map(|(name, te)| {
                let type_name = Some(Self::format_type_expr(te));
                (name.clone(), type_name)
            })
            .collect();

        // Adopt the variant's declared type for untyped numeric values.
        if let Some(Some(variant_type)) = variant_map.get(tag) {
            Self::adopt_variant_type(&mut val, variant_type);
        }

        Ok(Value::TaggedUnion(UzonTaggedUnion::new(val, tag, variant_map, None)))
    }

    /// Adopt a variant's declared type for untyped numeric values.
    fn adopt_variant_type(val: &mut Value, variant_type: &str) {
        match val {
            Value::Integer(n) if !n.explicit => {
                if let Some(it) = IntegerType::from_type_name(variant_type) {
                    n.type_ann = it;
                    n.explicit = true;
                }
            }
            Value::Float(f) if !f.explicit => {
                if let Some(ft) = FloatType::from_type_name(variant_type) {
                    f.type_ann = ft;
                    f.explicit = true;
                }
            }
            _ => {}
        }
    }
}
