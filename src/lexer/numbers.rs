// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::error::{Result, UzonError};
use crate::token::{Token, TokenType, is_token_boundary, is_value_token};

use super::Lexer;

impl Lexer {
    /// Try to lex `-inf` or `-nan` as a single token (§4.3).
    ///
    /// A minus immediately followed by `inf` or `nan` (no whitespace) is lexed
    /// as a negative literal, not subtraction. `-nan` is semantically identical
    /// to `nan` per IEEE 754 (§5.2).
    pub(crate) fn try_lex_negative_inf_nan(&mut self) -> Result<bool> {
        let after_minus = self.pos + 1;
        let remaining: String = self.source[after_minus..]
            .iter()
            .take(3)
            .collect();

        if remaining.starts_with("inf") {
            let end = after_minus + 3;
            if end < self.source.len() {
                let next = self.source[end];
                if !next.is_whitespace() && !is_token_boundary(next) && next != '/' {
                    return Ok(false);
                }
            }
            let line = self.line;
            let col = self.col;
            for _ in 0..4 {
                self.advance();
            }
            self.tokens
                .push(Token::new(TokenType::Float, "-inf", line, col));
            return Ok(true);
        }

        if remaining.starts_with("nan") {
            let end = after_minus + 3;
            if end < self.source.len() {
                let next = self.source[end];
                if !next.is_whitespace() && !is_token_boundary(next) && next != '/' {
                    return Ok(false);
                }
            }
            let line = self.line;
            let col = self.col;
            for _ in 0..4 {
                self.advance();
            }
            self.tokens
                .push(Token::new(TokenType::Float, "-nan", line, col));
            return Ok(true);
        }

        Ok(false)
    }

    /// Try to lex a numeric literal (integer or float).
    ///
    /// Supports decimal, hex (`0x`), octal (`0o`), and binary (`0b`) integers (§4.2),
    /// underscore separators, decimal floats, and scientific notation (§4.3).
    pub(crate) fn try_lex_number(&mut self) -> Result<bool> {
        let start_pos = self.pos;
        let start_line = self.line;
        let start_col = self.col;
        let ch = self.source[self.pos];

        let negative = ch == '-';
        if negative {
            if let Some(prev) = self.last_token_type() {
                if is_value_token(prev) {
                    return Ok(false);
                }
            }
            match self.peek_at(1) {
                Some(c) if c.is_ascii_digit() => {}
                _ => return Ok(false),
            }
        }

        let digit_start = if negative { self.pos + 1 } else { self.pos };
        let (base, prefix_len) = self.detect_base_prefix(digit_start);
        let num_start = digit_start + prefix_len;
        let mut i = num_start;

        // First character after base prefix must be a digit, not underscore (§4.2)
        if base != 10 && i < self.source.len() && self.source[i] == '_' {
            return Ok(false);
        }

        let (end_pos, has_digits, last_was_underscore) = self.consume_digits(i, base);
        i = end_pos;

        if !has_digits {
            return Ok(false);
        }

        // Underscores must be between digits only (§4.2)
        if last_was_underscore {
            return Err(UzonError::syntax(
                "trailing underscore in numeric literal",
                self.line,
                self.col,
            ));
        }

        let (end_pos, is_float) = self.try_consume_float_suffix(i, base, start_line, start_col)?;
        i = end_pos;

        // Check proper termination (not followed by identifier chars)
        if i < self.source.len() {
            let next = self.source[i];
            if !next.is_whitespace() && !is_token_boundary(next) && next != '/' {
                return Ok(false);
            }
        }

        let value: String = self.source[start_pos..i].iter().collect();
        let chars_to_advance = i - self.pos;
        for _ in 0..chars_to_advance {
            self.advance();
        }

        let tt = if is_float {
            TokenType::Float
        } else {
            TokenType::Integer
        };
        self.tokens
            .push(Token::new(tt, value, start_line, start_col));
        Ok(true)
    }

    /// Detect base prefix at the given position.
    fn detect_base_prefix(&self, digit_start: usize) -> (u32, usize) {
        if self.source.get(digit_start).copied() == Some('0') {
            match self.source.get(digit_start + 1).copied() {
                Some('x' | 'X') => (16, 2),
                Some('o' | 'O') => (8, 2),
                Some('b' | 'B') => (2, 2),
                _ => (10, 0),
            }
        } else {
            (10, 0)
        }
    }

    /// Consume digits for the given base, handling underscore separators.
    fn consume_digits(&self, start: usize, base: u32) -> (usize, bool, bool) {
        let mut i = start;
        let mut has_digits = false;
        let mut last_was_underscore = false;

        while i < self.source.len() {
            let c = self.source[i];
            if c == '_' {
                last_was_underscore = true;
                i += 1;
                continue;
            }
            let valid = match base {
                16 => c.is_ascii_hexdigit(),
                8 => matches!(c, '0'..='7'),
                2 => matches!(c, '0' | '1'),
                _ => c.is_ascii_digit(),
            };
            if !valid {
                break;
            }
            last_was_underscore = false;
            has_digits = true;
            i += 1;
        }

        (i, has_digits, last_was_underscore)
    }

    /// Try to consume float suffix (decimal point and/or exponent) (§4.3).
    fn try_consume_float_suffix(
        &self,
        start: usize,
        base: u32,
        start_line: usize,
        start_col: usize,
    ) -> Result<(usize, bool)> {
        let mut i = start;
        let mut is_float = false;

        if base != 10 {
            return Ok((i, false));
        }

        // Decimal point
        if i < self.source.len() && self.source[i] == '.' {
            let after_dot = i + 1;
            if after_dot < self.source.len() && self.source[after_dot].is_ascii_digit() {
                is_float = true;
                i = after_dot;
                while i < self.source.len()
                    && (self.source[i].is_ascii_digit() || self.source[i] == '_')
                {
                    i += 1;
                }
            }
        }

        // Exponent (§4.3)
        if i < self.source.len() && matches!(self.source[i], 'e' | 'E') {
            is_float = true;
            i += 1;
            if i < self.source.len() && matches!(self.source[i], '+' | '-') {
                i += 1;
            }
            let exp_start = i;
            while i < self.source.len()
                && (self.source[i].is_ascii_digit() || self.source[i] == '_')
            {
                i += 1;
            }
            if i == exp_start {
                return Err(UzonError::syntax(
                    "expected digits after exponent",
                    start_line,
                    start_col,
                ));
            }
        }

        Ok((i, is_float))
    }
}
