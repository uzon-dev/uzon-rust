// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::error::{Result, UzonError};
use crate::token::{Token, TokenType};

use super::{Lexer, Mode};

impl Lexer {
    /// Start lexing a string literal from the opening `"` (§4.4).
    ///
    /// Handles plain characters, escape sequences, and interpolation `{expr}` (§4.4.1).
    /// Control characters other than tab are rejected.
    pub(crate) fn lex_string_start(&mut self) -> Result<()> {
        let start_line = self.line;
        let start_col = self.col;
        self.advance(); // skip opening "
        self.mode_stack.push(Mode::String);

        let mut s = String::new();

        loop {
            match self.peek() {
                None => {
                    return Err(UzonError::syntax(
                        "unterminated string literal",
                        start_line,
                        start_col,
                    ));
                }
                Some('"') => {
                    self.advance();
                    self.mode_stack.pop();
                    self.tokens
                        .push(Token::new(TokenType::String, &s, start_line, start_col));
                    return Ok(());
                }
                Some('\\') => {
                    if self.peek_at(1) == Some('"') && self.is_inside_interpolation() {
                        self.advance(); // consume `\`
                    } else {
                        let esc = self.lex_escape_sequence()?;
                        s.push_str(&esc);
                    }
                }
                Some('{') => {
                    // Start interpolation (§4.4.1)
                    self.tokens
                        .push(Token::new(TokenType::String, &s, start_line, start_col));
                    s.clear();

                    let interp_line = self.line;
                    let interp_col = self.col;
                    self.advance(); // skip {
                    self.tokens.push(Token::new(
                        TokenType::InterpStart,
                        "{",
                        interp_line,
                        interp_col,
                    ));

                    self.mode_stack.pop(); // remove String
                    self.mode_stack.push(Mode::String); // re-push (return to after interpolation)
                    self.mode_stack.push(Mode::Interpolation);
                    self.brace_depth.push(0);
                    return Ok(());
                }
                Some(c) => {
                    if c.is_control() && c != '\t' {
                        return Err(UzonError::syntax(
                            format!("control character U+{:04X} in string literal", c as u32),
                            self.line,
                            self.col,
                        ));
                    }
                    s.push(c);
                    self.advance();
                }
            }
        }
    }

    /// Lex escape sequences within a string literal (§4.4).
    ///
    /// Supported: `\"`, `\\`, `\n`, `\r`, `\t`, `\0`, `\{`,
    /// `\xHH` (ASCII range only), `\u{HHHHHH}` (1–6 hex digits, valid Unicode scalar).
    pub(crate) fn lex_escape_sequence(&mut self) -> Result<String> {
        let esc_line = self.line;
        let esc_col = self.col;
        self.advance(); // skip backslash

        match self.peek() {
            Some('"') => { self.advance(); Ok("\"".to_string()) }
            Some('\\') => { self.advance(); Ok("\\".to_string()) }
            Some('n') => { self.advance(); Ok("\n".to_string()) }
            Some('r') => { self.advance(); Ok("\r".to_string()) }
            Some('t') => { self.advance(); Ok("\t".to_string()) }
            Some('0') => { self.advance(); Ok("\0".to_string()) }
            Some('{') => { self.advance(); Ok("{".to_string()) }
            Some('x') => self.lex_hex_escape(esc_line, esc_col),
            Some('u') => self.lex_unicode_escape(esc_line, esc_col),
            Some(c) => Err(UzonError::syntax(
                format!("unknown escape sequence: \\{c}"),
                esc_line,
                esc_col,
            )),
            None => Err(UzonError::syntax(
                "unexpected end of input in escape sequence",
                esc_line,
                esc_col,
            )),
        }
    }

    /// Lex `\xHH` hex escape (§4.4 — ASCII range 0x00–0x7F only).
    fn lex_hex_escape(&mut self, esc_line: usize, esc_col: usize) -> Result<String> {
        self.advance(); // skip 'x'
        let mut hex = String::new();
        for _ in 0..2 {
            match self.peek() {
                Some(c) if c.is_ascii_hexdigit() => {
                    hex.push(c);
                    self.advance();
                }
                _ => {
                    return Err(UzonError::syntax(
                        "\\x escape requires exactly 2 hex digits",
                        esc_line,
                        esc_col,
                    ));
                }
            }
        }
        let val = u8::from_str_radix(&hex, 16).unwrap();
        if val > 0x7F {
            return Err(UzonError::syntax(
                "\\x escape value must be in ASCII range (0x00-0x7F)",
                esc_line,
                esc_col,
            ));
        }
        Ok((val as char).to_string())
    }

