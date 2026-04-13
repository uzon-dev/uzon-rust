// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! Serde integration for [`Value`].
//!
//! - `Serialize` for `Value` — convert any UZON value to serde-compatible formats.
//! - `Deserializer` for `Value` — deserialize a `Value` into any `T: Deserialize`.

use indexmap::IndexMap;
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{self, Deserialize, Serialize};

use super::Value;

// ============================================================
// Serialize
// ============================================================

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Value::Null => serializer.serialize_none(),
            Value::Undefined => serializer.serialize_none(),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Integer(n) => {
                if let Ok(v) = i64::try_from(n.value) {
                    serializer.serialize_i64(v)
                } else {
                    serializer.serialize_i128(n.value)
                }
            }
            Value::BigInteger(n) => {
                // Best-effort: try i128 first, fall back to string
                use num_traits::ToPrimitive;
                if let Some(v) = n.to_i128() {
                    serializer.serialize_i128(v)
                } else {
                    serializer.serialize_str(&n.to_string())
                }
            }
            Value::Float(f) => serializer.serialize_f64(f.value),
            Value::String(s) => serializer.serialize_str(s),
            Value::List(l) => {
                let mut seq = serializer.serialize_seq(Some(l.elements.len()))?;
                for elem in &l.elements {
                    seq.serialize_element(elem)?;
                }
                seq.end()
            }
            Value::Tuple(t) => {
                let mut seq = serializer.serialize_seq(Some(t.elements.len()))?;
                for elem in &t.elements {
                    seq.serialize_element(elem)?;
                }
                seq.end()
            }
            Value::Struct(fields) => {
                let mut map = serializer.serialize_map(Some(fields.len()))?;
                for (k, v) in fields {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
            // Enum/Union/TaggedUnion: serialize the inner value
            Value::Enum(e) => serializer.serialize_str(&e.value),
            Value::Union(u) => u.value.serialize(serializer),
            Value::TaggedUnion(tu) => {
                // Serialize the inner value directly. The tag is metadata,
                // not data — adding a "_tag" field risks colliding with
                // user-defined fields.
                tu.value.serialize(serializer)
            }
            Value::Function(_) => serializer.serialize_none(),
        }
    }
}

// ============================================================
// Deserializer — walk a Value to produce T: Deserialize
// ============================================================

use serde::de::{self, DeserializeSeed, EnumAccess, MapAccess, SeqAccess, VariantAccess, Visitor};

/// Errors during deserialization from `Value`.
#[derive(Debug)]
pub struct DeError(String);

impl std::fmt::Display for DeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DeError {}

impl de::Error for DeError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        DeError(msg.to_string())
    }
}

impl serde::ser::Error for DeError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        DeError(msg.to_string())
    }
}

/// A serde `Deserializer` backed by a UZON [`Value`].
pub struct ValueDeserializer {
    value: Value,
}

impl ValueDeserializer {
    pub fn new(value: Value) -> Self {
        Self { value }
    }
}

macro_rules! deserialize_number {
    ($method:ident, $visit:ident, $ty:ty) => {
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            match self.value {
                Value::Integer(n) => visitor.$visit(n.value as $ty),
                Value::BigInteger(n) => {
                    use num_traits::ToPrimitive;
                    let v = n.to_i128().ok_or_else(|| DeError(format!("BigInteger out of range for {}", stringify!($ty))))?;
                    visitor.$visit(v as $ty)
                }
                Value::Float(f) => visitor.$visit(f.value as $ty),
                _ => Err(DeError(format!("expected number, got {}", self.value.type_name()))),
            }
        }
    };
}

