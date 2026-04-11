// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

mod format;
mod numeric;
pub mod ops;
pub mod serde_impl;

pub use format::format_float;
pub use numeric::{FloatType, IntegerType, UzonFloat, UzonInteger};

use std::collections::BTreeMap;
use std::fmt;

use indexmap::IndexMap;

use crate::ast::{Binding, FunctionParam, Node, TypeExpr};
use crate::scope::TypeDef;

/// The sentinel for the UZON `undefined` state (§3.1).
/// Unlike `null`, `undefined` means "does not exist" rather than "intentionally empty."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UzonUndefined;

impl fmt::Display for UzonUndefined {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "undefined")
    }
}

/// A UZON enum value: a selected variant from a set of possible variants (§3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UzonEnum {
    pub value: String,
    pub variants: Vec<String>,
    pub type_name: Option<String>,
}

impl UzonEnum {
    pub fn new(value: impl Into<String>, variants: Vec<String>, type_name: Option<String>) -> Self {
        Self {
            value: value.into(),
            variants,
            type_name,
        }
    }
}

impl fmt::Display for UzonEnum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

/// A UZON untagged union value: a value whose type is one of several possible types (§3.6).
#[derive(Debug, Clone, PartialEq)]
pub struct UzonUnion {
    pub value: Box<Value>,
    pub types: Vec<String>,
    pub type_name: Option<String>,
}

impl UzonUnion {
    pub fn new(value: Value, types: Vec<String>, type_name: Option<String>) -> Self {
        Self {
            value: Box::new(value),
            types,
            type_name,
        }
    }
}

/// A UZON tagged union value: a value with an explicit variant tag (§3.7).
#[derive(Debug, Clone, PartialEq)]
pub struct UzonTaggedUnion {
    pub value: Box<Value>,
    pub tag: String,
    pub variants: BTreeMap<String, Option<String>>,
    pub type_name: Option<String>,
}

impl UzonTaggedUnion {
    pub fn new(
        value: Value,
        tag: impl Into<String>,
        variants: BTreeMap<String, Option<String>>,
        type_name: Option<String>,
    ) -> Self {
        Self {
            value: Box::new(value),
            tag: tag.into(),
            variants,
            type_name,
        }
    }
}

/// A UZON tuple: a fixed-length, heterogeneous sequence (§3.3).
#[derive(Debug, Clone, PartialEq)]
pub struct UzonTuple {
    pub elements: Vec<Value>,
}

impl UzonTuple {
    pub fn new(elements: Vec<Value>) -> Self {
        Self { elements }
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}

/// A UZON list: a homogeneous sequence with optional element type annotation (§3.4).
#[derive(Debug, Clone, PartialEq)]
pub struct UzonList {
    pub elements: Vec<Value>,
    /// Element type stored from `as [Type]` annotations, needed for roundtripping
    /// empty and all-null lists.
    pub element_type: Option<String>,
}

impl UzonList {
    pub fn new(elements: Vec<Value>) -> Self {
        Self { elements, element_type: None }
    }

    pub fn with_type(elements: Vec<Value>, element_type: impl Into<String>) -> Self {
        Self { elements, element_type: Some(element_type.into()) }
    }
}

impl std::ops::Deref for UzonList {
    type Target = Vec<Value>;
    fn deref(&self) -> &Vec<Value> {
        &self.elements
    }
}

impl std::ops::DerefMut for UzonList {
    fn deref_mut(&mut self) -> &mut Vec<Value> {
        &mut self.elements
    }
}

impl IntoIterator for UzonList {
    type Item = Value;
    type IntoIter = std::vec::IntoIter<Value>;
    fn into_iter(self) -> Self::IntoIter {
        self.elements.into_iter()
    }
}

impl<'a> IntoIterator for &'a UzonList {
    type Item = &'a Value;
    type IntoIter = std::slice::Iter<'a, Value>;
    fn into_iter(self) -> Self::IntoIter {
        self.elements.iter()
    }
}

impl FromIterator<Value> for UzonList {
    fn from_iter<I: IntoIterator<Item = Value>>(iter: I) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

/// A UZON function value (closure) (§3.8).
#[derive(Debug, Clone)]
pub struct UzonFunction {
    pub params: Vec<FunctionParam>,
    pub return_type: TypeExpr,
    pub body_bindings: Vec<Binding>,
    pub body_expr: Node,
    pub captured_bindings: BTreeMap<String, Value>,
    pub captured_types: BTreeMap<String, TypeDef>,
    /// Named type assigned via `called` (nominal type identity).
    pub type_name: Option<String>,
}

impl PartialEq for UzonFunction {
    fn eq(&self, _other: &Self) -> bool {
        false // function equality is a type error per §5.2; should never be reached
    }
}

impl fmt::Display for UzonFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<function>")
    }
}

