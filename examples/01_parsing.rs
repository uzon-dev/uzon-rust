// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! Example: Parsing UZON strings into Values.
//!
//! Demonstrates `from_str`, `from_str_plain`, and `from_path` — the three
//! primary entry points for reading UZON data.

use uzon::{from_str, from_str_plain, Value};

fn main() {
    // ── 1. Basic parsing with `from_str` ──────────────────────────────
    //
    // `from_str` parses a UZON document and returns a BTreeMap<String, Value>.
    // Each top-level binding becomes a key in the map.

    let source = r#"
        name is "Alice"
        age is 30
        active is true
    "#;

    let bindings = from_str(source).expect("parse failed");

    println!("=== from_str ===");
    for (key, value) in &bindings {
        println!("  {key} = {value}");
    }
    // Output:
    //   name = Alice
    //   age = 30
    //   active = true

    // ── 2. Accessing individual bindings ──────────────────────────────

    let name = bindings.get("name").expect("missing 'name'");
    let age = bindings.get("age").expect("missing 'age'");

    assert_eq!(name.as_str(), Some("Alice"));
    assert_eq!(age.as_i64(), Some(30));

    // ── 3. Rich UZON types: enums, tuples, structs ───────────────────
    //
    // `from_str` preserves full UZON type information: enums carry their
    // variant set, tuples are distinct from lists, etc.

    let source = r#"
        _status is active from active, inactive, banned called Status
        user_status is active as Status

        point is (10, 20)

        server is {
            host is "localhost",
            port is 8080
        }
    "#;

    let bindings = from_str(source).expect("parse failed");

    println!("\n=== Rich types ===");

    // Enums preserve variant information
    if let Value::Enum(e) = &bindings["user_status"] {
        println!("  user_status = {} (variants: {:?})", e.value, e.variants);
    }

    // Tuples are distinct from lists
    if let Value::Tuple(t) = &bindings["point"] {
        println!("  point = ({}, {})", t.elements[0], t.elements[1]);
    }

    // Structs are ordered maps
    let server = &bindings["server"];
    println!("  server.host = {}", server["host"]);
    println!("  server.port = {}", server["port"]);

    // ── 4. Plain mode with `from_str_plain` ──────────────────────────
    //
    // `from_str_plain` strips UZON-specific wrappers:
    //   - Enums → strings
    //   - Tuples → lists
    //   - Unions/TaggedUnions → their inner values

    let source = r#"
        _status is active from active, inactive called Status
        current is active as Status
        coords is (10, 20)
    "#;

    let plain = from_str_plain(source).expect("parse failed");

    println!("\n=== from_str_plain ===");

    // Enum became a plain string
    assert!(matches!(&plain["current"], Value::String(s) if s == "active"));
    println!("  current = {} (type: {})", plain["current"], plain["current"].type_name());

    // Tuple became a list
    assert!(matches!(&plain["coords"], Value::List(_)));
    println!("  coords = {} (type: {})", plain["coords"], plain["coords"].type_name());

    // ── 5. All value types ───────────────────────────────────────────

    let source = r#"
        n is null
        b is true
        i is 42
        f is 3.14
        s is "hello"
        list is [1, 2, 3]
        tuple is (true, "mixed", 42)
        nested is {
            x is 1,
            y is { z is 2 }
        }
    "#;

    let bindings = from_str(source).expect("parse failed");

    println!("\n=== All value types ===");
    for (key, value) in &bindings {
        println!("  {key}: {value} ({})", value.type_name());
    }

    // ── 6. Numeric types with annotations ────────────────────────────

    let source = r#"
        default_int is 42
        typed_u8 is 255 as u8
        hex is 0xFF
        binary is 0b1010
        octal is 0o17
        big is 1_000_000
        default_float is 3.14
        typed_f32 is 1.5 as f32
        scientific is 6.0e2
    "#;

    let bindings = from_str(source).expect("parse failed");

    println!("\n=== Numeric types ===");
    for (key, value) in &bindings {
        match value {
            Value::Integer(n) => {
                println!("  {key} = {} (type: {})", n.value, n.type_ann.display_name());
            }
            Value::Float(f) => {
                println!("  {key} = {} (type: {})", f.value, f.type_ann.display_name());
            }
            _ => println!("  {key} = {value}"),
        }
    }

    // ── 7. Error handling ────────────────────────────────────────────

    let bad_source = r#"x is 1 ++"#;
    match from_str(bad_source) {
        Ok(_) => println!("unexpected success"),
        Err(e) => println!("\n=== Error handling ===\n  Parse error: {e}"),
    }

    println!("\nDone!");
}
