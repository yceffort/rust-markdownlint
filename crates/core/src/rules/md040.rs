use super::{LintContext, Rule, RuleMeta};
use crate::config::truthy;
use crate::error::ErrorSink;

pub(crate) struct Md040;

static META: RuleMeta = RuleMeta {
    names: &["MD040", "fenced-code-language"],
    description: "Fenced code blocks should have a language specified",
    tags: &["code", "language"],
    needs_tokens: true,
    fixable: false,
};

impl Rule for Md040 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        // 원본: `let allowed = params.config.allowed_languages; allowed = Array.isArray(allowed) ? allowed : [];`
        // 배열이 아닌 원소는 문자열과 절대 `===` 로 같을 수 없으니 문자열만 남긴다.
        let allowed: Vec<String> = ctx
            .config
            .get("allowed_languages")
            .and_then(|v| v.as_array())
            .map(|langs| {
                langs
                    .iter()
                    .filter_map(|l| l.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let language_only = ctx.config.get("language_only").is_some_and(truthy);

        for fenced_code in ctx.tokens.filter_by_types(&["codeFenced"]) {
            let Some(&opening_fence) = ctx
                .tokens
                .descendants_by_type(fenced_code, &[&["codeFencedFence"]])
                .first()
            else {
                continue;
            };
            let opening = ctx.tokens.get(opening_fence);
            let start_line = opening.start_line;
            let text = ctx.tokens.text(opening_fence).to_string();
            let info = ctx
                .tokens
                .descendants_by_type(opening_fence, &[&["codeFencedFenceInfo"]])
                .first()
                .map(|&id| ctx.tokens.text(id).to_string());
            // 원본: `if (!info) { ... } else if ((allowed.length > 0) && !allowed.includes(info)) { ... }`
            match &info {
                None => out.add_error_context(start_line, &text, false, false, None, None),
                Some(info) if !info.is_empty() => {
                    if !allowed.is_empty() && !allowed.contains(info) {
                        out.add_error(
                            start_line,
                            Some(&format!("\"{info}\" is not allowed")),
                            None,
                            None,
                            None,
                        );
                    }
                }
                Some(_) => out.add_error_context(start_line, &text, false, false, None, None),
            }
            if language_only
                && !ctx
                    .tokens
                    .descendants_by_type(opening_fence, &[&["codeFencedFenceMeta"]])
                    .is_empty()
            {
                out.add_error(
                    start_line,
                    Some(&format!(
                        "Info string contains more than language: \"{text}\""
                    )),
                    None,
                    None,
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
        let config = json!({ "default": false, "MD040": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md040_missing_language_reports_error() {
        let errs = lint_rule("MD040", "```\ncode\n```\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("```"));
        assert!(errs[0].error_detail.is_none());
    }

    #[test]
    fn md040_language_present_is_ok() {
        assert!(lint_rule("MD040", "```rust\ncode\n```\n").is_empty());
    }

    #[test]
    fn md040_allowed_languages_rejects_others() {
        let errs = lint_with(
            json!({ "allowed_languages": ["rust"] }),
            "```js\ncode\n```\n",
        );
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("\"js\" is not allowed")
        );
    }

    #[test]
    fn md040_allowed_languages_accepts_listed() {
        assert!(
            lint_with(
                json!({ "allowed_languages": ["rust"] }),
                "```rust\ncode\n```\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn md040_language_only_rejects_meta() {
        let errs = lint_with(json!({ "language_only": true }), "```rust foo\ncode\n```\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Info string contains more than language: \"```rust foo\"")
        );
    }

    #[test]
    fn md040_language_only_allows_language_alone() {
        assert!(lint_with(json!({ "language_only": true }), "```rust\ncode\n```\n").is_empty());
    }
}
