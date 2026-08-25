use std::path::Path;

use super::{ConfigValue, truthy};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Parse(String),
}

/// markdownlint `parse-configuration.mjs` + cli2 `parsers/parsers.mjs`.
/// 파서 우선순위는 원본 그대로 jsonc, toml, yaml 순.
fn parse_configuration(name: &str, content: &str) -> Result<ConfigValue, ConfigError> {
    let mut errors: Vec<String> = Vec::new();

    // Parser 0: jsonc (cli2 jsonc-parse.mjs: 주석과 trailing comma 만 허용)
    let jsonc_options = jsonc_parser::ParseOptions {
        allow_comments: true,
        allow_trailing_commas: true,
        allow_loose_object_property_names: false,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
    };
    match jsonc_parser::parse_to_serde_value::<ConfigValue>(content, &jsonc_options) {
        Ok(value) => return Ok(coerce_to_object(value)),
        Err(e) => errors.push(format!("Parser 0: {e}")),
    }

    // Parser 1: toml
    match toml::from_str::<toml::Value>(content).map_err(ConfigError::from_toml) {
        Ok(value) => match serde_json::to_value(value) {
            Ok(value) => return Ok(coerce_to_object(value)),
            Err(e) => errors.push(format!("Parser 1: {e}")),
        },
        Err(e) => errors.push(format!("Parser 1: {e}")),
    }

    // Parser 2: yaml
    match serde_saphyr::from_str::<ConfigValue>(content) {
        Ok(value) => return Ok(coerce_to_object(value)),
        Err(e) => errors.push(format!("Parser 2: {e}")),
    }

    Err(ConfigError::Parse(format!(
        "Unable to parse '{name}'; {}",
        errors.join("; ")
    )))
}

impl ConfigError {
    fn from_toml(e: toml::de::Error) -> ConfigError {
        ConfigError::Parse(e.message().to_string())
    }
}

/// parseConfiguration: 객체가 아닌 결과는 `{}` 로 강제.
fn coerce_to_object(value: ConfigValue) -> ConfigValue {
    if value.is_object() {
        value
    } else {
        ConfigValue::Object(serde_json::Map::new())
    }
}

/// helpers `expandTildePath`.
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return format!("{home}/{rest}");
    }
    path.to_string()
}

/// markdownlint `extendConfig`: `extends` 를 설정 파일 기준 상대 경로로 재귀 해석하고
/// `{...extendsConfig, ...config}` 얕은 병합 후 `extends` 를 제거한다.
fn extend_config(config: ConfigValue, file: &Path) -> Result<ConfigValue, ConfigError> {
    let extends = config
        .get("extends")
        .filter(|v| truthy(v))
        .and_then(|v| v.as_str())
        .map(expand_tilde);
    let Some(extends) = extends else {
        return Ok(config);
    };
    let resolved = file.parent().unwrap_or(Path::new("")).join(&extends);
    let extends_config = read_config_file(&resolved)?;

    let mut merged = match extends_config {
        ConfigValue::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    if let ConfigValue::Object(config) = config {
        for (key, value) in config {
            merged.insert(key, value);
        }
    }
    let merged: serde_json::Map<String, ConfigValue> = merged
        .into_iter()
        .filter(|(key, _)| key != "extends")
        .collect();
    Ok(ConfigValue::Object(merged))
}

/// markdownlint `readConfig`.
pub fn read_config_file(path: &Path) -> Result<ConfigValue, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let config = parse_configuration(&path.to_string_lossy(), &content)?;
    extend_config(config, path)
}

/// 인라인 `markdownlint-configure-file` 주석 등 파일이 아닌 설정 문자열 파싱.
pub fn parse_config_str(s: &str) -> Result<ConfigValue, ConfigError> {
    parse_configuration("content", s)
}
