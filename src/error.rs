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
        import_trace: Vec<Location>,
    },
    Type {
        message: String,
        location: Option<Location>,
        import_trace: Vec<Location>,
    },
    Runtime {
        message: String,
        location: Option<Location>,
        import_trace: Vec<Location>,
    },
    Circular {
        message: String,
        location: Option<Location>,
        import_trace: Vec<Location>,
    },
    /// Multiple errors collected during evaluation (e.g. independent cycles).
    Multiple {
        errors: Vec<UzonError>,
    },
}

impl UzonError {
    pub fn syntax(message: impl Into<String>, line: usize, col: usize) -> Self {
        UzonError::Syntax {
            message: message.into(),
            location: Some(Location { line, col, filename: None }),
            import_trace: Vec::new(),
        }
    }

    pub fn type_error(message: impl Into<String>, line: usize, col: usize) -> Self {
        UzonError::Type {
            message: message.into(),
            location: Some(Location { line, col, filename: None }),
            import_trace: Vec::new(),
        }
    }

    pub fn runtime(message: impl Into<String>, line: usize, col: usize) -> Self {
        UzonError::Runtime {
            message: message.into(),
            location: Some(Location { line, col, filename: None }),
            import_trace: Vec::new(),
        }
    }

    pub fn circular(message: impl Into<String>, line: usize, col: usize) -> Self {
        UzonError::Circular {
            message: message.into(),
            location: Some(Location { line, col, filename: None }),
            import_trace: Vec::new(),
        }
    }

    /// Returns true if this is a RuntimeError (suppressed in speculative eval per §D.5).
    pub fn is_runtime(&self) -> bool {
        matches!(self, UzonError::Runtime { .. })
    }

    /// Returns true if this is a Circular dependency error.
    pub fn is_circular(&self) -> bool {
        matches!(self, UzonError::Circular { .. })
    }

    /// Wrap multiple errors into a single error value.
    pub fn multiple(errors: Vec<UzonError>) -> Self {
        debug_assert!(!errors.is_empty());
        if errors.len() == 1 {
            return errors.into_iter().next().unwrap();
        }
        UzonError::Multiple { errors }
    }

    /// Attach a filename to the error's location (for imported file errors, §7).
    pub fn with_filename(mut self, filename: String) -> Self {
        if let UzonError::Multiple { ref mut errors } = self {
            for e in errors.iter_mut() {
                let loc = match e {
                    UzonError::Syntax { location, .. }
                    | UzonError::Type { location, .. }
                    | UzonError::Runtime { location, .. }
                    | UzonError::Circular { location, .. } => location,
                    UzonError::Multiple { .. } => continue,
                };
                if let Some(loc) = loc {
                    if loc.filename.is_none() {
                        loc.filename = Some(filename.clone());
                    }
                }
            }
            return self;
        }
        let loc = match &mut self {
            UzonError::Syntax { location, .. }
            | UzonError::Type { location, .. }
            | UzonError::Runtime { location, .. }
            | UzonError::Circular { location, .. } => location,
            UzonError::Multiple { .. } => unreachable!(),
        };
        if let Some(loc) = loc {
            if loc.filename.is_none() {
                loc.filename = Some(filename);
            }
        }
        self
    }

    /// Push an import callsite onto the trace. The most recent import site
    /// is pushed last, so the trace reads innermost-first (like a stack trace).
    pub fn with_import_site(mut self, line: usize, col: usize, filename: Option<String>) -> Self {
        if let UzonError::Multiple { ref mut errors } = self {
            for e in errors.iter_mut() {
                let trace = match e {
                    UzonError::Syntax { import_trace, .. }
                    | UzonError::Type { import_trace, .. }
                    | UzonError::Runtime { import_trace, .. }
                    | UzonError::Circular { import_trace, .. } => import_trace,
                    UzonError::Multiple { .. } => continue,
                };
                trace.push(Location { line, col: col, filename: filename.clone() });
            }
            return self;
        }
        let trace = match &mut self {
            UzonError::Syntax { import_trace, .. }
            | UzonError::Type { import_trace, .. }
            | UzonError::Runtime { import_trace, .. }
            | UzonError::Circular { import_trace, .. } => import_trace,
            UzonError::Multiple { .. } => unreachable!(),
        };
        trace.push(Location { line, col, filename });
        self
    }
}

/// Format a single error line: `{ErrorType} at {location}: {message}`
fn write_error_line(
    f: &mut fmt::Formatter<'_>,
    kind: &str,
    message: &str,
    location: &Option<Location>,
    import_trace: &[Location],
) -> fmt::Result {
    write!(f, "{kind}")?;
    if let Some(loc) = location {
        write!(f, " at {loc}")?;
    }
    write!(f, ": {message}")?;
    for site in import_trace.iter().rev() {
        write!(f, "\n  imported at {site}")?;
    }
    Ok(())
}

impl fmt::Display for UzonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UzonError::Syntax { message, location, import_trace } => {
                write_error_line(f, "SyntaxError", message, location, import_trace)
            }
            UzonError::Type { message, location, import_trace } => {
                write_error_line(f, "TypeError", message, location, import_trace)
            }
            UzonError::Runtime { message, location, import_trace } => {
                write_error_line(f, "RuntimeError", message, location, import_trace)
            }
            UzonError::Circular { message, location, import_trace } => {
                write_error_line(f, "CircularError", message, location, import_trace)
            }
            UzonError::Multiple { errors } => {
                for (i, e) in errors.iter().enumerate() {
                    if i > 0 {
                        write!(f, "\n")?;
                    }
                    write!(f, "{e}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for UzonError {}

pub type Result<T> = std::result::Result<T, UzonError>;