// ============================================================
// Value enum
// ============================================================

/// A UZON value — the core runtime representation.
///
/// Preserves full UZON type information including enums, unions, tagged unions,
/// tuples, and functions. This is the "UZON-native" value type.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Undefined,
    Bool(bool),
    Integer(UzonInteger),
    BigInteger(num_bigint::BigInt),
    Float(UzonFloat),
    String(String),
    List(UzonList),
    Tuple(UzonTuple),
    Struct(IndexMap<String, Value>),
    Enum(UzonEnum),
    Union(UzonUnion),
    TaggedUnion(UzonTaggedUnion),
    Function(UzonFunction),
}

impl Value {
    /// Convenience: create an integer with default type (i64).
    pub fn int(v: i128) -> Self {
        Value::Integer(UzonInteger::new(v))
    }

    /// Convenience: create a float with default type (f64).
    pub fn float(v: f64) -> Self {
        Value::Float(UzonFloat::new(v))
    }

    /// Convenience: create a list from a Vec.
    pub fn list(items: Vec<Value>) -> Self {
        Value::List(UzonList::new(items))
    }

    pub fn is_undefined(&self) -> bool {
        matches!(self, Value::Undefined)
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Convert to a plain Rust-friendly representation, stripping UZON-specific wrappers.
    pub fn to_plain(self) -> Value {
        match self {
            Value::Enum(e) => Value::String(e.value),
            Value::Union(u) => u.value.to_plain(),
            Value::TaggedUnion(tu) => tu.value.to_plain(),
            Value::Function(_) => self,
            Value::Tuple(t) => {
                Value::list(t.elements.into_iter().map(|v| v.to_plain()).collect())
            }
            Value::List(list) => {
                Value::list(list.into_iter().map(|v| v.to_plain()).collect())
            }
            Value::Struct(fields) => {
                let mut result = IndexMap::with_capacity(fields.len());
                for (k, v) in fields {
                    result.insert(k, v.to_plain());
                }
                Value::Struct(result)
            }
            other => other,
        }
    }

    /// Returns the UZON type category name for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Undefined => "undefined",
            Value::Bool(_) => "bool",
            Value::Integer(_) | Value::BigInteger(_) => "integer",
            Value::Float(_) => "float",
            Value::String(_) => "string",
            Value::List(_) => "list",
            Value::Tuple(_) => "tuple",
            Value::Struct(_) => "struct",
            Value::Enum(_) => "enum",
            Value::Union(_) => "union",
            Value::TaggedUnion(_) => "tagged union",
            Value::Function(_) => "function",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Undefined => write!(f, "undefined"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Integer(n) => write!(f, "{}", n.value),
            Value::BigInteger(n) => write!(f, "{n}"),
            Value::Float(v) => write!(f, "{}", format_float(v.value)),
            Value::String(s) => write!(f, "{s}"),
            Value::Enum(e) => write!(f, "{}", e.value),
            Value::Union(u) => write!(f, "{}", u.value),
            Value::TaggedUnion(tu) => write!(f, "{}", tu.value),
            Value::Function(_) => write!(f, "<function>"),
            Value::List(_) | Value::Tuple(_) | Value::Struct(_) => {
                write!(f, "[compound]")
            }
        }
    }
}
