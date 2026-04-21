// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::scope::Scope;
use crate::value::*;

use super::{Evaluator, values_equal, check_structural_compatibility, can_adopt_cross_category};

impl Evaluator {
    pub(crate) fn eval_binary_op(
        &mut self,
        op: BinaryOp,
        left: &Node,
        right: &Node,
        scope: &mut Scope,
        exclude: Option<&str>,
        node: &Node,
    ) -> Result<Value> {
        match op {
            BinaryOp::And => self.eval_binary_and(left, right, scope, exclude, node),
            BinaryOp::Or => self.eval_binary_or(left, right, scope, exclude, node),
            BinaryOp::Is | BinaryOp::IsNot => {
                self.eval_binary_is(op, left, right, scope, exclude, node)
            }
            BinaryOp::IsNamed | BinaryOp::IsNotNamed => {
                self.eval_binary_is_named(op, left, right, scope, exclude, node)
            }
            BinaryOp::In => self.eval_binary_in(left, right, scope, exclude, node),
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
            | BinaryOp::Mod | BinaryOp::Pow => {
                let lv = self.eval_node(left, scope, exclude)?;
                let rv = self.eval_node(right, scope, exclude)?;
                if lv.is_undefined() || rv.is_undefined() {
                    return Err(UzonError::runtime(
                        format!("cannot perform arithmetic on {}; use 'or else' to provide a fallback",
                            Self::describe_undefined(&[(&lv, left), (&rv, right)])),
                        node.span.line, node.span.col,
                    ));
                }
                // §3.7.1: unions are transparent in arithmetic
                let lv_unwrapped = Self::unwrap_union(&lv);
                let rv_unwrapped = Self::unwrap_union(&rv);
                Self::check_numeric_type_compat(lv_unwrapped, rv_unwrapped, node)?;
                self.eval_arithmetic(op, lv_unwrapped, rv_unwrapped, node)
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                let lv = self.eval_node(left, scope, exclude)?;
                let rv = self.eval_node(right, scope, exclude)?;
                if lv.is_undefined() || rv.is_undefined() {
                    return Err(UzonError::runtime(
                        format!("cannot compare {}; use 'or else' to provide a fallback",
                            Self::describe_undefined(&[(&lv, left), (&rv, right)])),
                        node.span.line, node.span.col,
                    ));
                }
                // §5.4 v0.8: ordered comparison on tagged unions, untagged unions, or functions → type error
                if matches!((&lv, &rv), (Value::TaggedUnion(_), Value::TaggedUnion(_))) {
                    return Err(UzonError::type_error(
                        "ordered comparison between two tagged unions is a type error — tags have no defined ordering",
                        node.span.line, node.span.col,
                    ));
                }
                if matches!(&lv, Value::Union(_)) || matches!(&rv, Value::Union(_)) {
                    return Err(UzonError::type_error(
                        "ordered comparison on untagged union is a type error",
                        node.span.line, node.span.col,
                    ));
                }
                if matches!(&lv, Value::Function(_)) || matches!(&rv, Value::Function(_)) {
                    return Err(UzonError::type_error(
                        "ordered comparison on functions is a type error",
                        node.span.line, node.span.col,
                    ));
                }
                let lv = Self::unwrap_union(&lv);
                let rv = Self::unwrap_union(&rv);
                Self::check_numeric_type_compat(lv, rv, node)?;
                self.eval_comparison(op, lv, rv, node)
            }
            BinaryOp::Concat => self.eval_binary_concat(left, right, scope, exclude, node),
            BinaryOp::Repeat => self.eval_binary_repeat(left, right, scope, exclude, node),
            BinaryOp::IsType | BinaryOp::IsNotType => {
                self.eval_binary_is_type(op, left, right, scope, exclude, node)
            }
        }
    }

