// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! Example: Serde integration for UZON values.
//!
//! Demonstrates:
//! - Deserializing Value into Rust structs via `from_value`
//! - Direct UZON string → Rust struct via `from_str_as`
//! - Serializing Value to JSON via serde
//! - Deserializing JSON into Value

use serde::{Deserialize, Serialize};
use uzon::{from_str_as, from_value, uzon, Value};

// ── Define some Rust types ───────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct ServerConfig {
    host: String,
    port: u16,
    tls: bool,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct DatabaseConfig {
    url: String,
    pool_size: u32,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct AppConfig {
    server: ServerConfig,
    database: DatabaseConfig,
    tags: Vec<String>,
    debug: bool,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Deserialize, PartialEq)]
struct LogConfig {
    level: LogLevel,
    file: Option<String>,
}

fn main() {
    // ── 1. Value → Rust struct via from_value ────────────────────────
    //
    // Build a Value (e.g., from parsing) then convert to a typed struct.

    println!("=== from_value ===");

    let value = uzon!({
        "host": "localhost",
        "port": 8080,
        "tls": false
    });

    let config: ServerConfig = from_value(value).unwrap();
    println!("  server: {config:?}");
    assert_eq!(config.host, "localhost");
    assert_eq!(config.port, 8080);
    assert_eq!(config.tls, false);

    // ── 2. Nested struct deserialization ──────────────────────────────

    println!("\n=== Nested structs ===");

    let value = uzon!({
        "server": {
            "host": "prod.example.com",
            "port": 443,
            "tls": true
        },
        "database": {
            "url": "postgres://db.example.com/app",
            "pool_size": 20,
            "timeout_ms": 5000
        },
        "tags": ["production", "us-east"],
        "debug": false
    });

    let app: AppConfig = from_value(value).unwrap();
    println!("  app config: {app:#?}");
    assert_eq!(app.server.host, "prod.example.com");
    assert_eq!(app.database.pool_size, 20);
    assert_eq!(app.tags, vec!["production", "us-east"]);

    // ── 3. Optional fields ───────────────────────────────────────────

    println!("\n=== Optional fields ===");

    // With value present
    let value = uzon!({
        "url": "postgres://localhost/dev",
        "pool_size": 5,
        "timeout_ms": 3000
    });
    let db: DatabaseConfig = from_value(value).unwrap();
    println!("  with timeout: {db:?}");
    assert_eq!(db.timeout_ms, Some(3000));

    // With null (becomes None)
    let value = uzon!({
        "url": "postgres://localhost/dev",
        "pool_size": 5,
        "timeout_ms": null
    });
    let db: DatabaseConfig = from_value(value).unwrap();
    println!("  with null:    {db:?}");
    assert_eq!(db.timeout_ms, None);

    // ── 4. Enum deserialization ──────────────────────────────────────

    println!("\n=== Enum deserialization ===");

    let value = uzon!({
        "level": "warn",
        "file": "/var/log/app.log"
    });
    let log: LogConfig = from_value(value).unwrap();
    println!("  log config: {log:?}");
    assert_eq!(log.level, LogLevel::Warn);

    // ── 5. UZON string → Rust struct via from_str_as ─────────────────
    //
    // Parses UZON text and deserializes directly — no intermediate Value.

    println!("\n=== from_str_as ===");

    let uzon_text = r#"
        host is "api.example.com"
        port is 443
        tls is true
    "#;

    let server: ServerConfig = from_str_as(uzon_text).unwrap();
    println!("  direct parse: {server:?}");
    assert_eq!(server.host, "api.example.com");
    assert_eq!(server.port, 443);

    // ── 6. Value → JSON via serde_json ───────────────────────────────
    //
    // Value implements Serialize, so it works with any serde format.

    println!("\n=== Value → JSON ===");

    let config = uzon!({
        "name": "my-app",
        "version": 2,
        "features": ["auth", "logging"],
        "settings": {
            "timeout": 30,
            "retries": 3
        }
    });

    let json = serde_json::to_string_pretty(&config).unwrap();
    println!("  {json}");

    // Compact JSON
    let json = serde_json::to_string(&config).unwrap();
    println!("  compact: {json}");

    // ── 7. JSON → Value ──────────────────────────────────────────────
    //
    // Value implements Deserialize, so you can read from any serde format.

    println!("\n=== JSON → Value ===");

    let json_str = r#"{
        "name": "from-json",
        "count": 42,
        "items": [1, 2, 3],
        "nested": {"x": true}
    }"#;

    let value: Value = serde_json::from_str(json_str).unwrap();
    println!("  parsed: {value}");
    println!("  name = {}", value["name"]);
    println!("  count = {}", value["count"]);
    println!("  items[0] = {}", value["items"][0]);
    println!("  nested.x = {}", value.get_path("nested.x").unwrap());

    // ── 8. JSON ↔ Value roundtrip ────────────────────────────────────

    println!("\n=== JSON roundtrip ===");

    let original_json = r#"{"a":1,"b":[true,null,"hello"],"c":{"x":3.14}}"#;
    let value: Value = serde_json::from_str(original_json).unwrap();
    let back_to_json = serde_json::to_string(&value).unwrap();

    assert_eq!(original_json, back_to_json);
    println!("  original: {original_json}");
    println!("  roundtrip: {back_to_json}");
    println!("  match: {}", original_json == back_to_json);

    // ── 9. Practical: UZON config → JSON API response ────────────────

    println!("\n=== Practical: UZON → JSON API ===");

    let uzon_source = r#"
        server is {
            host is "api.example.com",
            port is 443,
            tls is true
        }
        database is {
            url is "postgres://db.internal/app",
            pool_size is 25,
            timeout_ms is 5000
        }
        tags is ["production", "us-east-1"]
        debug is false
    "#;

    let bindings = uzon::from_str(uzon_source).unwrap();
    let root = Value::Struct(bindings.into_iter().collect());

    // Convert to typed config
    let app: AppConfig = from_value(root.clone()).unwrap();
    println!("  Parsed config: {app:#?}");

    // Serialize back to JSON (e.g., for an API response)
    let json = serde_json::to_string_pretty(&app).unwrap();
    println!("  As JSON:\n{json}");

    // ── 10. Error handling ───────────────────────────────────────────

    println!("\n=== Deserialization errors ===");

    // Missing required field
    let value = uzon!({"host": "localhost"});
    let err = from_value::<ServerConfig>(value).unwrap_err();
    println!("  missing field: {err}");

    // Wrong type
    let value = uzon!({"host": 123, "port": 8080, "tls": true});
    let err = from_value::<ServerConfig>(value).unwrap_err();
    println!("  wrong type: {err}");

    println!("\nDone!");
}
