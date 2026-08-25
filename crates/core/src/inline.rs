use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::config::{ConfigValue, effective_config, parse_config_str};
use crate::rules::registry;

/// helpers.cjs `inlineCommentStartRe`.
static INLINE_COMMENT_START_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(<!--\s*markdownlint-(disable|enable|capture|restore|disable-file|enable-file|disable-line|disable-next-line|configure-file))(?:\s|-->)",
    )
    .expect("inline comment regex")
});

pub struct InlineResult {
    pub config: ConfigValue,
    pub enabled_per_line: Vec<HashSet<&'static str>>,
}

/// `(action, parameter, line_number)`. action 은 대문자.
type InlineMatch = (String, String, usize);

/// markdownlint.mjs `handleInlineConfig` 의 매치 수집. 원본의 전역 정규식은 `-->`
/// 미발견으로 break 한 뒤 lastIndex 가 다음 exec 로 이월되므로 pass 안에서 재현한다.
fn collect_matches(input: &[&str]) -> Vec<InlineMatch> {
    let mut matches = Vec::new();
    let mut last_index = 0;
    for (line_index, line) in input.iter().enumerate() {
        for (action, parameter) in collect_matches_line(line, &mut last_index) {
            matches.push((action, parameter, line_index + 1));
        }
    }
    matches
}

