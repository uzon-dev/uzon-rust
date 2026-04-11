// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! Example: Building UZON values programmatically in Rust.
//!
//! Demonstrates `uzon!` macro, `StructBuilder`, `From` conversions,
//! and direct construction.

use uzon::{uzon, Value};

fn main() {
    // ── 1. The `uzon!` macro ─────────────────────────────────────────
    //
    // Build values using JSON-like syntax. This is the most ergonomic
    // way to create UZON values in Rust.

    println!("=== uzon! macro ===");

    // Primitives
    let null = uzon!(null);
    let boolean = uzon!(true);
    let integer = uzon!(42);
    let float = uzon!(3.14);
    let string = uzon!("hello");

    println!("  null:    {null}");
    println!("  bool:    {boolean}");
    println!("  integer: {integer}");
    println!("  float:   {float}");
    println!("  string:  {string}");

    // Lists
    let numbers = uzon!([1, 2, 3, 4, 5]);
    let mixed = uzon!(["hello", "world"]);
    let nested_list = uzon!([[1, 2], [3, 4]]);

    println!("  numbers: {numbers}");
    println!("  mixed:   {mixed}");
    println!("  nested:  {nested_list}");

    // Tuples
    let pair = uzon!((42, "answer"));
    let triple = uzon!((true, 3.14, "yes"));

    println!("  pair:    {pair}");
    println!("  triple:  {triple}");

    // Structs
    let person = uzon!({
        "name": "Alice",
        "age": 30,
        "active": true
    });

    println!("  person:  {person}");

    // Nested structs
    let config = uzon!({
        "server": {
            "host": "localhost",
            "port": 8080
        },
        "database": {
            "url": "postgres://localhost/mydb",
            "pool_size": 10
        },
        "debug": false
    });

    println!("  config:  {config}");
    println!("    server.host = {}", config["server"]["host"]);
    println!("    db.pool_size = {}", config["database"]["pool_size"]);

    // Variables in macro
    let host = "example.com";
    let port = 443;
    let server = uzon!({
        "host": host,
        "port": port,
        "tls": true
    });
    println!("  dynamic: {server}");

    // ── 2. StructBuilder ─────────────────────────────────────────────
    //
    // Fluent API for building structs field by field.

    println!("\n=== StructBuilder ===");

    let user = Value::struct_builder()
        .field("id", 1001)
        .field("name", "Bob")
        .field("email", "bob@example.com")
        .field("scores", vec![Value::int(95), Value::int(88), Value::int(92)])
        .field("address", Value::struct_builder()
            .field("city", "Seoul")
            .field("country", "KR")
            .build())
        .build();

    println!("  user = {user}");
    println!("  user.name = {}", user["name"]);
    println!("  user.address.city = {}", user.get_path("address.city").unwrap());

    // ── 3. From conversions ──────────────────────────────────────────
    //
    // Standard Rust types can be converted to Value via From/Into.

    println!("\n=== From conversions ===");

    // Booleans
    let v: Value = true.into();
    println!("  bool:   {v}");

    // Integers (various widths)
    let v: Value = 42i32.into();
    println!("  i32:    {v}");
    let v: Value = 42i64.into();
    println!("  i64:    {v}");
    let v: Value = 42i128.into();
    println!("  i128:   {v}");
    let v: Value = 42u32.into();
    println!("  u32:    {v}");
    let v: Value = 42u64.into();
    println!("  u64:    {v}");

    // Floats
    let v: Value = 3.14f32.into();
    println!("  f32:    {v}");
    let v: Value = 3.14f64.into();
    println!("  f64:    {v}");

    // Strings
    let v: Value = "hello".into();
    println!("  &str:   {v}");
    let v: Value = String::from("world").into();
    println!("  String: {v}");

    // Vectors become lists
    let v: Value = vec![Value::int(1), Value::int(2), Value::int(3)].into();
    println!("  Vec:    {v}");

    // Tuples (2 and 3 elements)
    let v: Value = (Value::int(1), Value::from("two")).into();
    println!("  tuple2: {v}");
    let v: Value = (Value::int(1), Value::int(2), Value::int(3)).into();
    println!("  tuple3: {v}");

    // ── 4. Convenience constructors ──────────────────────────────────

    println!("\n=== Convenience constructors ===");

    let i = Value::int(42);
    let f = Value::float(3.14);
    let l = Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]);

    println!("  int:  {i}");
    println!("  float: {f}");
    println!("  list: {l}");

    // ── 5. Combining approaches ──────────────────────────────────────
    //
    // Mix and match construction methods freely.

    println!("\n=== Combined construction ===");

    let app_config = Value::struct_builder()
        .field("version", "1.0.0")
        .field("servers", uzon!([
            {"host": "primary.example.com", "port": 443},
            {"host": "backup.example.com", "port": 443}
        ]))
        .field("features", uzon!({
            "auth": true,
            "logging": true,
            "metrics": false
        }))
        .field("limits", Value::struct_builder()
            .field("max_connections", 1000)
            .field("timeout_ms", 5000)
            .build())
        .build();

    println!("  app_config = {app_config}");
    println!("  servers[0].host = {}", app_config["servers"][0]["host"]);
    println!("  features.auth = {}", app_config["features"]["auth"]);
    println!("  limits.timeout = {}", app_config["limits"]["timeout_ms"]);

    println!("\nDone!");
}
