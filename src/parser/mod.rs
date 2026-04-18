// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

/// UZON Parser — recursive descent parser with precedence climbing.
///
/// Parses a token stream into an AST (§9). The parser implements the full UZON
/// expression grammar with 18 precedence levels, composite keyword decomposition,
/// multiline string joining (§4.4.2), and the NEWLINE_SEP separator rule (§8).
///
/// Expression precedence levels (highest to lowest):
///   1. Member access (`.`) and function call `()`
///   2. Conversion (`to`)
///   3. Struct override (`with`) / extension (`plus`)
///   4. Type annotation (`as`)
///   5. Type declaration (`from`, `named`)
///   7. Power (`^`, right-associative)
///   8. Unary negation (`-`)
///   9. Multiplication (`*`, `/`, `%`, `**`)
///  10. Addition (`+`, `-`)
///  11. Concatenation (`++`)
///  12. Relational (`<`, `<=`, `>`, `>=`)
///  13. Membership (`in`)
///  14. Equality (`is`, `is not`, `is named`, `is not named`)
///  15. Logical NOT (`not`)
///  16. Logical AND (`and`)
///  17. Logical OR (`or`)
///  18. Undefined coalescing (`or else`)
use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::token::{Token, TokenType, is_keyword};

mod expressions;
mod type_decl;
mod postfix;
mod literals;
mod control;
mod type_expr;
mod standalone_type;

#[cfg(test)]
mod tests;

