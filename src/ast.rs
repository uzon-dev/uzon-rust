// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

/// Source location for AST nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

/// A type expression (after `as`, `to`, `from union`, etc.) (§6).
///
/// Type expressions can be simple names (`i32`), paths (`Config.Port`),
/// list types (`[i32]`), tuple types (`(i32, string)`), or nullable (`null`).
#[derive(Debug, Clone, PartialEq)]
pub struct TypeExpr {
    pub path: Vec<String>,
    pub is_list: bool,
    pub inner: Option<Box<TypeExpr>>,
    pub is_null: bool,
    pub tuple_types: Option<Vec<TypeExpr>>,
    pub span: Span,
}

/// A function parameter: `name as Type [default expr]` (§3.8).
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionParam {
    pub name: String,
    pub type_expr: TypeExpr,
    pub default: Option<Box<Node>>,
    pub span: Span,
}

/// The mode of a `case` expression (§5.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseMode {
    Value,  // `case expr` — value matching
    Type,   // `case type expr` — type dispatch (untagged unions)
    Named,  // `case named expr` — variant dispatch (tagged unions)
}

/// A `when` clause in a `case` expression (§5.10).
#[derive(Debug, Clone, PartialEq)]
pub struct WhenClause {
    pub value: Node,
    pub result: Node,
    pub span: Span,
}

/// An AST node with span information.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub kind: NodeKind,
    pub span: Span,
}

impl Node {
    pub fn new(kind: NodeKind, line: usize, col: usize) -> Self {
        Self {
            kind,
            span: Span { line, col },
        }
    }
}

/// All AST node variants.
///
/// Organized by category: literals (§4), references (§5.12–5.13),
/// expressions (§5), type system (§6), compounds (§3), functions (§3.8),
/// and import/special constructs (§7).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    // === Literals (§4) ===
    IntegerLiteral {
        value: String,
    },
    FloatLiteral {
        value: String,
    },
    StringLiteral {
        parts: Vec<StringPart>,
    },
    BoolLiteral {
        value: bool,
    },
    NullLiteral,
    UndefinedLiteral,
    InfLiteral {
        negative: bool,
    },
    NanLiteral,

    // === References ===
    /// A bare identifier reference (§2.3).
    Identifier {
        name: String,
    },
    /// `env` keyword for environment variable access (§5.13).
    EnvRef,

    // === Expressions (§5) ===
    /// Dot-separated member access: `obj.field` (§5.12).
    MemberAccess {
        object: Box<Node>,
        member: String,
    },
    BinaryOp {
        op: BinaryOp,
        left: Box<Node>,
        right: Box<Node>,
    },
    UnaryOp {
        op: UnaryOp,
        operand: Box<Node>,
    },
    /// `expr or else fallback` — undefined coalescing (§5.7).
    OrElse {
        left: Box<Node>,
        right: Box<Node>,
    },
    /// `if cond then a else b` — conditional expression (§5.9).
    IfExpr {
        condition: Box<Node>,
        then_branch: Box<Node>,
        else_branch: Box<Node>,
    },
    /// `case [type|named] expr when v1 then r1 ... else default` — multi-branch match (§5.10).
    CaseExpr {
        mode: CaseMode,
        scrutinee: Box<Node>,
        when_clauses: Vec<WhenClause>,
        else_branch: Box<Node>,
    },

    // === Type system (§6) ===
    /// `expr as Type` — type annotation / assertion (§6.1).
    TypeAnnotation {
        expr: Box<Node>,
        type_expr: TypeExpr,
    },
    /// `expr to Type` — type conversion (§5.11).
    Conversion {
        expr: Box<Node>,
        type_expr: TypeExpr,
    },
    /// `value from v1, v2, ...` — enum definition (§3.5).
    FromEnum {
        value: Box<Node>,
        variants: Vec<String>,
    },
    /// `value from union T1, T2, ...` — union definition (§3.6).
    FromUnion {
        value: Box<Node>,
        types: Vec<TypeExpr>,
    },
    /// `value named tag from tag1 as T1, ...` — tagged union (§3.7).
    NamedVariant {
        value: Box<Node>,
        tag: String,
        variants: Vec<(String, TypeExpr)>,
    },
    /// `variant_name inner` — tagged union variant shorthand (§3.7 v0.10).
    /// The target tagged union type is determined by context (via `as Type`,
    /// struct field type, function parameter/return type). The evaluator
    /// resolves the shorthand against the context type; if no type context is
    /// available the shorthand is a type error.
    VariantShorthand {
        variant_name: String,
        inner: Box<Node>,
    },

    // === Compounds (§3) ===
    /// `{ field is value, ... }` — struct literal (§3.2).
    StructLiteral {
        fields: Vec<Binding>,
    },
    /// `[elem, ...]` — list literal (§3.4).
    ListLiteral {
        elements: Vec<Node>,
    },
    /// `(elem, ...)` — tuple literal (§3.3).
    TupleLiteral {
        elements: Vec<Node>,
    },
    /// Parenthesized grouping for precedence.
    Grouping {
        expr: Box<Node>,
    },
    /// `base with { overrides }` — struct override (§3.2.1).
    StructOverride {
        base: Box<Node>,
        overrides: Box<Node>,
    },
    /// `base plus { extension }` — struct extension (§3.2.2).
    StructExtension {
        base: Box<Node>,
        extension: Box<Node>,
    },

    // === Functions (§3.8) ===
    /// `function (params) returns Type { body }` — function expression.
    FunctionExpr {
        params: Vec<FunctionParam>,
        return_type: TypeExpr,
        body_bindings: Vec<Binding>,
        body_expr: Box<Node>,
    },
    /// `callee(args)` — function call (§5.15).
    FunctionCall {
        callee: Box<Node>,
        args: Vec<Node>,
    },

    // === Import & special ===
    /// `struct "path"` — file import (§7).
    StructImport {
        path: String,
    },
    /// `x is of source` — field extraction using binding name (§5.14).
    FieldExtraction {
        source: Box<Node>,
    },
    /// The default value for a type expression (§3.6 default value table).
    ///
    /// Used as the implicit value in standalone `union`/`tagged union` declarations:
    /// `Flexible is union i32, string` produces `0 as i32` (the default of the
    /// first member type). Resolved at evaluation time because it may reference
    /// named types in scope.
    DefaultForType {
        type_expr: TypeExpr,
    },
}

