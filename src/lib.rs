// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

//! # uzon
//!
//! A Rust library for parsing, evaluating, and manipulating
//! [UZON](https://uzon.dev) — a typed, human-readable data expression format.
//!
//! ## Quick Start
//!
//! ```ignore
//! use uzon::{from_str, uzon, Value};
//!
//! // Parse UZON text
//! let bindings = from_str(r#"name is "Alice"  age is 30"#).unwrap();
//! assert_eq!(bindings["name"].as_str(), Some("Alice"));
//!
//! // Build values programmatically
//! let v = uzon!({"name": "Bob", "age": 25});
//!
//! // Arithmetic
//! assert_eq!(Value::int(10) + Value::int(20), Value::int(30));
//!
//! // Deserialize into Rust types
//! #[derive(serde::Deserialize)]
//! struct Config { host: String, port: u16 }
//! let config: Config = uzon::from_str_as(r#"host is "localhost"  port is 8080"#).unwrap();
//! ```
//!
//! See the [README](https://github.com/uzon-dev/uzon-rust) for full API documentation.

pub mod ast;
pub mod error;
pub mod evaluator;
pub mod lexer;
pub mod parser;
pub mod scope;
pub mod stringify;
pub mod token;
pub mod value;

// Re-export primary API for ergonomic usage.
pub use error::{UzonError, Result};
pub use evaluator::{from_str, from_str_plain, from_path, Evaluator, EvalOptions};
pub use stringify::{to_string, to_string_with_options, StringifyOptions};
pub use value::{Value, UzonInteger, UzonFloat, IntegerType, FloatType, UzonEnum, UzonUnion, UzonTaggedUnion, UzonTuple, UzonList, UzonStruct, UzonFunction, UzonUndefined};
pub use value::ops::{ValueConversionError, ValueArithmeticError};
pub use value::serde_impl::{from_value, DeError};

/// Parse a UZON string and deserialize directly into `T`.
///
/// ```ignore
/// #[derive(serde::Deserialize)]
/// struct Config { host: String, port: u16 }
/// let config: Config = uzon::from_str_as("host is \"localhost\"\nport is 8080")?;
/// ```
pub fn from_str_as<T: serde::de::DeserializeOwned>(source: &str) -> std::result::Result<T, String> {
    let values = from_str(source).map_err(|e| e.to_string())?;
    let value = Value::Struct(UzonStruct::new(values.into_iter().collect()));
    from_value(value).map_err(|e| e.to_string())
}

/// Construct a [`Value`] using JSON-like syntax.
///
/// ```ignore
/// use uzon::uzon;
///
/// let v = uzon!({
///     "name": "Alice",
///     "age": 30,
///     "scores": [90, 85, 92],
///     "active": true,
///     "address": {
///         "city": "Seoul",
///         "zip": "06000"
///     }
/// });
/// ```
///
/// Supported forms:
/// - `null` → `Value::Null`
/// - `true`, `false` → `Value::Bool`
/// - integer literals → `Value::Integer`
/// - float literals (must contain `.`) → `Value::Float`
/// - string literals → `Value::String`
/// - `[a, b, c]` → `Value::List`
/// - `{ "key": value, ... }` → `Value::Struct`
/// - `(a, b)` → `Value::Tuple`
/// - any expression → via `Into<Value>`
#[macro_export]
macro_rules! uzon {
    // null
    (null) => { $crate::Value::Null };

    // bool
    (true) => { $crate::Value::Bool(true) };
    (false) => { $crate::Value::Bool(false) };

    // struct: { "key": value, ... }
    ({ $($key:tt : $val:tt),* $(,)? }) => {{
        let mut fields = indexmap::IndexMap::new();
        $(
            fields.insert(String::from($key), uzon!($val));
        )*
        $crate::Value::Struct($crate::UzonStruct::new(fields))
    }};

    // list: [a, b, c]
    ([ $($elem:tt),* $(,)? ]) => {
        $crate::Value::list(vec![ $( uzon!($elem) ),* ])
    };

    // tuple: (a, b, ...)
    (( $($elem:tt),* $(,)? )) => {
        $crate::Value::Tuple($crate::value::UzonTuple::new(vec![ $( uzon!($elem) ),* ]))
    };

    // expression fallback (literals, variables, function calls)
    ($other:expr) => {
        $crate::Value::from($other)
    };
}