    fn eval_binary_and(
        &mut self, left: &Node, right: &Node,
        scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        let lv = Self::unwrap_union_owned(self.eval_node(left, scope, exclude)?);
        // §3.1: undefined in logical operators is a runtime error
        if lv.is_undefined() {
            return Err(UzonError::runtime(
                format!("'and' requires bool operands, got {}", Self::describe_undefined(&[(&lv, left)])),
                node.span.line, node.span.col,
            ));
        }
        match lv {
            Value::Bool(false) => {
                // §5.9/§D.5: speculatively evaluate right side — suppress RuntimeError only
                match self.eval_node(right, scope, exclude) {
                    Ok(rv) => {
                        let rv = Self::unwrap_union_owned(rv);
                        self.assert_bool(&rv, node)?;
                    }
                    Err(e) if e.is_runtime() => {}
                    Err(e) => return Err(e),
                }
                Ok(Value::Bool(false))
            }
            Value::Bool(true) => {
                let rv = Self::unwrap_union_owned(self.eval_node(right, scope, exclude)?);
                self.assert_bool(&rv, node)?;
                Ok(rv)
            }
            _ => Err(UzonError::type_error("'and' requires bool operands", node.span.line, node.span.col)),
        }
    }

    fn eval_binary_or(
        &mut self, left: &Node, right: &Node,
        scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        let lv = Self::unwrap_union_owned(self.eval_node(left, scope, exclude)?);
        // §3.1: undefined in logical operators is a runtime error
        if lv.is_undefined() {
            return Err(UzonError::runtime(
                format!("'or' requires bool operands, got {}", Self::describe_undefined(&[(&lv, left)])),
                node.span.line, node.span.col,
            ));
        }
        match lv {
            Value::Bool(true) => {
                // §5.9/§D.5: speculatively evaluate right side — suppress RuntimeError only
                match self.eval_node(right, scope, exclude) {
                    Ok(rv) => {
                        let rv = Self::unwrap_union_owned(rv);
                        self.assert_bool(&rv, node)?;
                    }
                    Err(e) if e.is_runtime() => {}
                    Err(e) => return Err(e),
                }
                Ok(Value::Bool(true))
            }
            Value::Bool(false) => {
                let rv = Self::unwrap_union_owned(self.eval_node(right, scope, exclude)?);
                self.assert_bool(&rv, node)?;
                Ok(rv)
            }
            _ => Err(UzonError::type_error("'or' requires bool operands", node.span.line, node.span.col)),
        }
    }

