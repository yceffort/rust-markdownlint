use super::{LintContext, Rule, RuleMeta};
use crate::error::ErrorSink;

pub(crate) struct Md003;

static META: RuleMeta = RuleMeta {
    names: &["MD003", "heading-style"],
    description: "Heading style",
    tags: &["headings"],
    needs_tokens: true,
    fixable: false,
};

impl Rule for Md003 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let mut style = ctx
            .config
            .get("style")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("consistent")
            .to_string();
        for id in ctx.tokens.filter_by_types(&["atxHeading", "setextHeading"]) {
            let heading = ctx.tokens.get(id);
            let style_for_token = ctx.tokens.heading_style(id);
            if style == "consistent" {
                style = style_for_token.to_string();
            }
            if style_for_token != style {
                let h12 = ctx.tokens.heading_level(id) <= 2;
                let setext_with_atx = style == "setext_with_atx"
                    && ((h12 && style_for_token == "setext") || (!h12 && style_for_token == "atx"));
                let setext_with_atx_closed = style == "setext_with_atx_closed"
                    && ((h12 && style_for_token == "setext")
                        || (!h12 && style_for_token == "atx_closed"));
                if !setext_with_atx && !setext_with_atx_closed {
                    let expected = if style == "setext_with_atx" {
                        if h12 { "setext" } else { "atx" }
                    } else if style == "setext_with_atx_closed" {
                        if h12 { "setext" } else { "atx_closed" }
                    } else {
                        style.as_str()
                    };
                    out.add_error_detail_if(
                        heading.start_line,
                        expected,
                        style_for_token,
                        None,
                        None,
                        None,
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
        let config = json!({ "default": false, "MD003": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md003_consistent_ok() {
        assert!(lint_rule("MD003", "# H1\n\n## H2\n").is_empty());
    }

    /// 본문 첫 글자가 `#` 이어도 (micromark `resolveHeadingAtx`) 닫는 시퀀스가 아니다.
    #[test]
    fn md003_hash_at_start_of_text_is_not_closed() {
        assert!(lint_rule("MD003", "# H1\n\n## # a\n").is_empty());
    }

    #[test]
    fn md003_consistent_mismatch() {
        let errs = lint_rule("MD003", "# H1\n\n## H2 ##\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: atx; Actual: atx_closed")
        );
    }

    #[test]
    fn md003_style_atx_closed() {
        let errs = lint_with(json!({ "style": "atx_closed" }), "# H1 #\n\n## H2\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: atx_closed; Actual: atx")
        );
    }

    #[test]
    fn md003_setext_with_atx_allows_h3_atx() {
        let content = "H1\n==\n\nH2\n--\n\n### H3\n";
        assert!(lint_with(json!({ "style": "setext_with_atx" }), content).is_empty());
    }

    #[test]
    fn md003_setext_with_atx_rejects_h1_atx() {
        let content = "H1\n==\n\n## H2\n";
        let errs = lint_with(json!({ "style": "setext_with_atx" }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 4);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: setext; Actual: atx")
        );
    }
}
