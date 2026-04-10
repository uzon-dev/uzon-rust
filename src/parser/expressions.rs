// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use crate::ast::*;
use crate::error::{Result, UzonError};
use crate::token::TokenType;

use super::Parser;

impl Parser {
    // === Expressions (precedence climbing) ===

    pub(crate) fn parse_expression(&mut self) -> Result<Node> {
        self.parse_or_else()
    }

    /// Level 18: `or else` — undefined coalescing (§5.7).
    fn parse_or_else(&mut self) -> Result<Node> {
        let mut left = self.parse_or()?;
        while self.at(TokenType::OrElse) {
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let right = self.parse_or()?;
            left = Node::new(
                NodeKind::OrElse {
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span.line,
                span.col,
            );
        }
        Ok(left)
    }

    /// Level 17: `or` — logical OR (§5.6).
    fn parse_or(&mut self) -> Result<Node> {
        let mut left = self.parse_and()?;
        while self.at(TokenType::Or) {
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let right = self.parse_and()?;
            left = Node::new(
                NodeKind::BinaryOp {
                    op: BinaryOp::Or,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span.line,
                span.col,
            );
        }
        Ok(left)
    }

    /// Level 16: `and` — logical AND (§5.6).
    pub(crate) fn parse_and(&mut self) -> Result<Node> {
        let mut left = self.parse_not()?;
        while self.at(TokenType::And) {
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let right = self.parse_not()?;
            left = Node::new(
                NodeKind::BinaryOp {
                    op: BinaryOp::And,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span.line,
                span.col,
            );
        }
        Ok(left)
    }

    /// Level 15: `not` — logical NOT prefix (§5.6).
    pub(crate) fn parse_not(&mut self) -> Result<Node> {
        if self.at(TokenType::Not) {
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let operand = self.parse_not()?;
            Ok(Node::new(
                NodeKind::UnaryOp {
                    op: UnaryOp::Not,
                    operand: Box::new(operand),
                },
                span.line,
                span.col,
            ))
        } else {
            self.parse_equality()
        }
    }

    /// Helper for binding decomposition: parse `not <expr>` when `is not` was decomposed.
    ///
    /// Continues parsing at `and`, `or`, and `or else` levels so that
    /// `x is not A and B` parses as `x is (not A) and B`.
    pub(crate) fn parse_not_expr(&mut self, line: usize, col: usize) -> Result<Node> {
        let not_operand = self.parse_not()?;
        let not_node = Node::new(
            NodeKind::UnaryOp {
                op: UnaryOp::Not,
                operand: Box::new(not_operand),
            },
            line,
            col,
        );
        // Continue parsing at `and` level and above
        self.skip_newlines();
        let mut left = not_node;
        while self.at(TokenType::And) {
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let right = self.parse_not()?;
            left = Node::new(
                NodeKind::BinaryOp {
                    op: BinaryOp::And,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span.line,
                span.col,
            );
        }
        // Continue with `or` level
        while self.at(TokenType::Or) {
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let right = self.parse_and()?;
            left = Node::new(
                NodeKind::BinaryOp {
                    op: BinaryOp::Or,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span.line,
                span.col,
            );
        }
        // Continue with `or else` level
        while self.at(TokenType::OrElse) {
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let right = self.parse_or()?;
            left = Node::new(
                NodeKind::OrElse {
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span.line,
                span.col,
            );
        }
        Ok(left)
    }

    /// Helper for binding decomposition: `is named` at binding start.
    ///
    /// This pattern (`x is named tag ...`) cannot produce a valid binding value
    /// because `named` requires a preceding value expression. This is a user error.
    pub(crate) fn wrap_named(&mut self, tag_node: Node, _line: usize, _col: usize) -> Result<Node> {
        Err(UzonError::syntax(
            "'is named' cannot appear at the start of a binding value; \
             for tagged unions use: x is <value> named <tag> from ...",
            tag_node.span.line,
            tag_node.span.col,
        ))
    }

    pub(crate) fn wrap_not_named(&mut self, _expr: Node, line: usize, col: usize) -> Result<Node> {
        Err(UzonError::syntax(
            "'is not named' cannot appear at the start of a binding value",
            line,
            col,
        ))
    }

    /// Level 14: `is`, `is not`, `is named`, `is not named` — equality/variant check (§5.2, §3.7.2).
    ///
    /// No chaining: `a is b is c` is a syntax error.
    fn parse_equality(&mut self) -> Result<Node> {
        let left = self.parse_membership()?;
        self.skip_newlines();

        match self.peek_type() {
            TokenType::Is => {
                let span = self.current_span();
                self.advance();
                self.skip_newlines();
                let right = self.parse_membership()?;
                Ok(Node::new(
                    NodeKind::BinaryOp {
                        op: BinaryOp::Is,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    span.line,
                    span.col,
                ))
            }
            TokenType::IsNot => {
                let span = self.current_span();
                self.advance();
                self.skip_newlines();
                let right = self.parse_membership()?;
                Ok(Node::new(
                    NodeKind::BinaryOp {
                        op: BinaryOp::IsNot,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    span.line,
                    span.col,
                ))
            }
            TokenType::IsNamed => {
                let span = self.current_span();
                self.advance();
                self.skip_newlines();
                let name_tok = self.advance().clone();
                Ok(Node::new(
                    NodeKind::BinaryOp {
                        op: BinaryOp::IsNamed,
                        left: Box::new(left),
                        right: Box::new(Node::new(
                            NodeKind::Identifier {
                                name: name_tok.value,
                            },
                            name_tok.line,
                            name_tok.col,
                        )),
                    },
                    span.line,
                    span.col,
                ))
            }
            TokenType::IsNotNamed => {
                let span = self.current_span();
                self.advance();
                self.skip_newlines();
                let name_tok = self.advance().clone();
                Ok(Node::new(
                    NodeKind::BinaryOp {
                        op: BinaryOp::IsNotNamed,
                        left: Box::new(left),
                        right: Box::new(Node::new(
                            NodeKind::Identifier {
                                name: name_tok.value,
                            },
                            name_tok.line,
                            name_tok.col,
                        )),
                    },
                    span.line,
                    span.col,
                ))
            }
            _ => Ok(left),
        }
    }

    /// Level 13: `in` — membership test (§5.8.1).
    fn parse_membership(&mut self) -> Result<Node> {
        let left = self.parse_relational()?;
        self.skip_newlines();
        if self.at(TokenType::In) {
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let right = self.parse_relational()?;
            Ok(Node::new(
                NodeKind::BinaryOp {
                    op: BinaryOp::In,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span.line,
                span.col,
            ))
        } else {
            Ok(left)
        }
    }

    /// Level 12: `<`, `<=`, `>`, `>=` — comparison (§5.4). No chaining.
    fn parse_relational(&mut self) -> Result<Node> {
        let left = self.parse_concat()?;
        self.skip_newlines();
        let op = match self.peek_type() {
            TokenType::Lt => BinaryOp::Lt,
            TokenType::Le => BinaryOp::Le,
            TokenType::Gt => BinaryOp::Gt,
            TokenType::Ge => BinaryOp::Ge,
            _ => return Ok(left),
        };
        let span = self.current_span();
        self.advance();
        self.skip_newlines();
        let right = self.parse_concat()?;
        Ok(Node::new(
            NodeKind::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
            span.line,
            span.col,
        ))
    }

    /// Level 11: `++` — list concatenation (§5.8.2), left-associative.
    fn parse_concat(&mut self) -> Result<Node> {
        let mut left = self.parse_addition()?;
        while {
            self.skip_newlines();
            self.at(TokenType::PlusPlus)
        } {
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let right = self.parse_addition()?;
            left = Node::new(
                NodeKind::BinaryOp {
                    op: BinaryOp::Concat,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span.line,
                span.col,
            );
        }
        Ok(left)
    }

    /// Level 10: `+`, `-` — addition/subtraction (§5.3), left-associative.
    fn parse_addition(&mut self) -> Result<Node> {
        let mut left = self.parse_multiplication()?;
        loop {
            self.skip_newlines();
            let op = match self.peek_type() {
                TokenType::Plus => BinaryOp::Add,
                TokenType::Minus => BinaryOp::Sub,
                _ => break,
            };
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let right = self.parse_multiplication()?;
            left = Node::new(
                NodeKind::BinaryOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span.line,
                span.col,
            );
        }
        Ok(left)
    }

    /// Level 9: `*`, `/`, `%`, `**` — multiplication and repetition (§5.3, §5.8.3), left-associative.
    fn parse_multiplication(&mut self) -> Result<Node> {
        let mut left = self.parse_unary()?;
        loop {
            self.skip_newlines();
            let op = match self.peek_type() {
                TokenType::Star => BinaryOp::Mul,
                TokenType::Slash => BinaryOp::Div,
                TokenType::Percent => BinaryOp::Mod,
                TokenType::StarStar => BinaryOp::Repeat,
                _ => break,
            };
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let right = self.parse_unary()?;
            left = Node::new(
                NodeKind::BinaryOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span.line,
                span.col,
            );
        }
        Ok(left)
    }

    /// Level 8: unary `-` — negation (§5.3).
    fn parse_unary(&mut self) -> Result<Node> {
        if self.at(TokenType::Minus) {
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let operand = self.parse_power()?;
            Ok(Node::new(
                NodeKind::UnaryOp {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                },
                span.line,
                span.col,
            ))
        } else {
            self.parse_power()
        }
    }

    /// Level 7: `^` — exponentiation (§5.3), right-associative.
    fn parse_power(&mut self) -> Result<Node> {
        let base = self.parse_type_decl()?;
        self.skip_newlines();
        if self.at(TokenType::Caret) {
            let span = self.current_span();
            self.advance();
            self.skip_newlines();
            let exp = self.parse_unary()?; // right-associative: recurse to unary
            Ok(Node::new(
                NodeKind::BinaryOp {
                    op: BinaryOp::Pow,
                    left: Box::new(base),
                    right: Box::new(exp),
                },
                span.line,
                span.col,
            ))
        } else {
            Ok(base)
        }
    }
}
