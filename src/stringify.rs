// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write;

use indexmap::IndexMap;

use crate::value::*;

/// Options for UZON text generation (Appendix E).
pub struct StringifyOptions {
    /// Number of spaces per indentation level.
    pub indent: usize,
    /// Maximum number of struct fields for inline formatting.
    pub inline_threshold: usize,
}

impl Default for StringifyOptions {
    fn default() -> Self {
        Self {
            indent: 4,
            inline_threshold: 4,
        }
    }
}

/// Generate UZON text from a map of bindings with default formatting.
pub fn to_string(values: &BTreeMap<String, Value>) -> String {
    to_string_with_options(values, &StringifyOptions::default())
}

/// Generate UZON text from a map of bindings with custom options.
pub fn to_string_with_options(values: &BTreeMap<String, Value>, options: &StringifyOptions) -> String {
    let mut out = String::new();
    let mut st = HashSet::new();
    let names: Vec<&String> = values.keys().collect();
    for (i, name) in names.iter().enumerate() {
        let value = &values[*name];
        write_binding(&mut out, name, value, 0, options, &mut st);
        if i + 1 < names.len() {
            out.push('\n');
        }
    }
    out
}

/// Write a single binding: `name is value` or `name are [items]` (E.4).
fn write_binding(
    out: &mut String,
    name: &str,
    value: &Value,
    depth: usize,
    options: &StringifyOptions,
    st: &mut HashSet<String>,
) {
    write_indent(out, depth, options);
    write_identifier(out, name);

    // E.4: prefer `are` for non-empty lists when safe
    if let Value::List(list) = value {
        // §3.4.1: lists with a named type must preserve the type across
        // roundtrip. When `type_name` is present, the name subsumes the
        // element_type annotation so we may still use the compact `are` form.
        if !list.is_empty()
            && is_are_safe(list, st)
            && (list.element_type.is_none() || list.type_name.is_some())
        {
            let named_suffix = list_named_type_suffix(list, st);
            write_are_list(out, list, depth, options, st, &named_suffix);
            return;
        }
    }

    out.push_str(" is ");
    write_value(out, value, depth, options, st);
    if let Value::List(list) = value {
        out.push_str(&list_named_type_suffix(list, st));
    }
    out.push('\n');
}

/// §3.4.1: Compute the ` called Name` or ` as Name` suffix for a list with a
/// named type, tracking first-vs-subsequent use via `st`. Empty string when the
/// list has no `type_name`.
fn list_named_type_suffix(list: &UzonList, st: &mut HashSet<String>) -> String {
    let Some(ref name) = list.type_name else {
        return String::new();
    };
    if st.contains(name) {
        format!(" as {name}")
    } else {
        st.insert(name.clone());
        format!(" called {name}")
    }
}

/// Check whether `are` syntax is safe for a list.
/// Unsafe when elements produce `called`/`as` suffixes or complex `from` clauses.
fn is_are_safe(items: &[Value], _st: &HashSet<String>) -> bool {
    !items.iter().any(|v| match v {
        // Named enum: produces `called` (first def) or `as TypeName` (subsequent)
        Value::Enum(e) => e.type_name.is_some(),
        // Unions and tagged unions are always complex in `are` context
        Value::Union(_) | Value::TaggedUnion(_) => true,
        // Lists with element_type produce `[] as [Type]` or `[...] as [Type]` — trailing `as`
        Value::List(l) => l.element_type.is_some(),
        _ => false,
    })
}

