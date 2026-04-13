// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! Rust-native operator traits and accessor methods for [`Value`].
//!
//! Provides idiomatic Rust access to parsed UZON values:
//! - **Accessors**: `as_bool()`, `as_i64()`, `as_i128()`, `as_f64()`, `as_str()`, etc.
//! - **Indexing**: `value["key"]` for structs, `value[0]` for lists/tuples
//! - **From/Into**: `Value::from(42)`, `Value::from("hello")`, etc.
//! - **Arithmetic**: `+`, `-`, `*`, `/`, `%`, unary `-`
//! - **Comparison**: `PartialOrd` for numeric values

use std::ops;

use indexmap::IndexMap;

use super::{
    UzonFloat, UzonInteger, UzonList, UzonTuple,
    Value,
};

// ============================================================
// Accessor methods
// ============================================================

impl Value {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Integer(n) => i64::try_from(n.value).ok(),
            Value::BigInteger(n) => {
                use num_traits::ToPrimitive;
                n.to_i64()
            }
            _ => None,
        }
    }

    pub fn as_i128(&self) -> Option<i128> {
        match self {
            Value::Integer(n) => Some(n.value),
            Value::BigInteger(n) => {
                use num_traits::ToPrimitive;
                n.to_i128()
            }
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(f.value),
            Value::Integer(n) => Some(n.value as f64),
            Value::BigInteger(n) => {
                use num_traits::ToPrimitive;
                n.to_f64()
            }
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(l) => Some(&l.elements),
            _ => None,
        }
    }

    pub fn as_list_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            Value::List(l) => Some(&mut l.elements),
            _ => None,
        }
    }

    pub fn as_tuple(&self) -> Option<&[Value]> {
        match self {
            Value::Tuple(t) => Some(&t.elements),
            _ => None,
        }
    }

    pub fn as_tuple_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            Value::Tuple(t) => Some(&mut t.elements),
            _ => None,
        }
    }

    pub fn as_struct(&self) -> Option<&IndexMap<String, Value>> {
        match self {
            Value::Struct(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_struct_mut(&mut self) -> Option<&mut IndexMap<String, Value>> {
        match self {
            Value::Struct(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<&UzonInteger> {
        match self {
            Value::Integer(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<&UzonFloat> {
        match self {
            Value::Float(f) => Some(f),
            _ => None,
        }
    }


    /// Get a struct field by key. Returns `None` if not a struct or key missing.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Struct(map) => map.get(key),
            _ => None,
        }
    }

    /// Get a list/tuple element by index. Returns `None` if out of bounds or wrong type.
    pub fn get_index(&self, index: usize) -> Option<&Value> {
        match self {
            Value::List(l) => l.elements.get(index),
            Value::Tuple(t) => t.elements.get(index),
            _ => None,
        }
    }

    /// Get a mutable reference to a struct field.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        match self {
            Value::Struct(map) => map.get_mut(key),
            _ => None,
        }
    }

    /// Get a mutable reference to a list/tuple element.
    pub fn get_index_mut(&mut self, index: usize) -> Option<&mut Value> {
        match self {
            Value::List(l) => l.elements.get_mut(index),
            Value::Tuple(t) => t.elements.get_mut(index),
            _ => None,
        }
    }

    // --- Mutation ---

    /// Insert or update a field in a struct. Returns the previous value if the key existed.
    /// Panics if not a struct.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<Value>) -> Option<Value> {
        match self {
            Value::Struct(map) => map.insert(key.into(), value.into()),
            _ => panic!("insert: expected struct, got {}", self.type_name()),
        }
    }

    /// Remove a field from a struct. Returns the removed value.
    /// Panics if not a struct.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        match self {
            Value::Struct(map) => map.shift_remove(key),
            _ => panic!("remove: expected struct, got {}", self.type_name()),
        }
    }

    /// Push a value onto a list.
    /// Panics if not a list.
    pub fn push(&mut self, value: impl Into<Value>) {
        match self {
            Value::List(l) => l.elements.push(value.into()),
            _ => panic!("push: expected list, got {}", self.type_name()),
        }
    }

    /// Pop the last value from a list.
    /// Panics if not a list.
    pub fn pop(&mut self) -> Option<Value> {
        match self {
            Value::List(l) => l.elements.pop(),
            _ => panic!("pop: expected list, got {}", self.type_name()),
        }
    }

    /// Deep-merge `other` into `self`. Both must be structs.
    ///
    /// - Matching struct fields are merged recursively.
    /// - Non-struct fields in `other` overwrite fields in `self`.
    /// - Fields only in `other` are added.
    /// - Fields only in `self` are kept.
    ///
    /// ```ignore
    /// let mut base = uzon!({ "a": 1, "nested": { "x": 10, "y": 20 } });
    /// let overlay = uzon!({ "a": 2, "nested": { "y": 99, "z": 30 } });
    /// base.merge(overlay);
    /// // base = { a: 2, nested: { x: 10, y: 99, z: 30 } }
    /// ```
    pub fn merge(&mut self, other: Value) {
        // Unwrap union wrappers before merging.
        let other = match other {
            Value::Union(u) => *u.value,
            Value::TaggedUnion(tu) => *tu.value,
            v => v,
        };
        let self_inner = match self {
            Value::Union(u) => &mut *u.value,
            Value::TaggedUnion(tu) => &mut *tu.value,
            v => v,
        };
        match (self_inner, other) {
            (Value::Struct(base), Value::Struct(overlay)) => {
                for (key, oval) in overlay {
                    if let Some(bval) = base.get_mut(&key) {
                        if matches!(bval, Value::Struct(_)) && matches!(oval, Value::Struct(_)) {
                            bval.merge(oval);
                            continue;
                        }
                    }
                    base.insert(key, oval);
                }
            }
            _ => {}
        }
    }

    /// Navigate a dot-separated path into nested structs.
    ///
    /// ```ignore
    /// value.get_path("server.host") // == value.get("server").and_then(|v| v.get("host"))
    /// ```
    pub fn get_path(&self, path: &str) -> Option<&Value> {
        let mut current = self;
        for segment in path.split('.') {
            // Try struct key first, then numeric index
            if let Some(v) = current.get(segment) {
                current = v;
            } else if let Ok(idx) = segment.parse::<usize>() {
                current = current.get_index(idx)?;
            } else {
                return None;
            }
        }
        Some(current)
    }
}

// ============================================================
// Index traits
// ============================================================

static NULL_SENTINEL: Value = Value::Null;

impl ops::Index<&str> for Value {
    type Output = Value;

    fn index(&self, key: &str) -> &Value {
        match self {
            Value::Struct(map) => map.get(key).unwrap_or(&NULL_SENTINEL),
            _ => &NULL_SENTINEL,
        }
    }
}

impl ops::Index<usize> for Value {
    type Output = Value;

    fn index(&self, index: usize) -> &Value {
        match self {
            Value::List(l) => l.elements.get(index).unwrap_or(&NULL_SENTINEL),
            Value::Tuple(t) => t.elements.get(index).unwrap_or(&NULL_SENTINEL),
            _ => &NULL_SENTINEL,
        }
    }
}

// ============================================================
// From / Into conversions
// ============================================================

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::Integer(UzonInteger::new(v as i128))
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Integer(UzonInteger::new(v as i128))
    }
}

