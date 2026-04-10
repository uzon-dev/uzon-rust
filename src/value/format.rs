// SPDX-FileCopyrightText: © 2026 Suho Kang
// SPDX-License-Identifier: MIT

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
pub(crate) fn parse_scientific(s: &str) -> (String, i32) {
    let lower = s.to_lowercase();
    if let Some(e_pos) = lower.find('e') {
        let mantissa = lower[..e_pos].to_string();
        let exp: i32 = lower[e_pos + 1..].parse().unwrap_or(0);
        (mantissa, exp)
    } else {
        (lower, 0)
    }
}
