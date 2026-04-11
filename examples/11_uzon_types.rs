// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! Example: Working with UZON-specific types.
//!
//! Demonstrates enums, unions, tagged unions, typed integers/floats,
//! tuples vs lists, and the to_plain() conversion.

use uzon::{from_str, from_str_plain, Value};

fn main() {
    // ── 1. Enums ─────────────────────────────────────────────────────
    //
    // UZON enums are named variants from a defined set.
    // They preserve the variant set and type name for validation.

    println!("=== Enums ===");

    let source = r#"
        _color is red from red, green, blue called Color
        selected is green as Color
    "#;

    let bindings = from_str(source).expect("parse failed");

    if let Value::Enum(e) = &bindings["selected"] {
        println!("  selected = {}", e.value);
        println!("  variants = {:?}", e.variants);
        println!("  type_name = {:?}", e.type_name);
    }

    // ── 2. Typed integers ────────────────────────────────────────────
    //
    // UZON integers carry type annotations (i8, u16, i32, u64, etc.).

    println!("\n=== Typed integers ===");

    let source = r#"
        plain is 42
        byte is 255 as u8
        small is -128 as i8
        big is 1000000 as i64
    "#;

    let bindings = from_str(source).expect("parse failed");

    for (name, value) in &bindings {
        if let Value::Integer(n) = value {
            println!("  {name} = {} (type: {}, explicit: {})",
                n.value, n.type_ann.display_name(), n.explicit);
        }
    }

    // ── 3. Typed floats ──────────────────────────────────────────────

    println!("\n=== Typed floats ===");

    let source = r#"
        plain_f is 3.14
        single is 1.5 as f32
    "#;

    let bindings = from_str(source).expect("parse failed");

    for (name, value) in &bindings {
        if let Value::Float(f) = value {
            println!("  {name} = {} (type: {}, explicit: {})",
                f.value, f.type_ann.display_name(), f.explicit);
        }
    }

    // ── 4. Tuples vs Lists ───────────────────────────────────────────
    //
    // Tuples: fixed-length, heterogeneous (different types OK)
    // Lists: variable-length, homogeneous (same type expected)

    println!("\n=== Tuples vs Lists ===");

    let source = r#"
        my_tuple is (42, "hello", true)
        my_list is [1, 2, 3, 4, 5]
    "#;

    let bindings = from_str(source).expect("parse failed");

    let tuple = &bindings["my_tuple"];
    let list = &bindings["my_list"];

    println!("  tuple: {tuple} (type: {})", tuple.type_name());
    println!("  list:  {list} (type: {})", list.type_name());

    // Tuple elements can be different types
    if let Value::Tuple(t) = tuple {
        for (i, elem) in t.elements.iter().enumerate() {
            println!("    tuple[{i}] = {elem} ({})", elem.type_name());
        }
    }

    // List elements are typically the same type
    if let Value::List(l) = list {
        println!("    list element_type: {:?}", l.element_type);
        println!("    list len: {}", l.len());
    }

    // ── 5. Tagged unions ─────────────────────────────────────────────
    //
    // Tagged unions associate a value with a variant tag.
    // Each variant can have a different payload type.

    println!("\n=== Tagged unions ===");

    let source = r#"
        result is "success" named ok from ok as string, err as string called Result
    "#;

    let bindings = from_str(source).expect("parse failed");

    if let Value::TaggedUnion(tu) = &bindings["result"] {
        println!("  value = {}", tu.value);
        println!("  tag = {}", tu.tag);
        println!("  variants = {:?}", tu.variants);
        println!("  type_name = {:?}", tu.type_name);
    }

    // ── 6. Null vs Undefined ─────────────────────────────────────────
    //
    // null: "intentionally empty" — the value exists but is explicitly nothing
    // undefined: "does not exist" — checked with `is undefined`

    println!("\n=== Null vs Undefined ===");

    // In UZON, null is an explicit value; undefined means "does not exist".
    // undefined is typically the result of accessing a missing field.
    let source = r#"
        present is null
        data is { x is 1 }
        missing is data.y
    "#;

    let bindings = from_str(source).expect("parse failed");

    let null_val = &bindings["present"];
    let undef_val = &bindings["missing"];

    println!("  null: is_null={}, is_undefined={}, type={}", null_val.is_null(), null_val.is_undefined(), null_val.type_name());
    println!("  undefined: is_null={}, is_undefined={}, type={}", undef_val.is_null(), undef_val.is_undefined(), undef_val.type_name());

    // You can also construct undefined directly in Rust:
    let undef = Value::Undefined;
    println!("  Value::Undefined: is_null={}, is_undefined={}", undef.is_null(), undef.is_undefined());

    // ── 7. to_plain() — stripping UZON wrappers ─────────────────────
    //
    // Converts UZON-specific types to simpler representations:
    //   Enum → String
    //   Tuple → List
    //   Union → inner value
    //   TaggedUnion → inner value

    println!("\n=== to_plain() ===");

    let source = r#"
        _status is active from active, inactive called Status
        current is active as Status
        pair is (1, 2)
        simple is 42
        text is "hello"
    "#;

    let bindings = from_str(source).expect("parse failed");

    println!("  Before to_plain:");
    for (k, v) in &bindings {
        println!("    {k}: {v} ({})", v.type_name());
    }

    println!("  After to_plain:");
    for (k, v) in bindings {
        let plain = v.to_plain();
        println!("    {k}: {plain} ({})", plain.type_name());
    }

    // ── 8. from_str vs from_str_plain ────────────────────────────────
    //
    // from_str: preserves UZON type wrappers (for UZON-aware code)
    // from_str_plain: auto-calls to_plain() (for Rust-native consumption)

    println!("\n=== from_str vs from_str_plain ===");

    let source = r#"
        _color is red from red, green, blue called Color
        choice is red as Color
        point is (10, 20)
    "#;

    let rich = from_str(source).expect("parse");
    let plain = from_str_plain(source).expect("parse");

    println!("  from_str (rich):");
    for (k, v) in &rich {
        println!("    {k}: {} ({})", v, v.type_name());
    }

    println!("  from_str_plain:");
    for (k, v) in &plain {
        println!("    {k}: {} ({})", v, v.type_name());
    }

    // ── 9. Inspecting numeric type annotations ───────────────────────

    println!("\n=== Numeric type annotations ===");

    let source = r#"
        a is 42
        b is 42 as u8
        c is 42 as i32
        d is 42 as u64
        e is 3.14
        f is 3.14 as f32
    "#;

    let bindings = from_str(source).expect("parse failed");

    for (name, value) in &bindings {
        match value {
            Value::Integer(n) => {
                let range = n.type_ann.range();
                println!("  {name}: {} as {} (range: {:?}, explicit: {})",
                    n.value, n.type_ann.display_name(), range, n.explicit);
            }
            Value::Float(f) => {
                println!("  {name}: {} as {} (explicit: {})",
                    f.value, f.type_ann.display_name(), f.explicit);
            }
            _ => {}
        }
    }

    println!("\nDone!");
}