impl From<i128> for Value {
    fn from(v: i128) -> Self {
        Value::Integer(UzonInteger::new(v))
    }
}

impl From<u32> for Value {
    fn from(v: u32) -> Self {
        Value::Integer(UzonInteger::new(v as i128))
    }
}

impl From<u64> for Value {
    fn from(v: u64) -> Self {
        Value::Integer(UzonInteger::new(v as i128))
    }
}

impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Value::Float(UzonFloat::new(v as f64))
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(UzonFloat::new(v))
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::String(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::String(v.to_string())
    }
}

impl From<Vec<Value>> for Value {
    fn from(v: Vec<Value>) -> Self {
        Value::List(UzonList::new(v))
    }
}

impl From<IndexMap<String, Value>> for Value {
    fn from(v: IndexMap<String, Value>) -> Self {
        Value::Struct(crate::value::UzonStruct::new(v))
    }
}

impl From<(Value, Value)> for Value {
    fn from(v: (Value, Value)) -> Self {
        Value::Tuple(UzonTuple::new(vec![v.0, v.1]))
    }
}

impl From<(Value, Value, Value)> for Value {
    fn from(v: (Value, Value, Value)) -> Self {
        Value::Tuple(UzonTuple::new(vec![v.0, v.1, v.2]))
    }
}

// ============================================================
// TryFrom — Value into Rust types
// ============================================================

/// Error returned when a `Value` cannot be converted to the requested Rust type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueConversionError {
    pub from: &'static str,
    pub to: &'static str,
}

impl std::fmt::Display for ValueConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot convert {} to {}", self.from, self.to)
    }
}

impl std::error::Error for ValueConversionError {}

impl TryFrom<Value> for bool {
    type Error = ValueConversionError;
    fn try_from(v: Value) -> Result<Self, Self::Error> {
        match v {
            Value::Bool(b) => Ok(b),
            _ => Err(ValueConversionError { from: v.type_name(), to: "bool" }),
        }
    }
}

impl TryFrom<Value> for i64 {
    type Error = ValueConversionError;
    fn try_from(v: Value) -> Result<Self, Self::Error> {
        match v {
            Value::Integer(n) => i64::try_from(n.value)
                .map_err(|_| ValueConversionError { from: "integer (out of i64 range)", to: "i64" }),
            Value::BigInteger(n) => {
                use num_traits::ToPrimitive;
                n.to_i64().ok_or(ValueConversionError { from: "integer (out of i64 range)", to: "i64" })
            }
            _ => Err(ValueConversionError { from: v.type_name(), to: "i64" }),
        }
    }
}

impl TryFrom<Value> for i128 {
    type Error = ValueConversionError;
    fn try_from(v: Value) -> Result<Self, Self::Error> {
        match v {
            Value::Integer(n) => Ok(n.value),
            Value::BigInteger(n) => {
                use num_traits::ToPrimitive;
                n.to_i128().ok_or(ValueConversionError { from: "integer (out of i128 range)", to: "i128" })
            }
            _ => Err(ValueConversionError { from: v.type_name(), to: "i128" }),
        }
    }
}

impl TryFrom<Value> for u64 {
    type Error = ValueConversionError;
    fn try_from(v: Value) -> Result<Self, Self::Error> {
        match v {
            Value::Integer(n) => u64::try_from(n.value)
                .map_err(|_| ValueConversionError { from: "integer (out of u64 range)", to: "u64" }),
            Value::BigInteger(n) => {
                use num_traits::ToPrimitive;
                n.to_u64().ok_or(ValueConversionError { from: "integer (out of u64 range)", to: "u64" })
            }
            _ => Err(ValueConversionError { from: v.type_name(), to: "u64" }),
        }
    }
}

impl TryFrom<Value> for f64 {
    type Error = ValueConversionError;
    fn try_from(v: Value) -> Result<Self, Self::Error> {
        match v {
            Value::Float(f) => Ok(f.value),
            Value::Integer(n) => Ok(n.value as f64),
            Value::BigInteger(n) => {
                use num_traits::ToPrimitive;
                n.to_f64().ok_or(ValueConversionError { from: "integer (out of f64 range)", to: "f64" })
            }
            _ => Err(ValueConversionError { from: v.type_name(), to: "f64" }),
        }
    }
}

impl TryFrom<Value> for String {
    type Error = ValueConversionError;
    fn try_from(v: Value) -> Result<Self, Self::Error> {
        match v {
            Value::String(s) => Ok(s),
            _ => Err(ValueConversionError { from: v.type_name(), to: "String" }),
        }
    }
}

impl TryFrom<Value> for Vec<Value> {
    type Error = ValueConversionError;
    fn try_from(v: Value) -> Result<Self, Self::Error> {
        match v {
            Value::List(l) => Ok(l.elements),
            Value::Tuple(t) => Ok(t.elements),
            _ => Err(ValueConversionError { from: v.type_name(), to: "Vec<Value>" }),
        }
    }
}

