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
            "hasKey" => self.std_has_key(args, scope, exclude, node),
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
            "reverse" => self.std_reverse(args, scope, exclude, node),
            "all" => self.std_all(args, scope, exclude, node),
            "any" => self.std_any(args, scope, exclude, node),
            "contains" => self.std_contains(args, scope, exclude, node),
            "startsWith" => self.std_starts_with(args, scope, exclude, node),
            "endsWith" => self.std_ends_with(args, scope, exclude, node),
            _ => Err(UzonError::runtime(
                format!("unknown standard library function: std.{method}"),
                node.span.line, node.span.col,
            )),
        }
    }

    // === Individual stdlib functions ===

    /// §D.2: evaluate a std function argument, returning RuntimeError for undefined.
    fn eval_std_arg(
        &mut self,
        arg: &Node,
        func_name: &str,
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        let val = self.eval_node(arg, scope, exclude)?;
        if val.is_undefined() {
            return Err(UzonError::runtime(
                format!("std.{func_name} received undefined argument"),
                node.span.line, node.span.col,
            ));
        }
        Ok(val)
    }

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
        let val = self.eval_std_arg(&args[0], "len", scope, exclude, node)?;
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

    /// §5.16.1 v0.8: `std.hasKey(struct, key)` — struct key existence check.
    /// List value checking removed (use `in` operator instead).
    fn std_has_key(
        &mut self,
        args: &[Node],
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        if args.len() != 2 {
            return Err(UzonError::runtime("std.hasKey requires exactly 2 arguments", node.span.line, node.span.col));
        }
        let collection = self.eval_std_arg(&args[0], "hasKey", scope, exclude, node)?;
        let key = self.eval_std_arg(&args[1], "hasKey", scope, exclude, node)?;
        match Self::unwrap_union_owned(collection) {
            Value::Struct(fields) => {
                match key {
                    Value::String(k) => Ok(Value::Bool(fields.contains_key(&k))),
                    other => Err(UzonError::type_error(
                        format!("std.hasKey key must be string, got {}", other.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            other => Err(UzonError::type_error(
                format!("std.hasKey requires struct, got {}", other.type_name()),
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
        let collection = self.eval_std_arg(&args[0], "get", scope, exclude, node)?;
        let key = self.eval_std_arg(&args[1], "get", scope, exclude, node)?;
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
        let val = self.eval_std_arg(&args[0], "keys", scope, exclude, node)?;
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
        let val = self.eval_std_arg(&args[0], "values", scope, exclude, node)?;
        match Self::unwrap_union_owned(val) {
            Value::Struct(s) => {
                let vals: Vec<Value> = s.fields.into_values().collect();
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
        let list = self.eval_std_arg(&args[0], "map", scope, exclude, node)?;
        let func_val = self.eval_std_arg(&args[1], "map", scope, exclude, node)?;
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
        let list = self.eval_std_arg(&args[0], "filter", scope, exclude, node)?;
        let func_val = self.eval_std_arg(&args[1], "filter", scope, exclude, node)?;
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
        let list = self.eval_std_arg(&args[0], "reduce", scope, exclude, node)?;
        let initial = self.eval_std_arg(&args[1], "reduce", scope, exclude, node)?;
        let func_val = self.eval_std_arg(&args[2], "reduce", scope, exclude, node)?;
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
        let list = self.eval_std_arg(&args[0], "sort", scope, exclude, node)?;
        let func_val = self.eval_std_arg(&args[1], "sort", scope, exclude, node)?;
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
        let val = self.eval_std_arg(&args[0], "isNan", scope, exclude, node)?;
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
        let val = self.eval_std_arg(&args[0], "isInf", scope, exclude, node)?;
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
        let val = self.eval_std_arg(&args[0], "isFinite", scope, exclude, node)?;
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
        let input = self.eval_std_arg(&args[0], "split", scope, exclude, node)?;
        let delim = self.eval_std_arg(&args[1], "split", scope, exclude, node)?;
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
        let val = self.eval_std_arg(&args[0], "trim", scope, exclude, node)?;
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
        let input = self.eval_std_arg(&args[0], "replace", scope, exclude, node)?;
        let target = self.eval_std_arg(&args[1], "replace", scope, exclude, node)?;
        let replacement = self.eval_std_arg(&args[2], "replace", scope, exclude, node)?;
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
        let val = self.eval_std_arg(&args[0], "lower", scope, exclude, node)?;
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
        let val = self.eval_std_arg(&args[0], "upper", scope, exclude, node)?;
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
        let list_val = self.eval_std_arg(&args[0], "join", scope, exclude, node)?;
        let sep_val = self.eval_std_arg(&args[1], "join", scope, exclude, node)?;
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

    // §5.16 v0.8: new stdlib functions

    fn std_reverse(
        &mut self, args: &[Node], scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        if args.len() != 1 {
            return Err(UzonError::runtime("std.reverse requires exactly 1 argument", node.span.line, node.span.col));
        }
        let val = self.eval_std_arg(&args[0], "reverse", scope, exclude, node)?;
        match Self::unwrap_union_owned(val) {
            Value::List(items) => {
                let mut elements = items.elements;
                elements.reverse();
                Ok(Value::List(UzonList { elements, element_type: items.element_type }))
            }
            Value::String(s) => {
                Ok(Value::String(s.chars().rev().collect()))
            }
            other => Err(UzonError::type_error(
                format!("std.reverse requires list or string, got {}", other.type_name()),
                node.span.line, node.span.col,
            )),
        }
    }

    fn std_all(
        &mut self, args: &[Node], scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        if args.len() != 2 {
            return Err(UzonError::runtime("std.all requires exactly 2 arguments", node.span.line, node.span.col));
        }
        let list = self.eval_std_arg(&args[0], "all", scope, exclude, node)?;
        let func_val = self.eval_std_arg(&args[1], "all", scope, exclude, node)?;
        let func = match func_val {
            Value::Function(f) => f,
            _ => return Err(UzonError::type_error(
                format!("std.all second argument must be a function, got {}", func_val.type_name()),
                node.span.line, node.span.col,
            )),
        };
        let items = match Self::unwrap_union_owned(list) {
            Value::List(items) => items,
            other => return Err(UzonError::type_error(
                format!("std.all first argument must be a list, got {}", other.type_name()),
                node.span.line, node.span.col,
            )),
        };
        // Empty list → true
        for item in items {
            let result = self.call_function(&func, vec![item], node)?;
            match Self::unwrap_union_owned(result) {
                Value::Bool(false) => return Ok(Value::Bool(false)),
                Value::Bool(true) => {}
                other => return Err(UzonError::type_error(
                    format!("std.all predicate must return bool, got {}", other.type_name()),
                    node.span.line, node.span.col,
                )),
            }
        }
        Ok(Value::Bool(true))
    }

    fn std_any(
        &mut self, args: &[Node], scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        if args.len() != 2 {
            return Err(UzonError::runtime("std.any requires exactly 2 arguments", node.span.line, node.span.col));
        }
        let list = self.eval_std_arg(&args[0], "any", scope, exclude, node)?;
        let func_val = self.eval_std_arg(&args[1], "any", scope, exclude, node)?;
        let func = match func_val {
            Value::Function(f) => f,
            _ => return Err(UzonError::type_error(
                format!("std.any second argument must be a function, got {}", func_val.type_name()),
                node.span.line, node.span.col,
            )),
        };
        let items = match Self::unwrap_union_owned(list) {
            Value::List(items) => items,
            other => return Err(UzonError::type_error(
                format!("std.any first argument must be a list, got {}", other.type_name()),
                node.span.line, node.span.col,
            )),
        };
        // Empty list → false
        for item in items {
            let result = self.call_function(&func, vec![item], node)?;
            match Self::unwrap_union_owned(result) {
                Value::Bool(true) => return Ok(Value::Bool(true)),
                Value::Bool(false) => {}
                other => return Err(UzonError::type_error(
                    format!("std.any predicate must return bool, got {}", other.type_name()),
                    node.span.line, node.span.col,
                )),
            }
        }
        Ok(Value::Bool(false))
    }

    fn std_contains(
        &mut self, args: &[Node], scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        if args.len() != 2 {
            return Err(UzonError::runtime("std.contains requires exactly 2 arguments", node.span.line, node.span.col));
        }
        let input = self.eval_std_arg(&args[0], "contains", scope, exclude, node)?;
        let substr = self.eval_std_arg(&args[1], "contains", scope, exclude, node)?;
        match (Self::unwrap_union_owned(input), Self::unwrap_union_owned(substr)) {
            (Value::String(s), Value::String(sub)) => Ok(Value::Bool(s.contains(&sub))),
            (Value::String(_), other) => Err(UzonError::type_error(
                format!("std.contains second argument must be string, got {}", other.type_name()),
                node.span.line, node.span.col,
            )),
            (other, _) => Err(UzonError::type_error(
                format!("std.contains first argument must be string, got {}", other.type_name()),
                node.span.line, node.span.col,
            )),
        }
    }

    fn std_starts_with(
        &mut self, args: &[Node], scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        if args.len() != 2 {
            return Err(UzonError::runtime("std.startsWith requires exactly 2 arguments", node.span.line, node.span.col));
        }
        let input = self.eval_std_arg(&args[0], "startsWith", scope, exclude, node)?;
        let prefix = self.eval_std_arg(&args[1], "startsWith", scope, exclude, node)?;
        match (Self::unwrap_union_owned(input), Self::unwrap_union_owned(prefix)) {
            (Value::String(s), Value::String(p)) => Ok(Value::Bool(s.starts_with(&p))),
            (Value::String(_), other) => Err(UzonError::type_error(
                format!("std.startsWith second argument must be string, got {}", other.type_name()),
                node.span.line, node.span.col,
            )),
            (other, _) => Err(UzonError::type_error(
                format!("std.startsWith first argument must be string, got {}", other.type_name()),
                node.span.line, node.span.col,
            )),
        }
    }

    fn std_ends_with(
        &mut self, args: &[Node], scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        if args.len() != 2 {
            return Err(UzonError::runtime("std.endsWith requires exactly 2 arguments", node.span.line, node.span.col));
        }
        let input = self.eval_std_arg(&args[0], "endsWith", scope, exclude, node)?;
        let suffix = self.eval_std_arg(&args[1], "endsWith", scope, exclude, node)?;
        match (Self::unwrap_union_owned(input), Self::unwrap_union_owned(suffix)) {
            (Value::String(s), Value::String(suf)) => Ok(Value::Bool(s.ends_with(&suf))),
            (Value::String(_), other) => Err(UzonError::type_error(
                format!("std.endsWith second argument must be string, got {}", other.type_name()),
                node.span.line, node.span.col,
            )),
            (other, _) => Err(UzonError::type_error(
                format!("std.endsWith first argument must be string, got {}", other.type_name()),
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
        // §5.16.2: Stable insertion sort with strict weak ordering.
        // Equality: both comp(a,b) and comp(b,a) return false.
        // Insertion sort is inherently stable — equal elements preserve
        // original order without explicit both-direction checking.
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
            // §D.2: undefined as argument is a runtime error
            if val.is_undefined() {
                return Err(UzonError::runtime(
                    "undefined cannot be passed as a function argument",
                    node.span.line, node.span.col,
                ));
            }
            if let Some(type_name) = param.type_expr.path.last() {
                // §3.8: validate type before coercion
                self.check_type_assertion(&val, type_name, node)?;
                val = Self::coerce_to_param_type(val, type_name);
            }
            func_scope.define(param.name.clone(), val);
        }

        let prev_in_function_body = self.in_function_body;
        self.in_function_body = true;

        self.eval_bindings(&func.body_bindings, &mut func_scope)?;
        let result = self.eval_node(&func.body_expr, &mut func_scope, None)?;

        self.in_function_body = prev_in_function_body;

        // §3.8: check return type assertion
        if let Some(return_type_name) = func.return_type.path.last() {
            self.check_type_assertion(&result, return_type_name, node)?;
            // §6.3: check return type name for named struct types
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
