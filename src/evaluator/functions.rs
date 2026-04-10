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
                return Err(UzonError::runtime(
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

        // Check for too many arguments
        if arg_vals.len() > func.params.len() {
            return Err(UzonError::runtime(
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

    // === Standard library ===

    pub(crate) fn eval_std_call(
        &mut self,
        method: &str,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        match method {
            "len" => {
                if args.len() != 1 {
                    return Err(UzonError::runtime("std.len requires exactly 1 argument", node.span.line, node.span.col));
                }
                let val = self.eval_node(&args[0], scope, exclude)?;
                match Self::unwrap_union_owned(val) {
                    Value::List(items) => Ok(Value::int(items.len() as i128)),
                    Value::Tuple(t) => Ok(Value::int(t.len() as i128)),
                    Value::Struct(fields) => Ok(Value::int(fields.len() as i128)),
                    Value::String(s) => Ok(Value::int(s.len() as i128)),
                    other => Err(UzonError::type_error(
                        format!("std.len does not support {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            "has" => {
                if args.len() != 2 {
                    return Err(UzonError::runtime("std.has requires exactly 2 arguments", node.span.line, node.span.col));
                }
                let collection = self.eval_node(&args[0], scope, exclude)?;
                let key = self.eval_node(&args[1], scope, exclude)?;
                match Self::unwrap_union_owned(collection) {
                    Value::Struct(fields) => {
                        match key {
                            Value::String(k) => Ok(Value::Bool(fields.contains_key(&k))),
                            other => Err(UzonError::type_error(
                                format!("std.has key must be string for struct, got {}", other.type_name()),
                                node.span.line, node.span.col,
                            )),
                        }
                    }
                    Value::List(items) => {
                        // §5.16.1: value and element types MUST match (same rules as `in`)
                        // §5.8.1: null is exempt from type constraint on either side
                        if !key.is_null() && !items.is_empty() {
                            let first_non_null = items.iter().find(|i| !i.is_null());
                            if let Some(elem) = first_non_null {
                                let elem_type = elem.type_name();
                                let key_type = key.type_name();
                                if elem_type != key_type {
                                    return Err(UzonError::type_error(
                                        format!("std.has type mismatch: list element type is {elem_type}, search value is {key_type}"),
                                        node.span.line, node.span.col,
                                    ));
                                }
                            }
                        }
                        Ok(Value::Bool(items.contains(&key)))
                    }
                    other => Err(UzonError::type_error(
                        format!("std.has does not support {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            "get" => {
                if args.len() != 2 {
                    return Err(UzonError::runtime("std.get requires exactly 2 arguments", node.span.line, node.span.col));
                }
                let collection = self.eval_node(&args[0], scope, exclude)?;
                let key = self.eval_node(&args[1], scope, exclude)?;
                match (Self::unwrap_union_owned(collection), key) {
                    (Value::Struct(fields), Value::String(k)) => {
                        Ok(fields.get(&k).cloned().unwrap_or(Value::Undefined))
                    }
                    (Value::List(items), Value::Integer(idx)) => {
                        let i = idx.value as usize;
                        Ok(items.get(i).cloned().unwrap_or(Value::Undefined))
                    }
                    (Value::Tuple(t), Value::Integer(idx)) => {
                        let i = idx.value as usize;
                        Ok(t.elements.get(i).cloned().unwrap_or(Value::Undefined))
                    }
                    (other, _) => Err(UzonError::type_error(
                        format!("std.get does not support {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            "keys" => {
                if args.len() != 1 {
                    return Err(UzonError::runtime("std.keys requires exactly 1 argument", node.span.line, node.span.col));
                }
                let val = self.eval_node(&args[0], scope, exclude)?;
                match Self::unwrap_union_owned(val) {
                    Value::Struct(fields) => {
                        let keys: Vec<Value> = fields.keys().map(|k| Value::String(k.clone())).collect();
                        Ok(Value::List(keys))
                    }
                    other => Err(UzonError::type_error(
                        format!("std.keys does not support {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            "values" => {
                // §5.16.1: returns field values as a tuple (mixed types OK)
                if args.len() != 1 {
                    return Err(UzonError::runtime("std.values requires exactly 1 argument", node.span.line, node.span.col));
                }
                let val = self.eval_node(&args[0], scope, exclude)?;
                match Self::unwrap_union_owned(val) {
                    Value::Struct(fields) => {
                        let vals: Vec<Value> = fields.into_values().collect();
                        Ok(Value::Tuple(UzonTuple::new(vals)))
                    }
                    other => Err(UzonError::type_error(
                        format!("std.values does not support {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            "map" => {
                if args.len() != 2 {
                    return Err(UzonError::runtime("std.map requires exactly 2 arguments", node.span.line, node.span.col));
                }
                let list = self.eval_node(&args[0], scope, exclude)?;
                let func_val = self.eval_node(&args[1], scope, exclude)?;
                self.std_map(list, func_val, node)
            }
            "filter" => {
                if args.len() != 2 {
                    return Err(UzonError::runtime("std.filter requires exactly 2 arguments", node.span.line, node.span.col));
                }
                let list = self.eval_node(&args[0], scope, exclude)?;
                let func_val = self.eval_node(&args[1], scope, exclude)?;
                self.std_filter(list, func_val, node)
            }
            "reduce" => {
                if args.len() != 3 {
                    return Err(UzonError::runtime("std.reduce requires exactly 3 arguments", node.span.line, node.span.col));
                }
                let list = self.eval_node(&args[0], scope, exclude)?;
                let initial = self.eval_node(&args[1], scope, exclude)?;
                let func_val = self.eval_node(&args[2], scope, exclude)?;
                self.std_reduce(list, initial, func_val, node)
            }
            "sort" => {
                if args.len() != 2 {
                    return Err(UzonError::runtime("std.sort requires exactly 2 arguments", node.span.line, node.span.col));
                }
                let list = self.eval_node(&args[0], scope, exclude)?;
                let func_val = self.eval_node(&args[1], scope, exclude)?;
                self.std_sort(list, func_val, node)
            }
            "isNan" => {
                if args.len() != 1 {
                    return Err(UzonError::runtime("std.isNan requires exactly 1 argument", node.span.line, node.span.col));
                }
                let val = self.eval_node(&args[0], scope, exclude)?;
                match Self::unwrap_union_owned(val) {
                    Value::Float(f) => Ok(Value::Bool(f.value.is_nan())),
                    other => Err(UzonError::type_error(
                        format!("std.isNan requires float, got {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            "isInf" => {
                if args.len() != 1 {
                    return Err(UzonError::runtime("std.isInf requires exactly 1 argument", node.span.line, node.span.col));
                }
                let val = self.eval_node(&args[0], scope, exclude)?;
                match Self::unwrap_union_owned(val) {
                    Value::Float(f) => Ok(Value::Bool(f.value.is_infinite())),
                    other => Err(UzonError::type_error(
                        format!("std.isInf requires float, got {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            "isFinite" => {
                if args.len() != 1 {
                    return Err(UzonError::runtime("std.isFinite requires exactly 1 argument", node.span.line, node.span.col));
                }
                let val = self.eval_node(&args[0], scope, exclude)?;
                match Self::unwrap_union_owned(val) {
                    Value::Float(f) => Ok(Value::Bool(f.value.is_finite())),
                    other => Err(UzonError::type_error(
                        format!("std.isFinite requires float, got {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            // §5.16.4 String Utilities
            "split" => {
                if args.len() != 2 {
                    return Err(UzonError::runtime("std.split requires exactly 2 arguments", node.span.line, node.span.col));
                }
                let input = self.eval_node(&args[0], scope, exclude)?;
                let delim = self.eval_node(&args[1], scope, exclude)?;
                match (Self::unwrap_union_owned(input), Self::unwrap_union_owned(delim)) {
                    (Value::String(s), Value::String(d)) => {
                        let parts: Vec<Value> = s.split(&d).map(|p| Value::String(p.to_string())).collect();
                        Ok(Value::List(parts))
                    }
                    (Value::String(_), other) => Err(UzonError::type_error(
                        format!("std.split delimiter must be string, got {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                    (other, _) => Err(UzonError::type_error(
                        format!("std.split first argument must be string, got {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            "trim" => {
                if args.len() != 1 {
                    return Err(UzonError::runtime("std.trim requires exactly 1 argument", node.span.line, node.span.col));
                }
                let val = self.eval_node(&args[0], scope, exclude)?;
                match Self::unwrap_union_owned(val) {
                    Value::String(s) => Ok(Value::String(s.trim().to_string())),
                    other => Err(UzonError::type_error(
                        format!("std.trim requires string, got {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            "replace" => {
                if args.len() != 3 {
                    return Err(UzonError::runtime("std.replace requires exactly 3 arguments", node.span.line, node.span.col));
                }
                let input = self.eval_node(&args[0], scope, exclude)?;
                let target = self.eval_node(&args[1], scope, exclude)?;
                let replacement = self.eval_node(&args[2], scope, exclude)?;
                match (Self::unwrap_union_owned(input), Self::unwrap_union_owned(target), Self::unwrap_union_owned(replacement)) {
                    (Value::String(s), Value::String(t), Value::String(r)) => {
                        if t.is_empty() {
                            Ok(Value::String(s))
                        } else {
                            Ok(Value::String(s.replace(&t, &r)))
                        }
                    }
                    (Value::String(_), Value::String(_), other) => Err(UzonError::type_error(
                        format!("std.replace replacement must be string, got {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                    (Value::String(_), other, _) => Err(UzonError::type_error(
                        format!("std.replace target must be string, got {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                    (other, _, _) => Err(UzonError::type_error(
                        format!("std.replace first argument must be string, got {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            "lower" => {
                if args.len() != 1 {
                    return Err(UzonError::runtime("std.lower requires exactly 1 argument", node.span.line, node.span.col));
                }
                let val = self.eval_node(&args[0], scope, exclude)?;
                match Self::unwrap_union_owned(val) {
                    Value::String(s) => Ok(Value::String(s.to_lowercase())),
                    other => Err(UzonError::type_error(
                        format!("std.lower requires string, got {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            "upper" => {
                if args.len() != 1 {
                    return Err(UzonError::runtime("std.upper requires exactly 1 argument", node.span.line, node.span.col));
                }
                let val = self.eval_node(&args[0], scope, exclude)?;
                match Self::unwrap_union_owned(val) {
                    Value::String(s) => Ok(Value::String(s.to_uppercase())),
                    other => Err(UzonError::type_error(
                        format!("std.upper requires string, got {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            "join" => {
                if args.len() != 2 {
                    return Err(UzonError::runtime("std.join requires exactly 2 arguments", node.span.line, node.span.col));
                }
                let list_val = self.eval_node(&args[0], scope, exclude)?;
                let sep_val = self.eval_node(&args[1], scope, exclude)?;
                let sep = match Self::unwrap_union_owned(sep_val) {
                    Value::String(s) => s,
                    other => return Err(UzonError::type_error(
                        format!("std.join separator must be string, got {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                };
                match Self::unwrap_union_owned(list_val) {
                    Value::List(items) => {
                        let mut strings = Vec::with_capacity(items.len());
                        for item in items {
                            match item {
                                Value::String(s) => strings.push(s),
                                other => return Err(UzonError::type_error(
                                    format!("std.join requires [string], found {} in list", other.type_name()),
                                    node.span.line, node.span.col,
                                )),
                            }
                        }
                        Ok(Value::String(strings.join(&sep)))
                    }
                    other => Err(UzonError::type_error(
                        format!("std.join first argument must be a list, got {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            _ => Err(UzonError::runtime(
                format!("unknown standard library function: std.{method}"),
                node.span.line, node.span.col,
            )),
        }
    }

    // === Standard library helpers ===

    fn std_map(
        &mut self,
        list: Value,
        func_val: Value,
        node: &Node,
    ) -> Result<Value> {
        let func = match func_val {
            Value::Function(f) => f,
            _ => return Err(UzonError::type_error(
                format!("std.map second argument must be a function, got {}", func_val.type_name()),
                node.span.line, node.span.col,
            )),
        };
        let items = match Self::unwrap_union_owned(list) {
            Value::List(items) => items,
            other => return Err(UzonError::type_error(
                format!("std.map first argument must be a list, got {}", other.type_name()),
                node.span.line, node.span.col,
            )),
        };
        let mut results = Vec::with_capacity(items.len());
        for item in items {
            let result = self.call_function(&func, vec![item], node)?;
            results.push(result);
        }
        Ok(Value::List(results))
    }

    fn std_filter(
        &mut self,
        list: Value,
        func_val: Value,
        node: &Node,
    ) -> Result<Value> {
        let func = match func_val {
            Value::Function(f) => f,
            _ => return Err(UzonError::type_error(
                format!("std.filter second argument must be a function, got {}", func_val.type_name()),
                node.span.line, node.span.col,
            )),
        };
        let items = match Self::unwrap_union_owned(list) {
            Value::List(items) => items,
            other => return Err(UzonError::type_error(
                format!("std.filter first argument must be a list, got {}", other.type_name()),
                node.span.line, node.span.col,
            )),
        };
        let mut results = Vec::new();
        for item in items {
            let result = self.call_function(&func, vec![item.clone()], node)?;
            match Self::unwrap_union_owned(result) {
                Value::Bool(true) => results.push(item),
                Value::Bool(false) => {}
                other => return Err(UzonError::type_error(
                    format!("std.filter predicate must return bool, got {}", other.type_name()),
                    node.span.line, node.span.col,
                )),
            }
        }
        Ok(Value::List(results))
    }

    fn std_reduce(
        &mut self,
        list: Value,
        initial: Value,
        func_val: Value,
        node: &Node,
    ) -> Result<Value> {
        let func = match func_val {
            Value::Function(f) => f,
            _ => return Err(UzonError::type_error(
                format!("std.reduce third argument must be a function, got {}", func_val.type_name()),
                node.span.line, node.span.col,
            )),
        };
        let items = match Self::unwrap_union_owned(list) {
            Value::List(items) => items,
            other => return Err(UzonError::type_error(
                format!("std.reduce first argument must be a list, got {}", other.type_name()),
                node.span.line, node.span.col,
            )),
        };
        // Type check: initial value type must match function return type
        let return_type_name = func.return_type.path.last().cloned().unwrap_or_default();
        if !return_type_name.is_empty() {
            let init_type = initial.type_name();
            let expected_category = match return_type_name.as_str() {
                t if IntegerType::from_type_name(t).is_some() => "integer",
                t if FloatType::from_type_name(t).is_some() => "float",
                "string" => "string",
                "bool" => "bool",
                _ => "",
            };
            if !expected_category.is_empty() && init_type != expected_category {
                return Err(UzonError::type_error(
                    format!("std.reduce initial value type ({}) does not match function return type ({})",
                        init_type, return_type_name),
                    node.span.line, node.span.col,
                ));
            }
        }
        let mut acc = initial;
        for item in items {
            acc = self.call_function(&func, vec![acc, item], node)?;
        }
        Ok(acc)
    }

    fn std_sort(
        &mut self,
        list: Value,
        func_val: Value,
        node: &Node,
    ) -> Result<Value> {
        let func = match func_val {
            Value::Function(f) => f,
            _ => return Err(UzonError::type_error(
                format!("std.sort second argument must be a function, got {}", func_val.type_name()),
                node.span.line, node.span.col,
            )),
        };
        // Bug fix absorbed: comparator must take exactly 2 parameters (§5.16.2)
        if func.params.len() != 2 {
            return Err(UzonError::type_error(
                format!("std.sort comparator must take exactly 2 parameters, got {}", func.params.len()),
                node.span.line, node.span.col,
            ));
        }
        let mut items = match Self::unwrap_union_owned(list) {
            Value::List(items) => items,
            other => return Err(UzonError::type_error(
                format!("std.sort first argument must be a list, got {}", other.type_name()),
                node.span.line, node.span.col,
            )),
        };
        // Stable insertion sort — call_function requires &mut self,
        // so we can't use sort_by with a closure.
        for i in 1..items.len() {
            let mut j = i;
            while j > 0 {
                let result = self.call_function(
                    &func,
                    vec![items[j].clone(), items[j - 1].clone()],
                    node,
                )?;
                match result {
                    Value::Bool(true) => {
                        items.swap(j, j - 1);
                        j -= 1;
                    }
                    Value::Bool(false) => break,
                    other => return Err(UzonError::type_error(
                        format!("std.sort comparator must return bool, got {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
        }
        Ok(Value::List(items))
    }

    /// Helper to call a function value with given argument values.
    pub(crate) fn call_function(
        &mut self,
        func: &UzonFunction,
        arg_vals: Vec<Value>,
        node: &Node,
    ) -> Result<Value> {
        let mut func_scope = Scope::new();
        for (name, val) in &func.captured_bindings {
            func_scope.define(name.clone(), val.clone());
        }
        for (name, td) in &func.captured_types {
            func_scope.define_type(name.clone(), td.clone());
        }

        for (i, param) in func.params.iter().enumerate() {
            let mut val = if i < arg_vals.len() {
                arg_vals[i].clone()
            } else if let Some(ref default_expr) = param.default {
                self.eval_node(default_expr, &mut func_scope, None)?
            } else {
                return Err(UzonError::runtime(
                    format!("missing argument for parameter '{}'", param.name),
                    node.span.line, node.span.col,
                ));
            };
            if let Some(type_name) = param.type_expr.path.last() {
                val = Self::coerce_to_param_type(val, type_name);
            }
            func_scope.define(param.name.clone(), val);
        }

        let prev_in_function_body = self.in_function_body;
        self.in_function_body = true;

        self.eval_bindings(&func.body_bindings, &mut func_scope)?;
        let result = self.eval_node(&func.body_expr, &mut func_scope, None)?;

        self.in_function_body = prev_in_function_body;

        // Bug fix absorbed: check return type name for named struct types (§6.3)
        if let Some(return_type_name) = func.return_type.path.last() {
            if let Some(typedef) = func_scope.resolve_type_path(&func.return_type.path) {
                if let TypeDefKind::Struct { .. } = &typedef.kind {
                    if let Value::Struct(_) = &result {
                        self.check_return_type_name(&func.body_expr, return_type_name, &func_scope, node)?;
                    }
                }
            }
        }

        Ok(result)
    }

    /// Check if a body expression's type annotation matches the declared return type name.
    fn check_return_type_name(
        &self,
        body_expr: &Node,
        expected_name: &str,
        scope: &Scope,
        node: &Node,
    ) -> Result<()> {
        if let NodeKind::TypeAnnotation { ref type_expr, .. } = body_expr.kind {
            if let Some(actual_name) = type_expr.path.last() {
                if let Some(actual_typedef) = scope.resolve_type_path(&type_expr.path) {
                    if let TypeDefKind::Struct { .. } = &actual_typedef.kind {
                        if actual_name != expected_name {
                            return Err(UzonError::type_error(
                                format!(
                                    "function return type is {} but body returns {}",
                                    expected_name, actual_name
                                ),
                                node.span.line, node.span.col,
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
