mod effective;
mod load;
mod options;

pub use effective::{EffectiveConfig, effective_config};
pub use load::{
    ConfigError, Format, extend_config, parse_config_as, parse_config_str, read_config_file,
};
pub use options::{GitIgnore, OPTIONS_KEYS, Options, merge_options, options_from_value};

use crate::parser::is_js_whitespace;

pub type ConfigValue = serde_json::Value;

/// JS 의 truthiness.
pub fn truthy(value: &ConfigValue) -> bool {
    match value {
        ConfigValue::Null => false,
        ConfigValue::Bool(b) => *b,
        ConfigValue::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        ConfigValue::String(s) => !s.is_empty(),
        ConfigValue::Array(_) | ConfigValue::Object(_) => true,
    }
}

/// JS `String(value)` 상당의 표기.
pub fn js_string(value: &ConfigValue) -> String {
    match value {
        ConfigValue::String(s) => s.clone(),
        ConfigValue::Null => "null".to_string(),
        ConfigValue::Bool(value) => value.to_string(),
        ConfigValue::Number(value) => js_number_string(value.as_f64().unwrap_or(f64::NAN)),
        ConfigValue::Array(values) => values
            .iter()
            .map(|value| match value {
                // Array.prototype.toString() 은 null/undefined 원소를 빈 문자열로 연결한다.
                ConfigValue::Null => String::new(),
                other => js_string(other),
            })
            .collect::<Vec<_>>()
            .join(","),
        ConfigValue::Object(_) => "[object Object]".to_string(),
    }
}

/// JS `Number(value)` 상당의 변환. 변환 불가는 NaN.
pub fn to_number(value: &ConfigValue) -> f64 {
    match value {
        ConfigValue::Null => 0.0,
        ConfigValue::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        ConfigValue::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        ConfigValue::String(s) => js_string_to_number(s),
        ConfigValue::Array(values) => match values.as_slice() {
            [] => 0.0,
            [value] => to_number(&ConfigValue::String(match value {
                ConfigValue::Null => String::new(),
                other => js_string(other),
            })),
            _ => f64::NAN,
        },
        ConfigValue::Object(_) => f64::NAN,
    }
}