impl<'de> serde::Deserializer<'de> for ValueDeserializer {
    type Error = DeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            Value::Null | Value::Undefined => visitor.visit_none(),
            Value::Bool(b) => visitor.visit_bool(b),
            Value::Integer(n) => {
                if let Ok(v) = i64::try_from(n.value) {
                    visitor.visit_i64(v)
                } else {
                    visitor.visit_i128(n.value)
                }
            }
            Value::BigInteger(n) => {
                use num_traits::ToPrimitive;
                if let Some(v) = n.to_i128() {
                    visitor.visit_i128(v)
                } else {
                    visitor.visit_string(n.to_string())
                }
            }
            Value::Float(f) => visitor.visit_f64(f.value),
            Value::String(s) => visitor.visit_string(s),
            Value::List(l) => {
                let seq = SeqDeserializer::new(l.elements);
                visitor.visit_seq(seq)
            }
            Value::Tuple(t) => {
                let seq = SeqDeserializer::new(t.elements);
                visitor.visit_seq(seq)
            }
            Value::Struct(s) => {
                let map = MapDeserializer::new(s.fields);
                visitor.visit_map(map)
            }
            Value::Enum(e) => visitor.visit_string(e.value),
            Value::Union(u) => ValueDeserializer::new(*u.value).deserialize_any(visitor),
            Value::TaggedUnion(tu) => ValueDeserializer::new(*tu.value).deserialize_any(visitor),
            Value::Function(_) => visitor.visit_none(),
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            Value::Bool(b) => visitor.visit_bool(b),
            _ => Err(DeError(format!("expected bool, got {}", self.value.type_name()))),
        }
    }

    deserialize_number!(deserialize_i8, visit_i8, i8);
    deserialize_number!(deserialize_i16, visit_i16, i16);
    deserialize_number!(deserialize_i32, visit_i32, i32);
    deserialize_number!(deserialize_i64, visit_i64, i64);
    deserialize_number!(deserialize_i128, visit_i128, i128);
    deserialize_number!(deserialize_u8, visit_u8, u8);
    deserialize_number!(deserialize_u16, visit_u16, u16);
    deserialize_number!(deserialize_u32, visit_u32, u32);
    deserialize_number!(deserialize_u64, visit_u64, u64);
    deserialize_number!(deserialize_u128, visit_u128, u128);
    deserialize_number!(deserialize_f32, visit_f32, f32);
    deserialize_number!(deserialize_f64, visit_f64, f64);

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            Value::String(s) => {
                let mut chars = s.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => visitor.visit_char(c),
                    _ => Err(DeError("expected single char string".into())),
                }
            }
            _ => Err(DeError(format!("expected string, got {}", self.value.type_name()))),
        }
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_string(visitor)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            Value::String(s) => visitor.visit_string(s),
            Value::Enum(e) => visitor.visit_string(e.value),
            _ => Err(DeError(format!("expected string, got {}", self.value.type_name()))),
        }
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Self::Error> {
        Err(DeError("bytes not supported".into()))
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Self::Error> {
        Err(DeError("byte_buf not supported".into()))
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            Value::Null | Value::Undefined => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            Value::Null | Value::Undefined => visitor.visit_unit(),
            _ => Err(DeError(format!("expected null, got {}", self.value.type_name()))),
        }
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self, _name: &'static str, visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self, _name: &'static str, visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            Value::List(l) => visitor.visit_seq(SeqDeserializer::new(l.elements)),
            Value::Tuple(t) => visitor.visit_seq(SeqDeserializer::new(t.elements)),
            _ => Err(DeError(format!("expected list/tuple, got {}", self.value.type_name()))),
        }
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self, _len: usize, visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self, _name: &'static str, _len: usize, visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.value {
            Value::Struct(s) => visitor.visit_map(MapDeserializer::new(s.fields)),
            _ => Err(DeError(format!("expected struct, got {}", self.value.type_name()))),
        }
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self, _name: &'static str, _fields: &'static [&'static str], visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self, _name: &'static str, _variants: &'static [&'static str], visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self.value {
            Value::String(s) => visitor.visit_enum(StringEnumAccess { value: s }),
            Value::Enum(e) => visitor.visit_enum(StringEnumAccess { value: e.value }),
            _ => Err(DeError(format!("expected enum string, got {}", self.value.type_name()))),
        }
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_string(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_unit()
    }
}

