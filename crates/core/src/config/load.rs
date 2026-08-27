use std::path::Path;

use super::{ConfigValue, truthy};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Parse(String),
}

/// cli2 `parsers/*.mjs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Jsonc,
    Toml,
    Yaml,
}

/// 파서 하나로 파싱. 결과를 객체로 강제하지 않는다.
pub fn parse_config_as(format: Format, content: &str) -> Result<ConfigValue, ConfigError> {
    match format {
        // cli2 jsonc-parse.mjs: 주석과 trailing comma 만 허용
        Format::Jsonc => {
            let jsonc_options = jsonc_parser::ParseOptions {
                allow_comments: true,
                allow_trailing_commas: true,
                allow_loose_object_property_names: false,
                allow_missing_commas: false,
                allow_single_quoted_strings: false,
                allow_hexadecimal_numbers: false,
                allow_unary_plus_numbers: false,
            };
            jsonc_parser::parse_to_serde_value::<ConfigValue>(content, &jsonc_options)
                .map_err(|e| ConfigError::Parse(format!("Unable to parse JSONC content, {e}")))
        }
        Format::Toml => toml::from_str::<toml::Value>(content)
            .map_err(ConfigError::from_toml)
            .and_then(|value| {
                serde_json::to_value(value).map_err(|e| ConfigError::Parse(e.to_string()))
            }),
        Format::Yaml => {
            check_flow_collections_like_js_yaml(content)?;
            serde_saphyr::from_str::<ConfigValue>(content)
                .map_err(|e| ConfigError::Parse(yaml_message(&e.to_string())))
        }
    }
}

/// js-yaml `readFlowCollection` 은 YAML 명세보다 엄격하다. flow collection 안에서 (1) 암시적 키의 `:` 가
/// 키가 시작한 줄에 없거나 (2) 여러 줄 plain scalar 의 이어지는 줄이 flow collection 의 들여쓰기
/// (감싸는 블록 컬렉션 들여쓰기 + 1, 최상위는 문서 첫 내용 줄의 들여쓰기)보다 얕으면
/// `missed comma between flow collection entries` 를 낸다. serde-saphyr 는 둘 다 받아들이므로
/// 스캐너 토큰을 훑어 같은 경우에 같은 오류를 낸다. 스캔 오류는 serde-saphyr 에 맡긴다.
fn check_flow_collections_like_js_yaml(content: &str) -> Result<(), ConfigError> {
    use serde_saphyr::granit_parser::{ScalarStyle, Scanner, StrInput, TokenType};

    // js-yaml `lineIndent`: 앞선 공백(space)만 센다. 빈 줄은 None.
    let line_indent = |line: usize| {
        let text = content.lines().nth(line - 1)?;
        let rest = text.trim_start_matches(' ');
        (!rest.is_empty()).then_some(text.len() - rest.len())
    };
    let missed_comma = |line: usize, col: usize| {
        Err(ConfigError::Parse(format!(
            "missed comma between flow collection entries ({line}:{col})"
        )))
    };

    let mut block_indents: Vec<usize> = Vec::new();
    let mut flow_depth = 0usize;
    let mut flow_indent = 0usize;
    let mut document_line: Option<usize> = None;
    let mut pending_key: Option<(usize, bool)> = None;

    for token in Scanner::new(StrInput::new(content)) {
        let Ok(token) = token else { break };
        let (span, kind) = token.into_parts();
        let (line, col) = (span.start.line(), span.start.col());
        match &kind {
            TokenType::StreamStart | TokenType::DocumentStart | TokenType::DocumentEnd => {
                document_line = None;
                continue;
            }
            TokenType::Comment(_) => continue,
            _ => {
                document_line.get_or_insert(line);
            }
        }
        match kind {
            TokenType::BlockMappingStart | TokenType::BlockSequenceStart => block_indents.push(col),
            TokenType::BlockEnd => {
                block_indents.pop();
            }
            TokenType::FlowMappingStart | TokenType::FlowSequenceStart => {
                if flow_depth == 0 {
                    flow_indent = match block_indents.last() {
                        Some(indent) => indent + 1,
                        None => document_line.and_then(line_indent).unwrap_or(0),
                    };
                }
                flow_depth += 1;
                pending_key = None;
            }
            TokenType::FlowMappingEnd | TokenType::FlowSequenceEnd => {
                flow_depth = flow_depth.saturating_sub(1);
                pending_key = None;
            }
            TokenType::FlowEntry => pending_key = None,
            // 명시적 `?` 키는 토큰이 `?` 를 덮고, 암시적 키는 빈 span 이다.
            TokenType::Key if flow_depth > 0 => pending_key = Some((line, !span.is_empty())),
            TokenType::Value if flow_depth > 0 => {
                if let Some((key_line, false)) = pending_key.take()
                    && key_line != line
                {
                    return missed_comma(line, col + 1);
                }
            }
            TokenType::Scalar(ScalarStyle::Plain, _) if flow_depth > 0 => {
                for continued in line + 1..=span.end.line() {
                    if let Some(indent) = line_indent(continued)
                        && indent < flow_indent
                    {
                        return missed_comma(continued, indent + 1);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// 원본 파서의 오류 문구를 앞에 붙인다. cli2 는 jsonc-parser 의 `Unable to parse JSONC content, ...`,
/// smol-toml 의 `Invalid TOML document: ...`, js-yaml 의 `duplicated mapping key` 를 그대로 내보내며
/// 원본 테스트는 이 문구로 오류를 식별한다.
fn yaml_message(message: &str) -> String {
    if message.contains("duplicate mapping key") {
        format!("duplicated mapping key ({message})")
    } else {
        message.to_string()
    }
}

/// markdownlint `parse-configuration.mjs` + cli2 `parsers/parsers.mjs`.
/// 파서 우선순위는 원본 그대로 jsonc, toml, yaml 순.
fn parse_configuration(name: &str, content: &str) -> Result<ConfigValue, ConfigError> {
    let mut errors: Vec<String> = Vec::new();
    for (index, format) in [Format::Jsonc, Format::Toml, Format::Yaml]
        .into_iter()
        .enumerate()
    {
        match parse_config_as(format, content) {
            Ok(value) => return Ok(coerce_to_object(value)),
            Err(e) => errors.push(format!("Parser {index}: {e}")),
        }
    }
    Err(ConfigError::Parse(format!(
        "Unable to parse '{name}'; {}",
        errors.join("; ")
    )))
}

impl ConfigError {
    fn from_toml(e: toml::de::Error) -> ConfigError {
        ConfigError::Parse(format!("Invalid TOML document: {}", e.message()))
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
pub fn extend_config(config: ConfigValue, file: &Path) -> Result<ConfigValue, ConfigError> {
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
