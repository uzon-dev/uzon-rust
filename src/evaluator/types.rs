// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use indexmap::IndexMap;

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::scope::{Scope, StructFieldInfo, TypeDef, TypeDefKind};
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
        // §3.5 rule 4 + §3.7 v0.10: a bare identifier matching a nullary
        // tagged-union variant resolves to `null named variant` when annotated
        // `as TaggedUnionType`. Binding still wins over variant.
        if let NodeKind::Identifier { name } = &expr.kind {
            if scope.get(name, None).is_none() {
                if let Some(typedef) = scope.resolve_type_path(&type_expr.path) {
                    if let TypeDefKind::TaggedUnion { variants } = &typedef.kind {
                        if let Some(Some(inner_type)) = variants.get(name) {
                            if inner_type == "null" {
                                return Ok(Value::TaggedUnion(UzonTaggedUnion::new(
                                    Value::Null,
                                    name.clone(),
                                    variants.clone(),
                                    Some(typedef.name.clone()),
                                )));
                            }
                        }
                    }
                }
            }
        }

        // §3.7 v0.10: `variant_name inner as TaggedUnionType` — resolve the
        // shorthand against the annotated tagged union type.
        if let NodeKind::VariantShorthand { variant_name, inner } = &expr.kind {
            if let Some(typedef) = scope.resolve_type_path(&type_expr.path) {
                if let TypeDefKind::TaggedUnion { variants } = &typedef.kind {
                    return self.resolve_variant_shorthand(
                        variant_name, inner, variants, &typedef.name, scope, exclude, node,
                    );
                }
            }
        }

        // §6.3 / §7.3: `as TaggedUnionType` on an **already-tagged** value of
        // the same type is a no-op (identity preservation). `named` is only
        // required when adopting a **non-tagged** value as a tagged union.
        // NamedVariant AST nodes already carry a tag, so they are exempt.
        if !matches!(expr.kind, NodeKind::NamedVariant { .. }) {
            if let Some(typedef) = scope.resolve_type_path(&type_expr.path) {
                if let TypeDefKind::TaggedUnion { .. } = &typedef.kind {
                    // Try evaluating the expression: if it's already a tagged
                    // union of this same type, treat `as T` as identity.
                    let prev_in_ta = self.in_type_annotation;
                    self.in_type_annotation = true;
                    let eval_result = self.eval_node(expr, scope, exclude);
                    self.in_type_annotation = prev_in_ta;
                    if let Ok(val) = eval_result {
                        if let Value::TaggedUnion(ref tu) = val {
                            if tu.type_name.as_deref() == Some(typedef.name.as_str()) {
                                return Ok(val);
                            }
                        }
                        // Propagate undefined through `as`.
                        if val.is_undefined() {
                            return Ok(Value::Undefined);
                        }
                    }
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

        // §3.4.1: `as Name` where Name is a named list type — rewrite to
        // `as [element_type]` and stamp `list.type_name = Name` on the result.
        if let Some(named_list) = scope.resolve_type_path(&type_expr.path).and_then(|td| {
            if let TypeDefKind::List { ref element_type } = td.kind {
                Some((td.name.clone(), element_type.clone()))
            } else {
                None
            }
        }) {
            let (list_type_name, element_type) = named_list;
            let synthetic = TypeExpr {
                path: Vec::new(),
                is_list: true,
                inner: element_type.as_ref().map(|et| Box::new(TypeExpr {
                    path: vec![et.clone()],
                    is_list: false,
                    inner: None,
                    is_null: false,
                    tuple_types: None,
                    span: type_expr.span,
                })),
                is_null: false,
                tuple_types: None,
                span: type_expr.span,
            };
            let prev_in_ta = self.in_type_annotation;
            self.in_type_annotation = true;
            let result = self.eval_type_annotation_list_from_ast(expr, &synthetic, scope, exclude, node);
            self.in_type_annotation = prev_in_ta;
            let mut val = result?;
            if let Value::List(ref mut list) = val {
                list.type_name = Some(list_type_name);
            }
            return Ok(val);
        }

        // §6.1: list type annotation `as [Type]`
        // §3.5 rule 4: enum type-context inference via `as [EnumType]`
        // Must run BEFORE eval_node so bare identifiers in enum lists are resolved from AST.
        if type_expr.is_list {
            // §6.1/§6.4: validate the declared list type (including element type)
            // before touching the value, so `[] as [Tree]` inside Tree's own body
            // fails (Tree not yet in scope) and `[...] as [NotAType]` is rejected.
            self.validate_type_exists(type_expr, scope, node)?;
            // eval_type_annotation_list may re-evaluate from AST for enum resolution,
            // but still needs an evaluated fallback for non-enum lists.
            let prev_in_ta = self.in_type_annotation;
            self.in_type_annotation = true;
            let result = self.eval_type_annotation_list_from_ast(expr, type_expr, scope, exclude, node);
            self.in_type_annotation = prev_in_ta;
            return result;
        }

        // §3.5 rule 4 (v0.10): when the expression is a struct literal and the
        // target type is a named struct, evaluate each field with type context
        // so bare identifiers can resolve as enum variants. Bindings still win
        // over variants (rule 4 reversal).
        if let NodeKind::StructLiteral { fields: literal_fields } = &expr.kind {
            if let Some(typedef) = scope.resolve_type_path(&type_expr.path) {
                if let TypeDefKind::Struct { fields: field_infos } = &typedef.kind {
                    let prev_in_ta = self.in_type_annotation;
                    self.in_type_annotation = true;
                    let val = self.eval_struct_literal_with_type_context(
                        literal_fields, field_infos, scope, exclude,
                    );
                    self.in_type_annotation = prev_in_ta;
                    let val = val?;
                    return self.eval_type_annotation_struct(val, type_expr, scope, node);
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
            self.validate_type_exists(type_expr, scope, node)?;
            return Ok(Value::Undefined);
        }

        // §6.1: `null as T` is only valid when T is `null`, a union/tagged-union
        // including null, or a struct with a null member (handled by their own
        // paths above). Tuple types are never null-compatible.
        if val.is_null() && type_expr.tuple_types.is_some() {
            return Err(UzonError::type_error(
                "cannot cast null to tuple type; 'null as T' requires T to be \
                 'null', a union including null, or a tagged-union null variant",
                node.span.line, node.span.col,
            ));
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

        // §6.3 R7: untyped literal adopts a union member type
        if let Some(typedef) = scope.resolve_type_path(&type_expr.path) {
            if let TypeDefKind::Union { types } = &typedef.kind {
                return Self::eval_type_annotation_union(val, types, node);
            }
        }

        // §6.1: Validate type name exists before proceeding
        self.validate_type_exists(type_expr, scope, node)?;

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

    /// Handles `as [Type]` list type annotation, working from AST.
    /// For enum types, resolves bare identifiers as variants directly from AST.
    /// For non-enum types, evaluates the expression first then validates.
    fn eval_type_annotation_list_from_ast(
        &mut self,
        expr: &Node,
        type_expr: &TypeExpr,
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        if let Some(ref inner) = type_expr.inner {
            // Check if inner type is a named enum — resolve list elements as variants from AST
            let enum_info = scope.resolve_type_path(&inner.path).and_then(|td| {
                if let TypeDefKind::Enum { variants } = td.kind {
                    Some((td.name, variants))
                } else { None }
            });
            if let Some((enum_name, variants)) = enum_info {
                return self.eval_list_enum_resolution(expr, &enum_name, &variants, inner, scope, exclude, node);
            }

            // §3.5 + §3.7 v0.10: if the inner type is a named struct or tagged
            // union, evaluate each list element with that type as context so
            // nested variant and struct-field shorthands resolve.
            let named_compound = scope.resolve_type_path(&inner.path).and_then(|td| {
                match td.kind {
                    TypeDefKind::Struct { .. } | TypeDefKind::TaggedUnion { .. } => Some(td.name),
                    _ => None,
                }
            });
            if let Some(element_type_name) = named_compound {
                if let NodeKind::ListLiteral { elements } = &expr.kind {
                    let prev_in_ta = self.in_type_annotation;
                    self.in_type_annotation = true;
                    let mut resolved = Vec::with_capacity(elements.len());
                    let mut err: Option<UzonError> = None;
                    for elem in elements {
                        match self.eval_with_type_context(elem, inner, scope, exclude) {
                            Ok(v) => resolved.push(v),
                            Err(e) => { err = Some(e); break; }
                        }
                    }
                    self.in_type_annotation = prev_in_ta;
                    if let Some(e) = err { return Err(e); }
                    // Validate each element against the declared inner type.
                    let mut list_val = Value::List(UzonList::with_type(resolved, &element_type_name));
                    if let Value::List(list) = &mut list_val {
                        self.validate_list_elements(&mut list.elements, inner, scope, node)?;
                    }
                    return Ok(list_val);
                }
            }
        }
        // Non-enum list type annotation — evaluate the expression, then validate elements
        let mut val = self.eval_node(expr, scope, exclude)?;
        if val.is_undefined() {
            self.validate_type_exists(type_expr, scope, node)?;
            return Ok(Value::Undefined);
        }
        if let Value::List(list) = &mut val {
            if let Some(ref inner) = type_expr.inner {
                self.validate_list_elements(&mut list.elements, inner, scope, node)?;
                if let Some(type_name) = inner.path.last() {
                    list.element_type = Some(type_name.clone());
                }
            }
        } else {
            return Err(UzonError::type_error(
                format!("cannot annotate {} as list type", val.type_name()),
                node.span.line, node.span.col,
            ));
        }
        Ok(val)
    }

    /// Re-evaluate list elements from AST with enum variant context.
    fn eval_list_enum_resolution(
        &mut self,
        expr: &Node,
        enum_name: &str,
        variants: &[String],
        inner: &TypeExpr,
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        if let NodeKind::ListLiteral { elements } = &expr.kind {
            let mut resolved = Vec::with_capacity(elements.len());
            for elem in elements {
                let v = if let NodeKind::Identifier { ref name } = elem.kind {
                    if variants.contains(name) {
                        Value::Enum(UzonEnum::new(
                            name.clone(), variants.to_vec(), Some(enum_name.to_string()),
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
            return Ok(Value::List(UzonList::with_type(resolved, enum_name)));
        }
        // If the expression isn't a list literal, evaluate normally
        self.eval_node(expr, scope, exclude)
    }

    /// §3.5 rule 4 (v0.10): evaluate an expression with a known target-type
    /// context. Handles the bare-identifier-to-enum-variant shortcut and
    /// the struct-literal-with-enum-field-context path; falls through to a
    /// normal `eval_node` when no shortcut applies.
    ///
    /// Bindings win over variants (rule 4 reversal) — if a binding with the
    /// same name is in scope, the identifier is resolved as a binding.
    pub(crate) fn eval_with_type_context(
        &mut self,
        expr: &Node,
        target_type: &TypeExpr,
        scope: &mut Scope,
        exclude: Option<&str>,
    ) -> Result<Value> {
        // Bare identifier against a named enum → variant shortcut.
        if let NodeKind::Identifier { name } = &expr.kind {
            if scope.get(name, None).is_none() {
                if let Some(td) = scope.resolve_type_path(&target_type.path) {
                    if let TypeDefKind::Enum { variants } = &td.kind {
                        if variants.contains(name) {
                            return Ok(Value::Enum(UzonEnum::new(
                                name.clone(),
                                variants.clone(),
                                Some(td.name.clone()),
                            )));
                        }
                    }
                    // §3.7 v0.10: bare identifier against a tagged union → nullary
                    // variant shortcut (`e is cleared`).
                    if let TypeDefKind::TaggedUnion { variants } = &td.kind {
                        if let Some(Some(inner_type)) = variants.get(name) {
                            if inner_type == "null" {
                                return Ok(Value::TaggedUnion(UzonTaggedUnion::new(
                                    Value::Null,
                                    name.clone(),
                                    variants.clone(),
                                    Some(td.name.clone()),
                                )));
                            }
                        }
                    }
                }
            }
        }

        // §3.7 v0.10: `variant_name inner` against a tagged union target type.
        if let NodeKind::VariantShorthand { variant_name, inner } = &expr.kind {
            if let Some(td) = scope.resolve_type_path(&target_type.path) {
                if let TypeDefKind::TaggedUnion { variants } = &td.kind {
                    return self.resolve_variant_shorthand(
                        variant_name, inner, variants, &td.name, scope, exclude, expr,
                    );
                }
            }
        }

        // §3.7 v0.10: `variant_name ( ... )` parses as a function call because
        // LParen is excluded from `starts_variant_shorthand_inner` (to protect
        // regular function calls). When the target type is a tagged union and
        // the callee matches a variant that isn't a bound function, treat the
        // call as variant shorthand: a single arg becomes the inner; multiple
        // args become a tuple.
        if let NodeKind::FunctionCall { callee, args } = &expr.kind {
            if let NodeKind::Identifier { name } = &callee.kind {
                let is_bound_fn = matches!(scope.get(name, exclude), Some(Value::Function(_)));
                if !is_bound_fn {
                    if let Some(td) = scope.resolve_type_path(&target_type.path) {
                        if let TypeDefKind::TaggedUnion { variants } = &td.kind {
                            if variants.contains_key(name) {
                                let inner_node = if args.len() == 1 {
                                    args[0].clone()
                                } else {
                                    Node::new(
                                        NodeKind::TupleLiteral { elements: args.clone() },
                                        expr.span.line, expr.span.col,
                                    )
                                };
                                return self.resolve_variant_shorthand(
                                    name, &inner_node, variants, &td.name, scope, exclude, expr,
                                );
                            }
                        }
                    }
                }
            }
        }

        // Struct literal against a named struct → field-aware evaluation so
        // nested variant shorthand works. §3.2 defaults are filled in by
        // `eval_type_annotation_struct` after the literal fields are resolved.
        if let NodeKind::StructLiteral { fields } = &expr.kind {
            if let Some(td) = scope.resolve_type_path(&target_type.path) {
                if let TypeDefKind::Struct { fields: field_infos } = &td.kind {
                    let val = self.eval_struct_literal_with_type_context(
                        fields, field_infos, scope, exclude,
                    )?;
                    return self.eval_type_annotation_struct(val, target_type, scope, expr);
                }
            }
        }

        self.eval_node(expr, scope, exclude)
    }

    /// §3.7 v0.10: resolve a `variant_name inner` shorthand against a known
    /// tagged-union type. Validates the variant name, evaluates the inner
    /// expression with type context (for nested shorthand), adopts the
    /// variant's declared numeric type on untyped scalars, and wraps in a
    /// `TaggedUnion` value.
    pub(crate) fn resolve_variant_shorthand(
        &mut self,
        variant_name: &str,
        inner: &Node,
        variants: &IndexMap<String, Option<String>>,
        type_name: &str,
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        let inner_type = match variants.get(variant_name) {
            Some(Some(it)) => it.clone(),
            _ => {
                let valid: Vec<&str> = variants.keys().map(|k| k.as_str()).collect();
                return Err(UzonError::type_error(
                    format!("'{variant_name}' is not a variant of tagged union '{type_name}'; valid variants: {}",
                        valid.join(", ")),
                    node.span.line, node.span.col,
                ));
            }
        };

        // Nullary variants reject an inner payload.
        if inner_type == "null" {
            return Err(UzonError::type_error(
                format!("variant '{variant_name}' of '{type_name}' is nullary; use '{variant_name}' without an inner value"),
                node.span.line, node.span.col,
            ));
        }

        // If the variant's inner type is itself a named type, propagate type
        // context so nested variant/struct shorthand resolves.
        let synthetic = TypeExpr {
            path: vec![inner_type.clone()],
            is_list: false,
            inner: None,
            is_null: false,
            tuple_types: None,
            span: inner.span,
        };
        let mut val = if scope.resolve_type_path(&synthetic.path).is_some() {
            self.eval_with_type_context(inner, &synthetic, scope, exclude)?
        } else {
            self.eval_node(inner, scope, exclude)?
        };

        Self::adopt_variant_type_public(&mut val, &inner_type);

        Ok(Value::TaggedUnion(UzonTaggedUnion::new(
            val,
            variant_name,
            variants.clone(),
            Some(type_name.to_string()),
        )))
    }

    /// Mirror of `adopt_variant_type` in enums.rs — adopts a variant's
    /// declared numeric type on untyped scalars. Kept here to avoid a
    /// cross-module visibility change.
    fn adopt_variant_type_public(val: &mut Value, variant_type: &str) {
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

    /// §3.5 rule 4 (v0.10): evaluate struct literal fields with per-field enum
    /// type context so bare identifiers can resolve as variants.
    ///
    /// Resolution per field:
    /// 1. If the field's declared type is a named enum and the binding value is
    ///    a bare identifier, prefer a scope binding with that name (rule 4
    ///    reversal); otherwise resolve the identifier as the matching variant.
    /// 2. If the binding value is itself a struct literal and the field type is
    ///    a named struct, recurse — nested variant shorthand works.
    /// 3. Otherwise evaluate normally.
    pub(crate) fn eval_struct_literal_with_type_context(
        &mut self,
        literal_fields: &[Binding],
        field_infos: &indexmap::IndexMap<String, StructFieldInfo>,
        parent_scope: &mut Scope,
        exclude: Option<&str>,
    ) -> Result<Value> {
        let mut child_scope = Scope::with_parent(parent_scope.clone());
        for binding in literal_fields {
            let value = self.eval_field_with_type_context(
                binding, field_infos, &mut child_scope, exclude,
            )?;
            child_scope.define(binding.name.clone(), value);
        }

        let scope_map = child_scope.to_map();
        let mut result = IndexMap::with_capacity(literal_fields.len());
        for field in literal_fields {
            if let Some(val) = scope_map.get(&field.name) {
                result.insert(field.name.clone(), val.clone());
            }
        }
        Ok(Value::Struct(UzonStruct::new(result)))
    }

    /// Evaluate a single struct-literal binding with awareness of the declared
    /// field type (used for enum variant shorthand and nested named structs).
    fn eval_field_with_type_context(
        &mut self,
        binding: &Binding,
        field_infos: &indexmap::IndexMap<String, StructFieldInfo>,
        child_scope: &mut Scope,
        exclude: Option<&str>,
    ) -> Result<Value> {
        let field_info = match field_infos.get(&binding.name) {
            Some(info) => info,
            None => return self.eval_node(&binding.value, child_scope, exclude),
        };
        let ann = match &field_info.type_annotation {
            Some(a) => a,
            None => return self.eval_node(&binding.value, child_scope, exclude),
        };
        let typedef = match child_scope.get_type(ann).cloned() {
            Some(td) => td,
            None => return self.eval_node(&binding.value, child_scope, exclude),
        };

        // Synthesize a TypeExpr for the annotation and dispatch to the
        // type-context-aware evaluator so nested struct-field and enum
        // variant shortcuts both work.
        let synthetic = TypeExpr {
            path: vec![ann.clone()],
            is_list: false,
            inner: None,
            is_null: false,
            tuple_types: None,
            span: binding.value.span,
        };
        match &typedef.kind {
            TypeDefKind::Enum { .. }
            | TypeDefKind::Struct { .. }
            | TypeDefKind::TaggedUnion { .. } => {
                self.eval_with_type_context(&binding.value, &synthetic, child_scope, exclude)
            }
            _ => self.eval_node(&binding.value, child_scope, exclude),
        }
    }

    /// Validate and annotate each element of an already-evaluated list against its inner type.
    pub(crate) fn validate_list_elements(
        &self,
        items: &mut [Value],
        inner: &TypeExpr,
        scope: &mut Scope,
        node: &Node,
    ) -> Result<()> {
        let inner_type_name = match inner.path.last() {
            Some(name) => name.clone(),
            None => return Ok(()),
        };
        let int_type = IntegerType::from_type_name(&inner_type_name);
        let float_type = FloatType::from_type_name(&inner_type_name);
        let named_func_type = scope.resolve_type_path(&inner.path).and_then(|td| {
            if let TypeDefKind::Function { .. } = &td.kind { Some(td) } else { None }
        });
        let named_struct_type = scope.resolve_type_path(&inner.path).and_then(|td| {
            if let TypeDefKind::Struct { .. } = &td.kind { Some(td) } else { None }
        });
        for item in items.iter_mut() {
            if !item.is_null() {
                self.check_type_assertion(item, &inner_type_name, node)?;
                // §6.3: named function type — nominal check
                if let Some(ref func_td) = named_func_type {
                    Self::check_list_element_function_type(item, func_td, node)?;
                }
                // §6.3: named struct type conformance in list
                if let Some(ref struct_td) = named_struct_type {
                    Self::check_list_element_struct_conformance(item, struct_td, node)?;
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
        Ok(())
    }

    /// Check that a list element conforms to a named function type (§6.3).
    fn check_list_element_function_type(
        item: &Value,
        func_td: &TypeDef,
        node: &Node,
    ) -> Result<()> {
        if let Value::Function(f) = item {
            if f.type_name.as_deref() != Some(&func_td.name) {
                let got = f.type_name.as_deref().unwrap_or("anonymous");
                return Err(UzonError::type_error(
                    format!("function type mismatch: expected {}, got {got}", func_td.name),
                    node.span.line, node.span.col,
                ));
            }
        } else {
            return Err(UzonError::type_error(
                format!("cannot annotate {} as function type {}", item.type_name(), func_td.name),
                node.span.line, node.span.col,
            ));
        }
        Ok(())
    }

    /// Check that a list element struct conforms to a named struct type (§6.3).
    fn check_list_element_struct_conformance(
        item: &Value,
        struct_td: &TypeDef,
        node: &Node,
    ) -> Result<()> {
        if let TypeDefKind::Struct { ref fields } = struct_td.kind {
            if let Value::Struct(val_fields) = item {
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
        Ok(())
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
            let typedef_origin = typedef.origin_file.clone();
            if let Value::Struct(ref val_fields) = val {
                // §6.3 + §7.3: nominal identity is (declaring_file, type_name).
                // Reject when the value already carries a named struct type and
                // either the name or the declaring file differs.
                if let Some(ref existing) = val_fields.type_name {
                    let name_differs = existing != &type_name_str;
                    let origin_differs = match (&val_fields.origin_file, &typedef_origin) {
                        (Some(a), Some(b)) => a != b,
                        _ => false,
                    };
                    if name_differs || origin_differs {
                        let qual = |name: &str, file: &Option<String>| match file {
                            Some(f) => format!("{name} (declared in {f})"),
                            None => name.to_string(),
                        };
                        return Err(UzonError::type_error(
                            format!(
                                "cannot annotate value of type {} as {}; \
                                 named struct types are nominal, not structural",
                                qual(existing, &val_fields.origin_file),
                                qual(&type_name_str, &typedef_origin),
                            ),
                            node.span.line, node.span.col,
                        ));
                    }
                }
                // Check no extra fields
                for key in val_fields.keys() {
                    if !fields.contains_key(key) {
                        return Err(UzonError::type_error(
                            format!("field '{key}' does not exist in type {type_name_str}"),
                            node.span.line, node.span.col,
                        ));
                    }
                }
                // §3.2 v0.10: fill in missing fields with their declared defaults
                // in type-context positions (the caller supplied `as TypeName`).
                if let Value::Struct(ref mut val_fields) = val {
                    for (key, field_info) in fields {
                        if !val_fields.contains_key(key) {
                            val_fields.insert(key.clone(), field_info.default_value.clone());
                        }
                    }
                }
                // Re-borrow for subsequent checks below
                let val_fields = match &val {
                    Value::Struct(m) => m,
                    _ => unreachable!(),
                };
                // Check field type compatibility and annotations
                for (key, field_info) in fields {
                    let val_field = &val_fields[key];
                    if !val_field.is_null() {
                        // §3.2.1 deferred-null: a field declared as untyped
                        // `null` is a type-deferred placeholder — each
                        // construction site independently chooses the
                        // underlying type, so skip the category check.
                        let is_deferred_null = field_info.type_category == "null"
                            && field_info.type_annotation.is_none();
                        if !is_deferred_null
                            && val_field.type_name() != field_info.type_category
                        {
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
                    // §3.2.1 rule 5 + §7.3: stamp named type and declaring-
                    // file origin on the struct value (nominal identity).
                    val_fields.type_name = Some(type_name_str);
                    val_fields.origin_file = typedef_origin;
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

    /// §6.3 R7: a value applied to a named union adopts the first member type
    /// whose category exactly matches the literal's category (integer / float /
    /// string / bool). When no exact-match member exists, an untyped integer
    /// literal falls back to the first float member (integer-to-float
    /// promotion). Float literals never demote to integer members.
    fn eval_type_annotation_union(mut val: Value, types: &[String], node: &Node) -> Result<Value> {
        // §6.1 R6: `null as U` requires `null` to be an explicit member of U.
        if matches!(val, Value::Null) {
            if types.iter().any(|t| t == "null") {
                return Ok(val);
            }
            return Err(UzonError::type_error(
                format!(
                    "cannot cast null to union {}; add 'null' as a member to allow null",
                    types.join(", ")
                ),
                node.span.line, node.span.col,
            ));
        }

        if let Value::Integer(ref mut n) = val {
            if !n.explicit {
                for tn in types {
                    if let Some(it) = IntegerType::from_type_name(tn) {
                        n.type_ann = it;
                        n.explicit = true;
                        return Ok(val);
                    }
                }
                // Integer → float promotion fallback
                let int_value = n.value;
                for tn in types {
                    if let Some(ft) = FloatType::from_type_name(tn) {
                        return Ok(Value::Float(UzonFloat::with_type(int_value as f64, ft)));
                    }
                }
                return Err(UzonError::type_error(
                    format!(
                        "integer literal does not match any member of union {}",
                        types.join(", ")
                    ),
                    node.span.line, node.span.col,
                ));
            }
        }

        if let Value::Float(ref mut f) = val {
            if !f.explicit {
                for tn in types {
                    if let Some(ft) = FloatType::from_type_name(tn) {
                        f.type_ann = ft;
                        f.explicit = true;
                        return Ok(val);
                    }
                }
                return Err(UzonError::type_error(
                    format!(
                        "float literal does not match any member of union {}",
                        types.join(", ")
                    ),
                    node.span.line, node.span.col,
                ));
            }
        }

        // §3.6: tuple value matches a tuple member type structurally. Parse
        // the member signature and adopt element types recursively so that
        // `(42, "hello") as union (i32, string), null` coerces the untyped
        // integer to i32.
        if let Value::Tuple(tup) = &val {
            for tn in types {
                if let Some(elem_types) = Self::parse_tuple_type_signature(tn) {
                    if elem_types.len() == tup.elements.len() {
                        let mut new_elements = Vec::with_capacity(tup.elements.len());
                        let mut ok = true;
                        for (i, et) in elem_types.iter().enumerate() {
                            match Self::try_adopt_value_to_type(tup.elements[i].clone(), et) {
                                Some(v) => new_elements.push(v),
                                None => { ok = false; break; }
                            }
                        }
                        if ok {
                            return Ok(Value::Tuple(UzonTuple { elements: new_elements }));
                        }
                    }
                }
            }
        }

        // Typed value (explicit numerics, strings, bools, other): the value's
        // specific type name must appear in the union member list.
        let actual = Self::specific_type_name(&val);
        if types.iter().any(|t| t == &actual) {
            return Ok(val);
        }

        Err(UzonError::type_error(
            format!(
                "{} does not match any member of union {}",
                actual,
                types.join(", ")
            ),
            node.span.line, node.span.col,
        ))
    }

    /// Parse a formatted tuple type signature like `"(i32, string)"` into
    /// its component type strings. Respects nested parens/brackets so that
    /// `"((i32, string), f64)"` yields `["(i32, string)", "f64"]`. Returns
    /// `None` for non-tuple strings.
    pub(crate) fn parse_tuple_type_signature(s: &str) -> Option<Vec<String>> {
        let s = s.trim();
        if !(s.starts_with('(') && s.ends_with(')')) {
            return None;
        }
        let inner = &s[1..s.len() - 1];
        let mut parts = Vec::new();
        let mut depth: i32 = 0;
        let mut start = 0usize;
        for (i, c) in inner.char_indices() {
            match c {
                '(' | '[' => depth += 1,
                ')' | ']' => depth -= 1,
                ',' if depth == 0 => {
                    parts.push(inner[start..i].trim().to_string());
                    start = i + c.len_utf8();
                }
                _ => {}
            }
        }
        parts.push(inner[start..].trim().to_string());
        Some(parts)
    }

    /// Adopt a value to a structural type name (as produced by
    /// `format_type_expr`). Handles integers/floats (untyped literal
    /// adoption), nested tuples, and exact-match for typed values. Returns
    /// `None` if the value cannot be adopted.
    fn try_adopt_value_to_type(val: Value, type_name: &str) -> Option<Value> {
        if let Value::Integer(mut n) = val {
            if !n.explicit {
                if let Some(it) = IntegerType::from_type_name(type_name) {
                    n.type_ann = it;
                    n.explicit = true;
                    return Some(Value::Integer(n));
                }
                if let Some(ft) = FloatType::from_type_name(type_name) {
                    return Some(Value::Float(UzonFloat::with_type(n.value as f64, ft)));
                }
            }
            let actual = n.type_ann.display_name();
            if actual == type_name {
                return Some(Value::Integer(n));
            }
            return None;
        }
        if let Value::Float(mut f) = val {
            if !f.explicit {
                if let Some(ft) = FloatType::from_type_name(type_name) {
                    f.type_ann = ft;
                    f.explicit = true;
                    return Some(Value::Float(f));
                }
            }
            let actual = f.type_ann.display_name().to_string();
            if actual == type_name {
                return Some(Value::Float(f));
            }
            return None;
        }
        if let Value::Tuple(tup) = val {
            let elem_types = Self::parse_tuple_type_signature(type_name)?;
            if elem_types.len() != tup.elements.len() {
                return None;
            }
            let mut new_elements = Vec::with_capacity(tup.elements.len());
            for (i, et) in elem_types.iter().enumerate() {
                new_elements.push(Self::try_adopt_value_to_type(tup.elements[i].clone(), et)?);
            }
            return Some(Value::Tuple(UzonTuple { elements: new_elements }));
        }
        let actual = Self::specific_type_name(&val);
        if actual == type_name {
            return Some(val);
        }
        None
    }

    pub(crate) fn check_type_assertion(&self, val: &Value, type_name: &str, node: &Node) -> Result<()> {
        // §6.1 R6: `null as T` is valid only when T is `null` itself. Union/
        // tagged-union/struct cases are handled by dedicated paths earlier in
        // `eval_type_annotation`; by the time we reach here, any remaining T
        // must carry an actual null value itself.
        if matches!(val, Value::Null) {
            if type_name == "null" {
                return Ok(());
            }
            return Err(UzonError::type_error(
                format!(
                    "cannot cast null to {type_name}; 'null as T' requires T to be \
                     'null', a union including null, or a tagged-union null variant"
                ),
                node.span.line, node.span.col,
            ));
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
                    Value::Struct(_) | Value::List(_) | Value::Tuple(_) | Value::Function(_) => {
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
                    Value::Struct(_) | Value::List(_) | Value::Tuple(_) | Value::Function(_) => {
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
                // §5.11: conversions not in the permitted table are type errors
                return Err(UzonError::type_error(
                    format!("cannot convert null to {type_name}"),
                    node.span.line, node.span.col,
                ));
            }
        }

        match type_name {
            "string" => {
                // §5.11.2: compound types and functions cannot be converted to string
                match &val {
                    Value::Struct(_) | Value::List(_) | Value::Tuple(_) | Value::Function(_) => {
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
            "null" => {
                if val.is_null() {
                    Ok(val)
                } else {
                    Err(UzonError::type_error(
                        format!("cannot convert {} to null", val.type_name()),
                        node.span.line, node.span.col,
                    ))
                }
            }
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
                // §5.11.1: string-to-float recognizes hex/oct/bin integer prefixes (widening)
                if s.starts_with("0x") || s.starts_with("0X")
                    || s.starts_with("0o") || s.starts_with("0O")
                    || s.starts_with("0b") || s.starts_with("0B")
                    || s.starts_with("-0x") || s.starts_with("-0X")
                    || s.starts_with("-0o") || s.starts_with("-0O")
                    || s.starts_with("-0b") || s.starts_with("-0B")
                {
                    let n = self.eval_integer(&s, node)?;
                    if let Value::Integer(i) = n {
                        let parsed_float_type = FloatType::from_type_name(type_name).unwrap_or_default();
                        return Ok(Value::Float(UzonFloat::with_type(i.value as f64, parsed_float_type)));
                    }
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
