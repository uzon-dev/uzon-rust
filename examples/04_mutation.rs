// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! Example: Mutating UZON values in place.
//!
//! Demonstrates insert, remove, push, pop, get_mut, merge, and mutable accessors.

use uzon::{uzon, Value};

fn main() {
    // ── 1. Struct mutation: insert and remove ────────────────────────

    println!("=== Struct mutation ===");

    let mut config = uzon!({
        "host": "localhost",
        "port": 8080
    });
    println!("  initial:  {config}");

    // Insert a new field
    config.insert("debug", true);
    println!("  +debug:   {config}");

    // Update an existing field (returns old value)
    let old = config.insert("port", 3000);
    println!("  port→3000: {config} (old port was {old:?})");

    // Remove a field (returns the removed value)
    let removed = config.remove("debug");
    println!("  -debug:   {config} (removed {removed:?})");

    // ── 2. List mutation: push and pop ───────────────────────────────

    println!("\n=== List mutation ===");

    let mut items = uzon!([1, 2, 3]);
    println!("  initial: {items}");

    items.push(4);
    items.push(5);
    println!("  +4 +5:   {items}");

    let popped = items.pop();
    println!("  pop:     {items} (got {popped:?})");

    // Push different types
    let mut mixed = Value::list(vec![]);
    mixed.push("hello");
    mixed.push(42);
    mixed.push(true);
    println!("  mixed:   {mixed}");

    // ── 3. Mutable references with get_mut ───────────────────────────

    println!("\n=== get_mut ===");

    let mut data = uzon!({
        "count": 0,
        "items": [10, 20, 30]
    });
    println!("  initial: {data}");

    // Modify a struct field in place
    if let Some(count) = data.get_mut("count") {
        *count = Value::int(42);
    }
    println!("  count=42: {data}");

    // ── 4. Mutable list/tuple access ─────────────────────────────────

    println!("\n=== Mutable list access ===");

    let mut list = uzon!([10, 20, 30]);
    println!("  initial: {list}");

    // Modify via get_index_mut
    if let Some(elem) = list.get_index_mut(1) {
        *elem = Value::int(99);
    }
    println!("  [1]=99:  {list}");

    // Modify via as_list_mut (full Vec access)
    if let Some(vec) = list.as_list_mut() {
        vec.reverse();
    }
    println!("  reversed: {list}");

    // ── 5. Deep merge ────────────────────────────────────────────────
    //
    // merge() performs deep recursive merge of structs.
    // - Matching struct fields: merged recursively
    // - Non-struct or new fields: overwritten/added
    // - Fields only in base: kept

    println!("\n=== Deep merge ===");

    let mut base = uzon!({
        "server": {
            "host": "localhost",
            "port": 8080,
            "tls": false
        },
        "logging": {
            "level": "info",
            "file": "/var/log/app.log"
        },
        "debug": false
    });

    let production = uzon!({
        "server": {
            "host": "prod.example.com",
            "port": 443,
            "tls": true
        },
        "logging": {
            "level": "warn"
        },
        "debug": false
    });

    println!("  base:     {base}");
    println!("  overlay:  {production}");

    base.merge(production);
    println!("  merged:   {base}");

    // Verify the merge result
    assert_eq!(base.get_path("server.host"), Some(&Value::from("prod.example.com")));
    assert_eq!(base.get_path("server.port"), Some(&Value::int(443)));
    assert_eq!(base.get_path("server.tls"), Some(&Value::Bool(true)));
    assert_eq!(base.get_path("logging.level"), Some(&Value::from("warn")));
    // Kept from base (not in overlay)
    assert_eq!(base.get_path("logging.file"), Some(&Value::from("/var/log/app.log")));

    println!("  server.host = {}", base.get_path("server.host").unwrap());
    println!("  logging.level = {}", base.get_path("logging.level").unwrap());
    println!("  logging.file = {} (kept from base)", base.get_path("logging.file").unwrap());

    // ── 6. Building up a struct incrementally ────────────────────────

    println!("\n=== Incremental construction ===");

    let mut report = uzon!({"title": "Monthly Report"});

    // Add sections one by one
    report.insert("summary", uzon!({
        "total_users": 1500,
        "active_users": 1200
    }));

    report.insert("metrics", uzon!([
        {"name": "cpu", "value": 45.2},
        {"name": "memory", "value": 72.8}
    ]));

    println!("  report = {report}");
    println!("  report.summary.total_users = {}", report.get_path("summary.total_users").unwrap());

    println!("\nDone!");
}