/// Write a list using `are` syntax (E.4).
fn write_are_list(
    out: &mut String,
    items: &[Value],
    depth: usize,
    options: &StringifyOptions,
    st: &mut HashSet<String>,
    trailing_suffix: &str,
) {
    // Try inline: `name are elem1, elem2, elem3[ trailing_suffix]`
    if !has_nested_collection(items) {
        let mut inline = String::from(" are ");
        let mut inline_st = st.clone();
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                inline.push_str(", ");
            }
            write_value(&mut inline, item, depth, options, &mut inline_st);
        }
        inline.push_str(trailing_suffix);
        inline.push('\n');
        if inline.len() <= 80 {
            *st = inline_st;
            out.push_str(&inline);
            return;
        }
    }

    // Multiline: commas between elements, no trailing comma (E.2)
    out.push_str(" are\n");
    for (i, item) in items.iter().enumerate() {
        write_indent(out, depth + 1, options);
        write_value(out, item, depth + 1, options, st);
        if i + 1 < items.len() {
            out.push(',');
        }
        out.push('\n');
    }
    if !trailing_suffix.is_empty() {
        write_indent(out, depth, options);
        out.push_str(trailing_suffix.trim_start());
        out.push('\n');
    }
}

fn write_indent(out: &mut String, depth: usize, options: &StringifyOptions) {
    for _ in 0..depth * options.indent {
        out.push(' ');
    }
}

/// Write an identifier, applying @-prefix escaping for keywords (§2.4)
/// or single-quote quoting for non-standard identifiers.
fn write_identifier(out: &mut String, name: &str) {
    if name.is_empty() || needs_quoting(name) {
        if is_stringify_keyword(name) {
            out.push('@');
            out.push_str(name);
        } else {
            out.push('\'');
            out.push_str(name);
            out.push('\'');
        }
    } else {
        out.push_str(name);
    }
}

/// Check if an identifier needs quoting or @-prefix escaping.
/// Per §2.3, `-` is a token boundary character and must trigger quoting.
fn needs_quoting(name: &str) -> bool {
    if name.is_empty() {
        return true;
    }
    let first = name.chars().next().unwrap();
    if !first.is_alphabetic() && first != '_' {
        return true;
    }
    for ch in name.chars() {
        if !ch.is_alphanumeric() && ch != '_' {
            return true;
        }
    }
    is_stringify_keyword(name)
}

/// Keywords that require @-prefix escaping when used as identifiers.
fn is_stringify_keyword(name: &str) -> bool {
    matches!(
        name,
        "is" | "are"
            | "not" | "and" | "or"
            | "if" | "then" | "else" | "case" | "when"
            | "from" | "named" | "as" | "to" | "with" | "of" | "in"
            | "called" | "struct" | "union" | "enum" | "tagged"
            | "true" | "false" | "null" | "undefined"
            | "env" | "inf" | "nan"
            | "function" | "returns" | "default" | "plus"
            | "lazy" | "type"
    )
}

/// Write a value in UZON syntax.
fn write_value(
    out: &mut String,
    value: &Value,
    depth: usize,
    options: &StringifyOptions,
    st: &mut HashSet<String>,
) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Undefined => out.push_str("undefined"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Integer(n) => write!(out, "{}", n.value).unwrap(),
        Value::BigInteger(n) => write!(out, "{n}").unwrap(),
        Value::Float(f) => write_float(out, f.value),
        Value::String(s) => write_string(out, s),
        Value::List(list) => write_list(out, list, depth, options, st),
        Value::Tuple(t) => write_tuple(out, &t.elements, depth, options, st),
        Value::Struct(s) => write_struct(out, s, depth, options, st),
        Value::Enum(e) => write_enum(out, e, st),
        Value::Union(u) => write_union(out, u, depth, options, st),
        Value::TaggedUnion(tu) => write_tagged_union(out, tu, depth, options, st),
        Value::Function(_) => out.push_str("<function>"),
    }
}

/// Write a float using spec-compliant formatting (§5.11.2).
fn write_float(out: &mut String, f: f64) {
    out.push_str(&format_float(f));
}

/// Write a string with proper escape sequences (§4.4).
fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            '{' => out.push_str("\\{"),
            _ => out.push(ch),
        }
    }
    out.push('"');
}

