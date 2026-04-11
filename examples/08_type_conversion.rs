// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! Example: Converting between UZON values and Rust types.
//!
//! Demonstrates TryFrom/TryInto for extracting Rust types from Value,
//! and From/Into for creating Values from Rust types.

use indexmap::IndexMap;
use uzon::{uzon, Value};

fn main() {
    // ── 1. Value → Rust via TryFrom/TryInto ──────────────────────────
    //
    // Fallible conversions — return Err if the type doesn't match.

    println!("=== TryFrom (owned) ===");

    // Bool
    let b: bool = Value::Bool(true).try_into().unwrap();
    println!("  Bool → bool: {b}");

    // Integers
    let n: i64 = Value::int(42).try_into().unwrap();
    println!("  Integer → i64: {n}");

    let n: i128 = Value::int(i128::MAX).try_into().unwrap();
    println!("  Integer → i128: {n}");

    let n: u64 = Value::int(100).try_into().unwrap();
    println!("  Integer → u64: {n}");

    // Float
    let f: f64 = Value::float(3.14).try_into().unwrap();
    println!("  Float → f64: {f}");

    // Integer auto-converts to f64
    let f: f64 = Value::int(10).try_into().unwrap();
    println!("  Integer → f64: {f}");

    // String
    let s: String = Value::from("hello").try_into().unwrap();
    println!("  String → String: {s}");

    // Vec<Value>
    let v: Vec<Value> = uzon!([1, 2, 3]).try_into().unwrap();
    println!("  List → Vec<Value>: {} items", v.len());

    // IndexMap<String, Value>
    let m: IndexMap<String, Value> = uzon!({"a": 1, "b": 2}).try_into().unwrap();
    println!("  Struct → IndexMap: {} entries", m.len());

    // ── 2. TryFrom with borrowed values ──────────────────────────────
    //
    // Convert &Value without consuming it.

    println!("\n=== TryFrom (borrowed) ===");

    let value = Value::int(42);

    // Borrow conversion — original value remains usable
    let n: i64 = (&value).try_into().unwrap();
    println!("  &Integer → i64: {n}");
    println!("  original still usable: {value}");

    let n: i128 = (&value).try_into().unwrap();
    println!("  &Integer → i128: {n}");

    let n: f64 = (&value).try_into().unwrap();
    println!("  &Integer → f64: {n}");

    let value = Value::from("hello");
    let s: &str = (&value).try_into().unwrap();
    println!("  &String → &str: {s}");

    let b: bool = (&Value::Bool(true)).try_into().unwrap();
    println!("  &Bool → bool: {b}");

    // ── 3. Handling conversion errors ────────────────────────────────
    //
    // Type mismatches return ValueConversionError.

    println!("\n=== Conversion errors ===");

    let err = bool::try_from(Value::int(1)).unwrap_err();
    println!("  int→bool: {err}");

    let err = i64::try_from(Value::from("hello")).unwrap_err();
    println!("  string→i64: {err}");

    let err = String::try_from(Value::int(42)).unwrap_err();
    println!("  int→String: {err}");

    // Range errors
    let err = u64::try_from(Value::int(-1)).unwrap_err();
    println!("  int(-1)→u64: {err}");

    let err = i64::try_from(Value::int(i128::MAX)).unwrap_err();
    println!("  i128::MAX→i64: {err}");

    // ── 4. Rust → Value via From/Into ────────────────────────────────
    //
    // Infallible conversions from Rust types to Value.

    println!("\n=== From/Into ===");

    // All these are equivalent ways to create a Value:
    let v1 = Value::from(42);
    let v2: Value = 42.into();
    let v3 = Value::int(42);
    assert_eq!(v1, v2);
    assert_eq!(v2, v3);

    // Supported types:
    let examples: Vec<(&str, Value)> = vec![
        ("bool",   true.into()),
        ("i32",    42i32.into()),
        ("i64",    42i64.into()),
        ("i128",   42i128.into()),
        ("u32",    42u32.into()),
        ("u64",    42u64.into()),
        ("f32",    3.14f32.into()),
        ("f64",    3.14f64.into()),
        ("&str",   "hello".into()),
        ("String", String::from("world").into()),
    ];

    for (name, value) in &examples {
        println!("  {name} → {value} ({})", value.type_name());
    }

    // Collections
    let list: Value = vec![Value::int(1), Value::int(2)].into();
    println!("  Vec → {list}");

    let mut map = IndexMap::new();
    map.insert("x".into(), Value::int(1));
    let strct: Value = map.into();
    println!("  IndexMap → {strct}");

    // Tuples
    let t2: Value = (Value::int(1), Value::from("two")).into();
    println!("  (V, V) → {t2}");
    let t3: Value = (Value::int(1), Value::int(2), Value::int(3)).into();
    println!("  (V, V, V) → {t3}");

    // ── 5. to_plain: stripping UZON wrappers ─────────────────────────

    println!("\n=== to_plain ===");

    // Enums become strings, tuples become lists, unions unwrap
    let source = r#"
        _color is red from red, green, blue called Color
        selected is red as Color
        pair is (1, 2)
    "#;

    let bindings = uzon::from_str(source).expect("parse failed");

    println!("  Before to_plain:");
    for (k, v) in &bindings {
        println!("    {k}: {v} ({})", v.type_name());
    }

    println!("  After to_plain:");
    for (k, v) in bindings {
        let plain = v.to_plain();
        println!("    {k}: {plain} ({})", plain.type_name());
    }

    // ── 6. Practical: function that accepts Value ────────────────────

    println!("\n=== Practical: generic processing ===");

    fn describe(v: &Value) -> String {
        match v.type_name() {
            "null" => "nothing".into(),
            "bool" => format!("boolean: {}", v.as_bool().unwrap()),
            "integer" => format!("number: {}", v.as_i128().unwrap()),
            "float" => format!("decimal: {:.2}", v.as_f64().unwrap()),
            "string" => format!("text: {:?}", v.as_str().unwrap()),
            "list" => format!("list of {} items", v.len().unwrap()),
            "struct" => format!("object with {} fields", v.len().unwrap()),
            other => format!("({other})"),
        }
    }

    let test_values = vec![
        Value::Null,
        Value::Bool(true),
        Value::int(42),
        Value::float(3.14),
        Value::from("hello"),
        uzon!([1, 2, 3]),
        uzon!({"x": 1}),
    ];

    for v in &test_values {
        println!("  {}: {}", v, describe(v));
    }

    println!("\nDone!");
}