// --- Helper: string as enum variant ---

struct StringEnumAccess {
    value: String,
}

impl<'de> EnumAccess<'de> for StringEnumAccess {
    type Error = DeError;
    type Variant = UnitVariantAccess;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self, seed: V,
    ) -> Result<(V::Value, Self::Variant), Self::Error> {
        let val = seed.deserialize(self.value.into_deserializer())?;
        Ok((val, UnitVariantAccess))
    }
}

struct UnitVariantAccess;

impl<'de> VariantAccess<'de> for UnitVariantAccess {
    type Error = DeError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, _seed: T) -> Result<T::Value, Self::Error> {
        Err(DeError("expected unit variant".into()))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error> {
        Err(DeError("expected unit variant".into()))
    }

    fn struct_variant<V: Visitor<'de>>(self, _fields: &'static [&'static str], _visitor: V) -> Result<V::Value, Self::Error> {
        Err(DeError("expected unit variant".into()))
    }
}

// --- Helper: string into_deserializer ---

struct StrDeserializer(String);

impl<'de> serde::Deserializer<'de> for StrDeserializer {
    type Error = DeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_string(self.0)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

trait IntoDeserializer {
    fn into_deserializer(self) -> StrDeserializer;
}

impl IntoDeserializer for String {
    fn into_deserializer(self) -> StrDeserializer {
        StrDeserializer(self)
    }
}

// --- SeqDeserializer ---

struct SeqDeserializer {
    iter: std::vec::IntoIter<Value>,
}

impl SeqDeserializer {
    fn new(vec: Vec<Value>) -> Self {
        Self { iter: vec.into_iter() }
    }
}

impl<'de> SeqAccess<'de> for SeqDeserializer {
    type Error = DeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self, seed: T,
    ) -> Result<Option<T::Value>, Self::Error> {
        match self.iter.next() {
            Some(v) => seed.deserialize(ValueDeserializer::new(v)).map(Some),
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

// --- MapDeserializer ---

struct MapDeserializer {
    iter: indexmap::map::IntoIter<String, Value>,
    pending_value: Option<Value>,
}

impl MapDeserializer {
    fn new(map: IndexMap<String, Value>) -> Self {
        Self {
            iter: map.into_iter(),
            pending_value: None,
        }
    }
}

impl<'de> MapAccess<'de> for MapDeserializer {
    type Error = DeError;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self, seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        match self.iter.next() {
            Some((key, value)) => {
                self.pending_value = Some(value);
                seed.deserialize(StrDeserializer(key)).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self, seed: V,
    ) -> Result<V::Value, Self::Error> {
        let value = self.pending_value.take().expect("next_value called before next_key");
        seed.deserialize(ValueDeserializer::new(value))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

// ============================================================
// Public API: from_value / from_str_as
// ============================================================

/// Deserialize a UZON [`Value`] into any type that implements `serde::Deserialize`.
///
/// ```ignore
/// let config: MyConfig = uzon::from_value(value)?;
/// ```
pub fn from_value<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, DeError> {
    T::deserialize(ValueDeserializer::new(value))
}

// ============================================================
// Deserialize INTO Value (e.g. from JSON)
// ============================================================

/// Visitor that builds a [`Value`] from any serde data source.
struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("any valid value")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }

    fn visit_i8<E: de::Error>(self, v: i8) -> Result<Value, E> { Ok(Value::int(v as i128)) }
    fn visit_i16<E: de::Error>(self, v: i16) -> Result<Value, E> { Ok(Value::int(v as i128)) }
    fn visit_i32<E: de::Error>(self, v: i32) -> Result<Value, E> { Ok(Value::int(v as i128)) }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> { Ok(Value::int(v as i128)) }
    fn visit_i128<E: de::Error>(self, v: i128) -> Result<Value, E> { Ok(Value::int(v)) }

    fn visit_u8<E: de::Error>(self, v: u8) -> Result<Value, E> { Ok(Value::int(v as i128)) }
    fn visit_u16<E: de::Error>(self, v: u16) -> Result<Value, E> { Ok(Value::int(v as i128)) }
    fn visit_u32<E: de::Error>(self, v: u32) -> Result<Value, E> { Ok(Value::int(v as i128)) }
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> { Ok(Value::int(v as i128)) }

    fn visit_f32<E: de::Error>(self, v: f32) -> Result<Value, E> { Ok(Value::float(v as f64)) }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> { Ok(Value::float(v)) }

    fn visit_char<E: de::Error>(self, v: char) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.to_string()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }

    fn visit_none<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        Value::deserialize(deserializer)
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut elements = Vec::new();
        while let Some(elem) = seq.next_element()? {
            elements.push(elem);
        }
        Ok(Value::list(elements))
    }

    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut fields = IndexMap::new();
        while let Some((key, value)) = map.next_entry()? {
            fields.insert(key, value);
        }
        Ok(Value::Struct(crate::value::UzonStruct::new(fields)))
    }
}

impl<'de> serde::Deserialize<'de> for Value {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ValueVisitor)
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{UzonFloat, UzonInteger, UzonList};

