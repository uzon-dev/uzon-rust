// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! Example: Comparing UZON values.
//!
//! Demonstrates PartialEq and PartialOrd between Value instances
//! and with Rust primitives.

use uzon::Value;

fn main() {
    // ── 1. Equality: Value vs Value ──────────────────────────────────

    println!("=== Value equality ===");

    assert_eq!(Value::int(42), Value::int(42));
    assert_ne!(Value::int(42), Value::int(43));
    assert_eq!(Value::from("hello"), Value::from("hello"));
    assert_eq!(Value::Bool(true), Value::Bool(true));
    assert_eq!(Value::Null, Value::Null);
    assert_eq!(Value::float(3.14), Value::float(3.14));

    // Different types are never equal
    assert_ne!(Value::int(1), Value::float(1.0));
    assert_ne!(Value::int(0), Value::Bool(false));
    assert_ne!(Value::from("42"), Value::int(42));

    println!("  int(42) == int(42):   {}", Value::int(42) == Value::int(42));
    println!("  int(42) == int(43):   {}", Value::int(42) == Value::int(43));
    println!("  int(1) == float(1.0): {}", Value::int(1) == Value::float(1.0));
    println!("  null == null:         {}", Value::Null == Value::Null);

    // ── 2. Equality: Value vs primitives ─────────────────────────────
    //
    // Compare directly with Rust types — no explicit conversion needed.

    println!("\n=== Primitive equality ===");

    // Integer comparisons
    assert!(Value::int(42) == 42);        // i32
    assert!(Value::int(42) == 42i64);     // i64
    assert!(Value::int(42) == 42i128);    // i128
    assert!(Value::int(42) == 42u32);     // u32
    assert!(Value::int(42) == 42u64);     // u64

    // Float comparison
    assert!(Value::float(3.14) == 3.14);  // f64

    // Bool comparison
    assert!(Value::Bool(true) == true);
    assert!(Value::Bool(false) == false);

    // String comparison
    assert!(Value::from("hello") == "hello");               // &str
    assert!(Value::from("hello") == String::from("hello")); // String

    // Inequality
    assert!(Value::int(42) != 43);
    assert!(Value::from("hello") != "world");

    println!("  Value::int(42) == 42:        {}", Value::int(42) == 42);
    println!("  Value::float(π) == 3.14:     {}", Value::float(3.14) == 3.14);
    println!("  Value::Bool(true) == true:   {}", Value::Bool(true) == true);
    println!("  Value::from(\"hi\") == \"hi\":   {}", Value::from("hi") == "hi");

    // ── 3. Ordering: Value vs Value ──────────────────────────────────
    //
    // Ordering works for numeric, string, and bool values.
    // Cross-type numeric comparison (int vs float) is supported.

    println!("\n=== Value ordering ===");

    // Integer ordering
    assert!(Value::int(1) < Value::int(2));
    assert!(Value::int(5) > Value::int(3));
    assert!(Value::int(5) >= Value::int(5));
    assert!(Value::int(5) <= Value::int(5));

    // Float ordering
    assert!(Value::float(1.0) < Value::float(2.0));

    // Mixed int/float ordering (int is promoted to float for comparison)
    assert!(Value::int(1) < Value::float(1.5));
    assert!(Value::float(0.5) < Value::int(1));

    // String ordering (lexicographic)
    assert!(Value::from("apple") < Value::from("banana"));
    assert!(Value::from("z") > Value::from("a"));

    // Bool ordering (false < true)
    assert!(Value::Bool(false) < Value::Bool(true));

    // Incompatible types → no ordering (partial_cmp returns None)
    assert_eq!(Value::int(1).partial_cmp(&Value::Bool(true)), None);
    assert_eq!(Value::from("1").partial_cmp(&Value::int(1)), None);

    println!("  int(1) < int(2):       {}", Value::int(1) < Value::int(2));
    println!("  int(1) < float(1.5):   {}", Value::int(1) < Value::float(1.5));
    println!("  \"apple\" < \"banana\":    {}", Value::from("apple") < Value::from("banana"));
    println!("  int vs bool ordering:  {:?}", Value::int(1).partial_cmp(&Value::Bool(true)));

    // ── 4. Ordering: Value vs primitives ─────────────────────────────

    println!("\n=== Primitive ordering ===");

    assert!(Value::int(1) < 2);           // i32
    assert!(Value::int(10) > 5i64);       // i64
    assert!(Value::float(1.0) < 2.0);     // f64
    assert!(Value::from("a") < "b");      // &str

    println!("  Value::int(1) < 2:         {}", Value::int(1) < 2);
    println!("  Value::int(10) > 5i64:     {}", Value::int(10) > 5i64);
    println!("  Value::float(1.0) < 2.0:   {}", Value::float(1.0) < 2.0);
    println!("  Value::from(\"a\") < \"b\":   {}", Value::from("a") < "b");

    // ── 5. Practical: sorting and filtering ──────────────────────────

    println!("\n=== Practical: sorting ===");

    let mut scores = vec![
        Value::int(88),
        Value::int(95),
        Value::int(72),
        Value::int(91),
        Value::int(85),
    ];

    scores.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("  sorted: {:?}", scores.iter().map(|v| v.as_i64().unwrap()).collect::<Vec<_>>());

    // Filter: find scores above 85
    let high: Vec<_> = scores.iter().filter(|s| **s > 85).collect();
    println!("  above 85: {:?}", high.iter().map(|v| v.as_i64().unwrap()).collect::<Vec<_>>());

    println!("\nDone!");
}
