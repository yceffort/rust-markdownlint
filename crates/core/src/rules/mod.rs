use std::collections::HashSet;

use crate::error::ErrorSink;
use crate::parser::TokenTree;

mod md001;
mod md003;
mod md004;
mod md005;
mod md007;
mod md009;
mod md010;
mod md012;
mod md013;
mod md014;
mod md018;
mod md019;
mod md020;
mod md021;
mod md022;
mod md023;
mod md024;
mod md025;
mod md026;
mod md027;
mod md028;
mod md029;
mod md030;
mod md033;
mod md035;
mod md036;
mod md040;
mod md041;
mod md043;
mod md046;
mod md047;
pub mod registry;

pub struct RuleMeta {
    pub names: &'static [&'static str],
    pub description: &'static str,
    pub tags: &'static [&'static str],
    pub needs_tokens: bool,
    pub fixable: bool,
}

pub type RuleParams = serde_json::Map<String, serde_json::Value>;

pub struct LintContext<'a> {
    pub name: &'a str,
    pub lines: &'a [&'a str],
    pub tokens: &'a TokenTree,
    /// `params.frontMatterLines`: 제거된 front matter 의 줄 내용.
    pub front_matter: &'a [&'a str],
    pub front_matter_lines: usize,
    pub config: &'a RuleParams,
}

pub trait Rule: Sync {
    fn meta(&self) -> &'static RuleMeta;
    fn check(&self, ctx: &LintContext, out: &mut ErrorSink);
}

/// helpers.cjs `isBlankLine`: 빈 줄, 공백만 있는 줄, HTML 주석과 `>` 를 제거하면
/// 비는 줄을 blank 로 본다.
pub(crate) fn is_blank_line(line: &str) -> bool {
    const START_COMMENT: &str = "<!--";
    const END_COMMENT: &str = "-->";
    fn remove_comments(line: &str) -> String {
        let mut s = line.to_string();
        loop {
            let start = s.find(START_COMMENT);
            let end = s.find(END_COMMENT);
            match (start, end) {
                (Some(start), Some(end)) if start < end => {
                    // Start comment is before end comment
                    s = format!("{}{}", &s[..start], &s[end + END_COMMENT.len()..]);
                }
                (_, Some(end)) => {
                    // Unmatched end comment is first
                    s = s[end + END_COMMENT.len()..].to_string();
                }
                (Some(start), None) => {
                    // Unmatched start comment is last
                    s = s[..start].to_string();
                }
                (None, None) => return s,
            }
        }
    }
    line.is_empty()
        || line.trim().is_empty()
        || remove_comments(line).replace('>', "").trim().is_empty()
}

/// helpers/micromark-helpers.cjs `addRangeToSet`: `start`..=`end` (양 끝 포함) 를 set 에 채운다.
pub(crate) fn add_range_to_set(set: &mut HashSet<usize>, start: usize, end: usize) {
    for line in start..=end {
        set.insert(line);
    }
}

/// helpers.cjs `frontMatterHasTitle`: title 패턴에 맞는 front matter 줄이 있는지.
/// 패턴이 지정됐지만 falsy 면 front matter 를 무시한다. 패턴은 사용자 정규식이라
/// JS 문법에 가까운 `fancy_regex` 로 컴파일하고, 컴파일에 실패하면 false 로 본다
/// (원본은 예외를 던진다).
pub(crate) fn front_matter_has_title(
    front_matter_lines: &[&str],
    front_matter_title_pattern: Option<&serde_json::Value>,
) -> bool {
    let ignore_front_matter =
        front_matter_title_pattern.is_some_and(|value| !crate::config::truthy(value));
    let pattern = front_matter_title_pattern
        .filter(|value| crate::config::truthy(value))
        .map(|value| match value {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .unwrap_or_else(|| r#"^\s*"?title"?\s*[:=]"#.to_string());
    let Ok(front_matter_title_re) = fancy_regex::Regex::new(&format!("(?i){pattern}")) else {
        return false;
    };
    !ignore_front_matter
        && front_matter_lines
            .iter()
            .any(|line| front_matter_title_re.is_match(line).unwrap_or(false))
}

/// 규칙 하나만 활성화해 `lint_content` 로 lint 하는 테스트 helper.
#[cfg(test)]
pub(crate) fn lint_rule(name: &str, content: &str) -> Vec<crate::error::LintError> {
    let mut config = serde_json::Map::new();
    config.insert("default".into(), false.into());
    config.insert(name.into(), true.into());
    let config = serde_json::Value::Object(config);
    let opts = crate::lint::LintOptions {
        config: Some(&config),
        ..Default::default()
    };
    crate::lint::lint_content("test.md", content, &opts).unwrap()
}
