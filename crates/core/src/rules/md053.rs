use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::{LintContext, Rule, RuleMeta};
use crate::config::truthy;
use crate::error::{ErrorSink, FixInfo, ellipsify, utf16_len};

pub(crate) struct Md053;

static META: RuleMeta = RuleMeta {
    names: &["MD053", "link-image-reference-definitions"],
    description: "Link and image reference definitions should be needed",
    tags: &["images", "links"],
    // 원본은 `parser: "none"` 이라 micromark 토큰을 요구하지 않는다. 캐시된 토큰
    // (`getReferenceLinkImageData`) 을 쓰지만, `parser: "micromark"` 인 다른 규칙이 켜져
    // 있지 않으면 토큰이 비어 아무것도 보고하지 않는다 (cli2 와 동일).
    needs_tokens: false,
    fixable: true,
};

/// 원본 `linkReferenceDefinitionRe` (`/^ {0,3}\[([^\]]*[^\\])\]:/`).
static LINK_REFERENCE_DEFINITION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ {0,3}\[([^\]]*[^\\])\]:").expect("md053 definition regex"));

/// JS `String.prototype.trim` 이 지우는 문자 (`\s` 와 같은 집합, `JS_WHITESPACE`).
const JS_TRIM_CHARS: &[char] = &[
    '\t', '\n', '\x0B', '\x0C', '\r', ' ', '\u{a0}', '\u{1680}', '\u{2000}', '\u{2001}',
    '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}',
    '\u{200a}', '\u{2028}', '\u{2029}', '\u{202f}', '\u{205f}', '\u{3000}', '\u{feff}',
];

/// 원본 `new Set(params.config.ignored_definitions || [ "//" ])`.
/// 배열이면 원소를 JS `String()` 으로, 문자열이면 문자 단위로 집합을 만든다.
/// 그 밖의 truthy 값(숫자, 객체)은 원본이 TypeError 를 내므로 빈 집합으로 둔다.
fn ignored_definitions(value: Option<&Value>) -> HashSet<String> {
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
        _ => HashSet::from(["//".to_string()]),
    }
}

/// 원본 `singleLineDefinition(line)`: 정의 머리(`[label]:`)를 지우고 남은 게 있으면
/// 정의가 그 줄에서 끝난 것이므로 줄 전체를 지우는 fix 를 붙일 수 있다.
fn single_line_definition(line: &str) -> bool {
    !LINK_REFERENCE_DEFINITION_RE
        .replace(line, "")
        .trim_matches(JS_TRIM_CHARS)
        .is_empty()
}

