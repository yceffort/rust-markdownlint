use super::{ConfigValue, truthy};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitIgnore {
    Enabled(bool),
    Pattern(String),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Options {
    pub config: Option<ConfigValue>,
    pub fix: Option<bool>,
    pub front_matter: Option<String>,
    pub gitignore: Option<GitIgnore>,
    pub globs: Option<Vec<String>>,
    pub ignores: Option<Vec<String>>,
    pub no_banner: Option<bool>,
    pub no_inline_config: Option<bool>,
    pub no_progress: Option<bool>,
    pub show_found: Option<bool>,
}

/// cli2 `schema/markdownlint-cli2-config-schema.json` 의 속성 목록. "unknown" 파일 판별용.
pub const OPTIONS_KEYS: &[&str] = &[
    "$schema",
    "config",
    "customRules",
    "fix",
    "frontMatter",
    "gitignore",
    "globs",
    "ignores",
    "markdownItPlugins",
    "modulePaths",
    "noBanner",
    "noInlineConfig",
    "noProgress",
    "outputFormatters",
    "showFound",
];

/// JS 모듈 로딩이 필요해 지원하지 않는 키. stderr 경고 1줄 후 무시한다.
const UNSUPPORTED_KEYS: &[&str] = &[
    "customRules",
    "markdownItPlugins",
    "outputFormatters",
    "modulePaths",
];

fn string_array(value: &ConfigValue) -> Vec<String> {
    value
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

pub fn options_from_value(v: ConfigValue, warn: &mut dyn FnMut(&str)) -> Options {
    let mut options = Options::default();
    let ConfigValue::Object(map) = v else {
        return options;
    };
    for (key, value) in map {
        match key.as_str() {
            "config" => options.config = Some(value),
            "fix" => options.fix = Some(truthy(&value)),
            "frontMatter" => options.front_matter = value.as_str().map(str::to_string),
            "gitignore" => {
                options.gitignore = match value {
                    ConfigValue::Bool(b) => Some(GitIgnore::Enabled(b)),
                    ConfigValue::String(s) => Some(GitIgnore::Pattern(s)),
                    _ => None,
                }
            }
            "globs" => options.globs = Some(string_array(&value)),
            "ignores" => options.ignores = Some(string_array(&value)),
            "noBanner" => options.no_banner = Some(truthy(&value)),
            "noInlineConfig" => options.no_inline_config = Some(truthy(&value)),
            "noProgress" => options.no_progress = Some(truthy(&value)),
            "showFound" => options.show_found = Some(truthy(&value)),
            key if UNSUPPORTED_KEYS.contains(&key)
                && value.as_array().is_some_and(|a| !a.is_empty()) =>
            {
                warn(&format!("Ignoring unsupported option: {key}"));
            }
            _ => {}
        }
    }
    options
}

/// cli2 `merge-options.mjs`: 두 번째가 우선, `config` 는 최상위 key 단위 얕은 병합.
pub fn merge_options(first: &Options, second: &Options) -> Options {
    let config = match (&first.config, &second.config) {
        (None, None) => None,
        (first, second) => {
            let mut merged = serde_json::Map::new();
            for source in [first, second].into_iter().flatten() {
                if let Some(obj) = source.as_object() {
                    for (key, value) in obj {
                        merged.insert(key.clone(), value.clone());
                    }
                }
            }
            Some(ConfigValue::Object(merged))
        }
    };
    Options {
        config,
        fix: second.fix.or(first.fix),
        front_matter: second
            .front_matter
            .clone()
            .or_else(|| first.front_matter.clone()),
        gitignore: second.gitignore.clone().or_else(|| first.gitignore.clone()),
        globs: second.globs.clone().or_else(|| first.globs.clone()),
        ignores: second.ignores.clone().or_else(|| first.ignores.clone()),
        no_banner: second.no_banner.or(first.no_banner),
        no_inline_config: second.no_inline_config.or(first.no_inline_config),
        no_progress: second.no_progress.or(first.no_progress),
        show_found: second.show_found.or(first.show_found),
    }
}
