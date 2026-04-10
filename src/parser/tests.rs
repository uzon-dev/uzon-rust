// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use super::*;
use crate::lexer::Lexer;

fn parse(input: &str) -> Document {
    let (tokens, comment_lines) = Lexer::new(input).tokenize().unwrap();
    Parser::new(tokens, comment_lines).parse().unwrap()
}

fn try_parse(input: &str) -> Result<Document> {
    let (tokens, comment_lines) = Lexer::new(input).tokenize()?;
    Parser::new(tokens, comment_lines).parse()
}

#[test]
fn test_simple_binding() {
    let doc = parse("x is 42");
    assert_eq!(doc.bindings.len(), 1);
    assert_eq!(doc.bindings[0].name, "x");
    assert!(matches!(
        doc.bindings[0].value.kind,
        NodeKind::IntegerLiteral { ref value } if value == "42"
    ));
}

#[test]
fn test_multiple_bindings() {
    let doc = parse("x is 1\ny is 2\nz is 3");
    assert_eq!(doc.bindings.len(), 3);
}

#[test]
fn test_struct_literal() {
    let doc = parse("s is { a is 1, b is 2 }");
    if let NodeKind::StructLiteral { ref fields } = doc.bindings[0].value.kind {
        assert_eq!(fields.len(), 2);
    } else {
        panic!("expected StructLiteral");
    }
}

#[test]
fn test_list_literal() {
    let doc = parse("items is [1, 2, 3]");
    if let NodeKind::ListLiteral { ref elements } = doc.bindings[0].value.kind {
        assert_eq!(elements.len(), 3);
    } else {
        panic!("expected ListLiteral");
    }
}

#[test]
fn test_are_binding() {
    let doc = parse("items are 1, 2, 3");
    assert!(doc.bindings[0].is_are);
    if let NodeKind::ListLiteral { ref elements } = doc.bindings[0].value.kind {
        assert_eq!(elements.len(), 3);
    } else {
        panic!("expected ListLiteral from are binding");
    }
}

#[test]
fn test_enum() {
    let doc = parse("e is green from red, green, blue");
    if let NodeKind::FromEnum { ref variants, .. } = doc.bindings[0].value.kind {
        assert_eq!(variants, &["red", "green", "blue"]);
    } else {
        panic!("expected FromEnum");
    }
}

#[test]
fn test_enum_called() {
    let doc = parse("e is green from red, green, blue called RGB");
    assert_eq!(doc.bindings[0].called.as_deref(), Some("RGB"));
}

#[test]
fn test_if_expr() {
    let doc = parse("x is if true then 1 else 0");
    assert!(matches!(doc.bindings[0].value.kind, NodeKind::IfExpr { .. }));
}

#[test]
fn test_case_expr() {
    let doc = parse(
        "x is case 1\n    when 0 then \"zero\"\n    when 1 then \"one\"\n    else \"other\"",
    );
    if let NodeKind::CaseExpr { ref when_clauses, .. } = doc.bindings[0].value.kind {
        assert_eq!(when_clauses.len(), 2);
    } else {
        panic!("expected CaseExpr");
    }
}

#[test]
fn test_member_access() {
    let doc = parse("x is self.config.port");
    if let NodeKind::MemberAccess { ref member, .. } = doc.bindings[0].value.kind {
        assert_eq!(member, "port");
    } else {
        panic!("expected MemberAccess");
    }
}