/// Write a list in bracket syntax `[ ... ]`.
fn write_list(
    out: &mut String,
    list: &UzonList,
    depth: usize,
    options: &StringifyOptions,
    st: &mut HashSet<String>,
) {
    // §3.4.1: when the list has a named type AND at least one concrete
    // element, write_binding emits `called Name` or `as Name` alone — the
    // named type's registered element_type carries the element info. For
    // empty or all-null lists, keep `as [T]` so the re-parse can register
    // the named type with its element type at the declaration site.
    let emit_element_type = list.type_name.is_none()
        || list.is_empty()
        || list.iter().all(|v| v.is_null());

    if list.is_empty() {
        out.push_str("[]");
        // Emit type annotation for empty lists that have element_type
        if emit_element_type {
            if let Some(ref et) = list.element_type {
                out.push_str(" as [");
                out.push_str(et);
                out.push(']');
            }
        }
        return;
    }

    // All-null lists need type annotation too
    if list.iter().all(|v| v.is_null()) {
        if emit_element_type {
            if let Some(ref et) = list.element_type {
                // Inline: [ null, null ] as [Type]
                let mut inline = String::from("[ ");
                for (i, _) in list.iter().enumerate() {
                    if i > 0 { inline.push_str(", "); }
                    inline.push_str("null");
                }
                inline.push_str(" ] as [");
                inline.push_str(et);
                inline.push(']');
                out.push_str(&inline);
                return;
            }
        }
    }

    if !has_nested_collection(list) {
        let inline = format_inline_list(list, depth, options, &mut st.clone());
        if inline.len() <= 80 {
            out.push_str(&inline);
            // Emit type annotation for non-empty typed lists (needed for roundtrip)
            if emit_element_type {
                if let Some(ref et) = list.element_type {
                    out.push_str(" as [");
                    out.push_str(et);
                    out.push(']');
                }
            }
            return;
        }
    }

    // Multiline — commas between elements (E.2)
    out.push_str("[\n");
    for (i, item) in list.iter().enumerate() {
        write_indent(out, depth + 1, options);
        write_list_element(out, item, depth + 1, options, st, list.element_type.as_deref());
        if i + 1 < list.len() {
            out.push(',');
        }
        out.push('\n');
    }
    write_indent(out, depth, options);
    out.push(']');
    // Emit type annotation for non-empty typed lists (needed for roundtrip)
    if emit_element_type {
        if let Some(ref et) = list.element_type {
            out.push_str(" as [");
            out.push_str(et);
            out.push(']');
        }
    }
}

/// Write a list element. When the list has a matching `element_type`, suppress
/// a struct element's own `called TypeName` suffix — the list-level
/// `as [TypeName]` already provides the type at reparse, and repeating
/// `called` would declare the type twice.
fn write_list_element(
    out: &mut String,
    value: &Value,
    depth: usize,
    options: &StringifyOptions,
    st: &mut HashSet<String>,
    element_type: Option<&str>,
) {
    if let (Value::Struct(s), Some(et)) = (value, element_type) {
        if s.type_name.as_deref() == Some(et) {
            write_struct_body(out, &s.fields, depth, options, st);
            return;
        }
    }
    write_value(out, value, depth, options, st);
}

fn format_inline_list(
    list: &UzonList,
    depth: usize,
    options: &StringifyOptions,
    st: &mut HashSet<String>,
) -> String {
    let mut s = String::from("[ ");
    for (i, item) in list.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        write_list_element(&mut s, item, depth, options, st, list.element_type.as_deref());
    }
    s.push_str(" ]");
    s
}

/// Write a tuple `(a, b, c)` with trailing comma for single-element (§3.3).
fn write_tuple(
    out: &mut String,
    elements: &[Value],
    depth: usize,
    options: &StringifyOptions,
    st: &mut HashSet<String>,
) {
    if elements.is_empty() {
        out.push_str("()");
        return;
    }

    out.push('(');
    for (i, elem) in elements.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_value(out, elem, depth, options, st);
    }
    if elements.len() == 1 {
        out.push(',');
    }
    out.push(')');
}

