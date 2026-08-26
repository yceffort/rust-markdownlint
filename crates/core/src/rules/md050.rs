use super::md049::{check_style, style_config};
use super::{LintContext, Rule, RuleMeta};
use crate::error::ErrorSink;

pub(crate) struct Md050;

static META: RuleMeta = RuleMeta {
    names: &["MD050", "strong-style"],
    description: "Strong style",
    tags: &["emphasis"],
    needs_tokens: true,
    fixable: true,
};

impl Rule for Md050 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        check_style(
            ctx,
            out,
            "strong",
            "strongSequence",
            "**",
            "__",
            style_config(ctx),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::{LintOptions, lint_content};
    use crate::rules::lint_rule;
    use serde_json::json;

    fn lint_with(params: serde_json::Value, content: &str) -> Vec<crate::error::LintError> {
        let config = json!({ "default": false, "MD050": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md050_consistent_reports_second_style() {
        let errs = lint_rule("MD050", "**one** and __two__\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: asterisk; Actual: underscore")
        );
        assert_eq!(errs[0].error_range, Some((13, 2)));
        assert_eq!(errs[1].error_range, Some((18, 2)));
        let fix = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(fix.edit_column, Some(13));
        assert_eq!(fix.delete_count, Some(2));
        assert_eq!(fix.insert_text.as_deref(), Some("**"));
    }

    #[test]
    fn md050_consistent_same_style_is_ok() {
        assert!(lint_rule("MD050", "**one** and **two**\n").is_empty());
        assert!(lint_rule("MD050", "__one__ and __two__\n").is_empty());
    }

    #[test]
    fn md050_explicit_style() {
        let errs = lint_with(json!({ "style": "underscore" }), "**one**\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: underscore; Actual: asterisk")
        );
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some("__")
        );
        assert!(lint_with(json!({ "style": "asterisk" }), "**one**\n").is_empty());
    }

    #[test]
    fn md050_underscore_intraword_is_skipped() {
        assert!(lint_with(json!({ "style": "underscore" }), "a**b**c\n").is_empty());
        assert_eq!(
            lint_with(json!({ "style": "underscore" }), "a **b** c\n").len(),
            2
        );
    }

    #[test]
    fn md050_ignores_emphasis_and_html_flow() {
        assert!(lint_rule("MD050", "**one** and _two_\n").is_empty());
        assert!(lint_rule("MD050", "**one**\n\n<div>\n__two__\n</div>\n").is_empty());
    }
}
