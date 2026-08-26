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