    fn eval_binary_is(
        &mut self, op: BinaryOp, left: &Node, right: &Node,
        scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        let lv = self.eval_node(left, scope, exclude)?;
        // §3.5 rule 4: enum type-context inference
        let rv = if let Value::Enum(ref e) = lv {
            self.resolve_enum_context(right, e, scope, exclude)?
        } else {
            self.eval_node(right, scope, exclude)?
        };
        // Reverse inference
        let lv = if let Value::Enum(ref e) = rv {
            if let NodeKind::Identifier { ref name } = left.kind {
                if e.variants.contains(name) && !matches!(lv, Value::Enum(_)) {
                    Value::Enum(UzonEnum::new(name.clone(), e.variants.clone(), e.type_name.clone()))
                } else { lv }
            } else { lv }
        } else { lv };

        // §3.8 + §5.2: function equality is a type error — except comparison
        // against `null` or `undefined`, which is permitted by the universal
        // null/undefined exemption and returns bool.
        let has_fn = matches!(&lv, Value::Function(_)) || matches!(&rv, Value::Function(_));
        let other_is_null_or_undef =
            (matches!(&lv, Value::Function(_)) && (rv.is_null() || rv.is_undefined()))
            || (matches!(&rv, Value::Function(_)) && (lv.is_null() || lv.is_undefined()));
        if has_fn && !other_is_null_or_undef {
            return Err(UzonError::type_error(
                "functions cannot be compared for equality",
                node.span.line, node.span.col,
            ));
        }
        if has_fn {
            // function is never equal to null or undefined; is not → true
            return Ok(Value::Bool(op != BinaryOp::Is));
        }

        // §3.7.2: tagged union vs non-tagged-union is type error
        let lv_is_tu = matches!(&lv, Value::TaggedUnion(_));
        let rv_is_tu = matches!(&rv, Value::TaggedUnion(_));
        if !lv.is_null() && !lv.is_undefined() && !rv.is_null() && !rv.is_undefined() {
            if lv_is_tu != rv_is_tu {
                return Err(UzonError::type_error(
                    format!("'is'/'is not' requires same type, got {} and {}", lv.type_name(), rv.type_name()),
                    node.span.line, node.span.col,
                ));
            }
        }

        // §3.6 v0.8: union-to-union comparison
        if let (Value::Union(lu), Value::Union(ru)) = (&lv, &rv) {
            // Check union type identity
            match (&lu.type_name, &ru.type_name) {
                // Both named: nominal identity
                (Some(a), Some(b)) if a != b => {
                    return Err(UzonError::type_error(
                        format!("'is'/'is not' requires same union type, got {} and {}", a, b),
                        node.span.line, node.span.col,
                    ));
                }
                // One named, one anonymous: different types
                (Some(a), None) | (None, Some(a)) => {
                    return Err(UzonError::type_error(
                        format!("'is'/'is not' requires same union type, {} vs anonymous union", a),
                        node.span.line, node.span.col,
                    ));
                }
                // Both anonymous: structural identity (member set, order-independent)
                (None, None) => {
                    let mut l_types = lu.types.clone();
                    let mut r_types = ru.types.clone();
                    l_types.sort();
                    r_types.sort();
                    if l_types != r_types {
                        return Err(UzonError::type_error(
                            format!("'is'/'is not' requires same union type, got union {:?} and union {:?}",
                                lu.types, ru.types),
                            node.span.line, node.span.col,
                        ));
                    }
                }
                _ => {} // Both named and matching
            }
            // Same union type: compare inner values
            let lv_inner = Self::unwrap_union_owned(lv);
            let rv_inner = Self::unwrap_union_owned(rv);
            // Different runtime types → false (not error)
            if lv_inner.type_name() != rv_inner.type_name()
                && !can_adopt_cross_category(&lv_inner, &rv_inner)
            {
                return Ok(Value::Bool(op != BinaryOp::Is));
            }
            let eq = values_equal(&lv_inner, &rv_inner);
            return Ok(Value::Bool(if op == BinaryOp::Is { eq } else { !eq }));
        }

        // §3.6: untagged unions transparent for is/is not with non-union values
        let lv = if matches!(&lv, Value::Union(_)) { Self::unwrap_union_owned(lv) } else { lv };
        let rv = if matches!(&rv, Value::Union(_)) { Self::unwrap_union_owned(rv) } else { rv };

        if !lv.is_null() && !lv.is_undefined() && !rv.is_null() && !rv.is_undefined() {
            if lv.type_name() != rv.type_name() && !can_adopt_cross_category(&lv, &rv) {
                return Err(UzonError::type_error(
                    format!("'is'/'is not' requires same type, got {} and {}", lv.type_name(), rv.type_name()),
                    node.span.line, node.span.col,
                ));
            }
            check_structural_compatibility(&lv, &rv, node)?;
        }
        let eq = values_equal(&lv, &rv);
        Ok(Value::Bool(if op == BinaryOp::Is { eq } else { !eq }))
    }

    fn eval_binary_is_named(
        &mut self, op: BinaryOp, left: &Node, right: &Node,
        scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        let lv = self.eval_node(left, scope, exclude)?;
        // §3.1: undefined in 'is named' is a runtime error
        if lv.is_undefined() {
            return Err(UzonError::runtime(
                format!("'is named' requires tagged union, got {}", Self::describe_undefined(&[(&lv, left)])),
                node.span.line, node.span.col,
            ));
        }
        let tag_name = match &right.kind {
            NodeKind::Identifier { name } => name.as_str(),
            _ => return Err(UzonError::syntax("expected variant name after 'is named'", node.span.line, node.span.col)),
        };
        match &lv {
            Value::TaggedUnion(tu) => {
                if !tu.variants.contains_key(tag_name) {
                    return Err(UzonError::type_error(
                        format!("'{}' is not a valid variant of this tagged union", tag_name),
                        node.span.line, node.span.col,
                    ));
                }
                let matches = tu.tag == tag_name;
                Ok(Value::Bool(if op == BinaryOp::IsNamed { matches } else { !matches }))
            }
            _ => Err(UzonError::type_error(
                format!("'is named' requires tagged union, got {}", lv.type_name()),
                node.span.line, node.span.col,
            )),
        }
    }

