// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! Example: Iterating over UZON values.
//!
//! Demonstrates IntoIterator for borrowed and owned values,
//! iter_fields for structs, and len.

use uzon::{uzon, Value};

fn main() {
    // ── 1. Iterating over lists ──────────────────────────────────────
    //
    // `for item in &value` yields &Value for lists, tuples, and struct values.

    println!("=== List iteration ===");

    let numbers = uzon!([10, 20, 30, 40, 50]);

    // Borrowed iteration (value is still usable after the loop)
    print!("  numbers:");
    for item in &numbers {
        print!(" {item}");
    }
    println!();

    // Using iterator methods
    let sum: i64 = numbers.into_iter()
        .filter_map(|v| v.as_i64())
        .sum();
    println!("  sum = {sum}");

    // ── 2. Iterating over tuples ─────────────────────────────────────

    println!("\n=== Tuple iteration ===");

    let record = uzon!((42, "Alice", true));

    print!("  record:");
    for item in &record {
        print!(" {item} ({})", item.type_name());
    }
    println!();

    // ── 3. Iterating over struct values ──────────────────────────────
    //
    // Iterating a struct yields its VALUES (not keys).
    // Use `iter_fields()` for key-value pairs.

    println!("\n=== Struct iteration ===");

    let config = uzon!({
        "host": "localhost",
        "port": 8080,
        "debug": true
    });

    // Values only
    print!("  values:");
    for value in &config {
        print!(" {value}");
    }
    println!();

    // Key-value pairs via iter_fields
    println!("  fields:");
    if let Some(fields) = config.iter_fields() {
        for (key, value) in fields {
            println!("    {key}: {value} ({})", value.type_name());
        }
    }

    // ── 4. len() ─────────────────────────────────────────────────────
    //
    // Returns the number of elements/fields in a collection.
    // Returns None for non-collection types.

    println!("\n=== len() ===");

    let list = uzon!([1, 2, 3, 4, 5]);
    let tuple = uzon!((1, "two", true));
    let strct = uzon!({"a": 1, "b": 2, "c": 3});
    let scalar = Value::int(42);

    println!("  list len:   {:?}", list.len());    // Some(5)
    println!("  tuple len:  {:?}", tuple.len());   // Some(3)
    println!("  struct len: {:?}", strct.len());   // Some(3)
    println!("  scalar len: {:?}", scalar.len());  // None

    // ── 5. Owned iteration (consumes the value) ──────────────────────

    println!("\n=== Owned iteration ===");

    let items = uzon!([1, 2, 3, 4, 5]);

    // into_iter consumes the value — each item is an owned Value
    let doubled: Vec<Value> = items.into_iter()
        .map(|v| v + 2)  // can use arithmetic directly
        .collect();

    println!("  [1,2,3,4,5] + 2 each = {:?}",
        doubled.iter().map(|v| v.as_i64().unwrap()).collect::<Vec<_>>());

    // Owned struct iteration yields owned values
    let data = uzon!({"x": 10, "y": 20, "z": 30});
    let total: i64 = data.into_iter()
        .filter_map(|v| v.as_i64())
        .sum();
    println!("  sum of struct values = {total}");

    // ── 6. Non-iterable values yield empty iterators ─────────────────
    //
    // Iterating over a scalar silently produces nothing — no panic.

    println!("\n=== Non-iterable values ===");

    let scalar = Value::int(42);
    let count = (&scalar).into_iter().count();
    println!("  int(42) iteration count = {count}"); // 0

    let null = Value::Null;
    let count = (&null).into_iter().count();
    println!("  null iteration count = {count}"); // 0

    // ── 7. Practical: processing nested data ─────────────────────────

    println!("\n=== Practical: processing users ===");

    let users = uzon!([
        {"name": "Alice", "score": 95},
        {"name": "Bob", "score": 82},
        {"name": "Carol", "score": 91},
        {"name": "Dave", "score": 78},
        {"name": "Eve", "score": 88}
    ]);

    // Find users with score > 85
    println!("  High scorers (>85):");
    for user in &users {
        if let (Some(name), Some(score)) = (user["name"].as_str(), user["score"].as_i64()) {
            if score > 85 {
                println!("    {name}: {score}");
            }
        }
    }

    // Calculate average score
    let scores: Vec<i64> = (&users).into_iter()
        .filter_map(|u| u["score"].as_i64())
        .collect();
    let avg = scores.iter().sum::<i64>() as f64 / scores.len() as f64;
    println!("  Average score: {avg:.1}");

    // Find max score
    let max = (&users).into_iter()
        .filter_map(|u| u["score"].as_i64())
        .max()
        .unwrap_or(0);
    println!("  Max score: {max}");

    println!("\nDone!");
}