/// Write a struct `{ name is value, ... }`.
/// §6.2: Types defined inside a struct are scoped to that struct.
/// Emits `called TypeName` only for the declaration site (where `declares_type`
/// is set by `set_type_name` at the source-level `called` clause) and only on
/// first occurrence of the name in `st`. Inherited type_name propagated through
/// `with` / std.* does NOT emit any suffix — the list-level `as [T]` or a
/// subsequent `as T` annotation carries the nominal identity at reparse.
fn write_struct(
    out: &mut String,
    s: &UzonStruct,
    depth: usize,
    options: &StringifyOptions,
    st: &mut HashSet<String>,
) {
    write_struct_body(out, &s.fields, depth, options, st);
    if let Some(ref type_name) = s.type_name {
        if s.declares_type && !st.contains(type_name) {
            out.push_str(" called ");
            write_identifier(out, type_name);
            st.insert(type_name.clone());
        }
    }
}

/// Write a struct's `{ fields }` body without any `called`/`as` suffix. Used
/// when the enclosing context (e.g. a list with matching `element_type`)
/// already provides the named type, or for anonymous structs.
fn write_struct_body(
    out: &mut String,
    fields: &IndexMap<String, Value>,
    depth: usize,
    options: &StringifyOptions,
    st: &mut HashSet<String>,
) {
    if fields.is_empty() {
        out.push_str("{}");
        return;
    }

    if fields.len() <= options.inline_threshold && !has_nested_struct_fields(fields) {
        let inline = format_inline_struct(fields, depth, options, &mut st.clone());
        if inline.len() <= 80 {
            out.push_str(&inline);
            return;
        }
    }

    let mut child_st = st.clone();
    out.push_str("{\n");
    for (name, value) in fields {
        write_binding(out, name, value, depth + 1, options, &mut child_st);
    }
    write_indent(out, depth, options);
    out.push('}');
}

fn format_inline_struct(
    fields: &IndexMap<String, Value>,
    depth: usize,
    options: &StringifyOptions,
    st: &mut HashSet<String>,
) -> String {
    let mut s = String::from("{ ");
    let entries: Vec<_> = fields.iter().collect();
    for (i, (name, value)) in entries.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        write_identifier(&mut s, name);
        s.push_str(" is ");
        write_value(&mut s, value, depth, options, st);
    }
    s.push_str(" }");
    s
}

fn has_nested_collection(items: &[Value]) -> bool {
    items.iter().any(|v| matches!(v,
        Value::Struct(_) | Value::List(_) | Value::Tuple(_)
        | Value::Enum(_) | Value::Union(_) | Value::TaggedUnion(_)
    ))
}

fn has_nested_struct_fields(fields: &IndexMap<String, Value>) -> bool {
    fields.values().any(|v| matches!(v,
        Value::Struct(_) | Value::List(_) | Value::Tuple(_)
    ))
}

/// Write an enum value.
/// First occurrence of a named type: `variant from v1, v2, v3 called TypeName`
/// Subsequent: `variant as TypeName` (shorthand)
fn write_enum(out: &mut String, e: &UzonEnum, st: &mut HashSet<String>) {
    if let Some(ref type_name) = e.type_name {
        if st.contains(type_name) {
            // Shorthand: variant as TypeName
            write_identifier(out, &e.value);
            out.push_str(" as ");
            write_identifier(out, type_name);
            return;
        }
    }
    // Full definition
    write_identifier(out, &e.value);
    out.push_str(" from ");
    for (i, v) in e.variants.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_identifier(out, v);
    }
    if let Some(ref type_name) = e.type_name {
        out.push_str(" called ");
        write_identifier(out, type_name);
        st.insert(type_name.clone());
    }
}

/// Write a union value.
/// First occurrence: `value from union type1, type2 called TypeName`
/// Subsequent: `value as TypeName`
fn write_union(
    out: &mut String,
    u: &UzonUnion,
    depth: usize,
    options: &StringifyOptions,
    st: &mut HashSet<String>,
) {
    if let Some(ref type_name) = u.type_name {
        if st.contains(type_name) {
            write_value(out, &u.value, depth, options, st);
            out.push_str(" as ");
            write_identifier(out, type_name);
            return;
        }
    }
    write_value(out, &u.value, depth, options, st);
    out.push_str(" from union ");
    for (i, t) in u.types.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(t);
    }
    if let Some(ref type_name) = u.type_name {
        out.push_str(" called ");
        write_identifier(out, type_name);
        st.insert(type_name.clone());
    }
}

