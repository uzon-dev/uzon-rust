// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

mod control;
mod deps;
mod enums;
mod eval_node;
mod functions;
mod import;
mod stdlib;
mod member;
mod ops;
mod structs;
mod types;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::scope::{Scope, TypeDefKind};
use crate::value::*;

/// Options for the evaluator.
#[derive(Default)]
pub struct EvalOptions {
    pub filename: Option<PathBuf>,
    pub env: Option<HashMap<String, String>>,
    pub plain: bool,
}

/// Evaluates a parsed UZON document into a map of values.
pub struct Evaluator {
    pub(crate) env: HashMap<String, String>,
    pub(crate) filename: Option<PathBuf>,
    pub(crate) import_stack: Vec<PathBuf>,
    pub(crate) import_cache: HashMap<PathBuf, BTreeMap<String, Value>>,
    pub(crate) plain: bool,
    /// When true, integer literals skip the default i64 range check (inside `as` annotation).
    pub(crate) in_type_annotation: bool,
    /// When true, bare identifiers resolve against the current scope (function body mode §3.8).
    pub(crate) in_function_body: bool,
}

impl Evaluator {
    pub fn new(options: EvalOptions) -> Self {
        let env = options.env.unwrap_or_else(|| {
            std::env::vars().collect()
        });
        Self {
            env,
            filename: options.filename,
            import_stack: Vec::new(),
            import_cache: HashMap::new(),
            plain: options.plain,
            in_type_annotation: false,
            in_function_body: false,
        }
    }

    /// Unwrap union/tagged-union to inner value (spec §3.7.1: transparent).
    /// Tagged union equality is special — it compares tag+value, so do NOT unwrap for `is`/`is not`.
    pub(crate) fn unwrap_union(val: &Value) -> &Value {
        match val {
            Value::Union(u) => &u.value,
            Value::TaggedUnion(tu) => &tu.value,
            other => other,
        }
    }

    pub(crate) fn unwrap_union_owned(val: Value) -> Value {
        match val {
            Value::Union(u) => *u.value,
            Value::TaggedUnion(tu) => *tu.value,
            other => other,
        }
    }

    /// §3.5 rule 4: resolve a bare identifier as an enum variant when the expected type
    /// is a named enum. Falls back to normal evaluation if not a matching variant.
    pub(crate) fn resolve_enum_context(
        &mut self,
        node: &Node,
        enum_val: &UzonEnum,
        scope: &mut Scope,
        exclude: Option<&str>,
    ) -> Result<Value> {
        if let NodeKind::Identifier { ref name } = node.kind {
            if enum_val.variants.contains(name) {
                return Ok(Value::Enum(UzonEnum::new(
                    name.clone(),
                    enum_val.variants.clone(),
                    enum_val.type_name.clone(),
                )));
            }
        }
        self.eval_node(node, scope, exclude)
    }

