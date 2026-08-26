use serde_json::Value;

use super::{LintContext, Rule, RuleMeta};
use crate::config::truthy;
use crate::error::ErrorSink;

pub(crate) struct Md043;

static META: RuleMeta = RuleMeta {
    names: &["MD043", "required-headings"],
    description: "Required heading structure",
    tags: &["headings"],
    needs_tokens: true,
    fixable: false,
};

/// JS `String(value)` 상당의 표기.
fn js_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// 원본 `getExpected`: 다음 요소를 쓰고 커서를 전진시킨다. falsy 면 "[None]".
fn get_expected(required_headings: &[Value], i: &mut usize) -> String {
    let expected = match required_headings.get(*i) {
        Some(value) if truthy(value) => js_string(value),
        _ => "[None]".to_string(),
    };
    *i += 1;
    expected
}

impl Rule for Md043 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let Some(Value::Array(required_headings)) = ctx.config.get("headings") else {
            return;
        };
        let match_case = ctx.config.get("match_case").is_some_and(truthy);
        let handle_case = |s: &str| {
            if match_case {
                s.to_string()
            } else {
                s.to_lowercase()
            }
        };

        let mut i = 0;
        let mut match_any = false;
        let mut has_error = false;
        let mut any_headings = false;
        for id in ctx.tokens.filter_by_types(&["atxHeading", "setextHeading"]) {
            if has_error {
                continue;
            }
            let heading_text = ctx.tokens.heading_text(id);
            let heading_level = ctx.tokens.heading_level(id);
            any_headings = true;
            let actual = format!("{} {heading_text}", "#".repeat(heading_level));
            let expected = get_expected(required_headings, &mut i);
            if expected == "*" {
                let next_expected = get_expected(required_headings, &mut i);
                if handle_case(&next_expected) != handle_case(&actual) {
                    match_any = true;
                    i -= 1;
                }
            } else if expected == "+" {
                match_any = true;
            } else if expected == "?" {
                // Allow current, match next
            } else if handle_case(&expected) == handle_case(&actual) {
                match_any = false;
            } else if match_any {
                i -= 1;
            } else {
                out.add_error_detail_if(
                    ctx.tokens.get(id).start_line,
                    expected,
                    actual,
                    None,
                    None,
                    None,
                    None,
                );
                has_error = true;
            }
        }

        let extra_headings = required_headings.len() as isize - i as isize;
        if !has_error
            && (extra_headings > 1 || (extra_headings == 1 && required_headings[i] != "*"))
            && (any_headings || !required_headings.iter().all(|h| h == "*"))
        {
            out.add_error_context(
                ctx.lines.len(),
                &js_string(&required_headings[i]),
                true,
                true,
                None,
                None,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::{LintOptions, lint_content};
    use serde_json::json;

    fn lint_with(params: serde_json::Value, content: &str) -> Vec<crate::error::LintError> {
        let config = json!({ "default": false, "MD043": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md043_no_headings_param_does_nothing() {
        assert!(lint_with(json!(true), "# One\n\n## Two\n").is_empty());
    }

    #[test]
    fn md043_exact_match_and_mismatch() {
        let content = "# One\n\n## Two\n";
        assert!(lint_with(json!({ "headings": ["# One", "## Two"] }), content).is_empty());
        let errs = lint_with(json!({ "headings": ["# One", "## Three"] }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: ## Three; Actual: ## Two")
        );
    }

    #[test]
    fn md043_missing_last_reports_last_line() {
        let errs = lint_with(
            json!({ "headings": ["# One", "## Two"] }),
            "# One\n\ntext\n",
        );
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 4);
        assert_eq!(errs[0].error_context.as_deref(), Some("## Two"));
    }

    #[test]
    fn md043_wildcards() {
        let content = "# One\n\n## Skip\n\n### Skip\n\n## Last\n";
        assert!(lint_with(json!({ "headings": ["# One", "*", "## Last"] }), content).is_empty());
        assert!(lint_with(json!({ "headings": ["# One", "+", "## Last"] }), content).is_empty());
        // "?" 는 정확히 하나를 건너뛴다
        assert_eq!(
            lint_with(
                json!({ "headings": ["?", "## Skip"] }),
                "# One\n\n## Skip\n"
            )
            .len(),
            0
        );
    }

    #[test]
    fn md043_match_case() {
        let content = "# one\n";
        assert!(lint_with(json!({ "headings": ["# One"] }), content).is_empty());
        let errs = lint_with(
            json!({ "headings": ["# One"], "match_case": true }),
            content,
        );
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: # One; Actual: # one")
        );
    }
}
