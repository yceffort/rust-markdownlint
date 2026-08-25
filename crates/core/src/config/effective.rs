use std::collections::HashMap;

use super::{ConfigValue, truthy};
use crate::error::Severity;
use crate::rules::{RuleParams, registry};

pub struct EffectiveConfig {
    pub rules: HashMap<&'static str, (bool, Severity, RuleParams)>,
}

impl EffectiveConfig {
    pub fn enabled(&self, name: &str) -> bool {
        self.rules.get(name).is_some_and(|(enabled, _, _)| *enabled)
    }

    pub fn get(&self, name: &str) -> (bool, Severity, RuleParams) {
        self.rules
            .get(name)
            .cloned()
            .unwrap_or((false, Severity::Error, RuleParams::new()))
    }
}

/// markdownlint.mjs `getEffectiveConfig` 포팅. 설정 키는 삽입 순서로 처리한다.
pub fn effective_config(config: &ConfigValue) -> EffectiveConfig {
    let empty = serde_json::Map::new();
    let entries = config.as_object().unwrap_or(&empty);

    let mut rule_default_enable = true;
    let mut rule_default_severity = Severity::Error;
    for (key, value) in entries {
        if key.to_uppercase() == "DEFAULT" {
            rule_default_enable = truthy(value);
            if value == "warning" {
                rule_default_severity = Severity::Warning;
            }
            break;
        }
    }

    let mut rules = HashMap::new();
    for rule in registry::all_rules() {
        rules.insert(
            rule.meta().names[0],
            (
                rule_default_enable,
                rule_default_severity,
                RuleParams::new(),
            ),
        );
    }

    for (key, value) in entries {
        let (enabled, severity, effective_value) = match value {
            ConfigValue::Object(obj) => {
                let enabled = obj.get("enabled").map(truthy).unwrap_or(true);
                let severity = if obj.get("severity").is_some_and(|v| v == "warning") {
                    Severity::Warning
                } else {
                    Severity::Error
                };
                let params: RuleParams = obj
                    .iter()
                    .filter(|(k, _)| *k != "enabled" && *k != "severity")
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                (enabled, severity, params)
            }
            value if truthy(value) => (
                true,
                if value == "warning" {
                    Severity::Warning
                } else {
                    Severity::Error
                },
                RuleParams::new(),
            ),
            _ => (false, Severity::Error, RuleParams::new()),
        };
        for rule_name in registry::resolve_alias(key) {
            rules.insert(rule_name, (enabled, severity, effective_value.clone()));
        }
    }

    EffectiveConfig { rules }
}
