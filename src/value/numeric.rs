// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use std::fmt;

use super::format::format_float;

// ============================================================
// Numeric type annotations (§4.2, §4.3)
// ============================================================

/// Numeric type annotation for integers.
///
/// Covers `i{N}` (signed), `u{N}` (unsigned), and `Arbitrary` (unbounded).
/// The default for untyped integer literals is i64 (§4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerType {
    Arbitrary,
    I(u16),
    U(u16),
}

impl IntegerType {
    /// The default type for untyped integer literals (§4.2).
    pub const DEFAULT: IntegerType = IntegerType::I(64);

    pub fn is_default(&self) -> bool {
        matches!(self, IntegerType::I(64))
    }

    /// The valid value range for this type, as (min, max) inclusive.
    pub fn range(&self) -> Option<(i128, i128)> {
        match self {
            IntegerType::Arbitrary => Some((i128::MIN, i128::MAX)),
            // §4.2 v0.8: i0/u0 are unit types — only value `0` is valid (2^0 = 1 value).
            IntegerType::I(0) | IntegerType::U(0) => Some((0, 0)),
            IntegerType::I(n) => {
                let n = *n as u32;
                if n >= 128 {
                    Some((i128::MIN, i128::MAX))
                } else {
                    Some((-(1i128 << (n - 1)), (1i128 << (n - 1)) - 1))
                }
            }
            IntegerType::U(n) => {
                let n = *n as u32;
                if n >= 128 {
                    Some((0, i128::MAX))
                } else {
                    Some((0, (1i128 << n) - 1))
                }
            }
        }
    }

    /// Parse a type name like "i32" or "u8" into an IntegerType.
    pub fn from_type_name(name: &str) -> Option<IntegerType> {
        if let Some(bits) = name.strip_prefix('i') {
            bits.parse::<u16>().ok().map(IntegerType::I)
        } else if let Some(bits) = name.strip_prefix('u') {
            bits.parse::<u16>().ok().map(IntegerType::U)
        } else {
            None
        }
    }

    /// Display name for error messages.
    pub fn display_name(&self) -> String {
        match self {
            IntegerType::Arbitrary => "integer".to_string(),
            IntegerType::I(n) => format!("i{n}"),
            IntegerType::U(n) => format!("u{n}"),
        }
    }
}

impl Default for IntegerType {
    fn default() -> Self {
        IntegerType::DEFAULT
    }
}

/// Numeric type annotation for floats (§4.3).
///
/// Only the five spec-defined types are accepted: f16, f32, f64, f80, f128.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatType {
    F16,
    F32,
    F64,
    F80,
    F128,
}

impl FloatType {
    pub const DEFAULT: FloatType = FloatType::F64;

    pub fn is_default(&self) -> bool {
        matches!(self, FloatType::F64)
    }

    pub fn from_type_name(name: &str) -> Option<FloatType> {
        match name {
            "f16" => Some(FloatType::F16),
            "f32" => Some(FloatType::F32),
            "f64" => Some(FloatType::F64),
            "f80" => Some(FloatType::F80),
            "f128" => Some(FloatType::F128),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            FloatType::F16 => "f16",
            FloatType::F32 => "f32",
            FloatType::F64 => "f64",
            FloatType::F80 => "f80",
            FloatType::F128 => "f128",
        }
    }
}

impl Default for FloatType {
    fn default() -> Self {
        FloatType::DEFAULT
    }
}

// ============================================================
// UzonInteger — typed integer wrapper with checked arithmetic
// ============================================================

/// A UZON integer value with its type annotation.
///
/// Implements the type adoption rule (Appendix D.3): when two operands with different
/// type annotations are combined, the untyped operand adopts the typed one's annotation.
/// Two differently-typed operands are an error.
#[derive(Debug, Clone)]
pub struct UzonInteger {
    pub value: i128,
    pub type_ann: IntegerType,
    pub explicit: bool,
}

impl UzonInteger {
    pub fn new(value: i128) -> Self {
        Self {
            value,
            type_ann: IntegerType::DEFAULT,
            explicit: false,
        }
    }

