// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use super::*;

/// Helper: evaluate UZON source and return the result map.
fn eval(source: &str) -> Result<std::collections::BTreeMap<String, Value>> {
    from_str(source)
}

/// Helper: evaluate and return plain values.
fn eval_plain(source: &str) -> Result<std::collections::BTreeMap<String, Value>> {
    from_str_plain(source)
}

/// Helper: evaluate and extract a single binding value.
fn eval_val(source: &str, name: &str) -> Value {
    eval(source).unwrap().remove(name).unwrap()
}

/// Helper: assert that evaluation produces an error.
fn eval_err(source: &str) -> crate::error::UzonError {
    eval(source).unwrap_err()
}

// === Basic literals ===

#[test]
fn test_integer_literal() {
    assert_eq!(eval_val("x is 42", "x"), Value::int(42));
    assert_eq!(eval_val("x is -7", "x"), Value::int(-7));
    assert_eq!(eval_val("x is 0", "x"), Value::int(0));
}

#[test]
fn test_float_literal() {
    let v = eval_val("x is 3.14", "x");
    if let Value::Float(f) = v {
        assert!((f.value - 3.14).abs() < 1e-10);
    } else {
        panic!("expected float");
    }
}

#[test]
fn test_string_literal() {
    assert_eq!(eval_val(r#"x is "hello""#, "x"), Value::String("hello".into()));
}

#[test]
fn test_bool_literal() {
    assert_eq!(eval_val("x is true", "x"), Value::Bool(true));
    assert_eq!(eval_val("x is false", "x"), Value::Bool(false));
}

#[test]
fn test_case_variant_typo_rejected() {
    for input in ["x is True", "x is False", "x is NULL", "x is Null",
                   "x is Undefined", "x is Inf", "x is NaN"] {
        assert!(eval(input).is_err(), "{} should be rejected", input);
    }
}

#[test]
fn test_null_literal() {
    assert_eq!(eval_val("x is null", "x"), Value::Null);
}

// === Arithmetic ===

#[test]
fn test_integer_arithmetic() {
    assert_eq!(eval_val("x is 2 + 3", "x"), Value::int(5));
    assert_eq!(eval_val("x is 10 - 3", "x"), Value::int(7));
    assert_eq!(eval_val("x is 4 * 5", "x"), Value::int(20));
    assert_eq!(eval_val("x is 10 / 3", "x"), Value::int(3));
    assert_eq!(eval_val("x is 10 % 3", "x"), Value::int(1));
}

#[test]
fn test_float_arithmetic() {
    let v = eval_val("x is 1.0 + 2.5", "x");
    if let Value::Float(f) = v {
        assert!((f.value - 3.5).abs() < 1e-10);
    } else {
        panic!("expected float");
    }
}

#[test]
fn test_power() {
    // Power operator is `^` in UZON (§5.3)
    assert_eq!(eval_val("x is 2 ^ 10", "x"), Value::int(1024));
    // ^ 0 should produce 1
    assert_eq!(eval_val("x is 5 ^ 0", "x"), Value::int(1));
}

#[test]
fn test_unary_negation() {
    assert_eq!(eval_val("a is 5\nx is -a", "x"), Value::int(-5));
}

#[test]
fn test_unary_not() {
    assert_eq!(eval_val("x is not true", "x"), Value::Bool(false));
    assert_eq!(eval_val("x is not false", "x"), Value::Bool(true));
}

// === String operations ===

#[test]
fn test_string_concat() {
    assert_eq!(
        eval_val(r#"x is "hello" ++ " " ++ "world""#, "x"),
        Value::String("hello world".into())
    );
}

#[test]
fn test_string_repeat() {
    // `**` is the repeat operator for strings (§5.8.3)
    assert_eq!(
        eval_val(r#"x is "ab" ** 3"#, "x"),
        Value::String("ababab".into())
    );
    // ** 0 should produce empty string (bug fix absorbed)
    assert_eq!(
        eval_val(r#"x is "ab" ** 0"#, "x"),
        Value::String("".into())
    );
}

#[test]
fn test_string_interpolation() {
    // UZON uses {expr} for interpolation, not ${expr}
    assert_eq!(
        eval_val("name is \"world\"\nx is \"hello {name}\"", "x"),
        Value::String("hello world".into())
    );
}

// === Comparison ===

#[test]
fn test_comparison() {
    assert_eq!(eval_val("x is 1 < 2", "x"), Value::Bool(true));
    assert_eq!(eval_val("x is 2 > 1", "x"), Value::Bool(true));
    assert_eq!(eval_val("x is 1 <= 1", "x"), Value::Bool(true));
    assert_eq!(eval_val("x is 2 >= 3", "x"), Value::Bool(false));
}

#[test]
fn test_equality() {
    assert_eq!(eval_val("x is 1 is 1", "x"), Value::Bool(true));
    assert_eq!(eval_val("x is 1 is 2", "x"), Value::Bool(false));
    assert_eq!(eval_val("x is 1 is not 2", "x"), Value::Bool(true));
}

// === Logical ===

#[test]
fn test_logical_and_or() {
    assert_eq!(eval_val("x is true and true", "x"), Value::Bool(true));
    assert_eq!(eval_val("x is true and false", "x"), Value::Bool(false));
    assert_eq!(eval_val("x is false or true", "x"), Value::Bool(true));
    assert_eq!(eval_val("x is false or false", "x"), Value::Bool(false));
}

// === Structs ===

#[test]
fn test_struct_literal() {
    let v = eval_val("x is { a is 1, b is 2 }", "x");
    if let Value::Struct(fields) = v {
        assert_eq!(fields.get("a"), Some(&Value::int(1)));
        assert_eq!(fields.get("b"), Some(&Value::int(2)));
    } else {
        panic!("expected struct");
    }
}

#[test]
fn test_struct_member_access() {
    assert_eq!(
        eval_val("s is { x is 10 }\nv is s.x", "v"),
        Value::int(10)
    );
}

#[test]
fn test_struct_override() {
    let v = eval_val("base is { x is 1, y is 2 }\nresult is base with { x is 10 }", "result");
    if let Value::Struct(fields) = v {
        assert_eq!(fields.get("x"), Some(&Value::int(10)));
        assert_eq!(fields.get("y"), Some(&Value::int(2)));
    } else {
        panic!("expected struct");
    }
}

#[test]
fn test_struct_extension() {
    let v = eval_val("base is { x is 1 }\nresult is base plus { y is 2 }", "result");
    if let Value::Struct(fields) = v {
        assert_eq!(fields.get("x"), Some(&Value::int(1)));
        assert_eq!(fields.get("y"), Some(&Value::int(2)));
    } else {
        panic!("expected struct");
    }
}

// === Lists and Tuples ===

#[test]
fn test_list_literal() {
    let v = eval_val("x is [1, 2, 3]", "x");
    if let Value::List(items) = v {
        assert_eq!(items.elements, vec![Value::int(1), Value::int(2), Value::int(3)]);
    } else {
        panic!("expected list");
    }
}

#[test]
fn test_list_homogeneity_error() {
    eval_err("x is [1, true]");
}

#[test]
fn test_tuple_literal() {
    let v = eval_val("x is (1, true)", "x");
    if let Value::Tuple(t) = v {
        assert_eq!(t.elements, vec![Value::int(1), Value::Bool(true)]);
    } else {
        panic!("expected tuple");
    }
}

#[test]
fn test_list_ordinal_access() {
    assert_eq!(
        eval_val("x is [10, 20, 30]\nv is x.first", "v"),
        Value::int(10)
    );
    assert_eq!(
        eval_val("x is [10, 20, 30]\nv is x.third", "v"),
        Value::int(30)
    );
}

// === If/then/else ===

#[test]
fn test_if_then_else() {
    assert_eq!(eval_val("x is if true then 1 else 2", "x"), Value::int(1));
    assert_eq!(eval_val("x is if false then 1 else 2", "x"), Value::int(2));
}

#[test]
fn test_if_branch_type_mismatch() {
    eval_err("x is if true then 1 else \"str\"");
}

// === Or else ===

#[test]
fn test_or_else_with_defined_value() {
    assert_eq!(
        eval_val("a is 5\nx is a or else 10", "x"),
        Value::int(5)
    );
}

// === Enum ===

#[test]
fn test_enum_from() {
    // UZON enum syntax: `value from variant1, variant2, ...` (no parens)
    let v = eval_val("x is red from red, green, blue", "x");
    if let Value::Enum(e) = v {
        assert_eq!(e.value, "red");
        assert_eq!(e.variants, vec!["red", "green", "blue"]);
    } else {
        panic!("expected enum");
    }
}

#[test]
fn test_enum_invalid_variant() {
    eval_err("x is yellow from red, green, blue");
}

#[test]
fn test_enum_min_variants() {
    eval_err("x is a from a");
}

// === Type annotations ===

#[test]
fn test_integer_type_annotation() {
    let v = eval_val("x is 42 as i32", "x");
    if let Value::Integer(n) = v {
        assert_eq!(n.value, 42);
        assert_eq!(n.type_ann, IntegerType::I(32));
    } else {
        panic!("expected integer");
    }
}

#[test]
fn test_integer_range_error() {
    eval_err("x is 256 as u8");
}

#[test]
fn test_float_type_annotation() {
    let v = eval_val("x is 3.14 as f32", "x");
    if let Value::Float(f) = v {
        assert_eq!(f.type_ann, FloatType::F32);
    } else {
        panic!("expected float");
    }
}

// === Type conversion (to) ===

#[test]
fn test_int_to_string() {
    assert_eq!(eval_val("x is 42 to string", "x"), Value::String("42".into()));
}

#[test]
fn test_string_to_int() {
    assert_eq!(eval_val(r#"x is "123" to i32"#, "x"), Value::Integer(UzonInteger::with_type(123, IntegerType::I(32))));
}

#[test]
fn test_float_to_int_truncation() {
    assert_eq!(eval_val("x is 3.7 to i32", "x"), Value::Integer(UzonInteger::with_type(3, IntegerType::I(32))));
}

#[test]
fn test_int_to_float() {
    let v = eval_val("x is 42 to f64", "x");
    if let Value::Float(f) = v {
        assert!((f.value - 42.0).abs() < 1e-10);
        assert_eq!(f.type_ann, FloatType::F64);
    } else {
        panic!("expected float");
    }
}

#[test]
fn test_bool_to_string() {
    assert_eq!(eval_val("x is true to string", "x"), Value::String("true".into()));
}

#[test]
fn test_null_to_string() {
    assert_eq!(eval_val("x is null to string", "x"), Value::String("null".into()));
}

// === Case/when/else ===

#[test]
fn test_case_when_else() {
    assert_eq!(
        eval_val("x is 2\ny is case x when 1 then 10 when 2 then 20 else 0", "y"),
        Value::int(20)
    );
}

#[test]
fn test_case_else_fallthrough() {
    assert_eq!(
        eval_val("x is 99\ny is case x when 1 then 10 else 0", "y"),
        Value::int(0)
    );
}

// === `is type` / `is not type` (§3.6) ===

#[test]
fn test_is_type_basic() {
    assert_eq!(
        eval_val("x is 42\nresult is x is type i64", "result"),
        Value::Bool(true)
    );
    assert_eq!(
        eval_val("x is 42\nresult is x is type string", "result"),
        Value::Bool(false)
    );
    assert_eq!(
        eval_val("x is 42\nresult is x is not type f64", "result"),
        Value::Bool(true)
    );
}

#[test]
fn test_is_type_union() {
    assert_eq!(
        eval_val("u is \"hello\" from union i32, string\nresult is u is type string", "result"),
        Value::Bool(true)
    );
    assert_eq!(
        eval_val("u is \"hello\" from union i32, string\nresult is u is type i32", "result"),
        Value::Bool(false)
    );
}

#[test]
fn test_is_type_null() {
    assert_eq!(
        eval_val("x is null\nresult is x is type null", "result"),
        Value::Bool(true)
    );
}

// === `case type` / `case named` (§5.10) ===

#[test]
fn test_case_type_dispatch() {
    let src = r#"
        u is 42 as i32 from union i32, string
        result is case type u
            when i32 then "integer"
            when string then "text"
            else "other"
    "#;
    assert_eq!(eval_val(src, "result"), Value::String("integer".into()));
}

#[test]
fn test_case_type_fallthrough() {
    let src = r#"
        u is "hi" from union i64, string, bool
        result is case type u
            when i64 then "integer"
            else "other"
    "#;
    assert_eq!(eval_val(src, "result"), Value::String("other".into()));
}

#[test]
fn test_case_type_non_union_dispatch() {
    let src = r#"
        x is 42
        result is case type x
            when i64 then "integer"
            else "other"
    "#;
    let result = eval(src).unwrap();
    assert_eq!(result["result"], Value::String("integer".into()));
}

#[test]
fn test_case_type_branch_narrowing() {
    // §5.10: inside a when branch, the scrutinee is narrowed to the inner value.
    // `u` is a union(i32), but inside `when i32`, `u` is narrowed to plain i32.
    // The `when string` branch uses `u ++ "!"` which would fail on union(i32),
    // but narrowing means we skip speculative evaluation of non-selected branches.
    let src = r#"
        u is 42 from union i32, string
        msg is case type u
            when i32 then "got {u}"
            when string then u ++ "!"
            else "other"
    "#;
    let result = eval(src).unwrap();
    assert_eq!(result["msg"], Value::String("got 42".into()));
}

#[test]
fn test_case_type_narrowing_tagged_union() {
    // §3.7.1/§3.7.2: inside case type when, tagged union scrutinee is narrowed
    // to its inner value. Binding preserves the wrapper in general, but narrowing
    // extracts the inner value for the matched branch.
    let src = r#"
        tu is 42 as i32 named n from n as i32, f as f64 called Num
        result is case type tu
            when i32 then tu + 1
            else 0
    "#;
    let result = eval(src).unwrap();
    // tu is narrowed from TaggedUnion(42 as i32) to plain 42 as i32
    assert_eq!(result["result"], Value::int(43));
}

#[test]
fn test_case_type_invalid_member_type_error() {
    let src = r#"
        u is 42 as i32 from union i32, string
        result is case type u
            when bool then "boolean"
            else "other"
    "#;
    let err = eval_err(src);
    assert!(err.to_string().contains("not a member type"));
}

#[test]
fn test_case_named_dispatch() {
    let src = r#"
        status is "ok" named success from success as string, error as string
        result is case named status
            when success then "good"
            when error then "bad"
            else "unknown"
    "#;
    assert_eq!(eval_val(src, "result"), Value::String("good".into()));
}

// === Functions ===

#[test]
fn test_basic_function() {
    // UZON function syntax: `function param as Type returns Type { expr }`
    assert_eq!(
        eval_val("double is function x as i64 returns i64 { x * 2 }\nresult is double(5)", "result"),
        Value::int(10)
    );
}

#[test]
fn test_function_default_param() {
    assert_eq!(
        eval_val("add is function a as i64, b as i64 default 10 returns i64 { a + b }\nresult is add(5)", "result"),
        Value::int(15)
    );
}

#[test]
fn test_function_closure_capture() {
    assert_eq!(
        eval_val("base is 100\nadd_base is function x as i64 returns i64 { x + base }\nresult is add_base(5)", "result"),
        Value::int(105)
    );
}

// === Standard library ===

#[test]
fn test_std_len() {
    assert_eq!(eval_val("x is std.len([1, 2, 3])", "x"), Value::int(3));
    assert_eq!(eval_val(r#"x is std.len("hello")"#, "x"), Value::int(5));
}

#[test]
fn test_std_keys_values() {
    let v = eval_val("s is { a is 1, b is 2 }\nk is std.keys(s)", "k");
    if let Value::List(items) = v {
        assert_eq!(items.elements, vec![Value::String("a".into()), Value::String("b".into())]);
    } else {
        panic!("expected list");
    }
}

#[test]
fn test_std_has() {
    assert_eq!(
        eval_val("s is { a is 1 }\nx is std.hasKey(s, \"a\")", "x"),
        Value::Bool(true)
    );
    assert_eq!(
        eval_val("s is { a is 1 }\nx is std.hasKey(s, \"b\")", "x"),
        Value::Bool(false)
    );
}

#[test]
fn test_std_get() {
    assert_eq!(
        eval_val("s is { a is 42 }\nx is std.get(s, \"a\")", "x"),
        Value::int(42)
    );
}

#[test]
fn test_std_map() {
    let v = eval_val(
        "nums is [1, 2, 3]\ndoubled is std.map(nums, function x as i64 returns i64 { x * 2 })",
        "doubled"
    );
    if let Value::List(items) = v {
        assert_eq!(items.elements, vec![Value::int(2), Value::int(4), Value::int(6)]);
    } else {
        panic!("expected list");
    }
}

#[test]
fn test_std_filter() {
    let v = eval_val(
        "nums is [1, 2, 3, 4, 5]\nevens is std.filter(nums, function x as i64 returns bool { x % 2 is 0 })",
        "evens"
    );
    if let Value::List(items) = v {
        assert_eq!(items.elements, vec![Value::int(2), Value::int(4)]);
    } else {
        panic!("expected list");
    }
}

#[test]
fn test_std_reduce() {
    assert_eq!(
        eval_val(
            "nums is [1, 2, 3, 4]\nsum is std.reduce(nums, 0, function acc as i64, x as i64 returns i64 { acc + x })",
            "sum"
        ),
        Value::int(10)
    );
}

#[test]
fn test_std_sort() {
    let v = eval_val(
        "nums is [3, 1, 2]\nsorted is std.sort(nums, function a as i64, b as i64 returns bool { a < b })",
        "sorted"
    );
    if let Value::List(items) = v {
        assert_eq!(items.elements, vec![Value::int(1), Value::int(2), Value::int(3)]);
    } else {
        panic!("expected list");
    }
}

#[test]
fn test_std_sort_comparator_must_have_2_params() {
    // Bug fix absorbed: std.sort comparator must take exactly 2 parameters
    eval_err("nums is [1, 2]\nsorted is std.sort(nums, function a as i64 returns bool { true })");
}

#[test]
fn test_std_string_utils() {
    assert_eq!(
        eval_val(r#"x is std.lower("HELLO")"#, "x"),
        Value::String("hello".into())
    );
    assert_eq!(
        eval_val(r#"x is std.upper("hello")"#, "x"),
        Value::String("HELLO".into())
    );
    assert_eq!(
        eval_val(r#"x is std.trim("  hi  ")"#, "x"),
        Value::String("hi".into())
    );
    assert_eq!(
        eval_val(r#"x is std.replace("hello world", "world", "rust")"#, "x"),
        Value::String("hello rust".into())
    );
}

#[test]
fn test_std_split_join() {
    let v = eval_val(r#"x is std.split("a,b,c", ",")"#, "x");
    if let Value::List(items) = v {
        assert_eq!(items.elements, vec![
            Value::String("a".into()),
            Value::String("b".into()),
            Value::String("c".into()),
        ]);
    } else {
        panic!("expected list");
    }

    assert_eq!(
        eval_val(r#"x is std.join(["a", "b", "c"], "-")"#, "x"),
        Value::String("a-b-c".into())
    );
}

#[test]
fn test_std_float_predicates() {
    assert_eq!(eval_val("x is std.isNan(nan)", "x"), Value::Bool(true));
    assert_eq!(eval_val("x is std.isNan(1.0)", "x"), Value::Bool(false));
    assert_eq!(eval_val("x is std.isInf(inf)", "x"), Value::Bool(true));
    assert_eq!(eval_val("x is std.isInf(1.0)", "x"), Value::Bool(false));
    assert_eq!(eval_val("x is std.isFinite(1.0)", "x"), Value::Bool(true));
    assert_eq!(eval_val("x is std.isFinite(inf)", "x"), Value::Bool(false));
}

// === Dependency ordering ===

#[test]
fn test_topological_sort() {
    // b depends on a, but declared before a
    let result = eval("b is a + 1\na is 10").unwrap();
    assert_eq!(result.get("a"), Some(&Value::int(10)));
    assert_eq!(result.get("b"), Some(&Value::int(11)));
}

#[test]
fn test_circular_dependency() {
    eval_err("a is b\nb is a");
}

#[test]
fn test_multiple_circular_dependencies() {
    // Two independent cycles: a↔b and c↔d — should report all participants
    let err = eval_err("a is b\nb is a\nc is d\nd is c");
    if let crate::error::UzonError::Multiple { errors } = err {
        assert!(errors.len() >= 2, "expected multiple errors, got {}", errors.len());
        for e in &errors {
            assert!(e.is_circular(), "expected circular error, got: {e}");
        }
    } else {
        panic!("expected Multiple error, got: {err}");
    }
}

// === Bare name resolution (§5.12) ===

#[test]
fn test_bare_name_resolution() {
    let v = eval_val("s is { x is 10, y is x + 1 }", "s");
    if let Value::Struct(fields) = v {
        assert_eq!(fields.get("y"), Some(&Value::int(11)));
    } else {
        panic!("expected struct");
    }
}

// === Undefined propagation ===

#[test]
fn test_undefined_literal_rejected() {
    eval_err("x is undefined");
}

// === Plain mode ===

#[test]
fn test_plain_mode_strips_enums() {
    let result = eval_plain("x is red from red, green, blue").unwrap();
    assert_eq!(result.get("x"), Some(&Value::String("red".into())));
}

// === Duplicate binding ===

#[test]
fn test_duplicate_binding_error() {
    eval_err("x is 1\nx is 2");
}

// === Named types ===

#[test]
fn test_called_enum() {
    let v = eval_val(
        "color is red from red, green, blue called Color\nresult is green as Color",
        "result"
    );
    if let Value::Enum(e) = v {
        assert_eq!(e.value, "green");
        assert_eq!(e.type_name, Some("Color".into()));
    } else {
        panic!("expected enum");
    }
}

// === In operator ===

#[test]
fn test_in_operator() {
    assert_eq!(eval_val("x is 2 in [1, 2, 3]", "x"), Value::Bool(true));
    assert_eq!(eval_val("x is 5 in [1, 2, 3]", "x"), Value::Bool(false));
}

// === Inf/Nan ===

#[test]
fn test_inf_nan_literals() {
    let v = eval_val("x is inf", "x");
    if let Value::Float(f) = v {
        assert!(f.value.is_infinite() && f.value.is_sign_positive());
    } else {
        panic!("expected float");
    }
    let v = eval_val("x is nan", "x");
    if let Value::Float(f) = v {
        assert!(f.value.is_nan());
    } else {
        panic!("expected float");
    }
}

// === Empty list error ===

#[test]
fn test_empty_list_requires_type() {
    eval_err("x is []");
}
