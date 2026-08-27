use super::{LintContext, Rule, RuleMeta};
use crate::config::{js_string, truthy};
use crate::error::ErrorSink;

pub(crate) struct Md048;

static META: RuleMeta = RuleMeta {
    names: &["MD048", "code-fence-style"],
    description: "Code fence style",
    tags: &["code"],
    needs_tokens: true,
    fixable: false,
};

/// 원본 `fencedCodeBlockStyleFor`: 펜스 문자열의 스타일 이름.
fn fenced_code_block_style_for(markup: &str) -> &'static str {
    match markup.chars().next() {
        Some('~') => "tilde",
        _ => "backtick",
    }
}

impl Rule for Md048 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        // `String(params.config.style || "consistent")`
        let style = ctx
            .config
            .get("style")
            .filter(|value| truthy(value))
            .map(js_string)
            .unwrap_or_else(|| "consistent".to_string());
        let mut expected_style = style;
        let code_fenceds = ctx.tokens.filter_by_types(&["codeFenced"]);
        for code_fenced in code_fenceds {
            let Some(&code_fenced_fence_sequence) = ctx
                .tokens
                .descendants_by_type(
                    code_fenced,
                    &[&["codeFencedFence"], &["codeFencedFenceSequence"]],
                )
                .first()
            else {
                continue;
            };
            let token = ctx.tokens.get(code_fenced_fence_sequence);
            let (start_line, text) = (
                token.start_line,
                ctx.tokens.text(code_fenced_fence_sequence),
            );
            if expected_style == "consistent" {
                expected_style = fenced_code_block_style_for(text).to_string();
            }
            out.add_error_detail_if(
                start_line,
                &expected_style,
                fenced_code_block_style_for(text),
                None,
                None,
                None,
                None,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::{LintOptions, lint_content};
    use crate::rules::lint_rule;
    use serde_json::json;

    fn lint_with(params: serde_json::Value, content: &str) -> Vec<crate::error::LintError> {
        let config = json!({ "default": false, "MD048": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md048_consistent_uses_first_fence() {
        assert!(lint_rule("MD048", "```\na\n```\n\n```\nb\n```\n").is_empty());
        assert!(lint_rule("MD048", "~~~\na\n~~~\n\n~~~\nb\n~~~\n").is_empty());
        let errs = lint_rule("MD048", "~~~\na\n~~~\n\n```\nb\n```\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 5);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: tilde; Actual: backtick")
        );
        assert!(errs[0].fix_info.is_none());
    }

    #[test]
    fn md048_backtick_style() {
        let content = "```\na\n```\n\n~~~\nb\n~~~\n";
        let errs = lint_with(json!({ "style": "backtick" }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 5);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: backtick; Actual: tilde")
        );
    }

    #[test]
    fn md048_tilde_style() {
        let content = "```\na\n```\n\n~~~\nb\n~~~\n";
        let errs = lint_with(json!({ "style": "tilde" }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: tilde; Actual: backtick")
        );
    }

    #[test]
    fn md048_falsy_style_falls_back_to_consistent() {
        let content = "~~~\na\n~~~\n\n```\nb\n```\n";
        assert_eq!(lint_with(json!({ "style": "" }), content).len(), 1);
        assert_eq!(lint_with(json!({ "style": null }), content).len(), 1);
    }

    #[test]
    fn md048_indented_and_inline_code_ignored() {
        assert!(lint_rule("MD048", "    code\n\n`a` and ~~~ text\n").is_empty());
    }

    #[test]
    fn md048_fence_in_blockquote_and_list() {
        let errs = lint_rule(
            "MD048",
            "```\na\n```\n\n> ~~~\n> b\n> ~~~\n\n- ~~~\n  c\n  ~~~\n",
        );
        assert_eq!(
            errs.iter().map(|e| e.line_number).collect::<Vec<_>>(),
            vec![5, 9]
        );
    }
}