    pub fn with_type(value: i128, type_ann: IntegerType) -> Self {
        Self {
            value,
            type_ann,
            explicit: true,
        }
    }

    pub fn validate_range(&self) -> Result<(), String> {
        let type_name = self.type_ann.display_name();
        let (min, max) = self.type_ann.range().unwrap_or((i128::MIN, i128::MAX));
        if self.value < min || self.value > max {
            Err(format!(
                "{} does not fit in {type_name} (range {min}..{max})",
                self.value
            ))
        } else {
            Ok(())
        }
    }

    /// Type adoption rule (Appendix D.3).
    pub fn adopt_type(a: &IntegerType, b: &IntegerType) -> Result<IntegerType, String> {
        if a.is_default() && b.is_default() {
            Ok(IntegerType::DEFAULT)
        } else if a.is_default() {
            Ok(*b)
        } else if b.is_default() {
            Ok(*a)
        } else if a == b {
            Ok(*a)
        } else {
            Err(format!(
                "cannot combine {} and {} operands; use explicit type annotation",
                a.display_name(),
                b.display_name()
            ))
        }
    }

    pub fn checked_add(&self, other: &Self) -> Result<Self, String> {
        let type_ann = Self::adopt_type(&self.type_ann, &other.type_ann)?;
        let explicit = self.explicit || other.explicit;
        let value = self
            .value
            .checked_add(other.value)
            .ok_or_else(|| "integer overflow".to_string())?;
        let result = Self { value, type_ann, explicit };
        result.validate_range()?;
        Ok(result)
    }

    pub fn checked_sub(&self, other: &Self) -> Result<Self, String> {
        let type_ann = Self::adopt_type(&self.type_ann, &other.type_ann)?;
        let explicit = self.explicit || other.explicit;
        let value = self
            .value
            .checked_sub(other.value)
            .ok_or_else(|| "integer overflow".to_string())?;
        let result = Self { value, type_ann, explicit };
        result.validate_range()?;
        Ok(result)
    }

    pub fn checked_mul(&self, other: &Self) -> Result<Self, String> {
        let type_ann = Self::adopt_type(&self.type_ann, &other.type_ann)?;
        let explicit = self.explicit || other.explicit;
        let value = self
            .value
            .checked_mul(other.value)
            .ok_or_else(|| "integer overflow".to_string())?;
        let result = Self { value, type_ann, explicit };
        result.validate_range()?;
        Ok(result)
    }

    pub fn checked_div(&self, other: &Self) -> Result<Self, String> {
        if other.value == 0 {
            return Err("division by zero".to_string());
        }
        let type_ann = Self::adopt_type(&self.type_ann, &other.type_ann)?;
        let explicit = self.explicit || other.explicit;
        let value = self
            .value
            .checked_div(other.value)
            .ok_or_else(|| "integer overflow".to_string())?;
        let result = Self { value, type_ann, explicit };
        result.validate_range()?;
        Ok(result)
    }

    pub fn checked_rem(&self, other: &Self) -> Result<Self, String> {
        if other.value == 0 {
            return Err("modulo by zero".to_string());
        }
        let type_ann = Self::adopt_type(&self.type_ann, &other.type_ann)?;
        let explicit = self.explicit || other.explicit;
        let value = self.value % other.value;
        let result = Self { value, type_ann, explicit };
        result.validate_range()?;
        Ok(result)
    }

    pub fn checked_pow(&self, exp: &Self) -> Result<Self, String> {
        if exp.value < 0 {
            return Err("integer exponent must be non-negative".to_string());
        }
        let type_ann = Self::adopt_type(&self.type_ann, &exp.type_ann)?;
        let explicit = self.explicit || exp.explicit;
        let mut result: i128 = 1;
        for _ in 0..exp.value {
            result = result
                .checked_mul(self.value)
                .ok_or_else(|| "integer overflow".to_string())?;
        }
        let result = Self { value: result, type_ann, explicit };
        result.validate_range()?;
        Ok(result)
    }

