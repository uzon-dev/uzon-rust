// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::fmt;

use crate::ast::{Binding, FunctionParam, Node, TypeExpr};
use crate::scope::TypeDef;

/// The sentinel for the UZON `undefined` state (§3.1).
/// Unlike `null`, `undefined` means "does not exist" rather than "intentionally empty."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UzonUndefined;

impl fmt::Display for UzonUndefined {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "undefined")
    }
}

/// A UZON enum value: a selected variant from a set of possible variants (§3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UzonEnum {
    pub value: String,
    pub variants: Vec<String>,
    pub type_name: Option<String>,
}

impl UzonEnum {
    pub fn new(value: impl Into<String>, variants: Vec<String>, type_name: Option<String>) -> Self {
        Self {
            value: value.into(),
            variants,
            type_name,
        }
    }
}

impl fmt::Display for UzonEnum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

/// A UZON untagged union value: a value whose type is one of several possible types (§3.6).
#[derive(Debug, Clone, PartialEq)]
pub struct UzonUnion {
    pub value: Box<Value>,
    pub types: Vec<String>,
    pub type_name: Option<String>,
}

impl UzonUnion {
    pub fn new(value: Value, types: Vec<String>, type_name: Option<String>) -> Self {
        Self {
            value: Box::new(value),
            types,
            type_name,
        }
    }
}

/// A UZON tagged union value: a value with an explicit variant tag (§3.7).
#[derive(Debug, Clone, PartialEq)]
pub struct UzonTaggedUnion {
    pub value: Box<Value>,
    pub tag: String,
    pub variants: BTreeMap<String, Option<String>>,
    pub type_name: Option<String>,
}

impl UzonTaggedUnion {
    pub fn new(
        value: Value,
        tag: impl Into<String>,
        variants: BTreeMap<String, Option<String>>,
        type_name: Option<String>,
    ) -> Self {
        Self {
            value: Box::new(value),
            tag: tag.into(),
            variants,
            type_name,
        }
    }
}

/// A UZON tuple: a fixed-length, heterogeneous sequence (§3.3).
#[derive(Debug, Clone, PartialEq)]
pub struct UzonTuple {
    pub elements: Vec<Value>,
}

impl UzonTuple {
    pub fn new(elements: Vec<Value>) -> Self {
        Self { elements }
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}

/// A UZON function value (closure) (§3.8).
#[derive(Debug, Clone)]
pub struct UzonFunction {
    pub params: Vec<FunctionParam>,
    pub return_type: TypeExpr,
    pub body_bindings: Vec<Binding>,
    pub body_expr: Node,
    pub captured_bindings: BTreeMap<String, Value>,
    pub captured_types: BTreeMap<String, TypeDef>,
    /// Named type assigned via `called` (nominal type identity).
    pub type_name: Option<String>,
}

impl PartialEq for UzonFunction {
    fn eq(&self, _other: &Self) -> bool {
        false // function equality is a type error per §5.2; should never be reached
    }
}

impl fmt::Display for UzonFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<function>")
    }
}

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
            IntegerType::I(0) | IntegerType::U(0) => None,
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
        match self.type_ann.range() {
            None => Err(format!(
                "{} does not fit in {type_name} (no valid values for zero-bit type)",
                self.value
            )),
            Some((min, max)) => {
                if self.value < min || self.value > max {
                    Err(format!(
                        "{} does not fit in {type_name} (range {min}..{max})",
                        self.value
                    ))
                } else {
                    Ok(())
                }
            }
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

// ============================================================
// Value enum
// ============================================================

/// A UZON value — the core runtime representation.
///
/// Preserves full UZON type information including enums, unions, tagged unions,
/// tuples, and functions. This is the "UZON-native" value type.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Undefined,
    Bool(bool),
    Integer(UzonInteger),
    BigInteger(num_bigint::BigInt),
    Float(UzonFloat),
    String(String),
    List(Vec<Value>),
    Tuple(UzonTuple),
    Struct(BTreeMap<String, Value>),
    Enum(UzonEnum),
    Union(UzonUnion),
    TaggedUnion(UzonTaggedUnion),
    Function(UzonFunction),
}

impl Value {
    /// Convenience: create an integer with default type (i64).
    pub fn int(v: i128) -> Self {
        Value::Integer(UzonInteger::new(v))
    }

    /// Convenience: create a float with default type (f64).
    pub fn float(v: f64) -> Self {
        Value::Float(UzonFloat::new(v))
    }

    pub fn is_undefined(&self) -> bool {
        matches!(self, Value::Undefined)
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Convert to a plain Rust-friendly representation, stripping UZON-specific wrappers.
    pub fn to_plain(self) -> Value {
        match self {
            Value::Enum(e) => Value::String(e.value),
            Value::Union(u) => u.value.to_plain(),
            Value::TaggedUnion(tu) => tu.value.to_plain(),
            Value::Function(_) => self,
            Value::Tuple(t) => {
                Value::List(t.elements.into_iter().map(|v| v.to_plain()).collect())
            }
            Value::List(items) => {
                Value::List(items.into_iter().map(|v| v.to_plain()).collect())
            }
            Value::Struct(fields) => Value::Struct(
                fields
                    .into_iter()
                    .map(|(k, v)| (k, v.to_plain()))
                    .collect(),
            ),
            other => other,
        }
    }

    /// Returns the UZON type category name for error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Undefined => "undefined",
            Value::Bool(_) => "bool",
            Value::Integer(_) | Value::BigInteger(_) => "integer",
            Value::Float(_) => "float",
            Value::String(_) => "string",
            Value::List(_) => "list",
            Value::Tuple(_) => "tuple",
            Value::Struct(_) => "struct",
            Value::Enum(_) => "enum",
            Value::Union(_) => "union",
            Value::TaggedUnion(_) => "tagged union",
            Value::Function(_) => "function",
        }
    }
}

