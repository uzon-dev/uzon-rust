// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! Example: Arithmetic operations on UZON values.
//!
//! Demonstrates operator overloading (+, -, *, /, %, -), checked arithmetic,
//! mixed-type arithmetic, and primitive interop.

use uzon::Value;

fn main() {
    // ── 1. Basic arithmetic operators ────────────────────────────────
    //
    // Standard Rust operators work between Value instances.
    // Type mismatches (e.g., int + bool) panic — use checked methods
    // if you need error handling.

    println!("=== Basic arithmetic ===");

    let a = Value::int(10);
    let b = Value::int(3);

    println!("  {a} + {b} = {}", a.clone() + b.clone());
    println!("  {a} - {b} = {}", a.clone() - b.clone());
    println!("  {a} * {b} = {}", a.clone() * b.clone());
    println!("  {a} / {b} = {}", a.clone() / b.clone()); // integer division → 3
    println!("  {a} % {b} = {}", a.clone() % b.clone()); // remainder → 1
    println!("  -{a} = {}", -a.clone());

    // Float arithmetic
    let x = Value::float(10.0);
    let y = Value::float(3.0);

    println!("\n  {x} + {y} = {}", x.clone() + y.clone());
    println!("  {x} / {y} = {}", x.clone() / y.clone()); // 3.3333...
    println!("  -{x} = {}", -x.clone());

    // ── 2. Mixed integer/float arithmetic ────────────────────────────
    //
    // When mixing integer and float, the result is always float.

    println!("\n=== Mixed int/float ===");

    let i = Value::int(5);
    let f = Value::float(2.5);

    let result = i.clone() + f.clone();
    println!("  int({i}) + float({f}) = {result} (type: {})", result.type_name());

    let result = i.clone() * f.clone();
    println!("  int({i}) * float({f}) = {result}");

    // ── 3. String concatenation ──────────────────────────────────────
    //
    // The + operator concatenates strings.

    println!("\n=== String concatenation ===");

    let hello = Value::from("Hello, ");
    let world = Value::from("world!");
    let greeting = hello + world;
    println!("  \"Hello, \" + \"world!\" = {greeting}");

    // ── 4. Arithmetic with Rust primitives ───────────────────────────
    //
    // You can use Rust primitives directly on either side.

    println!("\n=== Primitive arithmetic ===");

    let v = Value::int(100);

    // Value op primitive
    println!("  {v} + 50 = {}", v.clone() + 50);
    println!("  {v} - 30 = {}", v.clone() - 30);
    println!("  {v} * 2 = {}", v.clone() * 2);
    println!("  {v} / 3 = {}", v.clone() / 3);
    println!("  {v} % 7 = {}", v.clone() % 7);

    // Primitive op Value (also works!)
    println!("  50 + {v} = {}", 50 + v.clone());
    println!("  200 - {v} = {}", 200 - v.clone());
    println!("  3 * {v} = {}", 3 * v.clone());

    // Float primitives
    let f = Value::float(10.0);
    println!("\n  {f} + 0.5 = {}", f.clone() + 0.5);
    println!("  {f} * 2.0 = {}", f.clone() * 2.0);
    println!("  0.5 + {f} = {}", 0.5 + f.clone());

    // Int + float primitive → promotes to float
    let i = Value::int(10);
    println!("\n  int({i}) + 0.5 = {}", i.clone() + 0.5);
    println!("  int({i}) * 1.5 = {}", i.clone() * 1.5);

    // String + &str
    let s = Value::from("Hello");
    println!("\n  \"{s}\" + \", world\" = {}", s.clone() + ", world");
    println!("  \"Hi \" + \"Hello\" = {}", "Hi " + s.clone());

    // Various integer widths
    println!("\n  Value + i32:  {}", Value::int(1) + 2i32);
    println!("  Value + i64:  {}", Value::int(1) + 2i64);
    println!("  Value + i128: {}", Value::int(1) + 2i128);
    println!("  Value + u32:  {}", Value::int(1) + 2u32);
    println!("  Value + u64:  {}", Value::int(1) + 2u64);

    // ── 5. Checked arithmetic ────────────────────────────────────────
    //
    // Checked methods return Result instead of panicking.
    // Use these when inputs are untrusted.

    println!("\n=== Checked arithmetic ===");

    let a = Value::int(10);
    let b = Value::int(3);

    match a.checked_add(&b) {
        Ok(result) => println!("  checked_add({a}, {b}) = {result}"),
        Err(e) => println!("  error: {e}"),
    }

    match a.checked_div(&b) {
        Ok(result) => println!("  checked_div({a}, {b}) = {result}"),
        Err(e) => println!("  error: {e}"),
    }

    // Division by zero → error (not panic)
    let zero = Value::int(0);
    match a.checked_div(&zero) {
        Ok(_) => println!("  unexpected success"),
        Err(e) => println!("  checked_div({a}, 0) = Err(\"{e}\")"),
    }

    // Overflow detection
    let max = Value::int(i128::MAX);
    match max.checked_add(&Value::int(1)) {
        Ok(_) => println!("  unexpected success"),
        Err(e) => println!("  i128::MAX + 1 = Err(\"{e}\")"),
    }

    // Type mismatch → error (not panic)
    let bool_val = Value::Bool(true);
    match a.checked_add(&bool_val) {
        Ok(_) => println!("  unexpected success"),
        Err(e) => println!("  int + bool = Err(\"{e}\")"),
    }

    // Checked negation
    match Value::int(42).checked_neg() {
        Ok(result) => println!("  checked_neg(42) = {result}"),
        Err(e) => println!("  error: {e}"),
    }

    // All checked methods:
    //   checked_add, checked_sub, checked_mul,
    //   checked_div, checked_rem, checked_neg
    println!("\n  All checked operations:");
    let x = Value::int(20);
    let y = Value::int(7);
    println!("    add: {:?}", x.checked_add(&y));
    println!("    sub: {:?}", x.checked_sub(&y));
    println!("    mul: {:?}", x.checked_mul(&y));
    println!("    div: {:?}", x.checked_div(&y));
    println!("    rem: {:?}", x.checked_rem(&y));
    println!("    neg: {:?}", x.checked_neg());

    // ── 6. Practical example: aggregating parsed data ────────────────

    println!("\n=== Practical: sum a parsed list ===");

    let scores = vec![
        Value::int(92), Value::int(85), Value::int(78),
        Value::int(95), Value::int(88),
    ];

    let mut total = Value::int(0);
    for score in &scores {
        total = total.checked_add(score).expect("overflow");
    }
    println!("  scores: {:?}", scores.iter().map(|v| v.as_i64().unwrap()).collect::<Vec<_>>());
    println!("  total: {total}");
    println!("  average: {}", total.clone() / Value::int(scores.len() as i128));

    println!("\nDone!");
}
