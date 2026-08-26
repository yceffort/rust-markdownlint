use std::collections::HashSet;

use crate::error::ErrorSink;
use crate::parser::TokenTree;

mod md003;
mod md004;
mod md005;
mod md007;
mod md009;
mod md010;
mod md012;
mod md018;
mod md019;
mod md020;
mod md021;
mod md022;
mod md023;
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