    #[test]
    fn test_serialize_primitives() {
        assert_eq!(serde_json::to_string(&Value::Null).unwrap(), "null");
        assert_eq!(serde_json::to_string(&Value::Bool(true)).unwrap(), "true");
        assert_eq!(serde_json::to_string(&Value::Integer(UzonInteger::new(42))).unwrap(), "42");
        assert_eq!(serde_json::to_string(&Value::Float(UzonFloat::new(3.14))).unwrap(), "3.14");
        assert_eq!(serde_json::to_string(&Value::String("hello".into())).unwrap(), "\"hello\"");
    }

    #[test]
    fn test_serialize_list() {
        let v = Value::List(UzonList::new(vec![Value::Integer(UzonInteger::new(1)), Value::Integer(UzonInteger::new(2))]));
        assert_eq!(serde_json::to_string(&v).unwrap(), "[1,2]");
    }

    #[test]
    fn test_serialize_struct() {
        let mut map = IndexMap::new();
        map.insert("a".into(), Value::Integer(UzonInteger::new(1)));
        let v = Value::Struct(crate::value::UzonStruct::new(map));
        assert_eq!(serde_json::to_string(&v).unwrap(), r#"{"a":1}"#);
    }

    #[test]
    fn test_deserialize_into_struct() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Config {
            host: String,
            port: u16,
            debug: bool,
        }

        let mut fields = IndexMap::new();
        fields.insert("host".into(), Value::String("localhost".into()));
        fields.insert("port".into(), Value::Integer(UzonInteger::new(8080)));
        fields.insert("debug".into(), Value::Bool(true));
        let value = Value::Struct(crate::value::UzonStruct::new(fields));

        let config: Config = from_value(value).unwrap();
        assert_eq!(config, Config {
            host: "localhost".into(),
            port: 8080,
            debug: true,
        });
    }

    #[test]
    fn test_deserialize_nested() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Server {
            host: String,
            port: u16,
        }
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Config {
            server: Server,
            tags: Vec<String>,
        }

        let mut server = IndexMap::new();
        server.insert("host".into(), Value::String("127.0.0.1".into()));
        server.insert("port".into(), Value::Integer(UzonInteger::new(3000)));

        let mut root = IndexMap::new();
        root.insert("server".into(), Value::Struct(crate::value::UzonStruct::new(server)));
        root.insert("tags".into(), Value::List(UzonList::new(vec![
            Value::String("web".into()),
            Value::String("api".into()),
        ])));