impl TryFrom<Value> for IndexMap<String, Value> {
    type Error = ValueConversionError;
    fn try_from(v: Value) -> Result<Self, Self::Error> {
        match v {
            Value::Struct(m) => Ok(m.fields),
            _ => Err(ValueConversionError { from: v.type_name(), to: "IndexMap<String, Value>" }),
        }
    }
}

// Also support TryFrom<&Value> for Copy types and &str

impl<'a> TryFrom<&'a Value> for bool {
    type Error = ValueConversionError;
    fn try_from(v: &'a Value) -> Result<Self, Self::Error> {
        match v {
            Value::Bool(b) => Ok(*b),
            _ => Err(ValueConversionError { from: v.type_name(), to: "bool" }),
        }
    }
}

impl<'a> TryFrom<&'a Value> for i64 {
    type Error = ValueConversionError;
    fn try_from(v: &'a Value) -> Result<Self, Self::Error> {
        match v {
            Value::Integer(n) => i64::try_from(n.value)
                .map_err(|_| ValueConversionError { from: "integer (out of i64 range)", to: "i64" }),
            Value::BigInteger(n) => {
                use num_traits::ToPrimitive;
                n.to_i64().ok_or(ValueConversionError { from: "integer (out of i64 range)", to: "i64" })
            }
            _ => Err(ValueConversionError { from: v.type_name(), to: "i64" }),
        }
    }
}

impl<'a> TryFrom<&'a Value> for i128 {
    type Error = ValueConversionError;
    fn try_from(v: &'a Value) -> Result<Self, Self::Error> {
        match v {
            Value::Integer(n) => Ok(n.value),
            Value::BigInteger(n) => {
                use num_traits::ToPrimitive;
                n.to_i128().ok_or(ValueConversionError { from: "integer (out of i128 range)", to: "i128" })
            }
            _ => Err(ValueConversionError { from: v.type_name(), to: "i128" }),
        }
    }
}

impl<'a> TryFrom<&'a Value> for f64 {
    type Error = ValueConversionError;
    fn try_from(v: &'a Value) -> Result<Self, Self::Error> {
        match v {
            Value::Float(f) => Ok(f.value),
            Value::Integer(n) => Ok(n.value as f64),
            Value::BigInteger(n) => {
                use num_traits::ToPrimitive;
                n.to_f64().ok_or(ValueConversionError { from: "integer (out of f64 range)", to: "f64" })
            }
            _ => Err(ValueConversionError { from: v.type_name(), to: "f64" }),
        }
    }
}

impl<'a> TryFrom<&'a Value> for &'a str {
    type Error = ValueConversionError;
    fn try_from(v: &'a Value) -> Result<Self, Self::Error> {
        match v {
            Value::String(s) => Ok(s.as_str()),
            _ => Err(ValueConversionError { from: v.type_name(), to: "&str" }),
        }
    }
}

// ============================================================
// Arithmetic error
// ============================================================

/// Error returned by checked arithmetic on [`Value`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueArithmeticError(pub String);

impl std::fmt::Display for ValueArithmeticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ValueArithmeticError {}

// ============================================================
// Checked arithmetic methods
// ============================================================

macro_rules! checked_binop {
    ($name:ident, $int_method:ident, $float_method:ident, $op_name:expr) => {
        pub fn $name(&self, rhs: &Value) -> Result<Value, ValueArithmeticError> {
            match (self, rhs) {
                (Value::Integer(a), Value::Integer(b)) => {
                    a.$int_method(b).map(Value::Integer).map_err(ValueArithmeticError)
                }
                (Value::Float(a), Value::Float(b)) => {
                    a.$float_method(b).map(Value::Float).map_err(ValueArithmeticError)
                }
                (Value::Integer(a), Value::Float(b)) => {
                    UzonFloat::new(a.value as f64).$float_method(b).map(Value::Float).map_err(ValueArithmeticError)
                }
                (Value::Float(a), Value::Integer(b)) => {
                    a.$float_method(&UzonFloat::new(b.value as f64)).map(Value::Float).map_err(ValueArithmeticError)
                }
                _ => Err(ValueArithmeticError(format!(
                    "cannot {} {} and {}", $op_name, self.type_name(), rhs.type_name()
                ))),
            }
        }
    };
}

impl Value {
    checked_binop!(checked_add, checked_add, add, "add");
    checked_binop!(checked_sub, checked_sub, sub, "subtract");
    checked_binop!(checked_mul, checked_mul, mul, "multiply");
    checked_binop!(checked_div, checked_div, div, "divide");
    checked_binop!(checked_rem, checked_rem, rem, "modulo");

    /// Checked negation. Returns `Err` on overflow or type mismatch.
    pub fn checked_neg(&self) -> Result<Value, ValueArithmeticError> {
        match self {
            Value::Integer(n) => n.checked_neg().map(Value::Integer).map_err(ValueArithmeticError),
            Value::Float(f) => Ok(Value::Float(f.neg())),
            _ => Err(ValueArithmeticError(format!("cannot negate {}", self.type_name()))),
        }
    }
}

// ============================================================
// Arithmetic operators (delegate to checked, panic on error)
// ============================================================

impl ops::Add for Value {
    type Output = Value;
    fn add(self, rhs: Value) -> Value {
        match (&self, &rhs) {
            (Value::String(a), Value::String(b)) => Value::String(format!("{a}{b}")),
            _ => self.checked_add(&rhs).expect("arithmetic error"),
        }
    }
}

impl ops::Sub for Value {
    type Output = Value;
    fn sub(self, rhs: Value) -> Value {
        self.checked_sub(&rhs).expect("arithmetic error")
    }
}

impl ops::Mul for Value {
    type Output = Value;
    fn mul(self, rhs: Value) -> Value {
        self.checked_mul(&rhs).expect("arithmetic error")
    }
}

impl ops::Div for Value {
    type Output = Value;
    fn div(self, rhs: Value) -> Value {
        self.checked_div(&rhs).expect("arithmetic error")
    }
}