impl Rule for Md053 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let ignored = ignored_definitions(ctx.config.get("ignored_definitions"));
        let lines = ctx.lines;
        let data = ctx.tokens.reference_link_image_data();
        let delete_fix_info = FixInfo {
            line_number: None,
            edit_column: None,
            delete_count: Some(-1),
            insert_text: None,
        };
        let report = |out: &mut ErrorSink, detail: String, line_index: usize| {
            let line = lines[line_index];
            out.add_error(
                line_index + 1,
                Some(&detail),
                Some(&ellipsify(line, false, false)),
                Some((1, utf16_len(line))),
                single_line_definition(line).then(|| delete_fix_info.clone()),
            );
        };
        // 어떤 링크/이미지도 참조하지 않는 정의를 찾는다
        for (label, (line_index, _destination)) in data.definitions.entries() {
            if !ignored.contains(label)
                && !data.references.contains_key(label)
                && !data.shortcuts.contains_key(label)
            {
                report(
                    out,
                    format!("Unused link or image reference definition: \"{label}\""),
                    *line_index,
                );
            }
        }
        // 두 번 이상 정의된 참조를 찾는다
        for (label, line_index) in &data.duplicate_definitions {
            if !ignored.contains(label) {
                report(
                    out,
                    format!("Duplicate link or image reference definition: \"{label}\""),
                    *line_index,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::{LintOptions, lint_content};
    use crate::rules::lint_rule;
    use serde_json::json;

    /// 원본처럼 micromark 규칙(MD001)이 함께 켜져 있어야 토큰을 받으므로 같이 켜고 MD053 만 거른다.
    fn lint_with(params: serde_json::Value, content: &str) -> Vec<crate::error::LintError> {
        let config = json!({ "default": false, "MD053": params, "MD001": true });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        let mut errs = lint_content("test.md", content, &opts).unwrap();
        errs.retain(|e| e.rule_names[0] == "MD053");
        errs
    }

    #[test]
    fn md053_alone_reports_nothing_like_original() {
        // 원본은 `parser: "none"` 이라 다른 micromark 규칙이 없으면 토큰이 비어 아무것도 보고하지 않는다
        assert!(lint_rule("MD053", "[unused]: https://example.com\n").is_empty());
    }

    #[test]
    fn md053_unused_definition_reports_with_delete_fix() {
        let errs = lint_with(json!(true), "text\n\n[unused]: https://example.com\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Unused link or image reference definition: \"unused\"")
        );
        assert_eq!(
            errs[0].error_context.as_deref(),
            Some("[unused]: https://example.com")
        );
        assert_eq!(errs[0].error_range, Some((1, 29)));
        assert_eq!(
            errs[0].fix_info.as_ref().map(|f| f.delete_count),
            Some(Some(-1))
        );
        // 30자를 넘으면 `ellipsify(line)` 이 앞 30자만 남긴다 (start/end 인자가 없어 둘 다 falsy)
        let errs = lint_with(
            json!(true),
            "text\n\n[unused]: https://example.com/very/long/path\n",
        );
        assert_eq!(
            errs[0].error_context.as_deref(),
            Some("[unused]: https://example.com/...")
        );
        assert_eq!(errs[0].error_range, Some((1, 44)));
    }

    #[test]
    fn md053_used_definition_is_fine() {
        assert!(
            lint_with(
                json!(true),
                "[text][label] and [shortcut]\n\n[label]: https://example.com\n[shortcut]: https://example.com\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn md053_duplicate_definition_reports() {
        let errs = lint_with(
            json!(true),
            "[text][label]\n\n[label]: https://a.com\n[label]: https://b.com\n",
        );
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 4);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Duplicate link or image reference definition: \"label\"")
        );
        assert_eq!(
            errs[0].error_context.as_deref(),
            Some("[label]: https://b.com")
        );
    }

    #[test]
    fn md053_ignored_definitions_default_and_custom() {
        // 기본값 ["//"] 는 주석 관용구 `[//]: <> (...)` 를 무시한다
        assert!(lint_with(json!(true), "text\n\n[//]: <> (comment)\n").is_empty());
        assert_eq!(lint_with(json!(true), "text\n\n[skip]: <> (c)\n").len(), 1);
        assert!(
            lint_with(
                json!({ "ignored_definitions": ["skip"] }),
                "text\n\n[skip]: <> (c)\n"
            )
            .is_empty()
        );
        // 빈 배열도 JS 에서 truthy 라 기본값 "//" 를 덮어쓴다
        assert_eq!(
            lint_with(
                json!({ "ignored_definitions": [] }),
                "text\n\n[//]: <> (comment)\n"
            )
            .len(),
            1
        );
    }

    #[test]
    fn md053_multiline_definition_has_no_fix() {
        // 정의가 다음 줄로 이어지면 (`singleLineDefinition` 이 false) fix 를 붙이지 않는다
        let errs = lint_with(json!(true), "text\n\n[unused]:\n  https://example.com\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert!(errs[0].fix_info.is_none());
    }

    #[test]
    fn md053_context_and_range_use_utf16_length() {
        // 기대값은 cli2 0.22.1 실행 결과 (ellipsify 30단위 절단과 `line.length` 모두 UTF-16 단위)
        let errs = lint_with(json!(true), "[unused🎸]: https://example.com\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_context.as_deref(),
            Some("[unused🎸]: https://example.co...")
        );
        assert_eq!(errs[0].error_range, Some((1, 31)));
    }
}
