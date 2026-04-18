// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

/// Token types for UZON lexical analysis.
///
/// UZON uses English-like keywords (§1.2) and a small set of operators (§2.6).
/// Grouped by: literals, keywords, composite operators, single-character operators,
/// delimiters, string interpolation, and structural tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenType {
    // Literals (§4)
    Integer,
    Float,
    String,
    True,
    False,
    Null,
    Undefined,
    Inf,
    Nan,

    // Keywords (§2.5)
    Is,
    Are,
    From,
    Called,
    As,
    Named,
    With,
    Union,
    Enum,
    Tagged,
    To,
    Of,
    And,
    Or,
    Not,
    If,
    Then,
    Else,
    Case,
    When,
    Env,
    Struct,
    In,
    Function,
    Returns,
    Default,
    PlusKw,  // `plus` keyword (struct extension), distinct from Plus (`+` operator)
    Type,    // `type` keyword (runtime type check)

    // Composite operators — synthesized from multi-token sequences (§5.2, §5.7)
    OrElse,      // `or else`
    IsNot,       // `is not`
    IsNamed,     // `is named`
    IsNotNamed,  // `is not named`
    IsType,      // `is type`
    IsNotType,   // `is not type`

    // Operators (§2.6)
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    PlusPlus,  // `++` concatenation (§5.8.2)
    StarStar,  // `**` repetition (§5.8.3)
    Lt,
    Le,
    Gt,
    Ge,
    Comma,
    Dot,
    At,        // `@` keyword escape prefix (§2.4)

    // Delimiters
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LParen,
    RParen,

    // String interpolation (§4.4.1)
    InterpStart,  // `"...{`
    InterpEnd,    // `}..."`

    // Structural
    Newline,
    Eof,

    // Identifiers (§2.3)
    Identifier,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub token_type: TokenType,
    pub value: String,
    pub line: usize,
    pub col: usize,
}

impl Token {
    pub fn new(token_type: TokenType, value: impl Into<String>, line: usize, col: usize) -> Self {
        Self {
            token_type,
            value: value.into(),
            line,
            col,
        }
    }
}

/// Returns true if the identifier text is a UZON keyword (§2.5).
pub fn is_keyword(s: &str) -> bool {
    keyword_token_type(s).is_some()
}

/// Maps a keyword string to its TokenType.
///
/// Reserved keyword (`lazy`) is recognized by `is_keyword()` /
/// `is_reserved_keyword()` but has no distinct token type — it is
/// reserved for future use (§2.5).
pub fn keyword_token_type(s: &str) -> Option<TokenType> {
    match s {
        "true" => Some(TokenType::True),
        "false" => Some(TokenType::False),
        "null" => Some(TokenType::Null),
        "undefined" => Some(TokenType::Undefined),
        "inf" => Some(TokenType::Inf),
        "nan" => Some(TokenType::Nan),
        "is" => Some(TokenType::Is),
        "are" => Some(TokenType::Are),
        "from" => Some(TokenType::From),
        "called" => Some(TokenType::Called),
        "as" => Some(TokenType::As),
        "named" => Some(TokenType::Named),
        "with" => Some(TokenType::With),
        "union" => Some(TokenType::Union),
        "enum" => Some(TokenType::Enum),
        "tagged" => Some(TokenType::Tagged),
        "to" => Some(TokenType::To),
        "of" => Some(TokenType::Of),
        "and" => Some(TokenType::And),
        "or" => Some(TokenType::Or),
        "not" => Some(TokenType::Not),
        "if" => Some(TokenType::If),
        "then" => Some(TokenType::Then),
        "else" => Some(TokenType::Else),
        "case" => Some(TokenType::Case),
        "when" => Some(TokenType::When),
        "env" => Some(TokenType::Env),
        "struct" => Some(TokenType::Struct),
        "in" => Some(TokenType::In),
        "function" => Some(TokenType::Function),
        "returns" => Some(TokenType::Returns),
        "default" => Some(TokenType::Default),
        "plus" => Some(TokenType::PlusKw),
        "type" => Some(TokenType::Type),
        // Reserved keywords (§2.5): recognized but no distinct token type.
        "lazy" => None,
        _ => None,
    }
}

/// Characters that act as token boundaries and cannot appear in unquoted identifiers (§2.3).
pub fn is_token_boundary(ch: char) -> bool {
    matches!(
        ch,
        '{' | '}'
            | '[' | ']'
            | '(' | ')'
            | ',' | '.' | '"' | '\'' | '@'
            | '+' | '-' | '*' | '/' | '%' | '^'
            | '<' | '>' | '='
            | '!' | '?' | ':' | ';'
            | '|' | '&' | '$' | '~' | '#' | '\\'
    )
}

/// Returns true if the token type represents a "value" token.
///
/// Used to determine whether a `-` is unary negation or binary subtraction:
/// if the preceding token is a value token, `-` is binary subtraction.
pub fn is_value_token(tt: TokenType) -> bool {
    matches!(
        tt,
        TokenType::Integer
            | TokenType::Float
            | TokenType::String
            | TokenType::Identifier
            | TokenType::True
            | TokenType::False
            | TokenType::Null
            | TokenType::Inf
            | TokenType::Nan
            | TokenType::Undefined
            | TokenType::Env
            | TokenType::RParen
            | TokenType::RBracket
            | TokenType::RBrace
    )
}

/// Returns true if the given string is a reserved keyword for future use (§2.5).
pub fn is_reserved_keyword(s: &str) -> bool {
    matches!(s, "lazy")
}
