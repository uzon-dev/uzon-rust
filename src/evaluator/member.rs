// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::scope::Scope;
use crate::value::*;

use super::Evaluator;

impl Evaluator {
    pub(crate) fn eval_member_access(
        &mut self,
        object: &Node,
        member: &str,
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        // env.NAME
        if matches!(object.kind, NodeKind::EnvRef) {
            return Ok(self.env.get(member).map(|s| Value::String(s.clone())).unwrap_or(Value::Undefined));
        }

        let obj = self.eval_node(object, scope, exclude)?;

        if obj.is_undefined() {
            return Ok(Value::Undefined);
        }

        // §5.12: member access on null is an error
        if obj.is_null() {
            return Err(UzonError::runtime(
                "cannot access member on null; null is a value, not a missing state",
                node.span.line, node.span.col,
            ));
        }

        // §3.7.1: unions are transparent for member access
        let obj = Self::unwrap_union_owned(obj);

        match &obj {
            Value::Struct(fields) => {
                Ok(fields.get(member).cloned().unwrap_or(Value::Undefined))
            }
            Value::List(items) => self.access_list_or_tuple(items, member),
            Value::Tuple(t) => self.access_list_or_tuple(&t.elements, member),
            _ => Ok(Value::Undefined),
        }
    }

    /// Access ordinal names (first..tenth) or numeric indices on lists/tuples.
    pub(crate) fn access_list_or_tuple(&self, elements: &[Value], member: &str) -> Result<Value> {
        let idx = match member {
            "first" => Some(0),
            "second" => Some(1),
            "third" => Some(2),
            "fourth" => Some(3),
            "fifth" => Some(4),
            "sixth" => Some(5),
            "seventh" => Some(6),
            "eighth" => Some(7),
            "ninth" => Some(8),
            "tenth" => Some(9),
            _ => member.parse::<usize>().ok(),
        };

        match idx {
            Some(i) if i < elements.len() => Ok(elements[i].clone()),
            _ => Ok(Value::Undefined),
        }
    }
}
