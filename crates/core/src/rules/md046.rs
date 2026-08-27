use super::{LintContext, Rule, RuleMeta};
use crate::config::truthy;
use crate::error::ErrorSink;
use serde_json::Value;

pub(crate) struct Md046;

static META: RuleMeta = RuleMeta {
    names: &["MD046", "code-block-style"],
    description: "Code block style",
    tags: &["code"],
    needs_tokens: true,
    fixable: false,
};

/// 원본 `tokenTypeToStyle`: `codeFenced` 면 "fenced", 아니면 "indented".
fn token_type_to_style(kind: &str) -> &'static str {
    if kind == "codeFenced" {
        "fenced"
    } else {
        "indented"
    }
}

impl Rule for Md046 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        // 원본: `String(params.config.style || "consistent")`
        let mut expected_style = match ctx.config.get("style").filter(|v| truthy(v)) {
            Some(Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => "consistent".to_string(),
        };
        for id in ctx.tokens.filter_by_types(&["codeFenced", "codeIndented"]) {
            let token = ctx.tokens.get(id);
            if expected_style == "consistent" {
                expected_style = token_type_to_style(token.kind).to_string();
            }
            out.add_error_detail_if(
                token.start_line,
                &expected_style,
                token_type_to_style(token.kind),
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
        let config = json!({ "default": false, "MD046": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md046_consistent_default_ok() {
        assert!(lint_rule("MD046", "    code one\n\n    code two\n").is_empty());
    }

    #[test]
    fn md046_consistent_default_mismatch() {
        let errs = lint_rule("MD046", "    code one\n\n```\ncode two\n```\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: indented; Actual: fenced")
        );
    }

    #[test]
    fn md046_configured_style() {
        let content = "    code one\n\n```\ncode two\n```\n";
        let errs = lint_with(json!({ "style": "fenced" }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: fenced; Actual: indented")
        );
    }

    #[test]
    fn md046_falsy_style_falls_back_to_consistent() {
        // 빈 문자열은 falsy 이므로 "consistent" 로 취급한다
        let content = "```\ncode one\n```\n\n    code two\n";
        let errs = lint_with(json!({ "style": "" }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 5);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: fenced; Actual: indented")
        );
    }

    #[test]
    fn md046_no_code_blocks_is_ok() {
        assert!(lint_rule("MD046", "text only, no code blocks\n").is_empty());
    }
}