/// ECMAScript `StringToNumber`: JS 공백을 떼고 빈 문자열은 0, `Infinity`(부호 허용), `0x`/`0o`/`0b`
/// 접두 정수(부호 불허), 십진 리터럴만 받는다. Rust `f64::from_str` 가 받는 `inf`/`nan` 은 NaN 이다.
fn js_string_to_number(s: &str) -> f64 {
    let s = s.trim_matches(is_js_whitespace);
    if s.is_empty() {
        return 0.0;
    }
    match s {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    let radix_digits = |prefixes: [&str; 2], radix: u32| {
        let digits = prefixes.iter().find_map(|prefix| s.strip_prefix(prefix))?;
        if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
            return Some(f64::NAN);
        }
        Some(digits.chars().fold(0.0, |acc, c| {
            acc * f64::from(radix) + f64::from(c.to_digit(radix).expect("radix digit"))
        }))
    };
    if let Some(value) = radix_digits(["0x", "0X"], 16)
        .or_else(|| radix_digits(["0o", "0O"], 8))
        .or_else(|| radix_digits(["0b", "0B"], 2))
    {
        return value;
    }
    if is_js_decimal_literal(s) {
        s.parse::<f64>().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

/// `StrDecimalLiteral`: `[+-]? (digits [. digits?]? | . digits) ([eE] [+-]? digits)?`.
fn is_js_decimal_literal(s: &str) -> bool {
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    let (mantissa, exponent) = match s.split_once(['e', 'E']) {
        Some((mantissa, exponent)) => (mantissa, Some(exponent)),
        None => (s, None),
    };
    let (int_part, frac_part) = match mantissa.split_once('.') {
        Some((int_part, frac_part)) => (int_part, Some(frac_part)),
        None => (mantissa, None),
    };
    let all_digits = |part: &str| part.chars().all(|c| c.is_ascii_digit());
    let mantissa_ok = all_digits(int_part)
        && frac_part.is_none_or(all_digits)
        && (!int_part.is_empty() || frac_part.is_some_and(|part| !part.is_empty()));
    let exponent_ok = exponent.is_none_or(|exponent| {
        let digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        !digits.is_empty() && all_digits(digits)
    });
    mantissa_ok && exponent_ok
}

/// ECMAScript `Number::toString(10)`: 최단 왕복 자릿수를 쓰고, 절댓값이 1e21 이상이거나 1e-6 미만이면
/// `1e+21`, `1e-7` 같은 지수 표기가 된다. `-0` 은 `0`.
pub fn js_number_string(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value == 0.0 {
        return "0".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    // `{:e}` 는 최단 왕복 자릿수와 지수를 준다 ("1.2345e20").
    let scientific = format!("{:e}", value.abs());
    let (mantissa, exponent) = scientific.split_once('e').expect("exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i64;
    let n = exponent.parse::<i64>().expect("exponent") + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let sign = if n - 1 < 0 { "-" } else { "+" };
        let magnitude = (n - 1).abs();
        if k == 1 {
            format!("{digits}e{sign}{magnitude}")
        } else {
            format!("{}.{}e{sign}{magnitude}", &digits[..1], &digits[1..])
        }
    };
    if value < 0.0 {
        format!("-{body}")
    } else {
        body
    }
}

/// V8 `String::kMaxLength` (64비트). 이보다 긴 문자열을 만들면 `RangeError: Invalid string length`.
const JS_MAX_STRING_LENGTH: f64 = 536_870_888.0;

/// JS `"".padEnd(width)`: `ToLength` 로 자르고(NaN/음수는 0, Infinity 는 2^53-1) 한도를 넘으면 RangeError.
pub fn js_pad_end(width: f64) -> Result<String, String> {
    let width = if width.is_nan() {
        0.0
    } else {
        width.trunc().clamp(0.0, 9_007_199_254_740_991.0)
    };
    if width > JS_MAX_STRING_LENGTH {
        return Err("Invalid string length".to_string());
    }
    Ok(" ".repeat(width as usize))
}

/// JS `text.repeat(count)`: 음수와 Infinity 는 `Invalid count value`, 결과가 한도를 넘으면 `Invalid string length`.
pub fn js_repeat(text: &str, count: f64) -> Result<String, String> {
    let count = if count.is_nan() { 0.0 } else { count.trunc() };
    if count < 0.0 || count == f64::INFINITY {
        return Err(format!("Invalid count value: {}", js_number_string(count)));
    }
    if text.encode_utf16().count() as f64 * count > JS_MAX_STRING_LENGTH {
        return Err("Invalid string length".to_string());
    }
    Ok(text.repeat(count as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_string_matches_javascript_number_to_string() {
        for (value, expected) in [
            (7.0, "7"),
            (1.5, "1.5"),
            (-0.0, "0"),
            (1e21, "1e+21"),
            (1.5e21, "1.5e+21"),
            (1e20, "100000000000000000000"),
            (1e-7, "1e-7"),
            (0.000001, "0.000001"),
            (0.1 + 0.2, "0.30000000000000004"),
            (f64::INFINITY, "Infinity"),
            (f64::NEG_INFINITY, "-Infinity"),
            (f64::NAN, "NaN"),
        ] {
            assert_eq!(js_number_string(value), expected);
        }
        assert_eq!(js_string(&serde_json::json!(7.0)), "7");
    }

    #[test]
    fn string_to_number_uses_javascript_grammar() {
        assert_eq!(js_string_to_number("0x4"), 4.0);
        assert_eq!(js_string_to_number("0b101"), 5.0);
        assert_eq!(js_string_to_number(" 12\u{feff}"), 12.0);
        assert_eq!(js_string_to_number("+.5e-2"), 0.005);
        assert_eq!(js_string_to_number("-Infinity"), f64::NEG_INFINITY);
        for invalid in ["inf", "nan", "+0x4", "1e", "1_0", "12abc", "."] {
            assert!(js_string_to_number(invalid).is_nan(), "{invalid}");
        }
    }

    #[test]
    fn pad_end_and_repeat_follow_v8_limits() {
        assert_eq!(js_pad_end(f64::NAN).unwrap(), "");
        assert_eq!(js_pad_end(-3.0).unwrap(), "");
        assert_eq!(js_pad_end(2.9).unwrap(), "  ");
        assert_eq!(js_pad_end(1e15).unwrap_err(), "Invalid string length");
        assert_eq!(
            js_pad_end(f64::INFINITY).unwrap_err(),
            "Invalid string length"
        );
        assert_eq!(js_repeat("\n", 2.0).unwrap(), "\n\n");
        assert_eq!(js_repeat("\n", 1e9).unwrap_err(), "Invalid string length");
        assert_eq!(js_repeat("", 1e15).unwrap(), "");
        assert_eq!(
            js_repeat("\n", f64::INFINITY).unwrap_err(),
            "Invalid count value: Infinity"
        );
    }
}