/// Write a tagged union value.
/// First occurrence: `value named tag from v1 as t1, v2 as t2 called TypeName`
/// Subsequent: `value as TypeName named tag`
fn write_tagged_union(
    out: &mut String,
    tu: &UzonTaggedUnion,
    depth: usize,
    options: &StringifyOptions,
    st: &mut HashSet<String>,
) {
    if let Some(ref type_name) = tu.type_name {
        if st.contains(type_name) {
            write_value(out, &tu.value, depth, options, st);
            out.push_str(" as ");
            write_identifier(out, type_name);
            out.push_str(" named ");
            write_identifier(out, &tu.tag);
            return;
        }
    }
    write_value(out, &tu.value, depth, options, st);
    out.push_str(" named ");
    write_identifier(out, &tu.tag);
    out.push_str(" from ");
    let entries: Vec<_> = tu.variants.iter().collect();
    for (i, (name, type_ref)) in entries.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_identifier(out, name);
        if let Some(t) = type_ref {
            if !t.is_empty() {
                out.push_str(" as ");
                out.push_str(t);
            }
        }
    }
    if let Some(ref type_name) = tu.type_name {
        out.push_str(" called ");
        write_identifier(out, type_name);
        st.insert(type_name.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_values() {
        let mut map = BTreeMap::new();
        map.insert("a".into(), Value::int(42));
        map.insert("b".into(), Value::String("hello".into()));
        map.insert("c".into(), Value::Bool(true));
        map.insert("d".into(), Value::Null);
        let result = to_string(&map);
        assert!(result.contains("a is 42"));
        assert!(result.contains("b is \"hello\""));
        assert!(result.contains("c is true"));
        assert!(result.contains("d is null"));
    }

    #[test]
    fn test_float_values() {
        let mut map = BTreeMap::new();
        map.insert("x".into(), Value::float(3.14));
        map.insert("y".into(), Value::float(1.0));
        map.insert("z".into(), Value::float(f64::INFINITY));
        map.insert("w".into(), Value::float(f64::NAN));
        let result = to_string(&map);
        assert!(result.contains("x is 3.14"));
        assert!(result.contains("y is 1.0"));
        assert!(result.contains("z is inf"));
        assert!(result.contains("w is nan"));
    }

    #[test]
    fn test_neg_inf() {
        let mut map = BTreeMap::new();
        map.insert("x".into(), Value::float(f64::NEG_INFINITY));
        let result = to_string(&map);
        assert!(result.contains("x is -inf"));
    }

    #[test]
    fn test_string_escaping() {
        let mut map = BTreeMap::new();
        map.insert("s".into(), Value::String("line1\nline2".into()));
        map.insert("q".into(), Value::String("say \"hi\"".into()));
        map.insert("b".into(), Value::String("{braces}".into()));
        let result = to_string(&map);
        assert!(result.contains(r#"s is "line1\nline2""#));
        assert!(result.contains(r#"q is "say \"hi\"""#));
        assert!(result.contains(r#"b is "\{braces}"#));
    }

    #[test]
    fn test_list_are_syntax() {
        let mut map = BTreeMap::new();
        map.insert(
            "items".into(),
            Value::list(vec![Value::int(1), Value::int(2), Value::int(3)]),
        );
        let result = to_string(&map);
        assert!(result.contains("items are 1, 2, 3"));
    }

    #[test]
    fn test_empty_list() {
        let mut map = BTreeMap::new();
        map.insert("items".into(), Value::list(vec![]));
        let result = to_string(&map);
        assert!(result.contains("items is []"));
    }

    #[test]
    fn test_tuple() {
        let mut map = BTreeMap::new();
        map.insert(
            "t".into(),
            Value::Tuple(UzonTuple::new(vec![
                Value::int(1),
                Value::String("hello".into()),
            ])),
        );
        let result = to_string(&map);
        assert!(result.contains("t is (1, \"hello\")"));
    }

    #[test]
    fn test_single_element_tuple() {
        let mut map = BTreeMap::new();
        map.insert(
            "t".into(),
            Value::Tuple(UzonTuple::new(vec![Value::int(42)])),
        );
        let result = to_string(&map);
        assert!(result.contains("t is (42,)"));
    }

    #[test]
    fn test_struct_inline() {
        let mut fields = IndexMap::new();
        fields.insert("x".into(), Value::int(1));
        fields.insert("y".into(), Value::int(2));
        let mut map = BTreeMap::new();
        map.insert("point".into(), Value::Struct(UzonStruct::new(fields)));
        let result = to_string(&map);
        assert!(result.contains("point is { x is 1, y is 2 }"));
    }

    #[test]
    fn test_enum() {
        let mut map = BTreeMap::new();
        map.insert(
            "color".into(),
            Value::Enum(UzonEnum::new(
                "red",
                vec!["red".into(), "green".into(), "blue".into()],
                None,
            )),
        );
        let result = to_string(&map);
        assert!(result.contains("color is red from red, green, blue"));
    }

    #[test]
    fn test_identifier_quoting() {
        let mut map = BTreeMap::new();
        map.insert("Content-Type".into(), Value::String("text/html".into()));
        let result = to_string(&map);
        assert!(result.contains("'Content-Type' is \"text/html\""), "got: {result}");
    }

    #[test]
    fn test_keyword_identifier_quoting() {
        let mut fields = IndexMap::new();
        fields.insert("is".into(), Value::int(1));
        let mut map = BTreeMap::new();
        map.insert("s".into(), Value::Struct(UzonStruct::new(fields)));
        let result = to_string(&map);
        assert!(result.contains("@is is 1"), "expected @is, got: {result}");
    }

    #[test]
    fn test_undefined() {
        let mut map = BTreeMap::new();
        map.insert("x".into(), Value::Undefined);
        let result = to_string(&map);
        assert!(result.contains("x is undefined"));
    }

    #[test]
    fn test_empty_struct() {
        let mut map = BTreeMap::new();
        map.insert("s".into(), Value::Struct(UzonStruct::new(IndexMap::new())));
        let result = to_string(&map);
        assert!(result.contains("s is {}"));
    }

    #[test]
    fn test_empty_tuple() {
        let mut map = BTreeMap::new();
        map.insert("t".into(), Value::Tuple(UzonTuple::new(vec![])));
        let result = to_string(&map);
        assert!(result.contains("t is ()"));
    }

    #[test]
    fn test_roundtrip() {
        let input = "x is 42\ny is \"hello\"\nz is true\nw is null";
        let result1 = crate::evaluator::from_str(input).unwrap();
        let text = to_string(&result1);
        let result2 = crate::evaluator::from_str(&text).unwrap();
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_roundtrip_struct() {
        let input = "s is { a is 1, b is \"two\" }";
        let result1 = crate::evaluator::from_str(input).unwrap();
        let text = to_string(&result1);
        let result2 = crate::evaluator::from_str(&text).unwrap();
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_roundtrip_enum() {
        let input = "e is green from red, green, blue";
        let result1 = crate::evaluator::from_str(input).unwrap();
        let text = to_string(&result1);
        let result2 = crate::evaluator::from_str(&text).unwrap();
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_roundtrip_list() {
        let input = "items are 1, 2, 3";
        let result1 = crate::evaluator::from_str(input).unwrap();
        let text = to_string(&result1);
        let result2 = crate::evaluator::from_str(&text).unwrap();
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_roundtrip_nested_struct() {
        let input = "config is {\n    host is \"localhost\"\n    port is 8080\n}";
        let result1 = crate::evaluator::from_str(input).unwrap();
        let text = to_string(&result1);
        let result2 = crate::evaluator::from_str(&text).unwrap();
        assert_eq!(result1, result2);
    }
}