    fn eval_binary_in(
        &mut self, left: &Node, right: &Node,
        scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        let rv = self.eval_node(right, scope, exclude)?;
        // §3.5 rule 4: enum type-context inference — list elements only (not tuple/struct)
        let lv = if let Value::List(ref items) = rv {
            let first_enum = items.iter().find_map(|item| {
                if let Value::Enum(e) = item { Some(e) } else { None }
            });
            if let Some(enum_val) = first_enum {
                self.resolve_enum_context(left, enum_val, scope, exclude)?
            } else {
                self.eval_node(left, scope, exclude)?
            }
        } else {
            self.eval_node(left, scope, exclude)?
        };
        if lv.is_undefined() || rv.is_undefined() {
            return Err(UzonError::runtime(
                format!("cannot use 'in' with {}; use 'or else' to provide a fallback",
                    Self::describe_undefined(&[(&lv, left), (&rv, right)])),
                node.span.line, node.span.col,
            ));
        }
        let lv = Self::unwrap_union_owned(lv);
        let rv = Self::unwrap_union_owned(rv);
        // §3.8: function equality is always a type error
        if matches!(&lv, Value::Function(_)) {
            return Err(UzonError::type_error(
                "function values cannot be compared with 'in'",
                node.span.line, node.span.col,
            ));
        }
        match rv {
            Value::List(items) => {
                if !lv.is_null() && !items.is_empty() {
                    let first_non_null = items.iter().find(|i| !i.is_null());
                    if let Some(elem) = first_non_null {
                        if lv.type_name() != elem.type_name() {
                            return Err(UzonError::type_error(
                                format!("'in' requires value and list elements to be the same type, got {} and [{}]",
                                    lv.type_name(), elem.type_name()),
                                node.span.line, node.span.col,
                            ));
                        }
                        // §6.3: enum nominal type check
                        if let (Value::Enum(le), Value::Enum(ee)) = (&lv, elem) {
                            if le.type_name != ee.type_name {
                                return Err(UzonError::type_error(
                                    format!("'in' requires same enum type, got {} and {}",
                                        le.type_name.as_deref().unwrap_or("anonymous"),
                                        ee.type_name.as_deref().unwrap_or("anonymous")),
                                    node.span.line, node.span.col,
                                ));
                            }
                        }
                        // D.3: numeric type_ann compatibility
                        Self::check_in_numeric_compat(&lv, elem, node)?;
                    }
                }
                let found = items.iter().any(|item| values_equal(&lv, item));
                Ok(Value::Bool(found))
            }
            Value::Tuple(t) => {
                // §5.8.1: tuple — heterogeneous, type mismatch elements are skipped (no error)
                let found = t.elements.iter().any(|elem| {
                    if elem.is_undefined() { return false; }
                    values_equal(&lv, elem)
                });
                Ok(Value::Bool(found))
            }
            Value::Struct(fields) => {
                // §5.8.1: struct — value membership (not key). Key check is std.hasKey.
                let found = fields.values().any(|val| {
                    if val.is_undefined() { return false; }
                    values_equal(&lv, val)
                });
                Ok(Value::Bool(found))
            }
            _ => Err(UzonError::type_error(
                format!("'in' requires list, tuple, or struct on the right, got {}", rv.type_name()),
                node.span.line, node.span.col,
            )),
        }
    }

