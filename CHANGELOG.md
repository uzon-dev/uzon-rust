# Changelog

All notable changes to this project will be documented in this file.

## [0.9.0] - 2026-04-18

### Added

- **Standalone type declarations** (§6.2) — direct type-naming syntax where the binding name is the type name (no `called` needed):
  - `X is struct { ... }` — struct type
  - `X is enum v1, v2, ...` — enum type
  - `X is union T1, T2, ...` — union type (value is the first member type's default)
  - `X is tagged union v1 as T1, v2 as T2, ...` — tagged union (value is the first variant's default, tagged with the first variant name)
- `enum` and `tagged` keywords added to the lexer
- `StandaloneTypeKind` on `Binding` records the standalone form for tooling
- `DefaultForType { type_expr }` AST node for computing per-§3.6 defaults at evaluation time

### Fixed

- **§3.8 defaults**: parameter defaults that reference another parameter of the same function are now a syntax error; `undefined` as a default value is rejected; default values are eagerly evaluated at function definition time and type-checked against the declared parameter type
- **§3.4/§6.1 typed list homogeneity**: `[1 as i32, 2 as i64]` now errors with a clear type mismatch; the same rule applies to explicitly-typed floats
- **§5.7 or-else static type guarantee**: when the left operand is undefined at runtime but carries a static type via `as T`/`to T`, the right operand is type-checked against T (catches `env.PORT to u16 or else "default"`)
- **§5.3 exponentiation**: negative base with a non-integer exponent is now a runtime error (e.g., `(-2.0) ^ 0.5`)
- **§6.3 nominal type identity**: re-annotating a struct value with a different named struct type (e.g., `a as B` when `a` has type `A`) is now a type error even when the shapes match

## [0.6.0] - 2026-04-12

### Changed

- Spec v0.6 compliance: error classifications, conversion handling, `std.len` Unicode scalar count
- README examples updated to follow style guide (Appendix E)

### Fixed

- `null.foo` classified as type error per §5.12
- `undefined` in `and`/`or`/`not`/`is named` classified as runtime error per §3.1
- `null to <non-string>` classified as type error per §5.11.0
- `<non-null> to null` rejected as type error per §5.11.0
- `"0xff" to f64` now parses hex/oct/bin prefixes via integer widening per §5.11.1
- `std.split` rule precedence: empty input → empty delimiter → normal split per §5.16.4
- `std.split("")` uses Unicode scalar splitting instead of Rust `str::split("")`
- `std.len` returns Unicode scalar count for strings per §5.16

## [0.5.0] - 2026-04-11

### Added

- **Stringify module** — `to_string`, `to_string_with_options`, `StringifyOptions` for converting values back to UZON text
- **Rust-native value operations**
  - Accessors: `as_bool`, `as_i64`, `as_i128`, `as_f64`, `as_str`, `as_list`, `as_tuple`, `as_struct`, `as_integer`, `as_float`
  - Safe navigation: `get`, `get_index`, `get_mut`, `get_index_mut`, `get_path`
  - Index traits: `value["key"]`, `value[0]`
  - Mutation: `insert`, `remove`, `push`, `pop`
  - Deep merge: `merge` with recursive struct merging
  - `StructBuilder` for fluent struct construction
  - `uzon!` macro for JSON-like value construction
- **Type conversions**
  - `From`/`Into` for `bool`, `i32`, `i64`, `i128`, `u32`, `u64`, `f32`, `f64`, `&str`, `String`, `Vec<Value>`, `IndexMap`, tuples
  - `TryFrom`/`TryInto` for `bool`, `i64`, `i128`, `u64`, `f64`, `String`, `Vec<Value>`, `IndexMap` (owned and borrowed)
- **Arithmetic operators**: `+`, `-`, `*`, `/`, `%`, unary `-` between Value instances and with Rust primitives
- **Checked arithmetic**: `checked_add`, `checked_sub`, `checked_mul`, `checked_div`, `checked_rem`, `checked_neg` returning `Result`
- **Comparison**: `PartialEq` and `PartialOrd` with primitives (`i32`, `i64`, `i128`, `u32`, `u64`, `f64`, `&str`, `String`, `bool`)
- **Iteration**: `IntoIterator` for borrowed and owned `Value`, `iter_fields`, `len`
- **Serde integration**
  - `Serialize` for `Value` (to any serde format)
  - `Deserialize` for `Value` (from any serde format, e.g., JSON → Value)
  - `from_value<T>` to deserialize `Value` into Rust types
  - `from_str_as<T>` for direct UZON text → Rust type deserialization
- **Public re-exports**: `UzonEnum`, `UzonUnion`, `UzonTaggedUnion`, `UzonTuple`, `UzonList`, `UzonFunction`, `UzonUndefined`, `ValueConversionError`, `ValueArithmeticError`, `DeError`
- **12 comprehensive examples** covering the full API
- Import stack traces in error messages

### Fixed

- Roundtrip conformance (2000/2000 tests pass)
- Unknown bare identifiers now error instead of silently becoming strings
- Keyword typo hints (e.g., `True` → "did you mean 'true'?")
- `BigInteger` handling in `TryFrom`, accessors, and serde deserialization
- `merge()` now unwraps `Union`/`TaggedUnion` before merging
- `TaggedUnion` serialization no longer injects a `_tag` field

## [0.3.0] - 2026-04-10

### Added

- Tree-walking evaluator with lazy declarative binding resolution
- Value system: `Null`, `Undefined`, `Bool`, `Integer`, `BigInteger`, `Float`, `String`, `List`, `Tuple`, `Struct`, `Enum`, `Union`, `TaggedUnion`, `Function`
- Typed numerics with `IntegerType` (i8–i128, u8–u128, arbitrary) and `FloatType` (f16–f128)
- Scope system with self-references, exclusion, and closure capture
- Standard library: `len`, `has`, `keys`, `values`, `map`, `filter`, `reduce`, `sort`, `join`, `split`, `trim`, `upper`, `lower`, `replace`, `get`, `string`, `abs`, `min`, `max`, `floor`, `ceil`, `round`
- UZON expressions: arithmetic, string concat/repeat/interpolation, comparisons, logical operators, conditionals, `case`/`when`, `or else`, `is`/`is not`, `in`, `to` conversions
- Struct operations: `extends`, `with`
- First-class functions with typed parameters and closures
- Enum, union, and tagged union types
- Import support with circular reference detection
- `from_str`, `from_str_plain`, `from_path` convenience functions

## [0.2.0] - 2026-04-09

### Added

- Recursive descent parser with 18-level precedence climbing
- Full UZON syntax support: bindings, expressions, type annotations, struct/list/tuple literals, function definitions, imports
- AST types: `Document`, `Binding`, `Node`, `NodeKind`, `TypeExpr`, `FunctionParam`

## [0.1.0] - 2026-04-08

### Added

- Token types for UZON lexical grammar
- Error system with syntax, type, runtime, and circular error variants
- Location tracking (line, column, filename)
- Lexer with full UZON token support: keywords, operators, literals (integer, float, string with escapes and interpolation), identifiers
