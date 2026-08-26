use super::{LintContext, Rule, RuleMeta};
use crate::config::truthy;
use crate::error::ErrorSink;
use serde_json::Value;

pub(crate) struct Md035;

static META: RuleMeta = RuleMeta {
    names: &["MD035", "hr-style"],
    description: "Horizontal rule style",
    tags: &["hr"],
    needs_tokens: true,
    fixable: false,
};

impl Rule for Md035 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        // 원본: `String(params.config.style || "consistent").trim()`
        let mut style = match ctx.config.get("style").filter(|v| truthy(v)) {
            Some(Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => "consistent".to_string(),
        };
        style = style.trim().to_string();
        for id in ctx.tokens.filter_by_types(&["thematicBreak"]) {
            let token = ctx.tokens.get(id);
            let text = token.text.clone();
            if style == "consistent" {
                style = text.clone();
            }
            out.add_error_detail_if(token.start_line, &style, &text, None, None, None, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::{LintOptions, lint_content};
    use crate::rules::lint_rule;
    use serde_json::json;

    fn lint_with(params: serde_json::Value, content: &str) -> Vec<crate::error::LintError> {
        let config = json!({ "default": false, "MD035": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md035_consistent_default_ok() {
        assert!(lint_rule("MD035", "---\n\ntext\n\n---\n").is_empty());
    }

    #[test]
    fn md035_consistent_default_mismatch() {
        let errs = lint_rule("MD035", "---\n\ntext\n\n***\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 5);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: ---; Actual: ***")
        );
    }

    #[test]
    fn md035_configured_style() {
        let content = "---\n\ntext\n\n***\n";
        let errs = lint_with(json!({ "style": "***" }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: ***; Actual: ---")
        );
    }

    #[test]
    fn md035_falsy_style_falls_back_to_consistent() {
        // 빈 문자열은 falsy 이므로 "consistent" 로 취급한다
        let content = "* * *\n\ntext\n\n***\n";
        let errs = lint_with(json!({ "style": "" }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 5);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: * * *; Actual: ***")
        );
    }
}