/// Format a float according to UZON spec §5.11.2.
///
/// Rules:
/// - Shortest round-trip representation.
/// - If 0 < n <= 21: plain decimal notation.
/// - If -6 < n <= 0: plain decimal with leading zeros.
/// - Otherwise: scientific notation with one digit before the decimal point.
/// - Result MUST always contain a decimal point.
/// - NaN always stringifies as "nan" (§5.2, -nan is semantically identical).
pub fn format_float(v: f64) -> String {
    if v.is_nan() {
        return "nan".to_string();
    }
    if v.is_infinite() {
        return if v.is_sign_negative() { "-inf" } else { "inf" }.to_string();
    }

    if v == 0.0 {
        return if v.is_sign_negative() { "-0.0" } else { "0.0" }.to_string();
    }

    let abs_val = v.abs();
    let n = abs_val.log10().floor() as i32 + 1;
    let sign = if v < 0.0 { "-" } else { "" };

    let repr = format!("{v:?}");

    if (1..=21).contains(&n) {
        if repr.contains('e') || repr.contains('E') {
            let stripped = repr.trim_start_matches('-');
            let (mantissa_str, exp) = parse_scientific(stripped);
            let digits = mantissa_str.replace('.', "");
            let dot_pos = if mantissa_str.contains('.') {
                mantissa_str.find('.').unwrap()
            } else {
                mantissa_str.len()
            } as i32
                + exp;
            let dot_pos = dot_pos as usize;

            let s = if dot_pos >= digits.len() {
                format!("{}{}.0", &digits, "0".repeat(dot_pos - digits.len()))
            } else {
                let mut s = format!("{}.{}", &digits[..dot_pos], &digits[dot_pos..]);
                while s.ends_with('0') && !s.ends_with(".0") {
                    s.pop();
                }
                s
            };
            format!("{sign}{s}")
        } else if repr.contains('.') {
            repr
        } else {
            format!("{repr}.0")
        }
    } else if (-5..=0).contains(&n) {
        if repr.contains('e') || repr.contains('E') {
            let stripped = repr.trim_start_matches('-');
            let (mantissa_str, exp) = parse_scientific(stripped);
            let digits = mantissa_str.replace('.', "");
            let frac_offset = if mantissa_str.contains('.') {
                mantissa_str.find('.').unwrap()
            } else {
                mantissa_str.len()
            } as i32
                + exp;

            let leading_zeros = (-frac_offset) as usize;
            let mut s = format!("0.{}{}", "0".repeat(leading_zeros), digits);
            while s.ends_with('0') && !s.ends_with(".0") {
                s.pop();
            }
            format!("{sign}{s}")
        } else if repr.contains('.') {
            repr
        } else {
            format!("{repr}.0")
        }
    } else {
        if repr.contains('e') || repr.contains('E') {
            let stripped = repr.trim_start_matches('-');
            let (mantissa, exp) = parse_scientific(stripped);
            let mantissa = if mantissa.contains('.') {
                mantissa
            } else {
                format!("{mantissa}.0")
            };
            format!("{sign}{mantissa}e{exp}")
        } else {
            let exp = n - 1;
            let shifted = abs_val / 10.0_f64.powi(exp);
            let mantissa = format!("{shifted:?}");
            let mantissa = if mantissa.contains('.') {
                mantissa
            } else {
                format!("{mantissa}.0")
            };
            format!("{sign}{mantissa}e{exp}")
        }
    }
}

/// Parse scientific notation string like "1.5e30" into ("1.5", 30).
fn parse_scientific(s: &str) -> (String, i32) {
    let lower = s.to_lowercase();
    if let Some(e_pos) = lower.find('e') {
        let mantissa = lower[..e_pos].to_string();
        let exp: i32 = lower[e_pos + 1..].parse().unwrap_or(0);
        (mantissa, exp)
    } else {
        (lower, 0)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Undefined => write!(f, "undefined"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Integer(n) => write!(f, "{}", n.value),
            Value::BigInteger(n) => write!(f, "{n}"),
            Value::Float(v) => write!(f, "{}", format_float(v.value)),
            Value::String(s) => write!(f, "{s}"),
            Value::Enum(e) => write!(f, "{}", e.value),
            Value::Union(u) => write!(f, "{}", u.value),
            Value::TaggedUnion(tu) => write!(f, "{}", tu.value),
            Value::Function(_) => write!(f, "<function>"),
            Value::List(_) | Value::Tuple(_) | Value::Struct(_) => {
                write!(f, "[compound]")
            }
        }
    }
}
