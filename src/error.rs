// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use std::fmt;

/// Source location for error reporting (§11.2.0).
///
/// Line numbers are 1-based. Column numbers are 1-based and count Unicode
/// scalar values from the start of the line. When a file is imported via
/// `struct "path"` (§7), the filename is attached to locate the source.
#[derive(Debug, Clone)]
pub struct Location {
    pub line: usize,
    pub col: usize,
    pub filename: Option<String>,
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref name) = self.filename {
            write!(f, "{name}:{}:{}", self.line, self.col)
        } else {
            write!(f, "{}:{}", self.line, self.col)
        }
    }
}

/// UZON error types, classified per §11.2.
///
/// Error priority (§11.2): Syntax > Circular > Type > Runtime.
/// - **Syntax**: malformed tokens, grammar violations, invalid literals
/// - **Circular**: dependency cycles in bindings or imports
/// - **Type**: type mismatches, annotation failures, conversion errors
/// - **Runtime**: division by zero, overflow, undefined in terminal contexts
#[derive(Debug, Clone)]
pub enum UzonError {
    Syntax {
        message: String,
        location: Option<Location>,
    },
    Type {
        message: String,
        location: Option<Location>,
    },
    Runtime {
        message: String,
        location: Option<Location>,
    },
    Circular {
        message: String,
        location: Option<Location>,
    },
}

impl UzonError {
    pub fn syntax(message: impl Into<String>, line: usize, col: usize) -> Self {
        UzonError::Syntax {
            message: message.into(),
            location: Some(Location { line, col, filename: None }),
        }
    }

    pub fn type_error(message: impl Into<String>, line: usize, col: usize) -> Self {
        UzonError::Type {
            message: message.into(),
            location: Some(Location { line, col, filename: None }),
        }
    }

    pub fn runtime(message: impl Into<String>, line: usize, col: usize) -> Self {
        UzonError::Runtime {
            message: message.into(),
            location: Some(Location { line, col, filename: None }),
        }
    }

    pub fn circular(message: impl Into<String>, line: usize, col: usize) -> Self {
        UzonError::Circular {
            message: message.into(),
            location: Some(Location { line, col, filename: None }),
        }
    }

    /// Attach a filename to the error's location (for imported file errors, §7).
    pub fn with_filename(mut self, filename: String) -> Self {
        let loc = match &mut self {
            UzonError::Syntax { location, .. }
            | UzonError::Type { location, .. }
            | UzonError::Runtime { location, .. }
            | UzonError::Circular { location, .. } => location,
        };
        if let Some(loc) = loc {
            if loc.filename.is_none() {
                loc.filename = Some(filename);
            }
        }
        self
    }
}

impl fmt::Display for UzonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UzonError::Syntax { message, location } => {
                write!(f, "SyntaxError")?;
                if let Some(loc) = location {
                    write!(f, " at {loc}")?;
                }
                write!(f, ": {message}")
            }
            UzonError::Type { message, location } => {
                write!(f, "TypeError")?;
                if let Some(loc) = location {
                    write!(f, " at {loc}")?;
                }
                write!(f, ": {message}")
            }
            UzonError::Runtime { message, location } => {
                write!(f, "RuntimeError")?;
                if let Some(loc) = location {
                    write!(f, " at {loc}")?;
                }
                write!(f, ": {message}")
            }
            UzonError::Circular { message, location } => {
                write!(f, "CircularError")?;
                if let Some(loc) = location {
                    write!(f, " at {loc}")?;
                }
                write!(f, ": {message}")
            }
        }
    }
}

impl std::error::Error for UzonError {}

pub type Result<T> = std::result::Result<T, UzonError>;