pub struct Parser {
    pub(crate) tokens: Vec<Token>,
    pub(crate) pos: usize,
    /// When true, `parse_type_annotation` skips `as` — used by `are` bindings (§3.4)
    /// so trailing `as` can be captured as a list-level annotation.
    pub(crate) suppress_as: bool,
    /// When true, `parse_string_literal` does not join adjacent strings across
    /// newlines. Used when parsing function body bindings (§3.8) so the body
    /// expression (which may start with a string) is not consumed as a continuation.
    pub(crate) suppress_multiline_string: bool,
    /// Lines on which `//` comments appeared (from lexer), for §4.4.2 rejection.
    pub(crate) comment_lines: Vec<usize>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, comment_lines: Vec<usize>) -> Self {
        Self {
            tokens,
            pos: 0,
            suppress_as: false,
            suppress_multiline_string: false,
            comment_lines,
        }
    }

    pub fn parse(&mut self) -> Result<Document> {
        let span = self.current_span();
        let bindings = self.parse_bindings(TokenType::Eof)?;
        Ok(Document { bindings, span })
    }

    // === Token helpers ===

    pub(crate) fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    pub(crate) fn peek_type(&self) -> TokenType {
        self.peek().token_type
    }

    pub(crate) fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos.min(self.tokens.len() - 1)];
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    pub(crate) fn expect(&mut self, tt: TokenType) -> Result<Token> {
        let tok = self.peek().clone();
        if tok.token_type == tt {
            self.advance();
            Ok(tok)
        } else {
            Err(UzonError::syntax(
                format!("expected {:?}, found {:?}", tt, tok.token_type),
                tok.line,
                tok.col,
            ))
        }
    }

    pub(crate) fn at(&self, tt: TokenType) -> bool {
        self.peek_type() == tt
    }

    pub(crate) fn eat(&mut self, tt: TokenType) -> bool {
        if self.at(tt) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Line number of the token immediately before the current position.
    /// Used by `parse_call_or_access` to enforce the same-line rule (§5.15).
    pub(crate) fn prev_line(&self) -> usize {
        if self.pos > 0 {
            self.tokens[self.pos - 1].line
        } else {
            0
        }
    }

    pub(crate) fn current_span(&self) -> Span {
        let tok = self.peek();
        Span {
            line: tok.line,
            col: tok.col,
        }
    }

    pub(crate) fn skip_newlines(&mut self) {
        while self.at(TokenType::Newline) {
            self.advance();
        }
    }

    /// Look past newlines without consuming them.
    fn peek_past_newlines_is(&self, tt: TokenType) -> bool {
        let mut i = self.pos;
        while i < self.tokens.len() && self.tokens[i].token_type == TokenType::Newline {
            i += 1;
        }
        i < self.tokens.len() && self.tokens[i].token_type == tt
    }

    /// Check if position `pos` starts a new binding: identifier followed by
    /// `is`, `are`, or a composite `is …` token.
    ///
    /// Uses the 2-token lookahead described in §8 to distinguish binding starts
    /// from expression continuations across newlines.  Composite tokens
    /// (`IsNot`, `IsNamed`, …) are included because binding decomposition (§9)
    /// splits them into `is` + the remainder.
    pub(crate) fn is_binding_start_at(&self, pos: usize) -> bool {
        self.is_binding_keyword_at(pos, true)
    }

    /// Like `is_binding_start_at` but only matches `is` (and its composites),
    /// not `are`.  Per §9 the `func_binding` production is `name "is" expression`.
    pub(crate) fn is_func_binding_start_at(&self, pos: usize) -> bool {
        self.is_binding_keyword_at(pos, false)
    }

    fn is_binding_keyword_at(&self, pos: usize, allow_are: bool) -> bool {
        let tok = &self.tokens[pos.min(self.tokens.len() - 1)];

        // Must be an identifier
        if !matches!(tok.token_type, TokenType::Identifier) {
            return false;
        }

        // Look for `is`/`are` after the identifier, skipping newlines.
        // Composite tokens (IsNot, IsNamed, etc.) are NOT included here —
        // they are handled by `parse_binding` decomposition. Including them
        // would cause `r is named error` expressions to be wrongly detected
        // as binding starts.
        let mut next = pos + 1;
        while next < self.tokens.len() && self.tokens[next].token_type == TokenType::Newline {
            next += 1;
        }
        if next >= self.tokens.len() {
            return false;
        }

        self.tokens[next].token_type == TokenType::Is
            || (allow_are && self.tokens[next].token_type == TokenType::Are)
    }

    /// Check if the current position is a trailing comma position:
    /// the next token is a terminator or starts a new binding.
    pub(crate) fn is_trailing_comma_position(&self) -> bool {
        if self.at(TokenType::Eof)
            || self.at(TokenType::RBrace)
            || self.at(TokenType::RBracket)
            || self.at(TokenType::RParen)
            || self.at(TokenType::Called)
        {
            return true;
        }
        let mut look = self.pos;
        while look < self.tokens.len() && self.tokens[look].token_type == TokenType::Newline {
            look += 1;
        }
        self.is_binding_start_at(look)
    }

    /// Skip newlines that act as separators, applying the NEWLINE_SEP rule (§8).
    ///
    /// Returns true if a separator (newline or comma) was consumed.
    /// Newlines count as separators when followed by a closing brace, EOF,
    /// or the start of a new binding.
    pub(crate) fn skip_separator(&mut self) -> bool {
        let mut found = false;
        loop {
            if self.at(TokenType::Comma) {
                self.advance();
                found = true;
                self.skip_newlines();
                continue;
            }
            if self.at(TokenType::Newline) {
                let mut look = self.pos + 1;
                while look < self.tokens.len()
                    && self.tokens[look].token_type == TokenType::Newline
                {
                    look += 1;
                }
                if look >= self.tokens.len()
                    || self.tokens[look].token_type == TokenType::Eof
                {
                    self.skip_newlines();
                    found = true;
                    continue;
                }
                // Closing brace = separator
                if self.tokens[look].token_type == TokenType::RBrace {
                    self.skip_newlines();
                    found = true;
                    continue;
                }
                // New binding = separator
                if self.is_binding_start_at(look) {
                    self.skip_newlines();
                    found = true;
                    continue;
                }
                // Otherwise newline is whitespace (expression continuation)
                self.skip_newlines();
                continue;
            }
            break;
        }
        found
    }

    // === Bindings (§5.1) ===

    pub(crate) fn parse_bindings(&mut self, until: TokenType) -> Result<Vec<Binding>> {
        let mut bindings = Vec::new();
        self.skip_newlines();

        while !self.at(until) && !self.at(TokenType::Eof) {
            let binding = self.parse_binding()?;
            bindings.push(binding);
            self.skip_separator();
        }

        Ok(bindings)
    }

    pub(crate) fn parse_binding(&mut self) -> Result<Binding> {
        // §11.2: suggest @escape when a keyword appears at binding-name position
        let tok = self.peek();
        if tok.token_type != TokenType::Identifier && is_keyword(&tok.value) {
            let kw = tok.value.clone();
            return Err(UzonError::syntax(
                format!(
                    "'{kw}' is a keyword and cannot be used as a binding name; \
                     use @{kw} to escape it"
                ),
                tok.line,
                tok.col,
            ));
        }
        let name_tok = self.expect(TokenType::Identifier)?;
        let name = name_tok.value.clone();
        let span = Span {
            line: name_tok.line,
            col: name_tok.col,
        };

        self.skip_newlines();

        // `are` binding (§3.4) — list sugar
        if self.at(TokenType::Are) {
            self.advance();
            self.skip_newlines();
            return self.parse_are_binding(name, span);
        }

        // `is` binding — handle composite token decomposition (§9, binding decomposition)
        let tok = self.peek().clone();
        match tok.token_type {
            TokenType::Is => {
                self.advance();
                self.skip_newlines();

                // v0.9 standalone type declarations (§6.2):
                //   X is enum v1, v2, ...
                //   X is union T1, T2, ...
                //   X is tagged union v1 as T1, v2 as T2, ...
                //   X is struct { ... }
                // The binding name becomes the type name. `called` is forbidden.
                match self.peek_type() {
                    TokenType::Enum => {
                        return self.parse_standalone_enum(name, span);
                    }
                    TokenType::Union => {
                        return self.parse_standalone_union(name, span);
                    }
                    TokenType::Tagged => {
                        return self.parse_standalone_tagged_union(name, span);
                    }
                    TokenType::Struct => {
                        // `struct {` = standalone struct type decl; `struct "..."` = import
                        let mut look = self.pos + 1;
                        while look < self.tokens.len()
                            && self.tokens[look].token_type == TokenType::Newline
                        {
                            look += 1;
                        }
                        if look < self.tokens.len()
                            && self.tokens[look].token_type == TokenType::LBrace
                        {
                            return self.parse_standalone_struct(name, span);
                        }
                    }
                    _ => {}
                }
            }
            // Binding decomposition: "x is not expr" -> x = (not expr)
            TokenType::IsNot => {
                self.advance();
                self.skip_newlines();
                let not_node = self.parse_not_expr(tok.line, tok.col)?;
                let called = self.try_parse_called()?;
                return Ok(Binding {
                    name,
                    value: not_node,
                    called,
                    is_are: false,
                    list_type_annotation: None,
                    standalone_type_kind: None,
                    span,
                });
            }
            // §9 binding decomposition: "x is named ..." → "named" becomes an identifier,
            // then continue parsing from type-declaration level (e.g., `named from v1, v2`)
            TokenType::IsNamed => {
                self.advance();
                self.skip_newlines();
                let ident = Node::new(
                    NodeKind::Identifier { name: "named".to_string() },
                    tok.line,
                    tok.col,
                );
                let value = self.continue_from_type_decl(ident)?;
                let called = self.try_parse_called()?;
                return Ok(Binding {
                    name,
                    value,
                    called,
                    is_are: false,
                    list_type_annotation: None,
                    standalone_type_kind: None,
                    span,
                });
            }
            TokenType::IsNotNamed => {
                self.advance();
                return Err(UzonError::syntax(
                    "'is not named' cannot appear at the start of a binding value",
                    tok.line,
                    tok.col,
                ));
            }
            // §9 binding decomposition: "x is type ..." → "type" becomes an identifier
            TokenType::IsType => {
                self.advance();
                self.skip_newlines();
                let ident = Node::new(
                    NodeKind::Identifier { name: "type".to_string() },
                    tok.line,
                    tok.col,
                );
                let value = self.continue_from_type_decl(ident)?;
                let called = self.try_parse_called()?;
                return Ok(Binding {
                    name,
                    value,
                    called,
                    is_are: false,
                    list_type_annotation: None,
                    standalone_type_kind: None,
                    span,
                });
            }
            TokenType::IsNotType => {
                self.advance();
                return Err(UzonError::syntax(
                    "'is not type' cannot appear at the start of a binding value",
                    tok.line,
                    tok.col,
                ));
            }
            _ => {
                return Err(UzonError::syntax(
                    format!(
                        "expected 'is' or 'are' after binding name, found {:?}",
                        tok.token_type
                    ),
                    tok.line,
                    tok.col,
                ));
            }
        }

        // Check for `is of` — field extraction (§5.14)
        if self.at(TokenType::Of) {
            self.advance();
            self.skip_newlines();
            let source = self.parse_member_access()?;
            return Ok(Binding {
                name,
                value: Node::new(
                    NodeKind::FieldExtraction {
                        source: Box::new(source),
                    },
                    span.line,
                    span.col,
                ),
                called: None,
                is_are: false,
                list_type_annotation: None,
                standalone_type_kind: None,
                span,
            });
        }

        let value = self.parse_expression()?;
        let called = self.try_parse_called()?;

        Ok(Binding {
            name,
            value,
            called,
            is_are: false,
            list_type_annotation: None,
            standalone_type_kind: None,
            span,
        })
    }

    /// Parse an `are` binding: `name are elem1, elem2, ... [as [Type]]` (§3.4).
    ///
    /// Suppresses element-level `as` so the trailing `as` is captured as the
    /// list-level type annotation. `as` still works inside parens/brackets.
    fn parse_are_binding(&mut self, name: String, span: Span) -> Result<Binding> {
        // §3.4.1: `as` disambiguation — trailing `as` at the end of an `are` binding
        // is list-level type annotation, not element-level. But `as` WITHIN non-last
        // elements must be allowed (e.g., `"x" as ApiResponse named loading, ...`).
        // Strategy: parse all elements with `as` enabled. After the loop, if there's
        // a remaining `as` token, it's the list-level annotation. If not, check if the
        // last element is a bare TypeAnnotation and lift it.
        let mut elements = Vec::new();
        // For the first element, save position in case we need to re-parse as last element
        let first_pos = self.pos;
        elements.push(self.parse_expression()?);

        loop {
            self.skip_newlines();
            if !self.at(TokenType::Comma) {
                break;
            }

            self.advance(); // consume comma
            self.skip_newlines();

            // Trailing comma not permitted in `are` bindings (§E.2)
            if self.is_trailing_comma_position() {
                let tok = self.peek();
                return Err(UzonError::syntax(
                    "trailing comma is not permitted in 'are' bindings".to_string(),
                    tok.line,
                    tok.col,
                ));
            }

            elements.push(self.parse_expression()?);
        }

        // List-level `as` annotation: if the trailing `as` was consumed by the last
        // element as TypeAnnotation (not wrapped by named/from), re-parse with suppression.
        let list_type_annotation = if self.peek_past_newlines_is(TokenType::As) {
            // Explicit trailing `as` after all elements
            self.skip_newlines();
            self.advance(); // consume `as`
            self.skip_newlines();
            Some(self.parse_type_expr()?)
        } else if elements.len() == 1 {
            // Single element: if it's a bare TypeAnnotation, lift it
            if let NodeKind::TypeAnnotation { .. } = &elements[0].kind {
                // Re-parse with suppress_as to separate element from list type
                self.pos = first_pos;
                elements.clear();
                self.suppress_as = true;
                elements.push(self.parse_expression()?);
                self.suppress_as = false;
                self.skip_newlines();
                if self.at(TokenType::As) {
                    self.advance();
                    self.skip_newlines();
                    Some(self.parse_type_expr()?)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            // Multiple elements: check if last is a bare TypeAnnotation
            let last = elements.last().unwrap();
            if let NodeKind::TypeAnnotation { expr, type_expr } = &last.kind {
                // Lift: unwrap the TypeAnnotation from the last element
                let inner = (**expr).clone();
                let ty = type_expr.clone();
                *elements.last_mut().unwrap() = inner;
                Some(ty)
            } else {
                None
            }
        };

        let called = self.try_parse_called()?;

        Ok(Binding {
            name,
            value: Node::new(
                NodeKind::ListLiteral { elements },
                span.line,
                span.col,
            ),
            called,
            is_are: true,
            list_type_annotation,
            standalone_type_kind: None,
            span,
        })
    }

    /// Try to parse `called TypeName` after a binding's value (§6.2).
    pub(crate) fn try_parse_called(&mut self) -> Result<Option<String>> {
        self.skip_newlines();
        if self.at(TokenType::Called) {
            self.advance();
            self.skip_newlines();
            let name_tok = self.expect(TokenType::Identifier)?;
            Ok(Some(name_tok.value))
        } else {
            Ok(None)
        }
    }
}
