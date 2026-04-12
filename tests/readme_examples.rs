// Tests that verify all UZON syntax examples from README.md parse correctly.

use std::collections::HashMap;
use uzon::{from_str, Evaluator, EvalOptions};
use uzon::lexer::Lexer;
use uzon::parser::Parser;

/// Helper: parse and evaluate with a custom environment.
fn eval_with_env(
    source: &str,
    env: HashMap<String, String>,
) -> Result<std::collections::BTreeMap<String, uzon::Value>, uzon::UzonError> {
    let (tokens, comment_lines) = Lexer::new(source).tokenize()?;
    let doc = Parser::new(tokens, comment_lines).parse()?;
    let mut eval = Evaluator::new(EvalOptions {
        env: Some(env),
        ..Default::default()
    });
    eval.evaluate(&doc)
}

/// The main UZON Syntax Overview block from the README (lines 93-166),
/// with the env.PORT line adapted to use a custom environment.
#[test]
fn syntax_overview_full_block() {
    // This is the complete "UZON Syntax Overview" example from the README,
    // except env.PORT is tested separately since it needs a custom env.
    let source = r#"
// Primitives
name is "Alice"
age is 30
score is 95.5
active is true
nothing is null

// Typed numbers
byte is 255 as u8
precise is 3.14 as f32

// Collections
tags are "web", "api"
ids are 1, 2, 3 as [i32]
point is (10, 20)
server is { host is "localhost", port is 8080 }

// Expressions and string interpolation
total is age + 1
greeting is "Hello, {name}!"
grade is if score >= 90.0 then "A" else "B"

// Case expressions — three modes
label is case grade
    when "A" then "excellent"
    when "B" then "good"
    else "ok"

// Type dispatch (untagged unions)
u is 42 as i32 from union i32, string
type_label is case type u
    when i32 then "integer"
    when string then "text"
    else "other"

// Variant dispatch (tagged unions)
status is "ok" named success
    from success as string, error as string
    called Result
status_label is case named status
    when success then "good"
    when error then "bad"
    else "unknown"

// Enums
_color is red from red, green, blue called Color
selected is green as Color

// Type checking
is_int is u is type i32
is_str is u is not type string

// Functions
add is function a as i32, b as i32 returns i32 { a + b }
sum is add(3, 4)

// Standard library
doubled is std.map(ids, function n as i32 returns i32 { n * 2 })
count is std.len(tags)

// Struct operations
modified is server with { port is 443 }
extended is server plus { tls is true, cert is "/path" }

// Undefined coalescing
fallback is server.missing or else "default"

// Field extraction
extracted_host is of server
"#;

    let result = from_str(source);
    assert!(
        result.is_ok(),
        "Syntax overview block failed to parse: {:?}",
        result.err()
    );
}

/// The env.PORT line from the syntax overview, tested with a custom environment.
#[test]
fn syntax_overview_env_port() {
    let source = r#"
port is env.PORT to u16 or else 8080
"#;

    let mut env = HashMap::new();
    env.insert("PORT".to_string(), "3000".to_string());

    let result = eval_with_env(source, env);
    assert!(
        result.is_ok(),
        "env.PORT example failed: {:?}",
        result.err()
    );
}

/// The env.PORT line with fallback (env var not set).
#[test]
fn syntax_overview_env_port_fallback() {
    let source = r#"
port is env.PORT to u16 or else 8080
"#;

    // Empty env: PORT is not set, should fall back to 8080
    let result = eval_with_env(source, HashMap::new());
    assert!(
        result.is_ok(),
        "env.PORT fallback example failed: {:?}",
        result.err()
    );
}

// ---- Individual sub-sections of the syntax overview ----

#[test]
fn primitives() {
    let source = r#"
name is "Alice"
age is 30
score is 95.5
active is true
nothing is null
"#;
    let result = from_str(source);
    assert!(result.is_ok(), "Primitives failed: {:?}", result.err());
}