        let config: Config = from_value(Value::Struct(crate::value::UzonStruct::new(root))).unwrap();
        assert_eq!(config, Config {
            server: Server { host: "127.0.0.1".into(), port: 3000 },
            tags: vec!["web".into(), "api".into()],
        });
    }

    #[test]
    fn test_deserialize_option() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Data {
            name: String,
            label: Option<String>,
        }

        let mut fields = IndexMap::new();
        fields.insert("name".into(), Value::String("test".into()));
        fields.insert("label".into(), Value::Null);
        let data: Data = from_value(Value::Struct(crate::value::UzonStruct::new(fields))).unwrap();
        assert_eq!(data, Data { name: "test".into(), label: None });
    }

    #[test]
    fn test_deserialize_enum() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        enum Color {
            #[serde(rename = "red")]
            Red,
            #[serde(rename = "green")]
            Green,
            #[serde(rename = "blue")]
            Blue,
        }

        let v = Value::String("red".into());
        let c: Color = from_value(v).unwrap();
        assert_eq!(c, Color::Red);
    }

    #[test]
    fn test_deserialize_vec_of_ints() {
        let v = Value::List(UzonList::new(vec![
            Value::Integer(UzonInteger::new(1)),
            Value::Integer(UzonInteger::new(2)),
            Value::Integer(UzonInteger::new(3)),
        ]));
        let nums: Vec<i32> = from_value(v).unwrap();
        assert_eq!(nums, vec![1, 2, 3]);
    }

    #[test]
    fn test_deserialize_f64() {
        let v = Value::Float(UzonFloat::new(2.718));
        let f: f64 = from_value(v).unwrap();
        assert!((f - 2.718).abs() < 1e-10);
    }

    #[test]
    fn test_deserialize_error() {
        let v = Value::Bool(true);
        let result: Result<String, _> = from_value(v);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expected string"));
    }

    // --- Deserialize INTO Value (from JSON) ---

    #[test]
    fn test_json_to_value_primitives() {
        let v: Value = serde_json::from_str("null").unwrap();
        assert_eq!(v, Value::Null);

        let v: Value = serde_json::from_str("true").unwrap();
        assert_eq!(v, Value::Bool(true));

        let v: Value = serde_json::from_str("42").unwrap();
        assert_eq!(v, Value::int(42));

        let v: Value = serde_json::from_str("3.14").unwrap();
        assert!(matches!(v, Value::Float(_)));

        let v: Value = serde_json::from_str(r#""hello""#).unwrap();
        assert_eq!(v, Value::String("hello".into()));
    }

    #[test]
    fn test_json_to_value_array() {
        let v: Value = serde_json::from_str("[1, 2, 3]").unwrap();
        assert_eq!(v.len(), Some(3));
        assert_eq!(v.get_index(0), Some(&Value::int(1)));
    }

    #[test]
    fn test_json_to_value_object() {
        let v: Value = serde_json::from_str(r#"{"name": "Alice", "age": 30}"#).unwrap();
        assert_eq!(v.get("name"), Some(&Value::String("Alice".into())));
        assert_eq!(v.get("age"), Some(&Value::int(30)));
    }

    #[test]
    fn test_json_to_value_nested() {
        let json = r#"{
            "server": {"host": "localhost", "port": 8080},
            "tags": ["web", "api"]
        }"#;
        let v: Value = serde_json::from_str(json).unwrap();
        assert_eq!(v.get_path("server.host"), Some(&Value::String("localhost".into())));
        assert_eq!(v.get_path("server.port"), Some(&Value::int(8080)));
        assert_eq!(v.get("tags").unwrap().get_index(1), Some(&Value::String("api".into())));
    }

    #[test]
    fn test_json_roundtrip() {
        let original = r#"{"a":1,"b":[true,null,"hello"],"c":{"x":3.14}}"#;
        let v: Value = serde_json::from_str(original).unwrap();
        let back = serde_json::to_string(&v).unwrap();
        assert_eq!(back, original);
    }
}
