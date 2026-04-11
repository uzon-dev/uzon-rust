// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! Example: Converting UZON values back to UZON text.
//!
//! Demonstrates `to_string`, `to_string_with_options`, and `StringifyOptions`.

use std::collections::BTreeMap;
use uzon::{from_str, to_string, to_string_with_options, StringifyOptions, Value};

fn main() {
    // ── 1. Basic stringification ─────────────────────────────────────
    //
    // `to_string` converts a BTreeMap<String, Value> back to UZON text.

    println!("=== Basic to_string ===");

    let source = r#"
        name is "Alice"
        age is 30
        active is true
    "#;

    let bindings = from_str(source).expect("parse failed");
    let output = to_string(&bindings);

    println!("{output}");

    // ── 2. Complex values roundtrip ──────────────────────────────────

    println!("=== Complex values ===");

    let source = r#"
        server is {
            host is "localhost",
            port is 8080,
            tls is false
        }
        tags is ["web", "api", "v2"]
        limits is {
            max_conn is 1000,
            timeout is 30
        }
    "#;

    let bindings = from_str(source).expect("parse failed");
    let output = to_string(&bindings);
    println!("{output}");

    // ── 3. Custom formatting with StringifyOptions ───────────────────
    //
    // Control indentation and inline threshold.

    println!("=== Custom options ===");

    let source = r#"
        config is {
            server is {
                host is "localhost",
                port is 8080
            },
            features is {
                auth is true,
                logging is true,
                metrics is false,
                tracing is false,
                profiling is false
            }
        }
    "#;

    let bindings = from_str(source).expect("parse failed");

    // Default: 4-space indent, inline threshold 4
    println!("--- Default (indent=4, inline=4) ---");
    println!("{}", to_string(&bindings));

    // 2-space indent
    println!("--- 2-space indent ---");
    let opts = StringifyOptions { indent: 2, ..Default::default() };
    println!("{}", to_string_with_options(&bindings, &opts));

    // Higher inline threshold — more fields shown on one line
    println!("--- inline_threshold=8 ---");
    let opts = StringifyOptions { inline_threshold: 8, ..Default::default() };
    println!("{}", to_string_with_options(&bindings, &opts));

    // Lower inline threshold — each field on its own line
    println!("--- inline_threshold=1 ---");
    let opts = StringifyOptions { inline_threshold: 1, ..Default::default() };
    println!("{}", to_string_with_options(&bindings, &opts));

    // ── 4. Roundtrip: parse → stringify → parse ──────────────────────
    //
    // UZON supports faithful roundtripping of all value types.

    println!("=== Roundtrip ===");

    let source = r#"
        i is 42
        f is 3.14
        s is "hello\nworld"
        b is true
        n is null
        list is [1, 2, 3]
        pair is (true, "yes")
    "#;

    let original = from_str(source).expect("parse 1");
    let text = to_string(&original);
    let reparsed = from_str(&text).expect("parse 2");

    println!("  Original bindings:");
    for (k, v) in &original {
        println!("    {k} = {v}");
    }
    println!("  Stringified:");
    println!("{text}");

    // Verify the roundtrip
    assert_eq!(original, reparsed, "roundtrip mismatch!");
    println!("  Roundtrip: OK");

    // ── 5. Stringifying programmatically built values ────────────────

    println!("\n=== Programmatic values ===");

    let mut bindings = BTreeMap::new();
    bindings.insert("version".into(), Value::from("1.0.0"));
    bindings.insert("count".into(), Value::int(42));
    bindings.insert("items".into(), Value::list(vec![
        Value::from("alpha"),
        Value::from("beta"),
        Value::from("gamma"),
    ]));

    let output = to_string(&bindings);
    println!("{output}");

    println!("Done!");
}
