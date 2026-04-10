// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::error::Result;
use crate::token::{Token, TokenType, is_value_token};

use super::Lexer;

impl Lexer {
    /// Try to lex a single or double-character operator/delimiter (§2.6).
    ///
    /// Returns `Ok(Some(()))` if an operator was consumed, `Ok(None)` if the current
    /// character is not an operator in this context.
    pub(crate) fn try_lex_operator(&mut self) -> Result<Option<()>> {
        let ch = match self.peek() {
            Some(c) => c,
            None => return Ok(None),
        };
        let line = self.line;
        let col = self.col;

        let (tt, value) = match ch {
            '{' => (TokenType::LBrace, "{"),
            '}' => (TokenType::RBrace, "}"),
            '[' => (TokenType::LBracket, "["),
            ']' => (TokenType::RBracket, "]"),
            '(' => (TokenType::LParen, "("),
            ')' => (TokenType::RParen, ")"),
            ',' => (TokenType::Comma, ","),
            '.' => (TokenType::Dot, "."),
            '^' => (TokenType::Caret, "^"),
            '%' => (TokenType::Percent, "%"),
            '+' => {
                if self.peek_at(1) == Some('+') {
                    self.advance();
                    self.advance();
                    self.tokens
                        .push(Token::new(TokenType::PlusPlus, "++", line, col));
                    return Ok(Some(()));
                }
                (TokenType::Plus, "+")
            }
            '*' => {
                if self.peek_at(1) == Some('*') {
                    self.advance();
                    self.advance();
                    self.tokens
                        .push(Token::new(TokenType::StarStar, "**", line, col));
                    return Ok(Some(()));
                }
                (TokenType::Star, "*")
            }
            '/' => {
                // `//` comments handled before this function
                (TokenType::Slash, "/")
            }
            '<' => {
                if self.peek_at(1) == Some('=') {
                    self.advance();
                    self.advance();
                    self.tokens
                        .push(Token::new(TokenType::Le, "<=", line, col));
                    return Ok(Some(()));
                }
                (TokenType::Lt, "<")
            }
            '>' => {
                if self.peek_at(1) == Some('=') {
                    self.advance();
                    self.advance();
                    self.tokens
                        .push(Token::new(TokenType::Ge, ">=", line, col));
                    return Ok(Some(()));
                }
                (TokenType::Gt, ">")
            }
            '-' => {
                // Minus after a value token is binary subtraction
                if let Some(prev) = self.last_token_type() {
                    if is_value_token(prev) {
                        self.advance();
                        self.tokens
                            .push(Token::new(TokenType::Minus, "-", line, col));
                        return Ok(Some(()));
                    }
                }
                return Ok(None);
            }
            _ => return Ok(None),
        };

        self.advance();
        self.tokens.push(Token::new(tt, value, line, col));
        Ok(Some(()))
    }
}
