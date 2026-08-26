use std::collections::HashSet;

use serde_json::Value;

use super::{LintContext, Rule, RuleMeta};
use crate::config::truthy;
use crate::error::ErrorSink;

pub(crate) struct Md052;

static META: RuleMeta = RuleMeta {
    names: &["MD052", "reference-links-images"],
    description: "Reference links and images should use a label that is defined",
    tags: &["images", "links"],
    // 원본은 `parser: "none"` 이지만 캐시된 micromark 토큰(`getReferenceLinkImageData`)을 쓴다.
    needs_tokens: true,
    fixable: false,
};

/// 원본 `new Set(config.ignored_labels || [ "x" ])`.
/// 배열이면 원소를 JS `String()` 으로, 문자열이면 문자 단위로 집합을 만든다.
/// 그 밖의 truthy 값(숫자, 객체)은 원본이 TypeError 를 내므로 빈 집합으로 둔다.
fn ignored_labels(value: Option<&Value>) -> HashSet<String> {
    match value {
        Some(v) if truthy(v) => match v {
            Value::Array(items) => items
                .iter()
                .map(|item| match item {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect(),
            Value::String(s) => s.chars().map(String::from).collect(),
            _ => HashSet::new(),
        },
        _ => HashSet::from(["x".to_string()]),
    }
}

impl Rule for Md052 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let shortcut_syntax = ctx.config.get("shortcut_syntax").is_some_and(truthy);
        let ignored_labels = ignored_labels(ctx.config.get("ignored_labels"));
        let data = ctx.tokens.reference_link_image_data();
        let definitions = &data.definitions;
        let entries = if shortcut_syntax {
            data.references
                .entries()
                .iter()
                .chain(data.shortcuts.entries())
                .collect::<Vec<_>>()
        } else {
            data.references.entries().iter().collect()
        };
        // 정의되지 않은 참조를 쓰는 링크/이미지를 찾는다
        for (label, datas) in entries {
            if !definitions.contains_key(label) && !ignored_labels.contains(label) {
                for &[line_index, index, length] in datas {
                    // 여러 줄에 걸친 링크면 context 가 잘린다
                    let context: String = ctx.lines[line_index]
                        .chars()
                        .skip(index)
                        .take(length)
                        .collect();
                    out.add_error(
                        line_index + 1,
                        Some(&format!(
                            "Missing link or image reference definition: \"{label}\""
                        )),
                        Some(&context),
                        Some((index + 1, context.chars().count())),
                        None,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::{LintOptions, lint_content};
    use crate::rules::lint_rule;
    use serde_json::json;

    fn lint_with(params: serde_json::Value, content: &str) -> Vec<crate::error::LintError> {
        let config = json!({ "default": false, "MD052": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md052_undefined_full_and_collapsed_reference() {
        let errs = lint_rule("MD052", "[text][missing] and [Missing][] here\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Missing link or image reference definition: \"missing\"")
        );
        assert_eq!(errs[0].error_context.as_deref(), Some("[text][missing]"));
        assert_eq!(errs[0].error_range, Some((1, 15)));
        assert_eq!(errs[1].error_context.as_deref(), Some("[Missing][]"));
        assert_eq!(errs[1].error_range, Some((21, 11)));
    }

    #[test]
    fn md052_defined_reference_is_fine() {
        assert!(
            lint_rule(
                "MD052",
                "[text][label] and ![img][label]\n\n[label]: https://example.com\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn md052_shortcut_only_with_shortcut_syntax() {
        let content = "Text [shortcut] text\n";
        assert!(lint_rule("MD052", content).is_empty());
        let errs = lint_with(json!({ "shortcut_syntax": true }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("[shortcut]"));
        assert_eq!(errs[0].error_range, Some((6, 10)));
    }

    #[test]
    fn md052_ignored_labels_default_and_custom() {
        // 기본값 ["x"] 는 체크박스 스타일 `[x]` 를 무시한다
        assert!(lint_rule("MD052", "[x][] done\n").is_empty());
        assert_eq!(lint_rule("MD052", "[todo][] later\n").len(), 1);
        assert!(lint_with(json!({ "ignored_labels": ["todo"] }), "[todo][] later\n").is_empty());
        // 사용자 목록으로 바꾸면 기본 "x" 는 더 이상 무시되지 않는다
        assert_eq!(
            lint_with(json!({ "ignored_labels": ["todo"] }), "[x][] done\n").len(),
            1
        );
    }

    #[test]
    fn md052_multiline_reference_reports_first_line_only() {
        let errs = lint_rule("MD052", "[multi\nline][missing]\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("[multi"));
        assert_eq!(errs[0].error_range, Some((1, 6)));
    }
}
