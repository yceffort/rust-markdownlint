use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::{LintContext, Rule, RuleMeta};
use crate::config::truthy;
use crate::error::ErrorSink;

pub(crate) struct Md033;

static META: RuleMeta = RuleMeta {
    names: &["MD033", "no-inline-html"],
    description: "Inline HTML",
    tags: &["html"],
    needs_tokens: true,
    fixable: false,
};

/// shared.cjs `nextLinesRe`: 첫 줄바꿈부터 끝까지.
static NEXT_LINES_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\r\n][\s\S]*$").expect("next lines regex"));

/// JS `String(value)` 상당의 표기. md043 에도 같은 helper 가 있다.
fn js_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// 원본 `toLowerCaseStringArray`: 배열이면 각 원소를 `String()` 후 소문자로, 아니면 빈 배열.
fn to_lower_case_string_array(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(arr)) => arr.iter().map(|e| js_string(e).to_lowercase()).collect(),
        _ => Vec::new(),
    }
}

impl Rule for Md033 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let allowed_elements = to_lower_case_string_array(ctx.config.get("allowed_elements"));
        // 정의되지 않았으면 하위 호환을 위해 allowed_elements 를 쓴다 (JS `||` 이라 falsy 면 대체).
        let table_allowed_elements = to_lower_case_string_array(
            ctx.config
                .get("table_allowed_elements")
                .filter(|value| truthy(value))
                .or_else(|| ctx.config.get("allowed_elements")),
        );
        for id in ctx.tokens.filter_by_types_html_flow(&["htmlText"], true) {
            let token = ctx.tokens.get(id);
            let Some(html_tag_info) = ctx.tokens.html_tag_info(id) else {
                continue;
            };
            if html_tag_info.close {
                continue;
            }
            let element_name = html_tag_info.name.to_lowercase();
            let in_table = ctx.tokens.parent_of_type(id, &["table"]).is_some();
            if (in_table || !allowed_elements.contains(&element_name))
                && (!in_table || !table_allowed_elements.contains(&element_name))
            {
                let range = (
                    token.start_column,
                    NEXT_LINES_RE.replace(&token.text, "").chars().count(),
                );
                out.add_error(
                    token.start_line,
                    Some(&format!("Element: {}", html_tag_info.name)),
                    None,
                    Some(range),
                    None,
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

    fn lint_with(params: serde_json::Value, content: &str) -> Vec<crate::error::LintError> {
        let config = json!({ "default": false, "MD033": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md033_inline_html_reported() {
        let errs = lint_rule("MD033", "Some <b>bold</b> text\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_detail.as_deref(), Some("Element: b"));
        assert_eq!(errs[0].error_range, Some((6, 3)));
        assert!(errs[0].fix_info.is_none());
    }

    #[test]
    fn md033_no_html_no_error() {
        assert!(lint_rule("MD033", "# Heading\n\nJust *text* here.\n").is_empty());
        assert!(lint_rule("MD033", "    <div>\n").is_empty());
        assert!(lint_rule("MD033", "`<div>`\n").is_empty());
    }

    #[test]
    fn md033_allowed_elements() {
        let content = "Some <b>bold</b> and <i>italic</i>\n";
        assert_eq!(lint_rule("MD033", content).len(), 2);
        let errs = lint_with(json!({ "allowed_elements": ["B"] }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_detail.as_deref(), Some("Element: i"));
    }

    #[test]
    fn md033_multiline_tag_range_stops_at_newline() {
        let errs = lint_rule("MD033", "Text <span\nclass=\"x\">y</span>\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_detail.as_deref(), Some("Element: span"));
        assert_eq!(errs[0].error_range, Some((6, 5)));
    }

    #[test]
    fn md033_html_flow_is_reported() {
        let errs = lint_rule("MD033", "<hr>\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_detail.as_deref(), Some("Element: hr"));
    }
}
