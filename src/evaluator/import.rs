// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::scope::{Scope, StructFieldInfo, TypeDef, TypeDefKind};
use crate::value::*;

use super::Evaluator;

impl Evaluator {
    pub(crate) fn eval_struct_import(&mut self, path: &str, node: &Node) -> Result<BTreeMap<String, Value>> {
        let base_dir = self.filename.as_ref()
            .and_then(|f| f.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let mut resolved = base_dir.join(path);

        // Auto-append .uzon if the last component does not contain a '.' (§7.1)
        if let Some(filename) = resolved.file_name().and_then(|n| n.to_str()) {
            if !filename.contains('.') {
                resolved.set_extension("uzon");
            }
        }

        let resolved = resolved.canonicalize().unwrap_or(resolved);

        // Circular import check
        if self.import_stack.contains(&resolved) {
            return Err(UzonError::circular(
                format!("circular import: {}", resolved.display()),
                node.span.line, node.span.col,
            ));
        }

        // Cache check
        if let Some(cached) = self.import_cache.get(&resolved) {
            return Ok(cached.clone());
        }

        let source = std::fs::read_to_string(&resolved).map_err(|e| {
            UzonError::runtime(
                format!("cannot read file '{}': {e}", resolved.display()),
                node.span.line, node.span.col,
            )
        })?;

        let fname = resolved.display().to_string();
        let caller_filename = self.filename.as_ref().map(|p| p.display().to_string());
        let import_line = node.span.line;
        let import_col = node.span.col;

        let add_trace = |e: UzonError| {
            e.with_filename(fname.clone())
             .with_import_site(import_line, import_col, caller_filename.clone())
        };

        let (tokens, comment_lines) = Lexer::new(&source).tokenize()
            .map_err(add_trace)?;
        let doc = Parser::new(tokens, comment_lines).parse()
            .map_err(add_trace)?;

        self.import_stack.push(resolved.clone());

        let mut child_eval = Evaluator {
            env: self.env.clone(),
            filename: Some(resolved.clone()),
            import_stack: self.import_stack.clone(),
            import_cache: self.import_cache.clone(),
            plain: self.plain,
            in_type_annotation: false,
            in_function_body: false,
            collected_errors: Vec::new(),
        };
        let result = child_eval.evaluate(&doc)
            .map_err(add_trace)?;

        self.import_stack.pop();
        self.import_cache.insert(resolved, result.clone());

        Ok(result)
    }

    // === Type registration ===

    pub(crate) fn register_type(
        &self,
        name: &str,
        value: &Value,
        expr: &Node,
        scope: &mut Scope,
    ) -> Result<()> {
        let typedef = match value {
            Value::Enum(e) => TypeDef {
                name: name.to_string(),
                kind: TypeDefKind::Enum { variants: e.variants.clone() },
            },
            Value::Union(u) => TypeDef {
                name: name.to_string(),
                kind: TypeDefKind::Union { types: u.types.clone() },
            },
            Value::TaggedUnion(tu) => TypeDef {
                name: name.to_string(),
                kind: TypeDefKind::TaggedUnion { variants: tu.variants.clone() },
            },
            Value::Struct(fields) => {
                let field_annotations = Self::extract_struct_field_annotations(expr);
                TypeDef {
                    name: name.to_string(),
                    kind: TypeDefKind::Struct {
                        fields: fields.iter().map(|(k, v)| {
                            let annotation = field_annotations.get(k).cloned().flatten();
                            (k.clone(), StructFieldInfo {
                                type_category: v.type_name().to_string(),
                                type_annotation: annotation,
                                default_value: v.clone(),
                            })
                        }).collect(),
                    },
                }
            }
            Value::Function(f) => {
                let param_types: Vec<String> = f.params.iter()
                    .map(|p| p.type_expr.path.last().cloned().unwrap_or_default())
                    .collect();
                let return_type = f.return_type.path.last().cloned().unwrap_or_default();
                TypeDef {
                    name: name.to_string(),
                    kind: TypeDefKind::Function { param_types, return_type },
                }
            }
            _ => return Ok(()),
        };

        scope.define_type(name, typedef);
        Ok(())
    }

    /// Extract field type annotations from a struct literal AST node.
    pub(crate) fn extract_struct_field_annotations(expr: &Node) -> BTreeMap<String, Option<String>> {
        let mut annotations = BTreeMap::new();
        let bindings = match &expr.kind {
            NodeKind::StructLiteral { fields } => fields,
            _ => return annotations,
        };
        for binding in bindings {
            annotations.insert(binding.name.clone(), Self::extract_annotation_from_node(&binding.value));
        }
        annotations
    }

    /// Walk an expression looking for the outermost `as TypeName` annotation.
    /// Handles wrapper forms (`x as T named tag`, `x named tag as T`) that carry
    /// the declared field type on an inner TypeAnnotation node.
    fn extract_annotation_from_node(node: &Node) -> Option<String> {
        match &node.kind {
            NodeKind::TypeAnnotation { type_expr, .. } => type_expr.path.last().cloned(),
            NodeKind::NamedVariant { value, .. } => Self::extract_annotation_from_node(value),
            _ => None,
        }
    }

    pub(crate) fn set_type_name(&self, value: Value, type_name: &str) -> Value {
        match value {
            Value::Enum(mut e) => {
                e.type_name = Some(type_name.to_string());
                Value::Enum(e)
            }
            Value::Union(mut u) => {
                u.type_name = Some(type_name.to_string());
                Value::Union(u)
            }
            Value::TaggedUnion(mut tu) => {
                tu.type_name = Some(type_name.to_string());
                Value::TaggedUnion(tu)
            }
            Value::Function(mut f) => {
                f.type_name = Some(type_name.to_string());
                Value::Function(f)
            }
            Value::Struct(mut s) => {
                s.type_name = Some(type_name.to_string());
                s.declares_type = true;
                Value::Struct(s)
            }
            Value::List(mut l) => {
                l.type_name = Some(type_name.to_string());
                Value::List(l)
            }
            other => other,
        }
    }

    /// §6.1: Validate that a type expression refers to known types. Recurses
    /// into list element types and tuple component types so that
    /// `[NotAType]` and `(i32, NotAType)` are rejected, and so that a
    /// self-referential struct field such as `children is [] as [Tree]`
    /// fails §6.4 (Tree is not yet in scope while evaluating its own body).
    pub(crate) fn validate_type_exists(&self, type_expr: &TypeExpr, scope: &Scope, node: &Node) -> Result<()> {
        if type_expr.is_list {
            if let Some(inner) = &type_expr.inner {
                return self.validate_type_exists(inner, scope, node);
            }
            return Ok(());
        }
        if let Some(tuple) = &type_expr.tuple_types {
            for t in tuple {
                self.validate_type_exists(t, scope, node)?;
            }
            return Ok(());
        }
        if type_expr.is_null {
            return Ok(());
        }
        let Some(type_name) = type_expr.path.last() else {
            return Ok(());
        };
        let is_builtin = type_name == "string"
            || type_name == "bool"
            || type_name == "null"
            || type_name.starts_with('i') && type_name[1..].parse::<u32>().is_ok()
            || type_name.starts_with('u') && type_name[1..].parse::<u32>().is_ok()
            || type_name.starts_with('f') && type_name[1..].parse::<u32>().is_ok();
        if is_builtin {
            return Ok(());
        }
        if scope.resolve_type_path(&type_expr.path).is_some() {
            return Ok(());
        }
        Err(UzonError::type_error(
            format!("unknown type '{type_name}'"),
            node.span.line, node.span.col,
        ))
    }
}
