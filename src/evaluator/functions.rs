// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::scope::{Scope, TypeDefKind};
use crate::value::*;

use super::Evaluator;

impl Evaluator {
    pub(crate) fn eval_function_call(
        &mut self,
        callee: &Node,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        let func_val = self.eval_node(callee, scope, exclude)?;
        // §3.1: calling undefined is a runtime error; calling non-function is a type error (§5.15)
        if func_val.is_undefined() {
            return Err(UzonError::runtime(
                "cannot call undefined, expected function",
                node.span.line, node.span.col,
            ));
        }
        let func = match func_val {
            Value::Function(f) => f,
            _ => return Err(UzonError::type_error(
                format!("cannot call {}, expected function", func_val.type_name()),
                node.span.line, node.span.col,
            )),
        };

        // Evaluate arguments before entering function body mode
        let mut arg_vals = Vec::with_capacity(args.len());
        for arg in args {
            let val = self.eval_node(arg, scope, exclude)?;
            // §3.1: undefined as argument is a runtime error
            if val.is_undefined() {
                return Err(UzonError::runtime(
                    "undefined cannot be passed as a function argument",
                    arg.span.line, arg.span.col,
                ));
            }
            arg_vals.push(val);
        }

        // Create function scope from captured bindings + types from enclosing scope
        let mut func_scope = Scope::new();
        for (name, val) in &func.captured_bindings {
            func_scope.define(name.clone(), val.clone());
        }
        for (name, td) in scope.all_types() {
            func_scope.define_type(name, td);
        }

        // §3.8: Default expressions are evaluated in the enclosing scope, not function body.
        let mut default_scope = Scope::new();
        for (name, val) in &func.captured_bindings {
            default_scope.define(name.clone(), val.clone());
        }

        // Bind parameters
        for (i, param) in func.params.iter().enumerate() {
            let mut val = if i < arg_vals.len() {
                arg_vals[i].clone()
            } else if let Some(ref default_expr) = param.default {
                self.eval_node(default_expr, &mut default_scope, None)?
            } else {
                // §5.15: wrong number of arguments is a type error
                return Err(UzonError::type_error(
                    format!("missing argument for parameter '{}'", param.name),
                    node.span.line, node.span.col,
                ));
            };

            // §3.3: Tuple type validation — check element count
            if let Some(ref tuple_types) = param.type_expr.tuple_types {
                if let Value::Tuple(ref t) = val {
                    if t.elements.len() != tuple_types.len() {
                        return Err(UzonError::type_error(
                            format!("parameter '{}' expects tuple of {} elements, got {}",
                                param.name, tuple_types.len(), t.elements.len()),
                            node.span.line, node.span.col,
                        ));
                    }
                } else {
                    return Err(UzonError::type_error(
                        format!("parameter '{}' expects tuple, got {}", param.name, val.type_name()),
                        node.span.line, node.span.col,
                    ));
                }
            }
            // Type check and coerce argument to parameter type
            if let Some(type_name) = param.type_expr.path.last() {
                self.check_type_assertion(&val, type_name, node)?;
                // §3.8 + §6.3: named struct type parameter — validate conformance
                if let Some(typedef) = func_scope.resolve_type_path(&param.type_expr.path) {
                    if let TypeDefKind::Struct { ref fields } = typedef.kind {
                        if let Value::Struct(ref val_fields) = val {
                            for key in val_fields.keys() {
                                if !fields.contains_key(key) {
                                    return Err(UzonError::type_error(
                                        format!("argument has field '{}' not in type {}", key, typedef.name),
                                        node.span.line, node.span.col,
                                    ));
                                }
                            }
                            for key in fields.keys() {
                                if !val_fields.contains_key(key) {
                                    return Err(UzonError::type_error(
                                        format!("argument missing field '{}' required by type {}", key, typedef.name),
                                        node.span.line, node.span.col,
                                    ));
                                }
                            }
                        } else {
                            return Err(UzonError::type_error(
                                format!("parameter '{}' expects struct type {}, got {}",
                                    param.name, typedef.name, val.type_name()),
                                node.span.line, node.span.col,
                            ));
                        }
                    }
                }
                val = Self::coerce_to_param_type(val, type_name);
            }

            func_scope.define(param.name.clone(), val);
        }

        // §5.15: wrong number of arguments is a type error
        if arg_vals.len() > func.params.len() {
            return Err(UzonError::type_error(
                format!(
                    "too many arguments: expected {}, got {}",
                    func.params.len(), arg_vals.len()
                ),
                node.span.line, node.span.col,
            ));
        }

        // Enter function body mode for bare identifier resolution (§3.8)
        let prev_in_function_body = self.in_function_body;
        self.in_function_body = true;

        self.eval_bindings(&func.body_bindings, &mut func_scope)?;
        let result = self.eval_node(&func.body_expr, &mut func_scope, None)?;

        self.in_function_body = prev_in_function_body;

        // Type check return value
        if let Some(type_name) = func.return_type.path.last() {
            self.check_type_assertion(&result, type_name, node)?;

            // §3.8 + §6.3: check return value against declared return type for named structs
            if let Value::Struct(_) = &result {
                if let Some(typedef) = func_scope.resolve_type_path(&func.return_type.path) {
                    if let TypeDefKind::Struct { .. } = &typedef.kind {
                        self.check_return_type_name(&func.body_expr, type_name, &func_scope, node)?;
                    }
                }
            }
        }

        Ok(result)
    }
}