    pub fn checked_neg(&self) -> Result<Self, String> {
        if let IntegerType::U(_) = &self.type_ann {
            return Err(format!(
                "cannot negate unsigned integer ({})",
                self.type_ann.display_name()
            ));
        }
        let value = self
            .value
            .checked_neg()
            .ok_or_else(|| "integer overflow".to_string())?;
        let result = Self {
            value,
            type_ann: self.type_ann,
            explicit: self.explicit,
        };
        result.validate_range()?;
        Ok(result)
    }
}

impl PartialEq for UzonInteger {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value // type annotation is metadata, not identity
    }
}

impl Eq for UzonInteger {}

impl fmt::Display for UzonInteger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

// ============================================================
// UzonFloat — typed float wrapper
// ============================================================

/// A UZON float value with its type annotation.
#[derive(Debug, Clone)]
pub struct UzonFloat {
    pub value: f64,
    pub type_ann: FloatType,
    pub explicit: bool,
}

impl UzonFloat {
    pub fn new(value: f64) -> Self {
        Self {
            value,
            type_ann: FloatType::DEFAULT,
            explicit: false,
        }
    }

    pub fn with_type(value: f64, type_ann: FloatType) -> Self {
        Self {
            value,
            type_ann,
            explicit: true,
        }
    }

    pub fn adopt_type(a: &FloatType, b: &FloatType) -> Result<FloatType, String> {
        if a.is_default() && b.is_default() {
            Ok(FloatType::DEFAULT)
        } else if a.is_default() {
            Ok(*b)
        } else if b.is_default() {
            Ok(*a)
        } else if a == b {
            Ok(*a)
        } else {
            Err(format!(
                "cannot combine {} and {} operands; use explicit type annotation",
                a.display_name(),
                b.display_name()
            ))
        }
    }

    pub fn add(&self, other: &Self) -> Result<Self, String> {
        let type_ann = Self::adopt_type(&self.type_ann, &other.type_ann)?;
        let explicit = self.explicit || other.explicit;
        Ok(Self { value: self.value + other.value, type_ann, explicit })
    }

    pub fn sub(&self, other: &Self) -> Result<Self, String> {
        let type_ann = Self::adopt_type(&self.type_ann, &other.type_ann)?;
        let explicit = self.explicit || other.explicit;
        Ok(Self { value: self.value - other.value, type_ann, explicit })
    }

    pub fn mul(&self, other: &Self) -> Result<Self, String> {
        let type_ann = Self::adopt_type(&self.type_ann, &other.type_ann)?;
        let explicit = self.explicit || other.explicit;
        Ok(Self { value: self.value * other.value, type_ann, explicit })
    }

    pub fn div(&self, other: &Self) -> Result<Self, String> {
        let type_ann = Self::adopt_type(&self.type_ann, &other.type_ann)?;
        let explicit = self.explicit || other.explicit;
        Ok(Self { value: self.value / other.value, type_ann, explicit }) // IEEE 754
    }

    pub fn rem(&self, other: &Self) -> Result<Self, String> {
        let type_ann = Self::adopt_type(&self.type_ann, &other.type_ann)?;
        let explicit = self.explicit || other.explicit;
        Ok(Self { value: self.value % other.value, type_ann, explicit })
    }

    pub fn powf(&self, other: &Self) -> Result<Self, String> {
        let type_ann = Self::adopt_type(&self.type_ann, &other.type_ann)?;
        let explicit = self.explicit || other.explicit;
        // §5.3: a negative base with a non-integer exponent would yield a
        // complex number, which UZON does not support — runtime error.
        if self.value < 0.0 && other.value.is_finite() && other.value.fract() != 0.0 {
            return Err(format!(
                "negative base ({}) with fractional exponent ({}) has no real result",
                self.value, other.value
            ));
        }
        Ok(Self { value: self.value.powf(other.value), type_ann, explicit })
    }

    pub fn neg(&self) -> Self {
        Self {
            value: -self.value,
            type_ann: self.type_ann,
            explicit: self.explicit,
        }
    }
}

impl PartialEq for UzonFloat {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value // NaN semantics preserved from f64
    }
}

impl fmt::Display for UzonFloat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format_float(self.value))
    }
}
