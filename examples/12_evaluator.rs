// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! Example: Advanced evaluator usage.
//!
//! Demonstrates the Evaluator struct, EvalOptions, UZON expressions
//! (arithmetic, string interpolation, conditionals, functions),
//! and struct operations (extends, with).

use uzon::from_str;

fn main() {
    // ── 1. UZON expressions ─────────────────────────────────────────
    //
    // UZON is not just a data format — it supports computed values.

    println!("=== Expressions ===");

    let source = r#"
        a is 10
        b is 20
        sum is self.a + self.b
        product is self.a * self.b
        negative is -self.a
        complex is (self.a + self.b) * 2 - 5
    "#;

    let bindings = from_str(source).expect("parse failed");

    for (k, v) in &bindings {
        println!("  {k} = {v}");
    }

    // ── 2. String operations ─────────────────────────────────────────

    println!("\n=== String operations ===");

    let source = r#"
        first is "Hello"
        last is "World"
        greeting is self.first ++ ", " ++ self.last ++ "!"
        repeated is self.first ** 3
        interpolated is "Result: {self.first ++ " " ++ self.last}"
    "#;

    let bindings = from_str(source).expect("parse failed");

    for (k, v) in &bindings {
        println!("  {k} = {v}");
    }

    // ── 3. Conditionals ─────────────────────────────────────────────

    println!("\n=== Conditionals ===");

    let source = r#"
        score is 85
        grade is if self.score >= 90 then "A"
                 else if self.score >= 80 then "B"
                 else if self.score >= 70 then "C"
                 else "F"
        passed is self.score >= 60
    "#;

    let bindings = from_str(source).expect("parse failed");

    for (k, v) in &bindings {
        println!("  {k} = {v}");
    }

    // ── 4. Struct extends and with ───────────────────────────────────
    //
    // `extends`: add/override fields (creates new struct)
    // `with`: copy struct and replace specific fields

    println!("\n=== Struct extends / with ===");

    let source = r#"
        base is {
            host is "localhost",
            port is 8080,
            debug is false
        }
        production is self.base extends {
            host is "prod.example.com",
            port is 443,
            tls is true
        }
        staging is self.base with { host is "staging.example.com" }
    "#;

    let bindings = from_str(source).expect("parse failed");

    println!("  base:");
    for (k, v) in bindings["base"].as_struct().unwrap() {
        println!("    {k} = {v}");
    }

    println!("  production (extends base):");
    for (k, v) in bindings["production"].as_struct().unwrap() {
        println!("    {k} = {v}");
    }

    println!("  staging (with override):");
    for (k, v) in bindings["staging"].as_struct().unwrap() {
        println!("    {k} = {v}");
    }

    // ── 5. Functions ─────────────────────────────────────────────────
    //
    // UZON supports first-class functions with type annotations.

    println!("\n=== Functions ===");

    let source = r#"
        add is function a as i32, b as i32 returns i32 { a + b }
        result is self.add(3, 4)

        double is function x as i32 returns i32 { x * 2 }
        doubled is self.double(21)
    "#;

    let bindings = from_str(source).expect("parse failed");

    println!("  add(3, 4) = {}", bindings["result"]);
    println!("  double(21) = {}", bindings["doubled"]);

    // ── 6. Self-references and declarative ordering ──────────────────
    //
    // UZON bindings are declarative — order doesn't matter.
    // `self.` references resolve regardless of declaration order.

    println!("\n=== Declarative ordering ===");

    let source = r#"
        greeting is "Hello, " ++ self.name ++ "!"
        name is "Alice"
        full is self.greeting ++ " Age: " ++ (self.age to string)
        age is 30
    "#;

    let bindings = from_str(source).expect("parse failed");

    for (k, v) in &bindings {
        println!("  {k} = {v}");
    }

    // ── 7. Standard library functions ────────────────────────────────
    //
    // UZON provides std.* built-in functions.

    println!("\n=== Standard library ===");

    let source = r#"
        items is [3, 1, 4, 1, 5, 9, 2, 6]
        count is std.len(self.items)
        sorted is std.sort(self.items, function a as i64, b as i64 returns bool { a < b })
        has_5 is std.has(self.items, 5)
        has_7 is std.has(self.items, 7)

        text is "  Hello, World!  "
        trimmed is std.trim(self.text)
        upper is std.upper(self.text)
        lower is std.lower(self.text)

        words is std.split("one,two,three", ",")
        joined is std.join(self.words, " | ")

        keys is std.keys({ a is 1, b is 2, c is 3 })
        vals is std.values({ x is 10, y is 20 })
    "#;

    let bindings = from_str(source).expect("parse failed");

    for (k, v) in &bindings {
        println!("  {k} = {v}");
    }

    // ── 8. Null handling ─────────────────────────────────────────────
    //
    // `or else` provides fallback values for null/undefined.

    println!("\n=== Null handling ===");

    // `or else` provides fallback for *undefined* values (missing fields).
    // null stays null — it's an intentional value, not a missing one.

    let source = r#"
        config is {
            host is "localhost",
            port is null
        }
        host is self.config.host or else "0.0.0.0"
        port is self.config.port or else 8080
        missing is self.config.timeout or else 30
    "#;

    let bindings = from_str(source).expect("parse failed");

    println!("  host = {} (present → used as-is)", bindings["host"]);
    println!("  port = {} (null stays null — it's intentional)", bindings["port"]);
    println!("  missing = {} (undefined → fallback applied)", bindings["missing"]);

    // ── 9. List operations (map, filter, reduce) ─────────────────────

    println!("\n=== List operations ===");

    let source = r#"
        numbers is [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        doubled is std.map(self.numbers, function x as i64 returns i64 { x * 2 })
        evens is std.filter(self.numbers, function x as i64 returns bool { x % 2 is 0 })
        total is std.reduce(self.numbers, 0, function acc as i64, x as i64 returns i64 { acc + x })
    "#;

    let bindings = from_str(source).expect("parse failed");

    println!("  numbers = {}", bindings["numbers"]);
    println!("  doubled = {}", bindings["doubled"]);
    println!("  evens   = {}", bindings["evens"]);
    println!("  total   = {}", bindings["total"]);

    // ── 10. Comparison and logical operators ─────────────────────────

    println!("\n=== Comparisons and logic ===");

    let source = r#"
        x is 42
        gt is self.x > 40
        lt is self.x < 50
        eq is self.x is 42
        ne is self.x is not 0
        both is self.gt and self.lt
        either is self.x < 0 or self.x > 0
    "#;

    let bindings = from_str(source).expect("parse failed");

    for (k, v) in &bindings {
        println!("  {k} = {v}");
    }

    println!("\nDone!");
}