#[test]
fn test_string_interpolation() {
    let doc = parse(r#"x is "hello {self.name}!""#);
    if let NodeKind::StringLiteral { ref parts } = doc.bindings[0].value.kind {
        assert_eq!(parts.len(), 3);
    } else {
        panic!("expected StringLiteral");
    }
}

#[test]
fn test_or_else() {
    let doc = parse("x is env.PORT to u16 or else 8080");
    assert!(matches!(doc.bindings[0].value.kind, NodeKind::OrElse { .. }));
}

#[test]
fn test_struct_override() {
    let doc = parse("x is self.base with { debug is true }");
    assert!(matches!(
        doc.bindings[0].value.kind,
        NodeKind::StructOverride { .. }
    ));
}

#[test]
fn test_tuple() {
    let doc = parse("t is (1, 2, 3)");
    if let NodeKind::TupleLiteral { ref elements } = doc.bindings[0].value.kind {
        assert_eq!(elements.len(), 3);
    } else {
        panic!("expected TupleLiteral");
    }
}

#[test]
fn test_grouping() {
    let doc = parse("x is (1 + 2)");
    assert!(matches!(
        doc.bindings[0].value.kind,
        NodeKind::Grouping { .. }
    ));
}

#[test]
fn test_empty_tuple() {
    let doc = parse("t is ()");
    if let NodeKind::TupleLiteral { ref elements } = doc.bindings[0].value.kind {
        assert_eq!(elements.len(), 0);
    } else {
        panic!("expected empty TupleLiteral");
    }
}

#[test]
fn test_single_element_tuple() {
    let doc = parse("t is (42,)");
    if let NodeKind::TupleLiteral { ref elements } = doc.bindings[0].value.kind {
        assert_eq!(elements.len(), 1);
    } else {
        panic!("expected 1-element TupleLiteral");
    }
}

#[test]
fn test_type_annotation() {
    let doc = parse("x is 42 as i32");
    assert!(matches!(
        doc.bindings[0].value.kind,
        NodeKind::TypeAnnotation { .. }
    ));
}

#[test]
fn test_conversion() {
    let doc = parse("x is env.PORT to u16");
    assert!(matches!(
        doc.bindings[0].value.kind,
        NodeKind::Conversion { .. }
    ));
}

#[test]
fn test_field_extraction() {
    let doc = parse("port is of self.config");
    assert!(matches!(
        doc.bindings[0].value.kind,
        NodeKind::FieldExtraction { .. }
    ));
}

#[test]
fn test_binding_decomposition_is_not() {
    let doc = parse("x is not true");
    assert!(matches!(
        doc.bindings[0].value.kind,
        NodeKind::UnaryOp {
            op: UnaryOp::Not,
            ..
        }
    ));
}

#[test]
fn test_arithmetic_precedence() {
    let doc = parse("x is 1 + 2 * 3");
    if let NodeKind::BinaryOp {
        op: BinaryOp::Add,
        ref right,
        ..
    } = doc.bindings[0].value.kind
    {
        assert!(matches!(
            right.kind,
            NodeKind::BinaryOp {
                op: BinaryOp::Mul,
                ..
            }
        ));
    } else {
        panic!("expected Add at top");
    }
}

#[test]
fn test_negative_inf_literal() {
    let doc = parse("x is -inf");
    assert!(matches!(
        doc.bindings[0].value.kind,
        NodeKind::FloatLiteral { ref value } if value == "-inf"
    ));
}

#[test]
fn test_struct_import() {
    let doc = parse(r#"s is struct "./shared""#);
    if let NodeKind::StructImport { ref path } = doc.bindings[0].value.kind {
        assert_eq!(path, "./shared");
    } else {
        panic!("expected StructImport");
    }
}

#[test]
fn test_union() {
    let doc = parse("u is 3.14 from union i32, f64, string");
    if let NodeKind::FromUnion { ref types, .. } = doc.bindings[0].value.kind {
        assert_eq!(types.len(), 3);
    } else {
        panic!("expected FromUnion");
    }
}

#[test]
fn test_tagged_union() {
    let doc = parse("t is 7 named ln from n as i32, ln as i128");
    if let NodeKind::NamedVariant {
        ref tag,
        ref variants,
        ..
    } = doc.bindings[0].value.kind
    {
        assert_eq!(tag, "ln");
        assert_eq!(variants.len(), 2);
    } else {
        panic!("expected NamedVariant");
    }
}

#[test]
fn test_enum_termination_in_struct() {
    let doc = parse("s is { color is red from red, green, blue, size is 10 }");
    if let NodeKind::StructLiteral { ref fields } = doc.bindings[0].value.kind {
        assert_eq!(fields.len(), 2);
    } else {
        panic!("expected StructLiteral");
    }
}

#[test]
fn test_expression_continuation_across_newline() {
    let doc = parse("x is 1\n+ 2");
    assert!(matches!(
        doc.bindings[0].value.kind,
        NodeKind::BinaryOp {
            op: BinaryOp::Add,
            ..
        }
    ));
}

#[test]
fn test_multiline_struct() {
    let doc = parse("s is {\n    a is 1\n    b is 2\n}");
    if let NodeKind::StructLiteral { ref fields } = doc.bindings[0].value.kind {
        assert_eq!(fields.len(), 2);
    } else {
        panic!("expected StructLiteral");
    }
}

#[test]
fn test_case_requires_when_clause() {
    let result = try_parse("x is case 1 else 0");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("at least one when"));
}

#[test]
fn test_struct_import_rejects_interpolation() {
    let result = try_parse(r#"s is struct "./path_{x}""#);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("plain string literal"));
}

#[test]
fn test_multiline_string_comment_between_parts_rejected() {
    let result = try_parse("x is \"hello\"\n// comment\n\"world\"");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("comment between multiline string parts"));
}

#[test]
fn test_multiline_string_blank_line_breaks_silently() {
    let doc = parse("x is \"hello\"\n\ny is \"world\"");
    assert_eq!(doc.bindings.len(), 2);
    if let NodeKind::StringLiteral { ref parts } = doc.bindings[0].value.kind {
        assert_eq!(parts.len(), 1);
        assert!(matches!(&parts[0], StringPart::Literal(s) if s == "hello"));
    } else {
        panic!("expected StringLiteral");
    }
}

#[test]
fn test_multiline_string_adjacent_ok() {
    let doc = parse("x is \"hello\"\n\"world\"");
    if let NodeKind::StringLiteral { ref parts } = doc.bindings[0].value.kind {
        assert_eq!(parts.len(), 3);
    } else {
        panic!("expected StringLiteral");
    }
}

#[test]
fn test_are_multiline_no_trailing_comma() {
    let doc = parse("x are\n    1,\n    2,\n    3\ny is 42");
    assert_eq!(doc.bindings.len(), 2);
    if let NodeKind::ListLiteral { ref elements } = doc.bindings[0].value.kind {
        assert_eq!(elements.len(), 3);
    } else {
        panic!("expected ListLiteral");
    }
}

#[test]
fn test_are_trailing_comma_rejected() {
    let result = try_parse("x are\n    1,\n    2,\n    3,\ny is 42");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("trailing comma"));
}