    /// §5.9: both branches of if/case MUST return the same type.
    /// Per D.3: "Same type" means exactly the same — i32 ≠ i64, f32 ≠ f64.
    /// Untyped literals adopt the typed operand's type (D.3 adoption rule).
    pub(crate) fn check_branch_types(a: &Value, b: &Value, node: &Node) -> Result<()> {
        if a.is_null() || b.is_null() || a.is_undefined() || b.is_undefined() {
            return Ok(());
        }
        if a.type_name() != b.type_name() {
            return Err(UzonError::type_error(
                format!("branches must return the same type, got {} and {}",
                    a.type_name(), b.type_name()),
                node.span.line, node.span.col,
            ));
        }
        // D.3: exact numeric type_ann must be compatible
        match (a, b) {
            (Value::Integer(ai), Value::Integer(bi)) => {
                if let Err(msg) = UzonInteger::adopt_type(&ai.type_ann, &bi.type_ann) {
                    return Err(UzonError::type_error(
                        format!("branches must return the same type: {msg}"),
                        node.span.line, node.span.col,
                    ));
                }
            }
            (Value::Float(af), Value::Float(bf)) => {
                if let Err(msg) = UzonFloat::adopt_type(&af.type_ann, &bf.type_ann) {
                    return Err(UzonError::type_error(
                        format!("branches must return the same type: {msg}"),
                        node.span.line, node.span.col,
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn evaluate(&mut self, doc: &Document) -> Result<BTreeMap<String, Value>> {
        let mut scope = Scope::new();
        self.eval_bindings(&doc.bindings, &mut scope)?;
        let result = scope.to_map();
        if self.plain {
            Ok(result.into_iter().map(|(k, v)| (k, v.to_plain())).collect())
        } else {
            Ok(result)
        }
    }

    pub(crate) fn eval_bindings(&mut self, bindings: &[Binding], scope: &mut Scope) -> Result<()> {
        // §3.8: Static call graph DAG check — detect recursive function calls
        self.check_function_call_dag(bindings)?;

        // Build dependency graph and topologically sort
        let order = self.topological_sort(bindings, scope)?;

        for idx in order {
            let binding = &bindings[idx];
            self.eval_single_binding(binding, scope)?;
        }

        Ok(())
    }

    /// Evaluate a single binding: validate, compute value, apply list type annotation,
    /// check constraints, and register into scope.
    fn eval_single_binding(&mut self, binding: &Binding, scope: &mut Scope) -> Result<()> {
        // Reject `undefined` literal as binding value (spec §3.1)
        if matches!(binding.value.kind, NodeKind::UndefinedLiteral) {
            return Err(UzonError::syntax(
                "literal 'undefined' cannot be assigned to a binding; use expressions like self.missing instead",
                binding.value.span.line,
                binding.value.span.col,
            ));
        }

        // Handle field extraction (`of`) — use binding name as key
        let mut value = if let NodeKind::FieldExtraction { ref source } = binding.value.kind {
            let source_val = self.eval_node(source, scope, Some(&binding.name))?;
            if source_val.is_undefined() {
                Value::Undefined
            } else if let Value::Struct(fields) = &source_val {
                fields.get(&binding.name).cloned().unwrap_or(Value::Undefined)
            } else {
                return Err(UzonError::type_error(
                    format!("'of' requires a struct, got {}", source_val.type_name()),
                    binding.value.span.line,
                    binding.value.span.col,
                ));
            }
        } else {
            self.eval_node(&binding.value, scope, Some(&binding.name))?
        };

        // Handle list type annotation for `are` bindings (e.g., `ids are 1, 2, 3 as [i32]`)
        self.apply_list_type_annotation(binding, &mut value, scope)?;

        // Spec §3.4: empty list or all-null list without type annotation is rejected
        Self::check_list_annotation_required(&value, binding)?;

        // §3.2: Duplicate binding names are forbidden UNLESS the new binding
        // references self.<name> (self-exclusion pattern, §5.12).
        if scope.has(&binding.name) {
            if !Self::expr_references_self_name(&binding.value, &binding.name) {
                return Err(UzonError::syntax(
                    format!("duplicate binding '{}' in the same scope", binding.name),
                    binding.span.line,
                    binding.span.col,
                ));
            }
        }

        // Set type_name on enum/union/tagged-union values when `called` is present
        let value = if let Some(ref type_name) = binding.called {
            self.set_type_name(value, type_name)
        } else {
            value
        };

        // Register type if `called` is present
        if let Some(ref type_name) = binding.called {
            if scope.get_type(type_name).is_some() {
                return Err(UzonError::syntax(
                    format!("duplicate type name '{type_name}'"),
                    binding.span.line,
                    binding.span.col,
                ));
            }
            self.register_type(type_name, &value, &binding.value, scope)?;
        }

        scope.define(&binding.name, value);
        Ok(())
    }

    /// Apply list type annotation from `are ... as [Type]` bindings.
    fn apply_list_type_annotation(
        &mut self,
        binding: &Binding,
        value: &mut Value,
        scope: &mut Scope,
    ) -> Result<()> {
        let type_ann = match binding.list_type_annotation {
            Some(ref ta) => ta,
            None => return Ok(()),
        };

        if type_ann.is_list {
            // §3.5 rule 4: enum type-context inference via `are ... as [EnumType]`
            if let Some(ref inner) = type_ann.inner {
                let enum_info = scope.resolve_type_path(&inner.path).and_then(|td| {
                    if let TypeDefKind::Enum { variants } = td.kind {
                        Some((td.name, variants))
                    } else { None }
                });
                if let Some((enum_name, variants)) = enum_info {
                    if let NodeKind::ListLiteral { elements } = &binding.value.kind {
                        let mut resolved = Vec::with_capacity(elements.len());
                        for elem in elements {
                            let v = if let NodeKind::Identifier { ref name } = elem.kind {
                                if variants.contains(name) {
                                    Value::Enum(UzonEnum::new(
                                        name.clone(), variants.clone(), Some(enum_name.clone()),
                                    ))
                                } else {
                                    self.eval_node(elem, scope, Some(&binding.name))?
                                }
                            } else {
                                self.eval_node(elem, scope, Some(&binding.name))?
                            };
                            if !v.is_null() {
                                if let Some(inner_type_name) = inner.path.last() {
                                    self.check_type_assertion(&v, inner_type_name, &binding.value)?;
                                }
                            }
                            resolved.push(v);
                        }
                        *value = Value::List(resolved);
                    }
                } else if let Value::List(items) = &*value {
                    if let Some(inner_type_name) = inner.path.last() {
                        for item in items {
                            if !item.is_null() {
                                self.check_type_assertion(item, inner_type_name, &binding.value)?;
                            }
                        }
                    }
                }
            }
        } else if let Some(type_name) = type_ann.path.last() {
            return Err(UzonError::type_error(
                format!("cannot annotate list as {type_name}; use as [{type_name}] for list type annotation"),
                binding.value.span.line, binding.value.span.col,
            ));
        }

        Ok(())
    }

    /// Check that lists that need type annotations have them (§3.4).
    fn check_list_annotation_required(value: &Value, binding: &Binding) -> Result<()> {
        if let Value::List(items) = value {
            if items.is_empty() {
                if matches!(binding.value.kind, NodeKind::ListLiteral { ref elements } if elements.is_empty()) {
                    return Err(UzonError::type_error(
                        "empty list requires a type annotation: [] as [Type]",
                        binding.value.span.line, binding.value.span.col,
                    ));
                }
            }
            // §3.4: all-null list without type annotation requires as [Type]
            if !items.is_empty() && items.iter().all(|v| v.is_null()) {
                if !matches!(binding.value.kind, NodeKind::TypeAnnotation { .. }) {
                    return Err(UzonError::type_error(
                        "list with only null elements requires a type annotation: as [Type]",
                        binding.value.span.line, binding.value.span.col,
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn assert_bool(&self, val: &Value, node: &Node) -> Result<()> {
        if !matches!(val, Value::Bool(_)) {
            Err(UzonError::type_error(
                format!("expected bool, got {}", val.type_name()),
                node.span.line, node.span.col,
            ))
        } else {
            Ok(())
        }
    }
}

/// §5.2: Check that compound types have the same structure before comparison.
pub(crate) fn check_structural_compatibility(a: &Value, b: &Value, node: &Node) -> Result<()> {
    match (a, b) {
        (Value::List(la), Value::List(lb)) => {
            let a_rep = la.iter().find(|v| !v.is_null());
            let b_rep = lb.iter().find(|v| !v.is_null());
            if let (Some(av), Some(bv)) = (a_rep, b_rep) {
                if av.type_name() != bv.type_name() {
                    return Err(UzonError::type_error(
                        format!("cannot compare lists with different element types: [{}] vs [{}]",
                            av.type_name(), bv.type_name()),
                        node.span.line, node.span.col,
                    ));
                }
            }
        }
        (Value::Tuple(ta), Value::Tuple(tb)) => {
            if ta.elements.len() != tb.elements.len() {
                return Err(UzonError::type_error(
                    format!("cannot compare tuples of different length: {} vs {}",
                        ta.elements.len(), tb.elements.len()),
                    node.span.line, node.span.col,
                ));
            }
            for (i, (ea, eb)) in ta.elements.iter().zip(tb.elements.iter()).enumerate() {
                if !ea.is_null() && !eb.is_null() && ea.type_name() != eb.type_name() {
                    return Err(UzonError::type_error(
                        format!("cannot compare tuples with different element types at position {i}: {} vs {}",
                            ea.type_name(), eb.type_name()),
                        node.span.line, node.span.col,
                    ));
                }
            }
        }
        (Value::Struct(sa), Value::Struct(sb)) => {
            let keys_a: HashSet<&String> = sa.keys().collect();
            let keys_b: HashSet<&String> = sb.keys().collect();
            if keys_a != keys_b {
                return Err(UzonError::type_error(
                    "cannot compare structs with different field names",
                    node.span.line, node.span.col,
                ));
            }
            for key in sa.keys() {
                let va = &sa[key];
                let vb = &sb[key];
                if !va.is_null() && !vb.is_null() && va.type_name() != vb.type_name() {
                    return Err(UzonError::type_error(
                        format!("cannot compare structs with different field types for '{}': {} vs {}",
                            key, va.type_name(), vb.type_name()),
                        node.span.line, node.span.col,
                    ));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Deep structural equality comparison for UZON values.
pub fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => return true,
        (Value::Undefined, Value::Undefined) => return true,
        (Value::Null, _) | (_, Value::Null) => return false,
        (Value::Undefined, _) | (_, Value::Undefined) => return false,
        _ => {}
    }

    match (a, b) {
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Integer(a), Value::Integer(b)) => a.value == b.value,
        (Value::Float(a), Value::Float(b)) => {
            if a.value.is_nan() || b.value.is_nan() {
                false
            } else {
                a.value == b.value
            }
        }
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Enum(a), Value::Enum(b)) => {
            match (&a.type_name, &b.type_name) {
                (Some(ta), Some(tb)) => ta == tb && a.value == b.value,
                (None, None) => a.value == b.value,
                _ => false,
            }
        }
        (Value::List(a), Value::List(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| values_equal(x, y))
        }
        (Value::Tuple(a), Value::Tuple(b)) => {
            a.elements.len() == b.elements.len()
                && a.elements.iter().zip(b.elements.iter()).all(|(x, y)| values_equal(x, y))
        }
        (Value::Struct(a), Value::Struct(b)) => {
            a.len() == b.len()
                && a.iter().all(|(k, v)| b.get(k).is_some_and(|bv| values_equal(v, bv)))
        }
        (Value::TaggedUnion(a), Value::TaggedUnion(b)) => {
            a.tag == b.tag && values_equal(&a.value, &b.value)
        }
        (Value::Union(a), Value::Union(b)) => {
            values_equal(&a.value, &b.value)
        }
        _ => false,
    }
}

// === Public API convenience functions ===

/// Parse and evaluate a UZON string, returning UZON-native values.
pub fn from_str(source: &str) -> Result<BTreeMap<String, Value>> {
    let (tokens, comment_lines) = Lexer::new(source).tokenize()?;
    let doc = Parser::new(tokens, comment_lines).parse()?;
    let mut eval = Evaluator::new(EvalOptions::default());
    eval.evaluate(&doc)
}

/// Parse and evaluate a UZON string, returning plain values (no UZON wrappers).
pub fn from_str_plain(source: &str) -> Result<BTreeMap<String, Value>> {
    let (tokens, comment_lines) = Lexer::new(source).tokenize()?;
    let doc = Parser::new(tokens, comment_lines).parse()?;
    let mut eval = Evaluator::new(EvalOptions { plain: true, ..Default::default() });
    eval.evaluate(&doc)
}

/// Parse and evaluate a UZON file, returning UZON-native values.
pub fn from_path(path: &Path) -> Result<BTreeMap<String, Value>> {
    let fname = path.display().to_string();
    let source = std::fs::read_to_string(path).map_err(|e| {
        UzonError::Runtime {
            message: format!("cannot read file '{fname}': {e}"),
            location: None,
            import_trace: Vec::new(),
        }
    })?;
    let (tokens, comment_lines) = Lexer::new(&source).tokenize()
        .map_err(|e| e.with_filename(fname.clone()))?;
    let doc = Parser::new(tokens, comment_lines).parse()
        .map_err(|e| e.with_filename(fname.clone()))?;
    let mut eval = Evaluator::new(EvalOptions {
        filename: Some(path.to_path_buf()),
        ..Default::default()
    });
    eval.evaluate(&doc).map_err(|e| e.with_filename(fname))
}