    /// Lex `\u{HHHHHH}` Unicode escape (§4.4 — 1–6 hex digits, valid scalar value).
    fn lex_unicode_escape(&mut self, esc_line: usize, esc_col: usize) -> Result<String> {
        self.advance(); // skip 'u'
        if self.peek() != Some('{') {
            return Err(UzonError::syntax(
                "expected '{' after \\u",
                esc_line,
                esc_col,
            ));
        }
        self.advance(); // skip {

        let mut hex = String::new();
        loop {
            match self.peek() {
                Some('}') => {
                    self.advance();
                    break;
                }
                Some(c) if c.is_ascii_hexdigit() => {
                    hex.push(c);
                    self.advance();
                }
                _ => {
                    return Err(UzonError::syntax(
                        "invalid character in \\u{...} escape",
                        esc_line,
                        esc_col,
                    ));
                }
            }
        }

        if hex.is_empty() || hex.len() > 6 {
            return Err(UzonError::syntax(
                "\\u{...} requires 1-6 hex digits",
                esc_line,
                esc_col,
            ));
        }

        let val = u32::from_str_radix(&hex, 16).unwrap();
        if val > 0x10FFFF || (0xD800..=0xDFFF).contains(&val) {
            return Err(UzonError::syntax(
                format!("\\u{{{}}} is not a valid Unicode scalar value", hex),
                esc_line,
                esc_col,
            ));
        }
        let c = char::from_u32(val).ok_or_else(|| {
            UzonError::syntax(
                format!("\\u{{{}}} is not a valid Unicode scalar value", hex),
                esc_line,
                esc_col,
            )
        })?;
        Ok(c.to_string())
    }

    /// Continue lexing inside a string after interpolation ends (§4.4.1).
    pub(crate) fn lex_string(&mut self) -> Result<()> {
        let start_line = self.line;
        let start_col = self.col;
        let mut s = String::new();

        loop {
            match self.peek() {
                None => {
                    return Err(UzonError::syntax(
                        "unterminated string literal",
                        start_line,
                        start_col,
                    ));
                }
                Some('"') => {
                    self.advance();
                    self.mode_stack.pop(); // exit String mode
                    self.tokens
                        .push(Token::new(TokenType::String, &s, start_line, start_col));
                    return Ok(());
                }
                Some('\\') => {
                    let esc = self.lex_escape_sequence()?;
                    s.push_str(&esc);
                }
                Some('{') => {
                    self.tokens
                        .push(Token::new(TokenType::String, &s, start_line, start_col));
                    s.clear();

                    let interp_line = self.line;
                    let interp_col = self.col;
                    self.advance();
                    self.tokens.push(Token::new(
                        TokenType::InterpStart,
                        "{",
                        interp_line,
                        interp_col,
                    ));

                    self.mode_stack.push(Mode::Interpolation);
                    self.brace_depth.push(0);
                    return Ok(());
                }
                Some(c) => {
                    if c.is_control() && c != '\t' {
                        return Err(UzonError::syntax(
                            format!("control character U+{:04X} in string literal", c as u32),
                            self.line,
                            self.col,
                        ));
                    }
                    s.push(c);
                    self.advance();
                }
            }
        }
    }

    // === Mode: Interpolation ===

    /// Lex tokens inside `{...}` interpolation (§4.4.1).
    ///
    /// Tracks brace depth so nested struct literals inside interpolation are
    /// handled correctly. When depth returns to 0 and `}` is encountered,
    /// interpolation ends.
    pub(crate) fn lex_interpolation(&mut self) -> Result<()> {
        let ch = match self.peek() {
            Some(c) => c,
            None => {
                return Err(UzonError::syntax(
                    "unterminated string interpolation",
                    self.line,
                    self.col,
                ));
            }
        };

        if ch == '{' {
            if let Some(depth) = self.brace_depth.last_mut() {
                *depth += 1;
            }
            let line = self.line;
            let col = self.col;
            self.advance();
            self.tokens
                .push(Token::new(TokenType::LBrace, "{", line, col));
            return Ok(());
        }

        if ch == '}' {
            let depth = self.brace_depth.last().copied().unwrap_or(0);
            if depth == 0 {
                let line = self.line;
                let col = self.col;
                self.advance();
                self.tokens
                    .push(Token::new(TokenType::InterpEnd, "}", line, col));
                self.brace_depth.pop();
                self.mode_stack.pop(); // exit Interpolation, back to String
                return Ok(());
            } else {
                if let Some(d) = self.brace_depth.last_mut() {
                    *d -= 1;
                }
                let line = self.line;
                let col = self.col;
                self.advance();
                self.tokens
                    .push(Token::new(TokenType::RBrace, "}", line, col));
                return Ok(());
            }
        }

        // `\"` inside interpolation starts a string literal
        if ch == '\\' && self.peek_at(1) == Some('"') {
            self.advance(); // consume backslash
        }

        self.lex_normal()
    }
}
