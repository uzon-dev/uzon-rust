// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::scope::{Scope, TypeDefKind};
use crate::value::*;

use super::Evaluator;

impl Evaluator {
    // === Standard library dispatch ===

    pub(crate) fn eval_std_call(
        &mut self,
        method: &str,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        match method {
            "len" => self.std_len(args, scope, exclude, node),
            "has" => self.std_has(args, scope, exclude, node),
            "get" => self.std_get(args, scope, exclude, node),
            "keys" => self.std_keys(args, scope, exclude, node),
            "values" => self.std_values(args, scope, exclude, node),
            "map" => self.std_map_dispatch(args, scope, exclude, node),
            "filter" => self.std_filter_dispatch(args, scope, exclude, node),
            "reduce" => self.std_reduce_dispatch(args, scope, exclude, node),
            "sort" => self.std_sort_dispatch(args, scope, exclude, node),
            "isNan" => self.std_is_nan(args, scope, exclude, node),
            "isInf" => self.std_is_inf(args, scope, exclude, node),
            "isFinite" => self.std_is_finite(args, scope, exclude, node),
            "split" => self.std_split(args, scope, exclude, node),
            "trim" => self.std_trim(args, scope, exclude, node),
            "replace" => self.std_replace(args, scope, exclude, node),
            "lower" => self.std_lower(args, scope, exclude, node),
            "upper" => self.std_upper(args, scope, exclude, node),
            "join" => self.std_join(args, scope, exclude, node),
            _ => Err(UzonError::runtime(
                format!("unknown standard library function: std.{method}"),
                node.span.line, node.span.col,
            )),
        }
    }

    // === Individual stdlib functions ===

    fn std_len(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        if args.len() != 1 {
            return Err(UzonError::runtime("std.len requires exactly 1 argument", node.span.line, node.span.col));
        }
        let val = self.eval_node(&args[0], scope, exclude)?;
        match Self::unwrap_union_owned(val) {
            Value::List(items) => Ok(Value::int(items.len() as i128)),
            Value::Tuple(t) => Ok(Value::int(t.len() as i128)),
            Value::Struct(fields) => Ok(Value::int(fields.len() as i128)),
            Value::String(s) => Ok(Value::int(s.chars().count() as i128)),
            other => Err(UzonError::type_error(
                format!("std.len does not support {}", other.type_name()),
                node.span.line, node.span.col,
            )),
        }
    }

    fn std_has(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
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

    fn std_get(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
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

    fn std_keys(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        if args.len() != 1 {
            return Err(UzonError::runtime("std.keys requires exactly 1 argument", node.span.line, node.span.col));
        }
        let val = self.eval_node(&args[0], scope, exclude)?;
        match Self::unwrap_union_owned(val) {
            Value::Struct(fields) => {
                let keys: Vec<Value> = fields.keys().map(|k| Value::String(k.clone())).collect();
                Ok(Value::list(keys))
            }
            other => Err(UzonError::type_error(
                format!("std.keys does not support {}", other.type_name()),
                node.span.line, node.span.col,
            )),
        }
    }

    fn std_values(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
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

    fn std_map_dispatch(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        if args.len() != 2 {
            return Err(UzonError::runtime("std.map requires exactly 2 arguments", node.span.line, node.span.col));
        }
        let list = self.eval_node(&args[0], scope, exclude)?;
        let func_val = self.eval_node(&args[1], scope, exclude)?;
        self.std_map(list, func_val, node)
    }

    fn std_filter_dispatch(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        if args.len() != 2 {
            return Err(UzonError::runtime("std.filter requires exactly 2 arguments", node.span.line, node.span.col));
        }
        let list = self.eval_node(&args[0], scope, exclude)?;
        let func_val = self.eval_node(&args[1], scope, exclude)?;
        self.std_filter(list, func_val, node)
    }

    fn std_reduce_dispatch(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        if args.len() != 3 {
            return Err(UzonError::runtime("std.reduce requires exactly 3 arguments", node.span.line, node.span.col));
        }
        let list = self.eval_node(&args[0], scope, exclude)?;
        let initial = self.eval_node(&args[1], scope, exclude)?;
        let func_val = self.eval_node(&args[2], scope, exclude)?;
        self.std_reduce(list, initial, func_val, node)
    }

    fn std_sort_dispatch(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        if args.len() != 2 {
            return Err(UzonError::runtime("std.sort requires exactly 2 arguments", node.span.line, node.span.col));
        }
        let list = self.eval_node(&args[0], scope, exclude)?;
        let func_val = self.eval_node(&args[1], scope, exclude)?;
        self.std_sort(list, func_val, node)
    }

    fn std_is_nan(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
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

    fn std_is_inf(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
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

    fn std_is_finite(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
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

    fn std_split(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        if args.len() != 2 {
            return Err(UzonError::runtime("std.split requires exactly 2 arguments", node.span.line, node.span.col));
        }
        let input = self.eval_node(&args[0], scope, exclude)?;
        let delim = self.eval_node(&args[1], scope, exclude)?;
        match (Self::unwrap_union_owned(input), Self::unwrap_union_owned(delim)) {
            (Value::String(s), Value::String(d)) => {
                // §5.16.4: rules checked in order — first match wins.
                // 1. empty input → [""]
                // 2. empty delimiter → split into Unicode scalar values
                // 3. otherwise → split by delimiter
                let parts: Vec<Value> = if s.is_empty() {
                    vec![Value::String(String::new())]
                } else if d.is_empty() {
                    s.chars().map(|c| Value::String(c.to_string())).collect()
                } else {
                    s.split(&d).map(|p| Value::String(p.to_string())).collect()
                };
                Ok(Value::list(parts))
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

    fn std_trim(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
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

    fn std_replace(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
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

    fn std_lower(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
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

    fn std_upper(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
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

    fn std_join(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
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

    // === Standard library helpers (HOF implementations) ===

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
        Ok(Value::list(results))
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
        Ok(Value::list(results))
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
            Value::List(list) => list.elements,
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
        Ok(Value::list(items))
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
    pub(crate) fn check_return_type_name(
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