    /// D.3: numeric type_ann compatibility check for `in` operator.
    fn check_in_numeric_compat(lv: &Value, elem: &Value, node: &Node) -> Result<()> {
        match (lv, elem) {
            (Value::Integer(li), Value::Integer(ei)) => {
                if li.explicit && li.type_ann != ei.type_ann {
                    return Err(UzonError::type_error(
                        format!("'in' requires value and list elements to be the same type, got {} and {}",
                            li.type_ann.display_name(), ei.type_ann.display_name()),
                        node.span.line, node.span.col,
                    ));
                }
                if ei.explicit && !li.explicit && li.type_ann != ei.type_ann {
                    // untyped adopts typed → ok
                } else if let Err(msg) = UzonInteger::adopt_type(&li.type_ann, &ei.type_ann) {
                    return Err(UzonError::type_error(
                        format!("'in' requires value and list elements to be the same type: {msg}"),
                        node.span.line, node.span.col,
                    ));
                }
            }
            (Value::Float(lf), Value::Float(ef)) => {
                if lf.explicit && lf.type_ann != ef.type_ann {
                    return Err(UzonError::type_error(
                        format!("'in' requires value and list elements to be the same type, got {} and {}",
                            lf.type_ann.display_name(), ef.type_ann.display_name()),
                        node.span.line, node.span.col,
                    ));
                }
                if ef.explicit && !lf.explicit && lf.type_ann != ef.type_ann {
                    // untyped adopts typed → ok
                } else if let Err(msg) = UzonFloat::adopt_type(&lf.type_ann, &ef.type_ann) {
                    return Err(UzonError::type_error(
                        format!("'in' requires value and list elements to be the same type: {msg}"),
                        node.span.line, node.span.col,
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn eval_binary_concat(
        &mut self, left: &Node, right: &Node,
        scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        let lv = self.eval_node(left, scope, exclude)?;
        let rv = self.eval_node(right, scope, exclude)?;
        if lv.is_undefined() || rv.is_undefined() {
            return Err(UzonError::runtime(
                format!("cannot concatenate {}; use 'or else' to provide a fallback",
                    Self::describe_undefined(&[(&lv, left), (&rv, right)])),
                node.span.line, node.span.col,
            ));
        }
        let lv = Self::unwrap_union_owned(lv);
        let rv = Self::unwrap_union_owned(rv);
        match (&lv, &rv) {
            (Value::String(a), Value::String(b)) => Ok(Value::String(format!("{a}{b}"))),
            (Value::List(a), Value::List(b)) => {
                if !a.is_empty() && !b.is_empty() {
                    let a_elem = a.iter().find(|v| !v.is_null());
                    let b_elem = b.iter().find(|v| !v.is_null());
                    if let (Some(av), Some(bv)) = (a_elem, b_elem) {
                        if av.type_name() != bv.type_name() {
                            return Err(UzonError::type_error(
                                format!("'++' requires lists with same element type, got [{}] ++ [{}]",
                                    av.type_name(), bv.type_name()),
                                node.span.line, node.span.col,
                            ));
                        }
                        match (av, bv) {
                            (Value::Integer(ai), Value::Integer(bi)) => {
                                if let Err(msg) = UzonInteger::adopt_type(&ai.type_ann, &bi.type_ann) {
                                    return Err(UzonError::type_error(
                                        format!("'++' requires lists with same element type: {msg}"),
                                        node.span.line, node.span.col,
                                    ));
                                }
                            }
                            (Value::Float(af), Value::Float(bf)) => {
                                if let Err(msg) = UzonFloat::adopt_type(&af.type_ann, &bf.type_ann) {
                                    return Err(UzonError::type_error(
                                        format!("'++' requires lists with same element type: {msg}"),
                                        node.span.line, node.span.col,
                                    ));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                let mut result = a.elements.clone();
                result.extend(b.iter().cloned());
                Ok(Value::list(result))
            }
            _ => Err(UzonError::type_error(
                format!("'++' requires string or list operands, got {} and {}", lv.type_name(), rv.type_name()),
                node.span.line, node.span.col,
            )),
        }
    }

    fn eval_binary_repeat(
        &mut self, left: &Node, right: &Node,
        scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        let lv = self.eval_node(left, scope, exclude)?;
        let rv = self.eval_node(right, scope, exclude)?;
        if lv.is_undefined() || rv.is_undefined() {
            return Err(UzonError::runtime(
                format!("cannot repeat {}; use 'or else' to provide a fallback",
                    Self::describe_undefined(&[(&lv, left), (&rv, right)])),
                node.span.line, node.span.col,
            ));
        }
        let lv = Self::unwrap_union_owned(lv);
        let rv = Self::unwrap_union_owned(rv);
        let count = match &rv {
            Value::Integer(n) => {
                // Bug fix absorbed: `** 0` produces empty result (n.value < 0 rejects negatives only)
                if n.value < 0 {
                    return Err(UzonError::runtime("repetition count must be a non-negative integer (≥ 0)", node.span.line, node.span.col));
                }
                n.value as usize
            }
            _ => return Err(UzonError::type_error("'**' requires integer right operand", node.span.line, node.span.col)),
        };
        match &lv {
            Value::String(s) => Ok(Value::String(s.repeat(count))),
            Value::List(items) => {
                let mut result = Vec::with_capacity(items.len() * count);
                for _ in 0..count {
                    result.extend(items.iter().cloned());
                }
                // Preserve element_type; infer from elements if not explicit
                // (important for count=0 to produce typed empty list)
                let element_type = items.element_type.clone().or_else(|| {
                    if result.is_empty() {
                        items.iter().find(|v| !v.is_null()).map(|v| Evaluator::specific_type_name(v))
                    } else {
                        None
                    }
                });
                Ok(Value::List(UzonList {
                    elements: result,
                    element_type,
                    type_name: items.type_name.clone(),
                }))
            }
            _ => Err(UzonError::type_error(
                format!("'**' requires string or list left operand, got {}", lv.type_name()),
                node.span.line, node.span.col,
            )),
        }
    }

    /// `is type` / `is not type` — runtime type check (§3.6, §5.2).
    ///
    /// For untagged unions, checks the inner value's type.
    /// For other values, checks the value's concrete type.
    fn eval_binary_is_type(
        &mut self, op: BinaryOp, left: &Node, right: &Node,
        scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        let lv = self.eval_node(left, scope, exclude)?;
        // §3.1: undefined in 'is type' is a runtime error
        if lv.is_undefined() {
            return Err(UzonError::runtime(
                format!("'is type' requires a concrete value, got {}",
                    Self::describe_undefined(&[(&lv, left)])),
                node.span.line, node.span.col,
            ));
        }
        let type_name = match &right.kind {
            NodeKind::Identifier { name } => name.as_str(),
            _ => return Err(UzonError::syntax(
                "expected type name after 'is type'",
                node.span.line, node.span.col,
            )),
        };
        // For unions (tagged or untagged), check the inner value's type
        let inner = Self::unwrap_union(&lv);
        let actual_type = Self::compound_type_name(inner);
        let matches = actual_type == type_name;
        Ok(Value::Bool(if op == BinaryOp::IsType { matches } else { !matches }))
    }

    /// §5.3/D.3: numeric type compatibility check with adoption rule.
    pub(crate) fn check_numeric_type_compat(lv: &Value, rv: &Value, node: &Node) -> Result<()> {
        match (lv, rv) {
            (Value::Integer(a), Value::Integer(b)) => {
                if a.explicit && b.explicit && a.type_ann != b.type_ann {
                    return Err(UzonError::type_error(
                        format!("same numeric type required, got {} and {}; use 'to' for explicit conversion",
                            a.type_ann.display_name(), b.type_ann.display_name()),
                        node.span.line, node.span.col,
                    ));
                }
                UzonInteger::adopt_type(&a.type_ann, &b.type_ann).map_err(|_| {
                    UzonError::type_error(
                        format!("same numeric type required, got {} and {}; use 'to' for explicit conversion",
                            a.type_ann.display_name(), b.type_ann.display_name()),
                        node.span.line, node.span.col,
                    )
                })?;
                Ok(())
            }
            (Value::Float(a), Value::Float(b)) => {
                if a.explicit && b.explicit && a.type_ann != b.type_ann {
                    return Err(UzonError::type_error(
                        format!("same numeric type required, got {} and {}; use 'to' for explicit conversion",
                            a.type_ann.display_name(), b.type_ann.display_name()),
                        node.span.line, node.span.col,
                    ));
                }
                UzonFloat::adopt_type(&a.type_ann, &b.type_ann).map_err(|_| {
                    UzonError::type_error(
                        format!("same numeric type required, got {} and {}; use 'to' for explicit conversion",
                            a.type_ann.display_name(), b.type_ann.display_name()),
                        node.span.line, node.span.col,
                    )
                })?;
                Ok(())
            }
            // §5 line 1220: cross-category int→float adoption
            (Value::Integer(i), Value::Float(_)) if !i.explicit => Ok(()),
            (Value::Float(_), Value::Integer(i)) if !i.explicit => Ok(()),
            _ => Ok(()),
        }
    }

    pub(crate) fn eval_arithmetic(&self, op: BinaryOp, lv: &Value, rv: &Value, node: &Node) -> Result<Value> {
        match (lv, rv) {
            (Value::Integer(a), Value::Integer(b)) => self.int_arithmetic(op, a, b, node),
            (Value::Float(a), Value::Float(b)) => self.float_arithmetic(op, a, b, node),
            // §5 line 1220: cross-category int→float adoption
            (Value::Integer(a), Value::Float(b)) if !a.explicit => {
                let promoted = UzonFloat { value: a.value as f64, type_ann: b.type_ann, explicit: false };
                self.float_arithmetic(op, &promoted, b, node)
            }
            (Value::Float(a), Value::Integer(b)) if !b.explicit => {
                let promoted = UzonFloat { value: b.value as f64, type_ann: a.type_ann, explicit: false };
                self.float_arithmetic(op, a, &promoted, node)
            }
            (Value::BigInteger(_), _) | (_, Value::BigInteger(_)) => Err(UzonError::runtime(
                "arithmetic on integers beyond i128 range is not supported; use 'as i128' or narrower types",
                node.span.line, node.span.col,
            )),
            _ => Err(UzonError::type_error(
                format!("arithmetic requires same numeric type, got {} and {}", lv.type_name(), rv.type_name()),
                node.span.line, node.span.col,
            )),
        }
    }

    pub(crate) fn int_arithmetic(&self, op: BinaryOp, a: &UzonInteger, b: &UzonInteger, node: &Node) -> Result<Value> {
        let result = match op {
            BinaryOp::Add => a.checked_add(b),
            BinaryOp::Sub => a.checked_sub(b),
            BinaryOp::Mul => a.checked_mul(b),
            BinaryOp::Div => a.checked_div(b),
            BinaryOp::Mod => a.checked_rem(b),
            BinaryOp::Pow => a.checked_pow(b),
            _ => unreachable!(),
        };
        result.map(Value::Integer)
            .map_err(|msg| UzonError::runtime(msg, node.span.line, node.span.col))
    }

    pub(crate) fn float_arithmetic(&self, op: BinaryOp, a: &UzonFloat, b: &UzonFloat, node: &Node) -> Result<Value> {
        let result = match op {
            BinaryOp::Add => a.add(b),
            BinaryOp::Sub => a.sub(b),
            BinaryOp::Mul => a.mul(b),
            BinaryOp::Div => a.div(b),
            BinaryOp::Mod => a.rem(b),
            BinaryOp::Pow => a.powf(b),
            _ => unreachable!(),
        };
        result.map(Value::Float)
            .map_err(|msg| UzonError::runtime(msg, node.span.line, node.span.col))
    }

    pub(crate) fn eval_comparison(&self, op: BinaryOp, lv: &Value, rv: &Value, node: &Node) -> Result<Value> {
        match (lv, rv) {
            (Value::Integer(a), Value::Integer(b)) => {
                let r = match op {
                    BinaryOp::Lt => a.value < b.value,
                    BinaryOp::Le => a.value <= b.value,
                    BinaryOp::Gt => a.value > b.value,
                    BinaryOp::Ge => a.value >= b.value,
                    _ => unreachable!(),
                };
                Ok(Value::Bool(r))
            }
            (Value::Float(a), Value::Float(b)) => {
                let r = match op {
                    BinaryOp::Lt => a.value < b.value,
                    BinaryOp::Le => a.value <= b.value,
                    BinaryOp::Gt => a.value > b.value,
                    BinaryOp::Ge => a.value >= b.value,
                    _ => unreachable!(),
                };
                Ok(Value::Bool(r))
            }
            (Value::String(a), Value::String(b)) => {
                let r = match op {
                    BinaryOp::Lt => a < b,
                    BinaryOp::Le => a <= b,
                    BinaryOp::Gt => a > b,
                    BinaryOp::Ge => a >= b,
                    _ => unreachable!(),
                };
                Ok(Value::Bool(r))
            }
            // §5 line 1220: cross-category int→float adoption for comparison
            (Value::Integer(a), Value::Float(b)) if !a.explicit => {
                let promoted = a.value as f64;
                let r = match op {
                    BinaryOp::Lt => promoted < b.value,
                    BinaryOp::Le => promoted <= b.value,
                    BinaryOp::Gt => promoted > b.value,
                    BinaryOp::Ge => promoted >= b.value,
                    _ => unreachable!(),
                };
                Ok(Value::Bool(r))
            }
            (Value::Float(a), Value::Integer(b)) if !b.explicit => {
                let promoted = b.value as f64;
                let r = match op {
                    BinaryOp::Lt => a.value < promoted,
                    BinaryOp::Le => a.value <= promoted,
                    BinaryOp::Gt => a.value > promoted,
                    BinaryOp::Ge => a.value >= promoted,
                    _ => unreachable!(),
                };
                Ok(Value::Bool(r))
            }
            _ => {
                // §5.4: distinguish "different types" from "type doesn't support ordering"
                if lv.type_name() == rv.type_name() {
                    Err(UzonError::type_error(
                        format!("ordered comparison not supported for {}", lv.type_name()),
                        node.span.line, node.span.col,
                    ))
                } else {
                    Err(UzonError::type_error(
                        format!("comparison requires same type, got {} and {}", lv.type_name(), rv.type_name()),
                        node.span.line, node.span.col,
                    ))
                }
            }
        }
    }

    pub(crate) fn eval_unary_op(
        &mut self, op: UnaryOp, operand: &Node,
        scope: &mut Scope, exclude: Option<&str>, node: &Node,
    ) -> Result<Value> {
        let val = self.eval_node(operand, scope, exclude)?;
        let val = Self::unwrap_union_owned(val);
        match op {
            UnaryOp::Neg => {
                // §3.1: undefined in arithmetic is a runtime error
                if val.is_undefined() {
                    return Err(UzonError::runtime(
                        format!("unary '-' requires numeric operand, got {}",
                            Self::describe_undefined(&[(&val, operand)])),
                        node.span.line, node.span.col,
                    ));
                }
                match val {
                    Value::Integer(n) => {
                        n.checked_neg().map(Value::Integer)
                            .map_err(|msg| UzonError::runtime(msg, node.span.line, node.span.col))
                    }
                    Value::Float(f) => Ok(Value::Float(f.neg())),
                    _ => Err(UzonError::type_error(
                        format!("unary '-' requires numeric operand, got {}", val.type_name()),
                        node.span.line, node.span.col,
                    )),
                }
            }
            UnaryOp::Not => {
                // §3.1: undefined in logical operators is a runtime error
                if val.is_undefined() {
                    return Err(UzonError::runtime(
                        format!("'not' requires bool operand, got {}",
                            Self::describe_undefined(&[(&val, operand)])),
                        node.span.line, node.span.col,
                    ));
                }
                match val {
                    Value::Bool(b) => Ok(Value::Bool(!b)),
                    _ => Err(UzonError::type_error("'not' requires bool operand", node.span.line, node.span.col)),
                }
            },
        }
    }
}
