// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

pub mod ast;
pub mod error;
pub mod evaluator;
pub mod lexer;
pub mod parser;
pub mod scope;
pub mod stringify;
pub mod token;
pub mod value;

// Re-export primary API for ergonomic usage.
pub use error::{UzonError, Result};
pub use evaluator::{from_str, from_str_plain, from_path, Evaluator, EvalOptions};
pub use stringify::{to_string, to_string_with_options, StringifyOptions};
pub use value::{Value, UzonInteger, UzonFloat, IntegerType, FloatType};
pub use value::ops::ValueConversionError;