/// Part of a string literal — plain text or interpolation (§4.4.1).
#[derive(Debug, Clone, PartialEq)]
pub enum StringPart {
    Literal(String),
    Interpolation(Node),
}

/// Binary operators (§5.3–5.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,         // `+`
    Sub,         // `-`
    Mul,         // `*`
    Div,         // `/`
    Mod,         // `%`
    Pow,         // `^` (§5.3)
    Concat,      // `++` (§5.8.2)
    Repeat,      // `**` (§5.8.3)
    Lt,          // `<`
    Le,          // `<=`
    Gt,          // `>`
    Ge,          // `>=`
    And,         // `and` (§5.6)
    Or,          // `or` (§5.6)
    Is,          // `is` equality (§5.2)
    IsNot,       // `is not` inequality (§5.2)
    IsNamed,     // `is named` tagged union variant check (§3.7.2)
    IsNotNamed,  // `is not named` (§3.7.2)
    IsType,      // `is type` runtime type check (§3.6)
    IsNotType,   // `is not type` (§3.6)
    In,          // `in` membership test (§5.8.1)
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,  // `-` (§5.3)
    Not,  // `not` (§5.6)
}

/// A binding: `name is expr [called TypeName]` (§5.1).
///
/// - `is_are`: true if the binding uses `are` (list sugar, §3.4.1).
/// - `list_type_annotation`: the trailing `as [Type]` in an `are` binding.
/// - `standalone_type_kind`: Some(kind) if the binding is a v0.9 standalone type
///   declaration (`X is enum ...`, `X is union ...`, `X is tagged union ...`,
///   `X is struct { ... }`). Drives roundtrip stringification (§6.2).
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub name: String,
    pub value: Node,
    pub called: Option<String>,
    pub is_are: bool,
    pub list_type_annotation: Option<TypeExpr>,
    pub standalone_type_kind: Option<StandaloneTypeKind>,
    pub span: Span,
}

/// Kind of v0.9 standalone type declaration (§6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandaloneTypeKind {
    Enum,
    Union,
    TaggedUnion,
    Struct,
}

/// The root of a parsed UZON document (§1).
///
/// A UZON file is an anonymous struct: a sequence of bindings at the top level.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub bindings: Vec<Binding>,
    pub span: Span,
}