impl ops::Rem for Value {
    type Output = Value;
    fn rem(self, rhs: Value) -> Value {
        self.checked_rem(&rhs).expect("arithmetic error")
    }
}

impl ops::Neg for Value {
    type Output = Value;
    fn neg(self) -> Value {
        self.checked_neg().expect("arithmetic error")
    }
}

// ============================================================
// Arithmetic with primitives
// ============================================================

macro_rules! impl_binop_int {
    ($trait:ident, $method:ident, $($ty:ty),+) => {
        $(
            impl ops::$trait<$ty> for Value {
                type Output = Value;
                fn $method(self, rhs: $ty) -> Value {
                    ops::$trait::$method(self, Value::from(rhs))
                }
            }
            impl ops::$trait<Value> for $ty {
                type Output = Value;
                fn $method(self, rhs: Value) -> Value {
                    ops::$trait::$method(Value::from(self), rhs)
                }
            }
        )+
    };
}

macro_rules! impl_binop_float {
    ($trait:ident, $method:ident, $($ty:ty),+) => {
        $(
            impl ops::$trait<$ty> for Value {
                type Output = Value;
                fn $method(self, rhs: $ty) -> Value {
                    ops::$trait::$method(self, Value::from(rhs))
                }
            }
            impl ops::$trait<Value> for $ty {
                type Output = Value;
                fn $method(self, rhs: Value) -> Value {
                    ops::$trait::$method(Value::from(self), rhs)
                }
            }
        )+
    };
}

macro_rules! impl_binop_str {
    ($trait:ident, $method:ident) => {
        impl ops::$trait<&str> for Value {
            type Output = Value;
            fn $method(self, rhs: &str) -> Value {
                ops::$trait::$method(self, Value::from(rhs))
            }
        }
        impl ops::$trait<Value> for &str {
            type Output = Value;
            fn $method(self, rhs: Value) -> Value {
                ops::$trait::$method(Value::from(self), rhs)
            }
        }
    };
}

impl_binop_int!(Add, add, i32, i64, i128, u32, u64);
impl_binop_int!(Sub, sub, i32, i64, i128, u32, u64);
impl_binop_int!(Mul, mul, i32, i64, i128, u32, u64);
impl_binop_int!(Div, div, i32, i64, i128, u32, u64);
impl_binop_int!(Rem, rem, i32, i64, i128, u32, u64);

impl_binop_float!(Add, add, f32, f64);
impl_binop_float!(Sub, sub, f32, f64);
impl_binop_float!(Mul, mul, f32, f64);
impl_binop_float!(Div, div, f32, f64);
impl_binop_float!(Rem, rem, f32, f64);

impl_binop_str!(Add, add);

// ============================================================
// PartialEq with primitives
// ============================================================

impl PartialEq<bool> for Value {
    fn eq(&self, other: &bool) -> bool {
        matches!(self, Value::Bool(b) if b == other)
    }
}

impl PartialEq<i32> for Value {
    fn eq(&self, other: &i32) -> bool {
        matches!(self, Value::Integer(n) if n.value == *other as i128)
    }
}

impl PartialEq<i64> for Value {
    fn eq(&self, other: &i64) -> bool {
        matches!(self, Value::Integer(n) if n.value == *other as i128)
    }
}

impl PartialEq<i128> for Value {
    fn eq(&self, other: &i128) -> bool {
        matches!(self, Value::Integer(n) if n.value == *other)
    }
}

impl PartialEq<u32> for Value {
    fn eq(&self, other: &u32) -> bool {
        matches!(self, Value::Integer(n) if n.value == *other as i128)
    }
}

impl PartialEq<u64> for Value {
    fn eq(&self, other: &u64) -> bool {
        matches!(self, Value::Integer(n) if n.value == *other as i128)
    }
}

impl PartialEq<f64> for Value {
    fn eq(&self, other: &f64) -> bool {
        matches!(self, Value::Float(f) if f.value == *other)
    }
}

impl PartialEq<&str> for Value {
    fn eq(&self, other: &&str) -> bool {
        matches!(self, Value::String(s) if s == *other)
    }
}

impl PartialEq<String> for Value {
    fn eq(&self, other: &String) -> bool {
        matches!(self, Value::String(s) if s == other)
    }
}

// ============================================================
// PartialOrd with primitives
// ============================================================

impl PartialOrd<i32> for Value {
    fn partial_cmp(&self, other: &i32) -> Option<std::cmp::Ordering> {
        self.partial_cmp(&Value::from(*other))
    }
}

impl PartialOrd<i64> for Value {
    fn partial_cmp(&self, other: &i64) -> Option<std::cmp::Ordering> {
        self.partial_cmp(&Value::from(*other))
    }
}

impl PartialOrd<f64> for Value {
    fn partial_cmp(&self, other: &f64) -> Option<std::cmp::Ordering> {
        self.partial_cmp(&Value::from(*other))
    }
}

impl PartialOrd<&str> for Value {
    fn partial_cmp(&self, other: &&str) -> Option<std::cmp::Ordering> {
        self.partial_cmp(&Value::from(*other))
    }
}