#[test]
fn typed_numbers() {
    let source = r#"
byte is 255 as u8
precise is 3.14 as f32
"#;
    let result = from_str(source);
    assert!(result.is_ok(), "Typed numbers failed: {:?}", result.err());
}

#[test]
fn collections() {
    let source = r#"
tags are "web", "api"
ids are 1, 2, 3 as [i32]
point is (10, 20)
server is { host is "localhost", port is 8080 }
"#;
    let result = from_str(source);
    assert!(result.is_ok(), "Collections failed: {:?}", result.err());
}

#[test]
fn expressions_and_interpolation() {
    let source = r#"
name is "Alice"
age is 30
score is 95.5
total is age + 1
greeting is "Hello, {name}!"
grade is if score >= 90.0 then "A" else "B"
"#;
    let result = from_str(source);
    assert!(
        result.is_ok(),
        "Expressions failed: {:?}",
        result.err()
    );
}

#[test]
fn case_expressions() {
    let source = r#"
score is 95.5
grade is if score >= 90.0 then "A" else "B"
label is case grade
    when "A" then "excellent"
    when "B" then "good"
    else "ok"
"#;
    let result = from_str(source);
    assert!(
        result.is_ok(),
        "Case expressions failed: {:?}",
        result.err()
    );
}

#[test]
fn type_dispatch_union() {
    let source = r#"
u is 42 as i32 from union i32, string
type_label is case type u
    when i32 then "integer"
    when string then "text"
    else "other"
"#;
    let result = from_str(source);
    assert!(
        result.is_ok(),
        "Type dispatch failed: {:?}",
        result.err()
    );
}

#[test]
fn variant_dispatch_tagged_union() {
    let source = r#"
status is "ok" named success
    from success as string, error as string
    called Result
status_label is case named status
    when success then "good"
    when error then "bad"
    else "unknown"
"#;
    let result = from_str(source);
    assert!(
        result.is_ok(),
        "Variant dispatch failed: {:?}",
        result.err()
    );
}

#[test]
fn enums() {
    let source = r#"
_color is red from red, green, blue called Color
selected is green as Color
"#;
    let result = from_str(source);
    assert!(result.is_ok(), "Enums failed: {:?}", result.err());
}

#[test]
fn type_checking() {
    let source = r#"
u is 42 as i32 from union i32, string
is_int is u is type i32
is_str is u is not type string
"#;
    let result = from_str(source);
    assert!(
        result.is_ok(),
        "Type checking failed: {:?}",
        result.err()
    );
}

#[test]
fn functions() {
    let source = r#"
add is function a as i32, b as i32 returns i32 { a + b }
sum is add(3, 4)
"#;
    let result = from_str(source);
    assert!(result.is_ok(), "Functions failed: {:?}", result.err());
}

#[test]
fn standard_library() {
    let source = r#"
ids are 1, 2, 3 as [i32]
tags are "web", "api"
doubled is std.map(ids, function n as i32 returns i32 { n * 2 })
count is std.len(tags)
"#;
    let result = from_str(source);
    assert!(
        result.is_ok(),
        "Standard library failed: {:?}",
        result.err()
    );
}

#[test]
fn struct_operations() {
    let source = r#"
server is { host is "localhost", port is 8080 }
modified is server with { port is 443 }
extended is server plus { tls is true, cert is "/path" }
"#;
    let result = from_str(source);
    assert!(
        result.is_ok(),
        "Struct operations failed: {:?}",
        result.err()
    );
}

#[test]
fn undefined_coalescing() {
    let source = r#"
server is { host is "localhost", port is 8080 }
fallback is server.missing or else "default"
"#;
    let result = from_str(source);
    assert!(
        result.is_ok(),
        "Undefined coalescing failed: {:?}",
        result.err()
    );
}

#[test]
fn field_extraction() {
    let source = r#"
server is { host is "localhost", port is 8080 }
host is of server
"#;
    let result = from_str(source);
    assert!(
        result.is_ok(),
        "Field extraction failed: {:?}",
        result.err()
    );
}
