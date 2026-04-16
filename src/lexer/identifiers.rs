// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::error::{Result, UzonError};
use crate::token::{
    Token, TokenType, is_keyword, is_reserved_keyword, is_token_boundary, keyword_token_type,
};

use super::Lexer;

impl Lexer {
    /// Lex an identifier or keyword (§2.3).
    ///
    /// Composite keywords (`or else`, `is not`, `is named`, `is not named`) are
    /// synthesized by lookahead after recognizing the first word.
    pub(crate) fn lex_identifier_or_keyword(&mut self) -> Result<()> {
        let start_line = self.line;
        let start_col = self.col;
        let mut ident = String::new();

        while let Some(ch) = self.peek() {
            if ch.is_whitespace() || is_token_boundary(ch) {
                break;
            }
            ident.push(ch);
            self.advance();
        }

        if ident.is_empty() {
            let ch = self.peek().unwrap_or('\0');
            self.advance();
            return Err(UzonError::syntax(
                format!("unexpected character: '{ch}'"),
                start_line,
                start_col,
            ));
        }

        // Composite keyword: `or else` (§5.7)
        if ident == "or" {
            if self.try_consume_keyword("else") {
                self.tokens.push(Token::new(
                    TokenType::OrElse,
                    "or else",
                    start_line,
                    start_col,
                ));
                return Ok(());
            }
            self.tokens
                .push(Token::new(TokenType::Or, "or", start_line, start_col));
            return Ok(());
        }

        // Composite keywords: `is not [named]`, `is named` (§5.2, §3.7.2)
        if ident == "is" {
            if self.try_consume_keyword("not") {
                if self.try_consume_keyword("named") {
                    self.tokens.push(Token::new(
                        TokenType::IsNotNamed,
                        "is not named",
                        start_line,
                        start_col,
                    ));
                } else if self.try_consume_keyword("type") {
                    self.tokens.push(Token::new(
                        TokenType::IsNotType,
                        "is not type",
                        start_line,
                        start_col,
                    ));
                } else {
                    self.tokens.push(Token::new(
                        TokenType::IsNot,
                        "is not",
                        start_line,
                        start_col,
                    ));
                }
                return Ok(());
            }
            if self.try_consume_keyword("named") {
                self.tokens.push(Token::new(
                    TokenType::IsNamed,
                    "is named",
                    start_line,
                    start_col,
                ));
                return Ok(());
            }
            if self.try_consume_keyword("type") {
                self.tokens.push(Token::new(
                    TokenType::IsType,
                    "is type",
                    start_line,
                    start_col,
                ));
                return Ok(());
            }
            self.tokens
                .push(Token::new(TokenType::Is, "is", start_line, start_col));
            return Ok(());
        }

        // Reserved keyword check (§2.5)
        if is_reserved_keyword(&ident) {
            return Err(UzonError::syntax(
                format!(
                    "reserved keyword '{ident}' cannot be used as an identifier; use @{ident} to escape"
                ),
                start_line,
                start_col,
            ));
        }

        // Normal keyword or identifier
        if let Some(tt) = keyword_token_type(&ident) {
            self.tokens
                .push(Token::new(tt, &ident, start_line, start_col));
        } else {
            self.tokens
                .push(Token::new(TokenType::Identifier, &ident, start_line, start_col));
        }

        Ok(())
    }

    /// Try to consume a keyword following the current position (skipping whitespace/comments).
    /// Used for composite keyword lookahead. Restores position on failure.
    fn try_consume_keyword(&mut self, kw: &str) -> bool {
        let saved_pos = self.pos;
        let saved_line = self.line;
        let saved_col = self.col;

        self.skip_whitespace_and_comments();

        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() || is_token_boundary(ch) {
                break;
            }
            self.advance();
        }

        let word: String = self.source[start..self.pos].iter().collect();
        if word == kw {
            return true;
        }

        self.pos = saved_pos;
        self.line = saved_line;
        self.col = saved_col;
        false
    }

    fn skip_whitespace_and_comments(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n' {
                self.advance();
                continue;
            }
            if ch == '/' && self.peek_at(1) == Some('/') {
                self.skip_line_comment();
                continue;
            }
            break;
        }
    }

    /// Lex `@keyword` — keyword escape syntax (§2.4).
    ///
    /// Allows using UZON keywords as identifiers: `@is` becomes the identifier `is`.
    pub(crate) fn lex_keyword_escape(&mut self) -> Result<()> {
        let start_line = self.line;
        let start_col = self.col;
        self.advance(); // skip @

        let mut ident = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() || is_token_boundary(ch) {
                break;
            }
            ident.push(ch);
            self.advance();
        }

        if ident.is_empty() {
            return Err(UzonError::syntax(
                "expected keyword after @",
                start_line,
                start_col,
            ));
        }

        if !is_keyword(&ident) && !is_reserved_keyword(&ident) {
            return Err(UzonError::syntax(
                format!("@ escape used on non-keyword '{ident}'"),
                start_line,
                start_col,
            ));
        }

        self.tokens
            .push(Token::new(TokenType::Identifier, &ident, start_line, start_col));
        Ok(())
    }

    /// Lex a quoted identifier: `'Content-Type'` (§2.3).
    ///
    /// Quoted identifiers allow characters that would otherwise be token boundaries.
    /// The enclosing quotes are syntax — `'key'` and `key` refer to the same name.
    pub(crate) fn lex_quoted_identifier(&mut self) -> Result<()> {
        let start_line = self.line;
        let start_col = self.col;
        self.advance(); // skip opening '

        let mut name = String::new();
        loop {
            match self.peek() {
                Some('\'') => {
                    self.advance();
                    break;
                }
                Some('\n') | None => {
                    return Err(UzonError::syntax(
                        "unterminated quoted identifier",
                        start_line,
                        start_col,
                    ));
                }
                Some(c) => {
                    name.push(c);
                    self.advance();
                }
            }
        }

        // Keywords cannot be quoted — use @keyword escape instead (§2.3)
        if is_keyword(&name) || crate::token::is_reserved_keyword(&name) {
            return Err(UzonError::syntax(
                format!("quoted identifier '{name}' is a keyword; use @{name} to escape"),
                start_line,
                start_col,
            ));
        }

        self.tokens
            .push(Token::new(TokenType::Identifier, &name, start_line, start_col));
        Ok(())
    }
}