/// markdownlint.mjs `applyEnableDisable`.
fn apply_enable_disable(
    action: &str,
    parameter: &str,
    state: &mut HashSet<&'static str>,
    all_rule_names: &[&'static str],
) {
    let enabled = action.starts_with("ENABLE");
    let trimmed = parameter.trim();
    let items: Vec<String> = if trimmed.is_empty() {
        all_rule_names.iter().map(|n| n.to_string()).collect()
    } else {
        trimmed
            .to_uppercase()
            .split_whitespace()
            .map(str::to_string)
            .collect()
    };
    for name in &items {
        for rule_name in registry::resolve_alias(name) {
            if enabled {
                state.insert(rule_name);
            } else {
                state.remove(rule_name);
            }
        }
    }
}

/// markdownlint.mjs `getEnabledRulesPerLineNumber` 포팅. `enabled_per_line[i]` 는
/// (front matter 를 제외한) i+1 번째 줄에서 활성인 규칙의 기본 이름 집합.
pub fn apply_inline_config(lines: &[&str], base: &ConfigValue, no_inline: bool) -> InlineResult {
    // 1단계: configure-file (전체 내용을 한 문자열로 합쳐 스캔)
    let mut config = base.clone();
    if !no_inline {
        let joined = lines.join("\n");
        for (action, parameter, _) in collect_matches(&[&joined]) {
            if action == "CONFIGURE-FILE"
                && let Ok(parsed) = parse_config_str(&parameter)
            {
                let mut merged = config.as_object().cloned().unwrap_or_default();
                if let ConfigValue::Object(parsed) = parsed {
                    merged.extend(parsed);
                }
                config = ConfigValue::Object(merged);
            }
        }
    }

    let effective = effective_config(&config);
    let all_rule_names: Vec<&'static str> = effective.rules.keys().copied().collect();
    let initial: HashSet<&'static str> = effective
        .rules
        .iter()
        .filter(|(_, (enabled, _, _))| *enabled)
        .map(|(name, _)| *name)
        .collect();

    if no_inline {
        return InlineResult {
            config,
            enabled_per_line: vec![initial; lines.len()],
        };
    }

    // 2단계: enable-file / disable-file
    let mut enabled_rules = initial.clone();
    for (action, parameter, _) in collect_matches(lines) {
        if action == "ENABLE-FILE" || action == "DISABLE-FILE" {
            apply_enable_disable(&action, &parameter, &mut enabled_rules, &all_rule_names);
        }
    }

    // 3단계: capture / restore / enable / disable, 줄마다 스냅샷.
    // 원본에서 기본 capturedRules 는 파일 주석 적용 전의 초기 맵이다.
    let mut captured_rules = initial;
    let mut enabled_per_line = Vec::with_capacity(lines.len());
    let mut last_index = 0;
    for line in lines {
        for (action, parameter) in collect_matches_line(line, &mut last_index) {
            match action.as_str() {
                "CAPTURE" => captured_rules = enabled_rules.clone(),
                "RESTORE" => enabled_rules = captured_rules.clone(),
                "ENABLE" | "DISABLE" => {
                    apply_enable_disable(&action, &parameter, &mut enabled_rules, &all_rule_names);
                }
                _ => {}
            }
        }
        enabled_per_line.push(enabled_rules.clone());
    }

    // 4단계: disable-line / disable-next-line
    for (action, parameter, line_number) in collect_matches(lines) {
        let target = match action.as_str() {
            "DISABLE-LINE" => line_number - 1,
            "DISABLE-NEXT-LINE" => line_number,
            _ => continue,
        };
        if let Some(state) = enabled_per_line.get_mut(target) {
            apply_enable_disable(&action, &parameter, state, &all_rule_names);
        }
    }

    InlineResult {
        config,
        enabled_per_line,
    }
}

/// 한 줄 분량의 `collect_matches`. 3단계에서 줄 사이에 스냅샷을 끼워 넣기 위해
/// lastIndex 이월 상태를 호출자가 들고 다닌다.
fn collect_matches_line(line: &str, last_index: &mut usize) -> Vec<(String, String)> {
    let mut matches = Vec::new();
    loop {
        if *last_index > line.len() || !line.is_char_boundary(*last_index) {
            *last_index = 0;
            break;
        }
        let Some(caps) = INLINE_COMMENT_START_RE.captures_at(line, *last_index) else {
            *last_index = 0;
            break;
        };
        let action = caps.get(2).unwrap().as_str().to_uppercase();
        let start_index = caps.get(1).unwrap().end();
        let Some(found) = line[start_index..].find("-->") else {
            *last_index = caps.get(0).unwrap().end();
            break;
        };
        matches.push((action, line[start_index..start_index + found].to_string()));
        *last_index = caps.get(0).unwrap().end();
    }
    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn disable_next_line_and_capture_restore() {
        let lines = [
            "<!-- markdownlint-disable MD018 -->",
            "#a",
            "<!-- markdownlint-capture -->",
            "<!-- markdownlint-enable -->",
            "#b",
            "<!-- markdownlint-restore -->",
            "#c",
        ];
        let r = apply_inline_config(&lines, &json!({}), false);
        assert!(!r.enabled_per_line[1].contains("MD018"));
        assert!(r.enabled_per_line[4].contains("MD018"));
        assert!(!r.enabled_per_line[6].contains("MD018"));
    }

    #[test]
    fn configure_file_merges() {
        let lines = [
            "<!-- markdownlint-configure-file { \"MD047\": false } -->",
            "x",
        ];
        let r = apply_inline_config(&lines, &json!({}), false);
        assert!(!r.enabled_per_line[1].contains("MD047"));
    }

    #[test]
    fn disable_line_and_next_line() {
        let lines = [
            "#a <!-- markdownlint-disable-line MD018 -->",
            "<!-- markdownlint-disable-next-line MD018 -->",
            "#b",
            "#c",
        ];
        let r = apply_inline_config(&lines, &json!({}), false);
        assert!(!r.enabled_per_line[0].contains("MD018"));
        assert!(!r.enabled_per_line[2].contains("MD018"));
        assert!(r.enabled_per_line[3].contains("MD018"));
    }

    #[test]
    fn disable_file_applies_to_all_lines() {
        let lines = ["#a", "<!-- markdownlint-disable-file MD018 -->", "#b"];
        let r = apply_inline_config(&lines, &json!({}), false);
        assert!(!r.enabled_per_line[0].contains("MD018"));
        assert!(!r.enabled_per_line[2].contains("MD018"));
    }

    /// 기대값은 원본 markdownlint@0.40.0 `lint` 를 Node 로 실행해 얻었다.
    #[test]
    fn restore_without_capture_resets_to_state_before_file_comments() {
        let lines = [
            "<!-- markdownlint-disable-file MD018 -->",
            "#a",
            "<!-- markdownlint-restore -->",
            "#b",
        ];
        let r = apply_inline_config(&lines, &json!({}), false);
        assert!(!r.enabled_per_line[1].contains("MD018"));
        assert!(r.enabled_per_line[3].contains("MD018"));
    }

    #[test]
    fn capture_and_disable_on_same_line_apply_in_order() {
        let lines = [
            "<!-- markdownlint-capture --><!-- markdownlint-disable MD018 -->",
            "#a",
            "<!-- markdownlint-restore -->",
            "#b",
        ];
        let r = apply_inline_config(&lines, &json!({}), false);
        assert!(!r.enabled_per_line[1].contains("MD018"));
        assert!(r.enabled_per_line[3].contains("MD018"));
    }

    #[test]
    fn unclosed_comment_carries_last_index_to_next_line() {
        let lines = [
            "<!-- markdownlint-disable MD018",
            "text <!-- markdownlint-disable MD018 -->",
            "#a",
        ];
        let r = apply_inline_config(&lines, &json!({}), false);
        assert!(r.enabled_per_line[1].contains("MD018"));
        assert!(r.enabled_per_line[2].contains("MD018"));
    }

    #[test]
    fn configure_file_spanning_multiple_lines() {
        let lines = [
            "<!-- markdownlint-configure-file {",
            "  \"MD018\": false",
            "} -->",
            "#a",
        ];
        let r = apply_inline_config(&lines, &json!({}), false);
        assert!(!r.enabled_per_line[3].contains("MD018"));
    }

    #[test]
    fn no_inline_config_ignores_comments() {
        let lines = ["<!-- markdownlint-disable MD018 -->", "#a"];
        let r = apply_inline_config(&lines, &json!({}), true);
        assert!(r.enabled_per_line[1].contains("MD018"));
        assert_eq!(r.config, json!({}));
    }
}