#[test]
fn test_function_expr() {
    let doc = parse("f is function x as i32 returns i32 { x + 1 }");
    assert!(matches!(
        doc.bindings[0].value.kind,
        NodeKind::FunctionExpr { .. }
    ));
}

#[test]
fn test_function_call() {
    let doc = parse("x is f(1, 2)");
    assert!(matches!(
        doc.bindings[0].value.kind,
        NodeKind::FunctionCall { .. }
    ));
}

#[test]
fn test_struct_extends() {
    let doc = parse("x is self.base extends { extra is true }");
    assert!(matches!(
        doc.bindings[0].value.kind,
        NodeKind::StructExtension { .. }
    ));
}

#[test]
fn test_type_expr_list() {
    let doc = parse("x is [1, 2] as [i32]");
    if let NodeKind::TypeAnnotation { ref type_expr, .. } = doc.bindings[0].value.kind {
        assert!(type_expr.is_list);
    } else {
        panic!("expected TypeAnnotation with list type");
    }
}

#[test]
fn test_type_expr_tuple() {
    let doc = parse("x is (1, \"hello\") as (i32, string)");
    if let NodeKind::TypeAnnotation { ref type_expr, .. } = doc.bindings[0].value.kind {
        assert!(type_expr.tuple_types.is_some());
        assert_eq!(type_expr.tuple_types.as_ref().unwrap().len(), 2);
    } else {
        panic!("expected TypeAnnotation with tuple type");
    }
}

#[test]
fn test_power_right_associative() {
    let doc = parse("x is 2 ^ 3 ^ 4");
    // Should parse as 2 ^ (3 ^ 4)
    if let NodeKind::BinaryOp {
        op: BinaryOp::Pow,
        ref right,
        ..
    } = doc.bindings[0].value.kind
    {
        assert!(matches!(
            right.kind,
            NodeKind::BinaryOp {
                op: BinaryOp::Pow,
                ..
            }
        ));
    } else {
        panic!("expected Pow at top");
    }
}

#[test]
fn test_logical_operators() {
    let doc = parse("x is true and false or true");
    // `and` binds tighter than `or`: (true and false) or true
    assert!(matches!(
        doc.bindings[0].value.kind,
        NodeKind::BinaryOp {
            op: BinaryOp::Or,
            ..
        }
    ));
}

#[test]
fn test_membership_in() {
    let doc = parse("x is 3 in [1, 2, 3]");
    assert!(matches!(
        doc.bindings[0].value.kind,
        NodeKind::BinaryOp {
            op: BinaryOp::In,
            ..
        }
    ));
}

#[test]
fn test_concat_operator() {
    let doc = parse("x is [1] ++ [2]");
    assert!(matches!(
        doc.bindings[0].value.kind,
        NodeKind::BinaryOp {
            op: BinaryOp::Concat,
            ..
        }
    ));
}

#[test]
fn test_repeat_operator() {
    let doc = parse("x is [0] ** 3");
    assert!(matches!(
        doc.bindings[0].value.kind,
        NodeKind::BinaryOp {
            op: BinaryOp::Repeat,
            ..
        }
    ));
}

#[test]
fn test_function_with_defaults() {
    let doc = parse("f is function x as i32, y as i32 default 10 returns i32 { x + y }");
    if let NodeKind::FunctionExpr { ref params, .. } = doc.bindings[0].value.kind {
        assert_eq!(params.len(), 2);
        assert!(params[0].default.is_none());
        assert!(params[1].default.is_some());
    } else {
        panic!("expected FunctionExpr");
    }
}

#[test]
fn test_function_required_after_default_rejected() {
    let result = try_parse("f is function x as i32 default 0, y as i32 returns i32 { x + y }");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("cannot appear after"));
}

#[test]
fn test_chained_with_rejected() {
    let result = try_parse("x is self.a with { b is 1 } with { c is 2 }");
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("cannot chain"));
}
