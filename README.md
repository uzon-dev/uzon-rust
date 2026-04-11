# uzon

A Rust library for parsing, evaluating, and manipulating [UZON](https://uzon.dev) — a typed, human-readable data expression format.

```rust
use uzon::{from_str, Value};

let bindings = from_str(r#"
    name is "Alice"
    age is 30
    server is { host is "localhost", port is 8080 }
"#)?;

assert_eq!(bindings["name"].as_str(), Some("Alice"));
assert_eq!(bindings["age"].as_i64(), Some(30));
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
uzon = "0.4"
```

## Table of Contents

- [Quick Start](#quick-start)
- [UZON Syntax Overview](#uzon-syntax-overview)
- [API Reference](#api-reference)
  - [Parsing](#parsing)
  - [Value Types](#value-types)
  - [Accessors](#accessors)
  - [Indexing](#indexing)
  - [Path Navigation](#path-navigation)
  - [Mutation](#mutation)
  - [Deep Merge](#deep-merge)
  - [Building Values](#building-values)
  - [Type Conversion](#type-conversion)
  - [Arithmetic](#arithmetic)
  - [Comparison](#comparison)
  - [Iteration](#iteration)
  - [Serde Integration](#serde-integration)
  - [Stringification](#stringification)
  - [Error Handling](#error-handling)
- [Examples](#examples)
- [License](#license)

## Quick Start

```rust
use uzon::{from_str, from_str_plain, to_string, uzon, Value};

// Parse UZON text
let bindings = from_str(r#"
    name is "Alice"
    scores is [95, 88, 92]
    config is { debug is true, port is 8080 }
"#).unwrap();

// Access values
let name = &bindings["name"];
assert_eq!(name.as_str(), Some("Alice"));

// Build values programmatically
let value = uzon!({
    "name": "Bob",
    "age": 25,
    "tags": ["rust", "uzon"]
});
assert_eq!(value["name"], "Bob");
assert_eq!(value["age"], 25);

// Arithmetic
let result = Value::int(10) + Value::int(20);
assert_eq!(result, 30);

// Deserialize into Rust types
#[derive(serde::Deserialize)]
struct Config { debug: bool, port: u16 }

let config: Config = uzon::from_str_as(r#"
    debug is true
    port is 8080
"#).unwrap();
```

## UZON Syntax Overview

UZON is a declarative data expression format. Bindings use `is` instead of `=`, types use `as`, and values are computed lazily.

```
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
tags is ["web", "api"]
point is (10, 20)              // tuple
server is { host is "localhost", port is 8080 }

// Expressions
total is self.a + self.b
greeting is "Hello, {self.name}!"
grade is if self.score >= 90 then "A" else "B"

// Enums
_color is red from red, green, blue called Color
selected is green as Color

// Functions
add is function a as i32, b as i32 returns i32 { a + b }
result is self.add(3, 4)

// Struct operations
production is self.base extends { host is "prod.example.com" }
modified is self.base with { port is 443 }

// Null handling
fallback is self.config.missing or else "default"
```

## API Reference

### Parsing

Three entry points for parsing UZON text:

```rust
use std::collections::BTreeMap;
use std::path::Path;
use uzon::{from_str, from_str_plain, from_path, Value, Result};

// Parse and evaluate, preserving UZON type wrappers (enums, unions, etc.)
fn from_str(source: &str) -> Result<BTreeMap<String, Value>>

// Parse and evaluate, stripping UZON wrappers to plain Rust-friendly types
fn from_str_plain(source: &str) -> Result<BTreeMap<String, Value>>

// Parse and evaluate from a file path
fn from_path(path: &Path) -> Result<BTreeMap<String, Value>>
```

**`from_str` vs `from_str_plain`**: `from_str` preserves UZON-specific types (enums carry variant sets, tuples are distinct from lists). `from_str_plain` calls `.to_plain()` on every value, converting enums to strings, tuples to lists, and unwrapping unions.

```rust
let source = r#"
    _color is red from red, green, blue called Color
    choice is red as Color
    pair is (1, 2)
"#;

// from_str: choice is Value::Enum, pair is Value::Tuple
let rich = from_str(source).unwrap();

// from_str_plain: choice is Value::String, pair is Value::List
let plain = from_str_plain(source).unwrap();
```

#### Evaluator (advanced)

For custom configuration, use `Evaluator` directly:

```rust
use uzon::{Evaluator, EvalOptions};

let options = EvalOptions {
    filename: Some("config.uzon".into()),  // for error messages
    env: None,                              // use std::env by default
    plain: false,                           // preserve UZON types
};

let mut evaluator = Evaluator::new(options);
// evaluator.evaluate(&document) — requires a parsed Document (internal API)
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `filename` | `Option<PathBuf>` | `None` | Source filename for error reporting |
| `env` | `Option<HashMap<String, String>>` | `None` | Environment variables (defaults to `std::env::vars()`) |
| `plain` | `bool` | `false` | If `true`, auto-calls `to_plain()` on results |

---

### Value Types

`Value` is the core runtime representation:

```rust
pub enum Value {
    Null,                              // null
    Undefined,                         // "does not exist"
    Bool(bool),                        // true / false
    Integer(UzonInteger),              // i128 + type annotation (i8..i128, u8..u128)
    BigInteger(num_bigint::BigInt),    // arbitrary precision
    Float(UzonFloat),                  // f64 + type annotation (f16..f128)
    String(String),                    // UTF-8 string
    List(UzonList),                    // [a, b, c] — homogeneous sequence
    Tuple(UzonTuple),                  // (a, b, c) — fixed-length heterogeneous
    Struct(IndexMap<String, Value>),   // { key is value } — ordered map
    Enum(UzonEnum),                    // variant from a set
    Union(UzonUnion),                  // untagged union
    TaggedUnion(UzonTaggedUnion),      // tagged union
    Function(UzonFunction),            // closure
}
```

#### UzonInteger

```rust
pub struct UzonInteger {
    pub value: i128,
    pub type_ann: IntegerType,  // i64 by default
    pub explicit: bool,         // true if annotation was in source (e.g., "42 as u8")
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(value: i128) -> Self` | Create with default type (i64) |
| `with_type` | `fn with_type(value: i128, type_ann: IntegerType) -> Self` | Create with explicit type |
| `validate_range` | `fn validate_range(&self) -> Result<(), String>` | Check value fits in annotated range |
| `checked_add` | `fn checked_add(&self, other: &Self) -> Result<Self, String>` | Add with overflow check |
| `checked_sub` | `fn checked_sub(&self, other: &Self) -> Result<Self, String>` | Subtract with overflow check |
| `checked_mul` | `fn checked_mul(&self, other: &Self) -> Result<Self, String>` | Multiply with overflow check |
| `checked_div` | `fn checked_div(&self, other: &Self) -> Result<Self, String>` | Divide (errors on zero) |
| `checked_rem` | `fn checked_rem(&self, other: &Self) -> Result<Self, String>` | Remainder (errors on zero) |
| `checked_pow` | `fn checked_pow(&self, exp: &Self) -> Result<Self, String>` | Power with overflow check |
| `checked_neg` | `fn checked_neg(&self) -> Result<Self, String>` | Negate with overflow check |

#### IntegerType

```rust
pub enum IntegerType {
    Arbitrary,  // unbounded (maps to BigInt if needed)
    I(u16),     // signed: I(8)=i8, I(16)=i16, ..., I(128)=i128
    U(u16),     // unsigned: U(8)=u8, U(16)=u16, ..., U(128)=u128
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `DEFAULT` | `const IntegerType::I(64)` | Default type for integer literals |
| `is_default` | `fn is_default(&self) -> bool` | Check if this is the default type |
| `range` | `fn range(&self) -> Option<(i128, i128)>` | `(min, max)` inclusive; `None` for Arbitrary |
| `from_type_name` | `fn from_type_name(name: &str) -> Option<IntegerType>` | Parse `"i32"`, `"u64"`, etc. |
| `display_name` | `fn display_name(&self) -> String` | `"i32"`, `"u64"`, `"integer"`, etc. |

#### UzonFloat

```rust
pub struct UzonFloat {
    pub value: f64,
    pub type_ann: FloatType,  // f64 by default
    pub explicit: bool,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(value: f64) -> Self` | Create with default type (f64) |
| `with_type` | `fn with_type(value: f64, type_ann: FloatType) -> Self` | Create with explicit type |
| `add`, `sub`, `mul`, `div`, `rem` | `fn op(&self, other: &Self) -> Result<Self, String>` | Arithmetic operations |
| `powf` | `fn powf(&self, other: &Self) -> Result<Self, String>` | Power |
| `neg` | `fn neg(&self) -> Self` | Negate |

#### FloatType

```rust
pub enum FloatType { F16, F32, F64, F80, F128 }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `DEFAULT` | `const FloatType::F64` | Default type for float literals |
| `is_default` | `fn is_default(&self) -> bool` | Check if this is the default type |
| `from_type_name` | `fn from_type_name(name: &str) -> Option<FloatType>` | Parse `"f32"`, `"f64"`, etc. |
| `display_name` | `fn display_name(&self) -> &'static str` | `"f32"`, `"f64"`, etc. |

#### UzonList

```rust
pub struct UzonList {
    pub elements: Vec<Value>,
    pub element_type: Option<String>,  // from `as [Type]` annotations
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(elements: Vec<Value>) -> Self` | Create without type annotation |
| `with_type` | `fn with_type(elements: Vec<Value>, element_type: impl Into<String>) -> Self` | Create with element type |
| `len` | `fn len(&self) -> usize` | Element count |
| `is_empty` | `fn is_empty(&self) -> bool` | Check if empty |

`UzonList` also implements `Deref<Target=Vec<Value>>`, `DerefMut`, `IntoIterator`, and `FromIterator<Value>`.

#### UzonTuple

```rust
pub struct UzonTuple {
    pub elements: Vec<Value>,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(elements: Vec<Value>) -> Self` | Create tuple |
| `len` | `fn len(&self) -> usize` | Element count |
| `is_empty` | `fn is_empty(&self) -> bool` | Check if empty |

#### UzonEnum

```rust
pub struct UzonEnum {
    pub value: String,              // selected variant
    pub variants: Vec<String>,      // all possible variants
    pub type_name: Option<String>,  // type name if defined
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(value: impl Into<String>, variants: Vec<String>, type_name: Option<String>) -> Self` | Create enum |

#### UzonUnion

```rust
pub struct UzonUnion {
    pub value: Box<Value>,
    pub types: Vec<String>,         // possible type names
    pub type_name: Option<String>,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(value: Value, types: Vec<String>, type_name: Option<String>) -> Self` | Create union |

#### UzonTaggedUnion

```rust
pub struct UzonTaggedUnion {
    pub value: Box<Value>,
    pub tag: String,                              // selected variant tag
    pub variants: BTreeMap<String, Option<String>>, // tag → type mapping
    pub type_name: Option<String>,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(value: Value, tag: impl Into<String>, variants: BTreeMap<String, Option<String>>, type_name: Option<String>) -> Self` | Create tagged union |

#### UzonFunction

```rust
pub struct UzonFunction {
    pub params: Vec<FunctionParam>,
    pub return_type: TypeExpr,
    pub body_bindings: Vec<Binding>,
    pub body_expr: Node,
    pub captured_bindings: BTreeMap<String, Value>,
    pub captured_types: BTreeMap<String, TypeDef>,
    pub type_name: Option<String>,
}
```

Functions are opaque closures — they cannot be compared (`PartialEq` always returns `false`).

#### UzonUndefined

```rust
pub struct UzonUndefined;  // sentinel type, Display prints "undefined"
```

#### Convenience constructors

```rust
Value::int(42)                     // Integer with default i64 type
Value::float(3.14)                 // Float with default f64 type
Value::list(vec![Value::int(1)])   // List from Vec
```

#### Type introspection

```rust
value.type_name()    // → "null", "bool", "integer", "float", "string",
                     //   "list", "tuple", "struct", "enum", "union",
                     //   "tagged union", "function"
value.is_null()      // → true for Value::Null
value.is_undefined() // → true for Value::Undefined
```

#### to_plain

Strip UZON-specific wrappers for simpler Rust consumption:

```rust
let plain = value.to_plain();
// Enum → String
// Tuple → List
// Union → inner value
// TaggedUnion → inner value
// Struct fields and List elements are recursively simplified
```

---

### Accessors

Type-safe extraction methods. All return `Option` — `None` if the type doesn't match.

```rust
value.as_bool()       // → Option<bool>
value.as_i64()        // → Option<i64>       (also handles BigInteger)
value.as_i128()       // → Option<i128>      (also handles BigInteger)
value.as_f64()        // → Option<f64>       (also handles Integer → f64)
value.as_str()        // → Option<&str>
value.as_list()       // → Option<&[Value]>
value.as_list_mut()   // → Option<&mut Vec<Value>>
value.as_tuple()      // → Option<&[Value]>
value.as_tuple_mut()  // → Option<&mut Vec<Value>>
value.as_struct()     // → Option<&IndexMap<String, Value>>
value.as_struct_mut() // → Option<&mut IndexMap<String, Value>>
value.as_integer()    // → Option<&UzonInteger>
value.as_float()      // → Option<&UzonFloat>
```

```rust
let bindings = from_str(r#"name is "Alice"  age is 30"#).unwrap();
assert_eq!(bindings["name"].as_str(), Some("Alice"));
assert_eq!(bindings["age"].as_i64(), Some(30));
assert_eq!(bindings["age"].as_f64(), Some(30.0));  // auto-converts
assert_eq!(bindings["name"].as_i64(), None);        // type mismatch → None
```

---

### Indexing

Bracket indexing for convenient access. Returns `&Value::Null` for missing keys or out-of-bounds indices — never panics.

```rust
// Struct: value["key"]
let name = &value["name"];      // → &Value::String("Alice") or &Value::Null

// List/Tuple: value[index]
let first = &value[0];          // → &Value or &Value::Null

// Chained indexing
let host = &config["server"]["host"];
```

```rust
let config = uzon!({
    "server": {"host": "localhost", "port": 8080},
    "tags": ["web", "api"]
});

assert_eq!(config["server"]["host"], "localhost");
assert_eq!(config["tags"][0], "web");
assert_eq!(config["missing"], Value::Null);  // no panic
assert_eq!(config["tags"][99], Value::Null);  // no panic
```

#### Safe get methods

Return `Option<&Value>` instead of `&Value::Null`:

```rust
value.get("key")           // → Option<&Value>       (structs)
value.get_index(0)         // → Option<&Value>       (lists/tuples)
value.get_mut("key")       // → Option<&mut Value>   (structs)
value.get_index_mut(0)     // → Option<&mut Value>   (lists/tuples)
```

---

### Path Navigation

Navigate nested structures with dot-separated paths:

```rust
value.get_path("server.host")     // struct key navigation
value.get_path("items.0")         // numeric segments index into lists
value.get_path("a.b.c.0.d")      // mixed struct/list navigation
```

```rust
let config = uzon!({
    "server": {"host": "localhost", "port": 8080},
    "items": ["a", "b", "c"]
});

assert_eq!(config.get_path("server.host"), Some(&Value::from("localhost")));
assert_eq!(config.get_path("items.1"), Some(&Value::from("b")));
assert_eq!(config.get_path("missing.path"), None);
```

---

### Mutation

In-place modification of struct and list values:

```rust
// Struct: insert / remove
value.insert("key", 42);           // add or update field, returns old value
value.remove("key");               // remove field, returns removed value

// List: push / pop
value.push(42);                    // append to list
value.pop();                       // remove and return last element
```

```rust
let mut config = uzon!({"host": "localhost", "port": 8080});

config.insert("debug", true);             // add field
config.insert("port", 3000);              // update field
config.remove("debug");                   // remove field

let mut items = uzon!([1, 2, 3]);
items.push(4);                            // [1, 2, 3, 4]
let last = items.pop();                   // Some(Value::int(4))
```

Mutable access for in-place updates:

```rust
if let Some(v) = config.get_mut("port") {
    *v = Value::int(9090);
}

if let Some(list) = items.as_list_mut() {
    list.reverse();
}
```

---

### Deep Merge

Recursively merge two struct values:

```rust
value.merge(other);
```

- Matching struct fields are merged recursively
- Non-struct fields in `other` overwrite fields in `self`
- Fields only in `other` are added
- Fields only in `self` are kept
- Union/TaggedUnion wrappers are unwrapped before merging

```rust
let mut base = uzon!({
    "server": {"host": "localhost", "port": 8080},
    "debug": false
});
let overlay = uzon!({
    "server": {"port": 443, "tls": true},
    "debug": true
});
base.merge(overlay);
// server.host = "localhost" (kept from base)
// server.port = 443 (overwritten)
// server.tls = true (added)
// debug = true (overwritten)
```

---

### Building Values

#### `uzon!` macro

JSON-like syntax for building values:

```rust
use uzon::uzon;

// Primitives
uzon!(null)             // Value::Null
uzon!(true)             // Value::Bool(true)
uzon!(42)               // Value::Integer
uzon!(3.14)             // Value::Float
uzon!("hello")          // Value::String

// Collections
uzon!([1, 2, 3])        // Value::List
uzon!((1, "two", true)) // Value::Tuple
uzon!({"key": "value"}) // Value::Struct

// Nested
uzon!({
    "name": "Alice",
    "scores": [95, 88, 92],
    "address": {
        "city": "Seoul"
    }
})

// Variables
let name = "Bob";
let age = 25;
uzon!({"name": name, "age": age})
```

#### StructBuilder

Fluent API for building structs:

```rust
let user = Value::struct_builder()
    .field("name", "Alice")
    .field("age", 30)
    .field("scores", vec![Value::int(95), Value::int(88)])
    .field("address", Value::struct_builder()
        .field("city", "Seoul")
        .build())
    .build();
```

#### From / Into conversions

| Rust type | Value variant |
|-----------|---------------|
| `bool` | `Bool` |
| `i32`, `i64`, `i128`, `u32`, `u64` | `Integer` |
| `f32`, `f64` | `Float` |
| `&str`, `String` | `String` |
| `Vec<Value>` | `List` |
| `IndexMap<String, Value>` | `Struct` |
| `(Value, Value)` | `Tuple` (2 elements) |
| `(Value, Value, Value)` | `Tuple` (3 elements) |

```rust
let v: Value = 42.into();
let v: Value = "hello".into();
let v: Value = vec![Value::int(1), Value::int(2)].into();
```

---

### Type Conversion

#### TryFrom / TryInto (owned)

Fallible conversions that consume the value:

| Target | Source variant(s) | Notes |
|--------|-------------------|-------|
| `bool` | `Bool` | |
| `i64` | `Integer`, `BigInteger` | Errors if out of range |
| `i128` | `Integer`, `BigInteger` | Errors if BigInteger out of range |
| `u64` | `Integer`, `BigInteger` | Errors if negative or out of range |
| `f64` | `Float`, `Integer`, `BigInteger` | Integer auto-converts |
| `String` | `String` | |
| `Vec<Value>` | `List`, `Tuple` | |
| `IndexMap<String, Value>` | `Struct` | |

```rust
let n: i64 = Value::int(42).try_into().unwrap();
let f: f64 = Value::int(10).try_into().unwrap();  // auto-converts to 10.0
let s: String = Value::from("hi").try_into().unwrap();
```

#### TryFrom / TryInto (borrowed)

Convert `&Value` without consuming:

| Target | Source variant(s) |
|--------|-------------------|
| `bool` | `Bool` |
| `i64` | `Integer`, `BigInteger` |
| `i128` | `Integer`, `BigInteger` |
| `f64` | `Float`, `Integer`, `BigInteger` |
| `&str` | `String` |

```rust
let value = Value::int(42);
let n: i64 = (&value).try_into().unwrap();
// value is still usable
println!("{value}");
```

#### Error type

```rust
pub struct ValueConversionError {
    pub from: &'static str,  // e.g., "integer"
    pub to: &'static str,    // e.g., "bool"
}
// Display: "cannot convert integer to bool"
```

---

### Arithmetic

#### Operator overloading

Standard Rust operators between `Value` instances:

| Operator | Types | Notes |
|----------|-------|-------|
| `+` | Integer, Float, String | String concatenation via `+` |
| `-` | Integer, Float | |
| `*` | Integer, Float | |
| `/` | Integer, Float | Integer division truncates |
| `%` | Integer, Float | |
| `-` (unary) | Integer, Float | |

Mixed `Integer + Float` promotes to `Float`.

```rust
Value::int(10) + Value::int(3)      // → int(13)
Value::float(1.5) + Value::float(2) // → float(3.5)
Value::int(5) + Value::float(0.5)   // → float(5.5)
Value::from("he") + Value::from("llo") // → "hello"
-Value::int(42)                      // → int(-42)
```

#### Arithmetic with Rust primitives

Primitives work on either side:

```rust
Value::int(10) + 5       // → int(15)
5 + Value::int(10)       // → int(15)
Value::float(1.0) + 0.5  // → float(1.5)
Value::from("hi") + "!"  // → "hi!"
"hi" + Value::from("!")  // → "hi!"
```

Supported primitive types: `i32`, `i64`, `i128`, `u32`, `u64`, `f32`, `f64`, `&str`.

#### Checked arithmetic

Return `Result<Value, ValueArithmeticError>` instead of panicking:

```rust
value.checked_add(&other)   // → Result<Value, ValueArithmeticError>
value.checked_sub(&other)
value.checked_mul(&other)
value.checked_div(&other)   // errors on division by zero
value.checked_rem(&other)   // errors on division by zero
value.checked_neg()         // errors on i128::MIN negation
```

```rust
// Overflow detection
let max = Value::int(i128::MAX);
assert!(max.checked_add(&Value::int(1)).is_err());

// Division by zero
assert!(Value::int(10).checked_div(&Value::int(0)).is_err());

// Type mismatch
assert!(Value::int(1).checked_add(&Value::Bool(true)).is_err());
```

---

### Comparison

#### PartialEq (Value vs Value)

```rust
Value::int(42) == Value::int(42)        // true
Value::from("hi") == Value::from("hi")  // true
Value::Null == Value::Null              // true
Value::int(1) == Value::float(1.0)     // false (different types)
```

#### PartialEq with primitives

```rust
Value::int(42) == 42         // i32
Value::int(42) == 42i64      // i64
Value::int(42) == 42i128     // i128
Value::int(42) == 42u32      // u32
Value::int(42) == 42u64      // u64
Value::float(3.14) == 3.14   // f64
Value::Bool(true) == true    // bool
Value::from("hi") == "hi"    // &str
Value::from("hi") == String::from("hi") // String
```

#### PartialOrd (Value vs Value)

Ordering for numeric, string, and bool values. Mixed int/float comparison promotes to float.

```rust
Value::int(1) < Value::int(2)       // true
Value::float(1.0) < Value::float(2) // true
Value::int(1) < Value::float(1.5)   // true (cross-type)
Value::from("a") < Value::from("b") // true (lexicographic)
Value::Bool(false) < Value::Bool(true) // true

// Incompatible types → None
Value::int(1).partial_cmp(&Value::Bool(true)) // None
```

#### PartialOrd with primitives

```rust
Value::int(1) < 2        // i32
Value::int(10) > 5i64    // i64
Value::float(1.0) < 2.0  // f64
Value::from("a") < "b"   // &str
```

---

### Iteration

#### Borrowed iteration (`&Value`)

```rust
for item in &value {
    // item: &Value
}
```

| Value type | Yields |
|------------|--------|
| `List` | each element |
| `Tuple` | each element |
| `Struct` | each value (insertion order) |
| Others | empty iterator |

#### Owned iteration (`Value`)

```rust
for item in value {
    // item: Value (consumed)
}
```

#### Struct key-value iteration

```rust
if let Some(fields) = value.iter_fields() {
    for (key, val) in fields {
        // key: &String, val: &Value
    }
}
```

#### len

```rust
value.len()  // → Option<usize>
// List → Some(element count)
// Tuple → Some(element count)
// Struct → Some(field count)
// Others → None
```

#### Example

```rust
let users = uzon!([
    {"name": "Alice", "score": 95},
    {"name": "Bob", "score": 82}
]);

// Filter and process
let high_scorers: Vec<&str> = (&users).into_iter()
    .filter(|u| u["score"].as_i64().unwrap_or(0) > 85)
    .filter_map(|u| u["name"].as_str())
    .collect();
```

---

### Serde Integration

`Value` implements both `serde::Serialize` and `serde::Deserialize`.

#### Value → Rust type (`from_value`)

```rust
use uzon::from_value;

#[derive(serde::Deserialize)]
struct Config {
    host: String,
    port: u16,
    debug: bool,
}

let value = uzon!({"host": "localhost", "port": 8080, "debug": true});
let config: Config = from_value(value).unwrap();
```

#### UZON text → Rust type (`from_str_as`)

One-step parse and deserialize:

```rust
use uzon::from_str_as;

let config: Config = from_str_as(r#"
    host is "localhost"
    port is 8080
    debug is true
"#).unwrap();
```

#### Value → JSON (Serialize)

```rust
let value = uzon!({"name": "Alice", "scores": [95, 88]});
let json = serde_json::to_string(&value).unwrap();
// → {"name":"Alice","scores":[95,88]}
```

Serialization rules:

| UZON type | Serialized as |
|-----------|---------------|
| `Null`, `Undefined` | `null` |
| `Bool` | JSON boolean |
| `Integer` | JSON number (i64 or i128) |
| `BigInteger` | i128 if fits, otherwise string |
| `Float` | JSON number (f64) |
| `String` | JSON string |
| `List`, `Tuple` | JSON array |
| `Struct` | JSON object |
| `Enum` | JSON string (variant name) |
| `Union`, `TaggedUnion` | inner value |
| `Function` | `null` |

#### JSON → Value (Deserialize)

```rust
let value: Value = serde_json::from_str(r#"{"name": "Alice", "age": 30}"#).unwrap();
assert_eq!(value["name"], "Alice");
assert_eq!(value["age"], 30);
```

#### Error type

```rust
pub struct DeError(String);
// implements Display, Error, serde::de::Error, serde::ser::Error
```

---

### Stringification

Convert values back to UZON text:

```rust
use std::collections::BTreeMap;
use uzon::{to_string, to_string_with_options, StringifyOptions, Value};

// Default formatting (4-space indent, inline up to 4 fields)
let text = to_string(&bindings);

// Custom formatting
let opts = StringifyOptions {
    indent: 2,
    inline_threshold: 8,
};
let text = to_string_with_options(&bindings, &opts);
```

#### StringifyOptions

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `indent` | `usize` | `4` | Spaces per indentation level |
| `inline_threshold` | `usize` | `4` | Max struct fields for single-line format |

#### Roundtrip

UZON supports faithful roundtripping:

```rust
let source = r#"name is "Alice"  age is 30"#;
let parsed = from_str(source).unwrap();
let output = to_string(&parsed);
let reparsed = from_str(&output).unwrap();
assert_eq!(parsed, reparsed);
```

---

### Error Handling

All parsing functions return `Result<T, UzonError>`:

```rust
pub enum UzonError {
    Syntax { message, location, import_trace },    // lexer/parser errors
    Type { message, location, import_trace },       // type annotation errors
    Runtime { message, location, import_trace },    // evaluation errors
    Circular { message, location, import_trace },   // circular reference
}

pub struct Location {
    pub line: usize,            // 1-based
    pub col: usize,             // 1-based, Unicode scalar count
    pub filename: Option<String>,
}

pub type Result<T> = std::result::Result<T, UzonError>;
```

```rust
match from_str("x is 1 ++") {
    Ok(bindings) => { /* use bindings */ }
    Err(e) => eprintln!("Error: {e}"),
    // "SyntaxError at 1:10: unexpected token: Eof ''"
}
```

---

## Examples

The `examples/` directory contains 12 runnable examples covering every aspect of the API:

```bash
cargo run --example 01_parsing           # Parsing UZON strings
cargo run --example 02_accessing_values  # Accessors, indexing, path navigation
cargo run --example 03_building_values   # uzon! macro, StructBuilder, From/Into
cargo run --example 04_mutation          # insert, remove, push, pop, merge
cargo run --example 05_arithmetic        # Operators, checked arithmetic, primitives
cargo run --example 06_comparison        # PartialEq, PartialOrd, sorting
cargo run --example 07_iteration         # IntoIterator, iter_fields, len
cargo run --example 08_type_conversion   # TryFrom/TryInto, From/Into, to_plain
cargo run --example 09_serde             # from_value, from_str_as, JSON interop
cargo run --example 10_stringify         # to_string, StringifyOptions, roundtrip
cargo run --example 11_uzon_types        # Enums, typed numerics, tagged unions
cargo run --example 12_evaluator         # Expressions, functions, std library
```

## License

MIT
