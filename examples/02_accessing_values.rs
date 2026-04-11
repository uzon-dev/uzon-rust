// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! Example: Accessing and navigating UZON values.
//!
//! Demonstrates accessor methods, indexing, path navigation, and type checking.

use uzon::{from_str, Value};

fn main() {
    let source = r#"
        name is "Alice"
        age is 30
        score is 95.5
        active is true
        nothing is null

        tags is ["rust", "uzon", "parser"]

        address is {
            city is "Seoul",
            zip is "06000",
            geo is {
                lat is 37.5665,
                lng is 126.978
            }
        }

        coords is (37.5665, 126.978)
    "#;

    let bindings = from_str(source).expect("parse failed");

    // For convenience, wrap in a struct Value
    let root = Value::Struct(bindings.into_iter().collect());

    // ── 1. Type-safe accessors ───────────────────────────────────────
    //
    // Each accessor returns Option — None if the type doesn't match.

    println!("=== Type-safe accessors ===");

    assert_eq!(root["name"].as_str(), Some("Alice"));
    assert_eq!(root["age"].as_i64(), Some(30));
    assert_eq!(root["age"].as_i128(), Some(30));
    assert_eq!(root["score"].as_f64(), Some(95.5));
    assert_eq!(root["active"].as_bool(), Some(true));
    assert_eq!(root["nothing"].is_null(), true);

    // Wrong type returns None, never panics
    assert_eq!(root["name"].as_i64(), None);
    assert_eq!(root["age"].as_str(), None);
    assert_eq!(root["age"].as_bool(), None);

    println!("  name: {:?}", root["name"].as_str());
    println!("  age:  {:?}", root["age"].as_i64());
    println!("  score: {:?}", root["score"].as_f64());
    println!("  active: {:?}", root["active"].as_bool());

    // ── 2. Indexing with [] ──────────────────────────────────────────
    //
    // Structs: value["key"]  —  returns &Value::Null for missing keys
    // Lists:   value[index]  —  returns &Value::Null for out-of-bounds

    println!("\n=== Indexing ===");

    // Struct indexing
    println!("  root[\"name\"] = {}", root["name"]);
    println!("  root[\"missing\"] = {}", root["missing"]); // Null, no panic

    // List indexing
    let tags = &root["tags"];
    println!("  tags[0] = {}", tags[0]);
    println!("  tags[1] = {}", tags[1]);
    println!("  tags[2] = {}", tags[2]);
    println!("  tags[99] = {}", tags[99]); // Null, no panic

    // Tuple indexing
    let coords = &root["coords"];
    println!("  coords[0] = {}", coords[0]);
    println!("  coords[1] = {}", coords[1]);

    // Non-collection indexing returns Null
    println!("  42[\"x\"] = {}", Value::int(42)["x"]);
    println!("  42[0] = {}", Value::int(42)[0]);

    // ── 3. Safe get methods ──────────────────────────────────────────
    //
    // Unlike indexing, `get` and `get_index` return Option<&Value>.

    println!("\n=== Safe get ===");

    assert_eq!(root.get("name"), Some(&Value::from("Alice")));
    assert_eq!(root.get("nonexistent"), None);

    assert_eq!(tags.get_index(0), Some(&Value::from("rust")));
    assert_eq!(tags.get_index(999), None);

    // get on non-struct / get_index on non-list returns None
    assert_eq!(Value::int(42).get("key"), None);
    assert_eq!(Value::int(42).get_index(0), None);

    // ── 4. Dot-path navigation ───────────────────────────────────────
    //
    // `get_path` navigates nested structs and lists with dot-separated keys.

    println!("\n=== Path navigation ===");

    assert_eq!(
        root.get_path("address.city"),
        Some(&Value::from("Seoul"))
    );
    assert_eq!(
        root.get_path("address.geo.lat"),
        Some(&Value::float(37.5665))
    );
    assert_eq!(
        root.get_path("address.geo.lng"),
        Some(&Value::float(126.978))
    );

    // Numeric segments index into lists
    assert_eq!(root.get_path("tags.0"), Some(&Value::from("rust")));
    assert_eq!(root.get_path("tags.2"), Some(&Value::from("parser")));

    // Missing paths return None
    assert_eq!(root.get_path("address.country"), None);
    assert_eq!(root.get_path("missing.deep.path"), None);
    assert_eq!(root.get_path("tags.99"), None);

    println!("  address.city = {:?}", root.get_path("address.city"));
    println!("  address.geo.lat = {:?}", root.get_path("address.geo.lat"));
    println!("  tags.0 = {:?}", root.get_path("tags.0"));
    println!("  missing.path = {:?}", root.get_path("missing.path"));

    // ── 5. Collection accessors ──────────────────────────────────────

    println!("\n=== Collection accessors ===");

    // as_list: borrow the inner slice
    if let Some(items) = tags.as_list() {
        println!("  tags has {} items: {:?}", items.len(), items);
    }

    // as_tuple
    if let Some(items) = coords.as_tuple() {
        println!("  coords has {} items: {:?}", items.len(), items);
    }

    // as_struct
    let address = &root["address"];
    if let Some(fields) = address.as_struct() {
        println!("  address has {} fields: {:?}", fields.len(), fields.keys().collect::<Vec<_>>());
    }

    // ── 6. Type checking ─────────────────────────────────────────────

    println!("\n=== Type checking ===");

    let values: Vec<(&str, &Value)> = vec![
        ("name", &root["name"]),
        ("age", &root["age"]),
        ("score", &root["score"]),
        ("active", &root["active"]),
        ("nothing", &root["nothing"]),
        ("tags", &root["tags"]),
        ("coords", &root["coords"]),
        ("address", &root["address"]),
    ];

    for (name, value) in values {
        println!("  {name}: type = {}", value.type_name());
    }

    // ── 7. Null and undefined checks ─────────────────────────────────

    println!("\n=== Null/undefined checks ===");

    assert!(root["nothing"].is_null());
    assert!(!root["name"].is_null());

    // Missing key via indexing returns Null
    assert!(root["nonexistent"].is_null());

    println!("  nothing.is_null() = {}", root["nothing"].is_null());
    println!("  name.is_null() = {}", root["name"].is_null());
    println!("  nonexistent.is_null() = {}", root["nonexistent"].is_null());

    println!("\nDone!");
}