// ============================================================
// Comparison: PartialOrd (numeric only)
// ============================================================

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Value::Integer(a), Value::Integer(b)) => a.value.partial_cmp(&b.value),
            (Value::Float(a), Value::Float(b)) => a.value.partial_cmp(&b.value),
            (Value::Integer(a), Value::Float(b)) => (a.value as f64).partial_cmp(&b.value),
            (Value::Float(a), Value::Integer(b)) => a.value.partial_cmp(&(b.value as f64)),
            (Value::String(a), Value::String(b)) => a.partial_cmp(b),
            (Value::Bool(a), Value::Bool(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

// ============================================================
// Struct builder
// ============================================================

/// Builder for constructing `Value::Struct` ergonomically.
///
/// ```ignore
/// let v = Value::struct_builder()
///     .field("name", "Alice")
///     .field("age", 30)
///     .field("scores", vec![Value::int(90), Value::int(85)])
///     .build();
/// ```
pub struct StructBuilder {
    fields: IndexMap<String, Value>,
}

impl StructBuilder {
    fn new() -> Self {
        Self { fields: IndexMap::new() }
    }

    /// Add a field. The value can be anything that implements `Into<Value>`.
    pub fn field(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }

    /// Finish building and return the `Value::Struct`.
    pub fn build(self) -> Value {
        Value::Struct(crate::value::UzonStruct::new(self.fields))
    }
}

impl Value {
    /// Start building a struct value.
    pub fn struct_builder() -> StructBuilder {
        StructBuilder::new()
    }
}

// ============================================================
// IntoIterator
// ============================================================

/// Iterator over borrowed `Value` elements.
///
/// Yields items from lists, tuples, or struct values (in insertion order).
/// Non-iterable values yield an empty iterator.
pub enum ValueIter<'a> {
    Slice(std::slice::Iter<'a, Value>),
    Map(indexmap::map::Values<'a, String, Value>),
    Empty,
}

impl<'a> Iterator for ValueIter<'a> {
    type Item = &'a Value;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            ValueIter::Slice(it) => it.next(),
            ValueIter::Map(it) => it.next(),
            ValueIter::Empty => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            ValueIter::Slice(it) => it.size_hint(),
            ValueIter::Map(it) => it.size_hint(),
            ValueIter::Empty => (0, Some(0)),
        }
    }
}

impl<'a> ExactSizeIterator for ValueIter<'a> {}

impl<'a> IntoIterator for &'a Value {
    type Item = &'a Value;
    type IntoIter = ValueIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Value::List(l) => ValueIter::Slice(l.elements.iter()),
            Value::Tuple(t) => ValueIter::Slice(t.elements.iter()),
            Value::Struct(m) => ValueIter::Map(m.values()),
            _ => ValueIter::Empty,
        }
    }
}

/// Owned iterator over `Value` elements.
pub enum ValueIntoIter {
    Vec(std::vec::IntoIter<Value>),
    Map(indexmap::map::IntoValues<String, Value>),
    Empty,
}

impl Iterator for ValueIntoIter {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            ValueIntoIter::Vec(it) => it.next(),
            ValueIntoIter::Map(it) => it.next(),
            ValueIntoIter::Empty => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            ValueIntoIter::Vec(it) => it.size_hint(),
            ValueIntoIter::Map(it) => it.size_hint(),
            ValueIntoIter::Empty => (0, Some(0)),
        }
    }
}

impl ExactSizeIterator for ValueIntoIter {}

impl IntoIterator for Value {
    type Item = Value;
    type IntoIter = ValueIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Value::List(l) => ValueIntoIter::Vec(l.elements.into_iter()),
            Value::Tuple(t) => ValueIntoIter::Vec(t.elements.into_iter()),
            Value::Struct(m) => ValueIntoIter::Map(m.fields.into_values()),
            _ => ValueIntoIter::Empty,
        }
    }
}

impl Value {
    /// Returns the number of elements (list/tuple) or fields (struct).
    /// Returns `None` for non-collection types.
    pub fn len(&self) -> Option<usize> {
        match self {
            Value::List(l) => Some(l.elements.len()),
            Value::Tuple(t) => Some(t.elements.len()),
            Value::Struct(m) => Some(m.len()),
            _ => None,
        }
    }

    /// Iterate over (key, value) pairs for structs.
    /// Returns `None` for non-struct types.
    pub fn iter_fields(&self) -> Option<indexmap::map::Iter<'_, String, Value>> {
        match self {
            Value::Struct(m) => Some(m.iter()),
            _ => None,
        }
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uzon;

    // --- Accessors ---

    #[test]
    fn test_as_bool() {
        assert_eq!(Value::Bool(true).as_bool(), Some(true));
        assert_eq!(Value::Null.as_bool(), None);
    }

    #[test]
    fn test_as_i64() {
        assert_eq!(Value::int(42).as_i64(), Some(42));
        assert_eq!(Value::float(1.5).as_i64(), None);
    }

    #[test]
    fn test_as_i128() {
        let big = i128::MAX;
        assert_eq!(Value::int(big).as_i128(), Some(big));
    }

    #[test]
    fn test_as_f64() {
        assert_eq!(Value::float(3.14).as_f64(), Some(3.14));
        // integer auto-converts to f64
        assert_eq!(Value::int(10).as_f64(), Some(10.0));
        assert_eq!(Value::Null.as_f64(), None);
    }

    #[test]
    fn test_as_str() {
        assert_eq!(Value::String("hello".into()).as_str(), Some("hello"));
        assert_eq!(Value::int(1).as_str(), None);
    }

    #[test]
    fn test_as_list() {
        let v = Value::list(vec![Value::int(1), Value::int(2)]);
        assert_eq!(v.as_list().unwrap().len(), 2);
        assert_eq!(Value::Null.as_list(), None);
    }

    #[test]
    fn test_as_struct() {
        let mut map = IndexMap::new();
        map.insert("x".into(), Value::int(1));
        let v = Value::Struct(crate::value::UzonStruct::new(map));
        assert!(v.as_struct().is_some());
        assert_eq!(Value::Null.as_struct(), None);
    }

    // --- Mutation ---

    #[test]
    fn test_insert_remove() {
        let mut v = Value::struct_builder().field("a", 1).build();
        assert_eq!(v.insert("b", 2), None);
        assert_eq!(v.get("b"), Some(&Value::int(2)));
        let old = v.insert("a", 99);
        assert_eq!(old, Some(Value::int(1)));
        assert_eq!(v.get("a"), Some(&Value::int(99)));
        assert_eq!(v.remove("a"), Some(Value::int(99)));
        assert_eq!(v.get("a"), None);
    }

    #[test]
    fn test_push_pop() {
        let mut v = Value::list(vec![Value::int(1)]);
        v.push(2);
        v.push(3);
        assert_eq!(v.len(), Some(3));
        assert_eq!(v.pop(), Some(Value::int(3)));
        assert_eq!(v.pop(), Some(Value::int(2)));
        assert_eq!(v.pop(), Some(Value::int(1)));
        assert_eq!(v.pop(), None);
    }

