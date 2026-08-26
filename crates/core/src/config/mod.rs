mod effective;
mod load;
mod options;

pub use effective::{EffectiveConfig, effective_config};
pub use load::{
    ConfigError, Format, extend_config, parse_config_as, parse_config_str, read_config_file,
};
pub use options::{GitIgnore, OPTIONS_KEYS, Options, merge_options, options_from_value};

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
        other => other.to_string(),
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
        ConfigValue::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                0.0
            } else {
                trimmed.parse::<f64>().unwrap_or(f64::NAN)
            }
        }
        _ => f64::NAN,
    }
}
