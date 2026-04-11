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
            _ => None,
        }
    }

    pub fn as_i128(&self) -> Option<i128> {
        match self {
            Value::Integer(n) => Some(n.value),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(f.value),
            Value::Integer(n) => Some(n.value as f64),
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
        Value::Struct(v)
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
// Arithmetic operators
// ============================================================

impl ops::Add for Value {
    type Output = Value;

    fn add(self, rhs: Value) -> Value {
        match (&self, &rhs) {
            (Value::Integer(a), Value::Integer(b)) => {
                Value::Integer(a.checked_add(b).expect("integer add overflow"))
            }
            (Value::Float(a), Value::Float(b)) => {
                Value::Float(a.add(b).expect("float add type mismatch"))
            }
            (Value::Integer(a), Value::Float(b)) => {
                let af = UzonFloat::new(a.value as f64);
                Value::Float(af.add(b).expect("float add type mismatch"))
            }
            (Value::Float(a), Value::Integer(b)) => {
                let bf = UzonFloat::new(b.value as f64);
                Value::Float(a.add(&bf).expect("float add type mismatch"))
            }
            (Value::String(a), Value::String(b)) => {
                Value::String(format!("{a}{b}"))
            }
            _ => panic!("cannot add {} and {}", self.type_name(), rhs.type_name()),
        }
    }
}

impl ops::Sub for Value {
    type Output = Value;

    fn sub(self, rhs: Value) -> Value {
        match (&self, &rhs) {
            (Value::Integer(a), Value::Integer(b)) => {
                Value::Integer(a.checked_sub(b).expect("integer sub overflow"))
            }
            (Value::Float(a), Value::Float(b)) => {
                Value::Float(a.sub(b).expect("float sub type mismatch"))
            }
            (Value::Integer(a), Value::Float(b)) => {
                let af = UzonFloat::new(a.value as f64);
                Value::Float(af.sub(b).expect("float sub type mismatch"))
            }
            (Value::Float(a), Value::Integer(b)) => {
                let bf = UzonFloat::new(b.value as f64);
                Value::Float(a.sub(&bf).expect("float sub type mismatch"))
            }
            _ => panic!("cannot subtract {} from {}", rhs.type_name(), self.type_name()),
        }
    }
}

impl ops::Mul for Value {
    type Output = Value;

    fn mul(self, rhs: Value) -> Value {
        match (&self, &rhs) {
            (Value::Integer(a), Value::Integer(b)) => {
                Value::Integer(a.checked_mul(b).expect("integer mul overflow"))
            }
            (Value::Float(a), Value::Float(b)) => {
                Value::Float(a.mul(b).expect("float mul type mismatch"))
            }
            (Value::Integer(a), Value::Float(b)) => {
                let af = UzonFloat::new(a.value as f64);
                Value::Float(af.mul(b).expect("float mul type mismatch"))
            }
            (Value::Float(a), Value::Integer(b)) => {
                let bf = UzonFloat::new(b.value as f64);
                Value::Float(a.mul(&bf).expect("float mul type mismatch"))
            }
            _ => panic!("cannot multiply {} and {}", self.type_name(), rhs.type_name()),
        }
    }
}

impl ops::Div for Value {
    type Output = Value;

    fn div(self, rhs: Value) -> Value {
        match (&self, &rhs) {
            (Value::Integer(a), Value::Integer(b)) => {
                Value::Integer(a.checked_div(b).expect("integer division error"))
            }
            (Value::Float(a), Value::Float(b)) => {
                Value::Float(a.div(b).expect("float div type mismatch"))
            }
            (Value::Integer(a), Value::Float(b)) => {
                let af = UzonFloat::new(a.value as f64);
                Value::Float(af.div(b).expect("float div type mismatch"))
            }
            (Value::Float(a), Value::Integer(b)) => {
                let bf = UzonFloat::new(b.value as f64);
                Value::Float(a.div(&bf).expect("float div type mismatch"))
            }
            _ => panic!("cannot divide {} by {}", self.type_name(), rhs.type_name()),
        }
    }
}

impl ops::Rem for Value {
    type Output = Value;

    fn rem(self, rhs: Value) -> Value {
        match (&self, &rhs) {
            (Value::Integer(a), Value::Integer(b)) => {
                Value::Integer(a.checked_rem(b).expect("integer modulo error"))
            }
            (Value::Float(a), Value::Float(b)) => {
                Value::Float(a.rem(b).expect("float rem type mismatch"))
            }
            (Value::Integer(a), Value::Float(b)) => {
                let af = UzonFloat::new(a.value as f64);
                Value::Float(af.rem(b).expect("float rem type mismatch"))
            }
            (Value::Float(a), Value::Integer(b)) => {
                let bf = UzonFloat::new(b.value as f64);
                Value::Float(a.rem(&bf).expect("float rem type mismatch"))
            }
            _ => panic!("cannot modulo {} by {}", self.type_name(), rhs.type_name()),
        }
    }
}

impl ops::Neg for Value {
    type Output = Value;

    fn neg(self) -> Value {
        match &self {
            Value::Integer(n) => Value::Integer(n.checked_neg().expect("integer neg overflow")),
            Value::Float(f) => Value::Float(f.neg()),
            _ => panic!("cannot negate {}", self.type_name()),
        }
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
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

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
        let v = Value::Struct(map);
        assert!(v.as_struct().is_some());
        assert_eq!(Value::Null.as_struct(), None);
    }

    // --- Index ---

    #[test]
    fn test_index_struct() {
        let mut map = IndexMap::new();
        map.insert("name".into(), Value::String("Alice".into()));
        let v = Value::Struct(map);
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
    #[should_panic(expected = "cannot add")]
    fn test_add_type_mismatch() {
        let _ = Value::int(1) + Value::Bool(true);
    }

    // --- PartialOrd ---

    #[test]
    fn test_ordering() {
        assert!(Value::int(1) < Value::int(2));
        assert!(Value::float(1.0) < Value::float(2.0));
        assert!(Value::int(1) < Value::float(1.5));
        assert!(Value::from("a") < Value::from("b"));
        // incompatible types => None
        assert_eq!(Value::int(1).partial_cmp(&Value::Bool(true)), None);
    }
}