    // --- Merge ---

    #[test]
    fn test_merge_simple() {
        let mut base = uzon!({"a": 1, "b": 2});
        let overlay = uzon!({"b": 99, "c": 3});
        base.merge(overlay);
        assert_eq!(base.get("a"), Some(&Value::int(1)));
        assert_eq!(base.get("b"), Some(&Value::int(99)));
        assert_eq!(base.get("c"), Some(&Value::int(3)));
    }

    #[test]
    fn test_merge_deep() {
        let mut base = uzon!({
            "server": {
                "host": "localhost",
                "port": 8080
            },
            "debug": false
        });
        let overlay = uzon!({
            "server": {
                "port": 3000,
                "tls": true
            },
            "debug": true
        });
        base.merge(overlay);
        assert_eq!(base.get_path("server.host"), Some(&Value::from("localhost")));
        assert_eq!(base.get_path("server.port"), Some(&Value::int(3000)));
        assert_eq!(base.get_path("server.tls"), Some(&Value::Bool(true)));
        assert_eq!(base.get("debug"), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_merge_non_struct_noop() {
        let mut v = Value::int(42);
        v.merge(Value::int(99));
        assert_eq!(v, 42); // no-op
    }

    // --- get ---

    #[test]
    fn test_get_struct() {
        let mut map = IndexMap::new();
        map.insert("name".into(), Value::from("Alice"));
        let v = Value::Struct(crate::value::UzonStruct::new(map));
        assert_eq!(v.get("name"), Some(&Value::from("Alice")));
        assert_eq!(v.get("missing"), None);
        assert_eq!(Value::int(1).get("x"), None);
    }

    #[test]
    fn test_get_index() {
        let v = Value::list(vec![Value::int(10), Value::int(20)]);
        assert_eq!(v.get_index(0), Some(&Value::int(10)));
        assert_eq!(v.get_index(99), None);
        assert_eq!(Value::int(1).get_index(0), None);
    }

    #[test]
    fn test_get_mut() {
        let mut map = IndexMap::new();
        map.insert("x".into(), Value::int(1));
        let mut v = Value::Struct(crate::value::UzonStruct::new(map));
        *v.get_mut("x").unwrap() = Value::int(42);
        assert_eq!(v.get("x"), Some(&Value::int(42)));
    }

    #[test]
    fn test_get_path() {
        let mut inner = IndexMap::new();
        inner.insert("host".into(), Value::from("localhost"));
        inner.insert("port".into(), Value::int(8080));
        let mut outer = IndexMap::new();
        outer.insert("server".into(), Value::Struct(crate::value::UzonStruct::new(inner)));
        outer.insert("items".into(), Value::list(vec![Value::from("a"), Value::from("b")]));
        let v = Value::Struct(crate::value::UzonStruct::new(outer));

        assert_eq!(v.get_path("server.host"), Some(&Value::from("localhost")));
        assert_eq!(v.get_path("server.port"), Some(&Value::int(8080)));
        assert_eq!(v.get_path("server.missing"), None);
        assert_eq!(v.get_path("missing.host"), None);
        // numeric index in path
        assert_eq!(v.get_path("items.0"), Some(&Value::from("a")));
        assert_eq!(v.get_path("items.1"), Some(&Value::from("b")));
        assert_eq!(v.get_path("items.99"), None);
    }

    // --- Index ---

    #[test]
    fn test_index_struct() {
        let mut map = IndexMap::new();
        map.insert("name".into(), Value::String("Alice".into()));
        let v = Value::Struct(crate::value::UzonStruct::new(map));
        assert_eq!(v["name"], Value::String("Alice".into()));
        assert_eq!(v["missing"], Value::Null);
    }

    #[test]
    fn test_index_list() {
        let v = Value::list(vec![Value::int(10), Value::int(20)]);
        assert_eq!(v[0], Value::int(10));
        assert_eq!(v[1], Value::int(20));
        assert_eq!(v[99], Value::Null);
    }

    #[test]
    fn test_index_non_collection() {
        assert_eq!(Value::int(5)["key"], Value::Null);
        assert_eq!(Value::int(5)[0], Value::Null);
    }

    // --- TryFrom ---

    #[test]
    fn test_try_into_bool() {
        let v = Value::Bool(true);
        let b: bool = v.try_into().unwrap();
        assert_eq!(b, true);
        assert!(bool::try_from(Value::int(1)).is_err());
    }

    #[test]
    fn test_try_into_i64() {
        let n: i64 = Value::int(42).try_into().unwrap();
        assert_eq!(n, 42);
        assert!(i64::try_from(Value::float(1.5)).is_err());
    }

    #[test]
    fn test_try_into_i128() {
        let n: i128 = Value::int(i128::MAX).try_into().unwrap();
        assert_eq!(n, i128::MAX);
    }

    #[test]
    fn test_try_into_u64() {
        let n: u64 = Value::int(100).try_into().unwrap();
        assert_eq!(n, 100);
        assert!(u64::try_from(Value::int(-1)).is_err());
    }

    #[test]
    fn test_try_into_f64() {
        let f: f64 = Value::float(3.14).try_into().unwrap();
        assert_eq!(f, 3.14);
        // integer auto-converts
        let f: f64 = Value::int(10).try_into().unwrap();
        assert_eq!(f, 10.0);
    }

    #[test]
    fn test_try_into_string() {
        let s: String = Value::from("hello").try_into().unwrap();
        assert_eq!(s, "hello");
        assert!(String::try_from(Value::int(1)).is_err());
    }

    #[test]
    fn test_try_into_vec() {
        let v: Vec<Value> = Value::list(vec![Value::int(1), Value::int(2)]).try_into().unwrap();
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn test_try_into_map() {
        let mut map = IndexMap::new();
        map.insert("x".into(), Value::int(1));
        let m: IndexMap<String, Value> = Value::Struct(crate::value::UzonStruct::new(map)).try_into().unwrap();
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn test_try_from_ref() {
        let v = Value::int(42);
        let n: i64 = (&v).try_into().unwrap();
        assert_eq!(n, 42);
        // v is still usable
        assert_eq!(v, Value::int(42));

        let v = Value::from("hello");
        let s: &str = (&v).try_into().unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn test_conversion_error_display() {
        let err = bool::try_from(Value::int(1)).unwrap_err();
        assert_eq!(err.to_string(), "cannot convert integer to bool");
    }

    // --- From ---

    #[test]
    fn test_from_primitives() {
        assert_eq!(Value::from(true), Value::Bool(true));
        assert_eq!(Value::from(42i32), Value::int(42));
        assert_eq!(Value::from(42i64), Value::int(42));
        assert_eq!(Value::from(42i128), Value::int(42));
        assert_eq!(Value::from(42u32), Value::int(42));
        assert_eq!(Value::from(42u64), Value::int(42));
        assert!(matches!(Value::from(3.14f64), Value::Float(_)));
        assert!(matches!(Value::from(3.14f32), Value::Float(_)));
        assert_eq!(Value::from("hello"), Value::String("hello".into()));
        assert_eq!(Value::from("hello".to_string()), Value::String("hello".into()));
    }

    #[test]
    fn test_from_vec() {
        let v: Value = vec![Value::int(1), Value::int(2)].into();
        assert!(matches!(v, Value::List(_)));
    }

    #[test]
    fn test_from_tuple() {
        let v: Value = (Value::int(1), Value::from("two")).into();
        assert_eq!(v.as_tuple().unwrap().len(), 2);
    }

    // --- Arithmetic ---

    #[test]
    fn test_add() {
        assert_eq!(Value::int(2) + Value::int(3), Value::int(5));
        assert_eq!(Value::float(1.5) + Value::float(2.5), Value::float(4.0));
        // mixed
        assert_eq!((Value::int(1) + Value::float(0.5)).as_f64(), Some(1.5));
        // string concat
        assert_eq!(
            Value::from("he") + Value::from("llo"),
            Value::String("hello".into())
        );
    }

    #[test]
    fn test_sub() {
        assert_eq!(Value::int(10) - Value::int(3), Value::int(7));
        assert_eq!(Value::float(5.0) - Value::float(1.5), Value::float(3.5));
    }

    #[test]
    fn test_mul() {
        assert_eq!(Value::int(4) * Value::int(5), Value::int(20));
        assert_eq!(Value::float(2.0) * Value::float(3.0), Value::float(6.0));
    }

    #[test]
    fn test_div() {
        assert_eq!(Value::int(10) / Value::int(3), Value::int(3));
        assert_eq!(Value::float(10.0) / Value::float(4.0), Value::float(2.5));
    }

    #[test]
    fn test_rem() {
        assert_eq!(Value::int(10) % Value::int(3), Value::int(1));
    }

    #[test]
    fn test_neg() {
        assert_eq!(-Value::int(5), Value::int(-5));
        assert_eq!(-Value::float(3.0), Value::float(-3.0));
    }

    #[test]
    #[should_panic(expected = "arithmetic error")]
    fn test_add_type_mismatch() {
        let _ = Value::int(1) + Value::Bool(true);
    }

    // --- Checked arithmetic ---

    #[test]
    fn test_checked_add_ok() {
        assert_eq!(Value::int(2).checked_add(&Value::int(3)), Ok(Value::int(5)));
        assert!(Value::float(1.5).checked_add(&Value::float(2.5)).is_ok());
    }

    #[test]
    fn test_checked_add_overflow() {
        let max = Value::int(i128::MAX);
        let err = max.checked_add(&Value::int(1));
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("overflow"));
    }

    #[test]
    fn test_checked_div_zero() {
        let err = Value::int(10).checked_div(&Value::int(0));
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("zero"));
    }

    #[test]
    fn test_checked_type_mismatch() {
        let err = Value::int(1).checked_add(&Value::Bool(true));
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("cannot"));
    }

    #[test]
    fn test_checked_neg() {
        assert_eq!(Value::int(5).checked_neg(), Ok(Value::int(-5)));
        assert!(Value::Bool(true).checked_neg().is_err());
    }

    // --- Primitive arithmetic ---

    #[test]
    fn test_add_primitive() {
        assert_eq!(Value::int(10) + 5, Value::int(15));
        assert_eq!(5 + Value::int(10), Value::int(15));
        assert_eq!(Value::float(1.5) + 0.5, Value::float(2.0));
        assert_eq!(0.5 + Value::float(1.5), Value::float(2.0));
        assert_eq!(Value::int(10) + 0.5, Value::float(10.5));
        assert_eq!(Value::from("he") + "llo", Value::from("hello"));
        assert_eq!("he" + Value::from("llo"), Value::from("hello"));
    }

    #[test]
    fn test_sub_primitive() {
        assert_eq!(Value::int(10) - 3, Value::int(7));
        assert_eq!(20 - Value::int(3), Value::int(17));
    }

    #[test]
    fn test_mul_primitive() {
        assert_eq!(Value::int(4) * 5, Value::int(20));
        assert_eq!(5 * Value::int(4), Value::int(20));
    }

    #[test]
    fn test_div_primitive() {
        assert_eq!(Value::int(10) / 3, Value::int(3));
        assert_eq!(Value::float(10.0) / 4.0, Value::float(2.5));
    }

    #[test]
    fn test_rem_primitive() {
        assert_eq!(Value::int(10) % 3, Value::int(1));
    }

    // --- Primitive PartialEq ---

    #[test]
    fn test_eq_primitive() {
        assert!(Value::int(42) == 42);
        assert!(Value::int(42) == 42i64);
        assert!(Value::int(42) == 42i128);
        assert!(Value::float(3.14) == 3.14);
        assert!(Value::Bool(true) == true);
        assert!(Value::from("hello") == "hello");
        assert!(Value::from("hello") == String::from("hello"));

        assert!(Value::int(42) != 43);
        assert!(Value::from("hello") != "world");
    }

    // --- Primitive PartialOrd ---

    #[test]
    fn test_ord_primitive() {
        assert!(Value::int(1) < 2);
        assert!(Value::int(3) > 2i64);
        assert!(Value::float(1.0) < 2.0);
        assert!(Value::from("a") < "b");
    }

    // --- PartialOrd (Value vs Value) ---

    #[test]
    fn test_ordering() {
        assert!(Value::int(1) < Value::int(2));
        assert!(Value::float(1.0) < Value::float(2.0));
        assert!(Value::int(1) < Value::float(1.5));
        assert!(Value::from("a") < Value::from("b"));
        // incompatible types => None
        assert_eq!(Value::int(1).partial_cmp(&Value::Bool(true)), None);
    }

    // --- uzon! macro ---

    #[test]
    fn test_uzon_macro_primitives() {
        assert_eq!(uzon!(null), Value::Null);
        assert_eq!(uzon!(true), Value::Bool(true));
        assert_eq!(uzon!(false), Value::Bool(false));
        assert_eq!(uzon!(42), Value::int(42));
        assert_eq!(uzon!(3.14), Value::float(3.14));
        assert_eq!(uzon!("hello"), Value::from("hello"));
    }

    #[test]
    fn test_uzon_macro_list() {
        let v = uzon!([1, 2, 3]);
        assert_eq!(v.get_index(0), Some(&Value::int(1)));
        assert_eq!(v.len(), Some(3));
    }

    #[test]
    fn test_uzon_macro_struct() {
        let v = uzon!({
            "name": "Alice",
            "age": 30,
            "active": true
        });
        assert_eq!(v.get("name"), Some(&Value::from("Alice")));
        assert_eq!(v.get("age"), Some(&Value::int(30)));
        assert_eq!(v.get("active"), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_uzon_macro_nested() {
        let v = uzon!({
            "server": {
                "host": "localhost",
                "port": 8080
            },
            "tags": ["web", "api"],
            "coord": (1, 2)
        });
        assert_eq!(v.get_path("server.host"), Some(&Value::from("localhost")));
        assert_eq!(v.get_path("server.port"), Some(&Value::int(8080)));
        assert_eq!(v.get("tags").unwrap().get_index(1), Some(&Value::from("api")));
        let tuple = v.get("coord").unwrap();
        assert_eq!(tuple.as_tuple().unwrap().len(), 2);
    }

    #[test]
    fn test_uzon_macro_trailing_comma() {
        let v = uzon!([1, 2,]);
        assert_eq!(v.len(), Some(2));
        let v = uzon!({"a": 1, "b": 2,});
        assert_eq!(v.len(), Some(2));
    }

    // --- StructBuilder ---

    #[test]
    fn test_struct_builder() {
        let v = Value::struct_builder()
            .field("name", "Alice")
            .field("age", 30i32)
            .field("active", true)
            .field("scores", vec![Value::int(90), Value::int(85)])
            .build();

        assert_eq!(v.get("name"), Some(&Value::from("Alice")));
        assert_eq!(v.get("age"), Some(&Value::int(30)));
        assert_eq!(v.get("active"), Some(&Value::Bool(true)));
        assert_eq!(v.get("scores").unwrap().get_index(0), Some(&Value::int(90)));
    }

    #[test]
    fn test_struct_builder_nested() {
        let v = Value::struct_builder()
            .field("server", Value::struct_builder()
                .field("host", "localhost")
                .field("port", 8080i32)
                .build())
            .build();

        assert_eq!(v.get_path("server.host"), Some(&Value::from("localhost")));
        assert_eq!(v.get_path("server.port"), Some(&Value::int(8080)));
    }

    // --- IntoIterator ---

    #[test]
    fn test_iter_list() {
        let v = Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]);
        let items: Vec<&Value> = (&v).into_iter().collect();
        assert_eq!(items, vec![&Value::int(1), &Value::int(2), &Value::int(3)]);
    }

    #[test]
    fn test_iter_list_for_loop() {
        let v = Value::list(vec![Value::int(10), Value::int(20)]);
        let mut sum = 0i64;
        for item in &v {
            sum += item.as_i64().unwrap();
        }
        assert_eq!(sum, 30);
    }

    #[test]
    fn test_iter_tuple() {
        let v = Value::from((Value::int(1), Value::from("two")));
        assert_eq!((&v).into_iter().count(), 2);
    }

    #[test]
    fn test_iter_struct() {
        let mut map = IndexMap::new();
        map.insert("a".into(), Value::int(1));
        map.insert("b".into(), Value::int(2));
        let v = Value::Struct(crate::value::UzonStruct::new(map));
        let vals: Vec<&Value> = (&v).into_iter().collect();
        assert_eq!(vals, vec![&Value::int(1), &Value::int(2)]);
    }

    #[test]
    fn test_iter_owned() {
        let v = Value::list(vec![Value::int(1), Value::int(2)]);
        let items: Vec<Value> = v.into_iter().collect();
        assert_eq!(items, vec![Value::int(1), Value::int(2)]);
    }

    #[test]
    fn test_iter_non_collection() {
        let v = Value::int(42);
        assert_eq!((&v).into_iter().count(), 0);
        assert_eq!(v.into_iter().count(), 0);
    }

    #[test]
    fn test_len() {
        assert_eq!(Value::list(vec![Value::int(1), Value::int(2)]).len(), Some(2));
        let mut map = IndexMap::new();
        map.insert("x".into(), Value::int(1));
        assert_eq!(Value::Struct(crate::value::UzonStruct::new(map)).len(), Some(1));
        assert_eq!(Value::int(42).len(), None);
    }

    #[test]
    fn test_iter_fields() {
        let mut map = IndexMap::new();
        map.insert("name".into(), Value::from("Alice"));
        map.insert("age".into(), Value::int(30));
        let v = Value::Struct(crate::value::UzonStruct::new(map));
        let fields: Vec<(&String, &Value)> = v.iter_fields().unwrap().collect();
        assert_eq!(fields[0].0, "name");
        assert_eq!(fields[1].0, "age");
        assert!(Value::int(1).iter_fields().is_none());
    }
}
