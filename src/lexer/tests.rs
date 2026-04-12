// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use super::*;
use crate::token::TokenType;

fn lex(input: &str) -> Vec<Token> {
    Lexer::new(input).tokenize().unwrap().0
}

fn types(input: &str) -> Vec<TokenType> {
    lex(input)
        .into_iter()
        .filter(|t| t.token_type != TokenType::Newline && t.token_type != TokenType::Eof)
        .map(|t| t.token_type)
        .collect()
}

#[test]
fn test_simple_binding() {
    let toks = types("x is 42");
    assert_eq!(
        toks,
        vec![TokenType::Identifier, TokenType::Is, TokenType::Integer]
    );
}

#[test]
fn test_string_literal() {
    let tokens = lex(r#""hello, world""#);
    assert_eq!(tokens[0].token_type, TokenType::String);
    assert_eq!(tokens[0].value, "hello, world");
}

#[test]
fn test_string_interpolation() {
    let toks = types(r#""hello {name}!""#);
    assert_eq!(
        toks,
        vec![
            TokenType::String,
            TokenType::InterpStart,
            TokenType::Identifier,
            TokenType::InterpEnd,
            TokenType::String,
        ]
    );
}

#[test]
fn test_composite_operators() {
    assert_eq!(types("or else"), vec![TokenType::OrElse]);
    assert_eq!(types("is not"), vec![TokenType::IsNot]);
    assert_eq!(types("is named"), vec![TokenType::IsNamed]);
    assert_eq!(types("is not named"), vec![TokenType::IsNotNamed]);
}

#[test]
fn test_number_bases() {
    let toks = lex("0xff 0o77 0b1010 42 3.14 1_000");
    let vals: Vec<_> = toks
        .iter()
        .filter(|t| t.token_type != TokenType::Newline && t.token_type != TokenType::Eof)
        .map(|t| (t.token_type, t.value.as_str()))
        .collect();
    assert_eq!(
        vals,
        vec![
            (TokenType::Integer, "0xff"),
            (TokenType::Integer, "0o77"),
            (TokenType::Integer, "0b1010"),
            (TokenType::Integer, "42"),
            (TokenType::Float, "3.14"),
            (TokenType::Integer, "1_000"),
        ]
    );
}

#[test]
fn test_negative_number() {
    let toks = types("x is -7");
    assert_eq!(
        toks,
        vec![TokenType::Identifier, TokenType::Is, TokenType::Integer]
    );
    let all = lex("x is -7");
    assert_eq!(all[2].value, "-7");
}

#[test]
fn test_minus_as_operator() {
    let toks = types("10 - 3");
    assert_eq!(
        toks,
        vec![TokenType::Integer, TokenType::Minus, TokenType::Integer]
    );
}

#[test]
fn test_keyword_escape() {
    let toks = lex("@is is 3");
    assert_eq!(toks[0].token_type, TokenType::Identifier);
    assert_eq!(toks[0].value, "is");
    assert_eq!(toks[1].token_type, TokenType::Is);
}

#[test]
fn test_quoted_identifier() {
    let toks = lex("'Content-Type' is \"application/json\"");
    assert_eq!(toks[0].token_type, TokenType::Identifier);
    assert_eq!(toks[0].value, "Content-Type");
}

#[test]
fn test_identifier_starting_with_digit() {
    let toks = types("1st is \"first\"");
    assert_eq!(
        toks,
        vec![TokenType::Identifier, TokenType::Is, TokenType::String]
    );
}

#[test]
fn test_escape_sequences() {
    let tokens = lex(r#""\n\t\\\"\0\{""#);
    assert_eq!(tokens[0].value, "\n\t\\\"\0{");
}

#[test]
fn test_unicode_escape() {
    let tokens = lex(r#""\u{1F600}""#);
    assert_eq!(tokens[0].value, "\u{1F600}");
}

#[test]
fn test_hex_escape() {
    let tokens = lex(r#""\x41""#);
    assert_eq!(tokens[0].value, "A");
}

#[test]
fn test_comments() {
    let toks = types("x is 42 // this is a comment\ny is 1");
    assert_eq!(
        toks,
        vec![
            TokenType::Identifier,
            TokenType::Is,
            TokenType::Integer,
            TokenType::Identifier,
            TokenType::Is,
            TokenType::Integer,
        ]
    );
}

#[test]
fn test_struct_literal() {
    let toks = types("{ a is 1, b is 2 }");
    assert_eq!(
        toks,
        vec![
            TokenType::LBrace,
            TokenType::Identifier,
            TokenType::Is,
            TokenType::Integer,
            TokenType::Comma,
            TokenType::Identifier,
            TokenType::Is,
            TokenType::Integer,
            TokenType::RBrace,
        ]
    );
}

#[test]
fn test_inf_nan() {
    let toks = types("inf -inf nan");
    assert_eq!(
        toks,
        vec![TokenType::Inf, TokenType::Minus, TokenType::Inf, TokenType::Nan]
    );

    let toks2 = lex("x is -inf");
    assert_eq!(toks2[2].token_type, TokenType::Float);
    assert_eq!(toks2[2].value, "-inf");

    let toks3 = lex("x is -nan");
    assert_eq!(toks3[2].token_type, TokenType::Float);
    assert_eq!(toks3[2].value, "-nan");

    let toks4 = lex("x is - inf");
    assert_eq!(toks4[2].token_type, TokenType::Minus);
    assert_eq!(toks4[3].token_type, TokenType::Inf);

    // "-info" should NOT be lexed as -inf + o
    let toks5 = types("-info");
    assert_eq!(toks5, vec![TokenType::Minus, TokenType::Identifier]);
    let all5 = lex("-info");
    assert_eq!(all5[1].value, "info");
}

#[test]
fn test_trailing_underscore_rejected() {
    let result = Lexer::new("42_").tokenize();
    assert!(result.is_err());
}

#[test]
fn test_leading_underscore_after_prefix_rejected() {
    let toks = types("0x_FF");
    assert!(!toks.contains(&TokenType::Integer));
}

#[test]
fn test_comment_lines_tracked() {
    let (_, comment_lines) = Lexer::new("x is 1 // comment\ny is 2").tokenize().unwrap();
    assert_eq!(comment_lines, vec![1]);
}

#[test]
fn test_reserved_keyword_rejected() {
    // `lazy` is the only remaining reserved keyword (§2.5)
    let result = Lexer::new("lazy is 1").tokenize();
    assert!(result.is_err());
}

#[test]
fn test_reserved_keyword_escaped() {
    let toks = lex("@type is 1");
    assert_eq!(toks[0].token_type, TokenType::Identifier);
    assert_eq!(toks[0].value, "type");
}

#[test]
fn test_bom_skipped() {
    let toks = types("\u{FEFF}x is 1");
    assert_eq!(
        toks,
        vec![TokenType::Identifier, TokenType::Is, TokenType::Integer]
    );
}

#[test]
fn test_scientific_notation() {
    let toks = lex("1e10 2.5E-3");
    let vals: Vec<_> = toks
        .iter()
        .filter(|t| t.token_type == TokenType::Float)
        .map(|t| t.value.as_str())
        .collect();
    assert_eq!(vals, vec!["1e10", "2.5E-3"]);
}

#[test]
fn test_empty_quoted_identifier() {
    let toks = lex("'' is \"empty\"");
    assert_eq!(toks[0].token_type, TokenType::Identifier);
    assert_eq!(toks[0].value, "");
}

#[test]
fn test_all_delimiters() {
    let toks = types("{ } [ ] ( )");
    assert_eq!(
        toks,
        vec![
            TokenType::LBrace,
            TokenType::RBrace,
            TokenType::LBracket,
            TokenType::RBracket,
            TokenType::LParen,
            TokenType::RParen,
        ]
    );
}

#[test]
fn test_all_operators() {
    let toks = types("+ * / % ^ ++ **");
    assert_eq!(
        toks,
        vec![
            TokenType::Plus,
            TokenType::Star,
            TokenType::Slash,
            TokenType::Percent,
            TokenType::Caret,
            TokenType::PlusPlus,
            TokenType::StarStar,
        ]
    );
}

#[test]
fn test_comparison_operators() {
    let toks = types("< <= > >=");
    assert_eq!(
        toks,
        vec![TokenType::Lt, TokenType::Le, TokenType::Gt, TokenType::Ge]
    );
}
