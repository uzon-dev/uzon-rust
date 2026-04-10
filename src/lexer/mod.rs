// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

/// UZON Lexer — tokenizes UZON source text into a token stream.
///
/// The lexer operates in three modes (§2, §4.4):
/// - **Normal**: scans keywords, identifiers, numbers, operators, and delimiters.
/// - **String**: scans string literal content, handling escape sequences (§4.4).
/// - **Interpolation**: scans expressions inside `{...}` within strings (§4.4.1).
///
/// A mode stack tracks nesting so that `"outer {inner} rest"` properly transitions
/// between string and expression contexts. Brace depth tracking allows nested
/// struct literals inside interpolation.
use crate::error::Result;
use crate::token::{Token, TokenType, is_value_token};

mod numbers;
mod identifiers;
mod strings;
mod operators;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Normal,
    String,
    Interpolation,
}

pub struct Lexer {
    pub(crate) source: Vec<char>,
    pub(crate) pos: usize,
    pub(crate) line: usize,
    pub(crate) col: usize,
    pub(crate) tokens: Vec<Token>,
    pub(crate) mode_stack: Vec<Mode>,
    pub(crate) brace_depth: Vec<usize>,
    /// Lines on which a `//` comment was found (for multiline string rejection §4.4.2).
    pub(crate) comment_lines: Vec<usize>,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
            tokens: Vec::new(),
            mode_stack: vec![Mode::Normal],
            brace_depth: Vec::new(),
            comment_lines: Vec::new(),
        }
    }

    /// Tokenize the source, returning the token stream and comment-line list.
    ///
    /// The comment-line list is used by the parser to reject multiline string
    /// continuation across comment lines (§4.4.2).
    pub fn tokenize(mut self) -> Result<(Vec<Token>, Vec<usize>)> {
        // Skip BOM (§2.1)
        if self.peek() == Some('\u{FEFF}') {
            self.advance();
        }

        while self.pos < self.source.len() {
            let mode = *self.mode_stack.last().unwrap_or(&Mode::Normal);
            match mode {
                Mode::Normal => self.lex_normal()?,
                Mode::String => self.lex_string()?,
                Mode::Interpolation => self.lex_interpolation()?,
            }
        }

        self.tokens
            .push(Token::new(TokenType::Eof, "", self.line, self.col));
        Ok((self.tokens, self.comment_lines))
    }

    /// Check if current String mode is nested inside an Interpolation mode.
    pub(crate) fn is_inside_interpolation(&self) -> bool {
        let len = self.mode_stack.len();
        len >= 2 && self.mode_stack[len - 2] == Mode::Interpolation
    }

    // === Character helpers ===

    pub(crate) fn peek(&self) -> Option<char> {
        self.source.get(self.pos).copied()
    }

    pub(crate) fn peek_at(&self, offset: usize) -> Option<char> {
        self.source.get(self.pos + offset).copied()
    }

    pub(crate) fn advance(&mut self) -> Option<char> {
        let ch = self.source.get(self.pos).copied();
        if let Some(c) = ch {
            self.pos += 1;
            if c == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        ch
    }

    pub(crate) fn last_token_type(&self) -> Option<TokenType> {
        self.tokens.last().map(|t| t.token_type)
    }

    // === Mode: Normal ===

    /// Main dispatch for Normal mode. Handles whitespace, newlines, comments,
    /// strings, quoted identifiers, keyword escapes, operators, numbers, and identifiers.
    fn lex_normal(&mut self) -> Result<()> {
        let ch = match self.peek() {
            Some(c) => c,
            None => return Ok(()),
        };

        // Whitespace (not newline)
        if ch == ' ' || ch == '\t' || ch == '\r' {
            self.advance();
            return Ok(());
        }

        // Newline — significant for separator detection (§8)
        if ch == '\n' {
            let line = self.line;
            let col = self.col;
            self.advance();
            self.tokens
                .push(Token::new(TokenType::Newline, "\n", line, col));
            return Ok(());
        }

        // Line comment (§2.2)
        if ch == '/' && self.peek_at(1) == Some('/') {
            self.skip_line_comment();
            return Ok(());
        }

        // String literal (§4.4)
        if ch == '"' {
            return self.lex_string_start();
        }

        // Quoted identifier (§2.3)
        if ch == '\'' {
            return self.lex_quoted_identifier();
        }

        // Keyword escape: `@keyword` → identifier (§2.4)
        if ch == '@' {
            return self.lex_keyword_escape();
        }

        // Operators and punctuation
        if let Some(()) = self.try_lex_operator()? {
            return Ok(());
        }

        // Number, -inf, -nan, or identifier
        if ch == '-' || ch.is_ascii_digit() {
            if self.try_lex_number()? {
                return Ok(());
            }
            // -inf / -nan when minus is not after a value token
            if ch == '-' && !self.is_after_value_token() {
                if self.try_lex_negative_inf_nan()? {
                    return Ok(());
                }
            }
        }

        // Unary minus (not after value token, not followed by digit/inf/nan)
        if ch == '-' {
            let line = self.line;
            let col = self.col;
            self.advance();
            self.tokens
                .push(Token::new(TokenType::Minus, "-", line, col));
            return Ok(());
        }

        // Identifier / keyword
        self.lex_identifier_or_keyword()
    }

    pub(crate) fn is_after_value_token(&self) -> bool {
        self.last_token_type()
            .map(is_value_token)
            .unwrap_or(false)
    }

    /// Skip a line comment, recording the line number for multiline string rejection (§4.4.2).
    pub(crate) fn skip_line_comment(&mut self) {
        self.comment_lines.push(self.line);
        self.advance(); // skip first /
        self.advance(); // skip second /
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.advance();
        }
    }
}
