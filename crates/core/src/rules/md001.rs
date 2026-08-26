use super::{LintContext, Rule, RuleMeta, front_matter_has_title};
use crate::error::ErrorSink;

pub(crate) struct Md001;

static META: RuleMeta = RuleMeta {
    names: &["MD001", "heading-increment"],
    description: "Heading levels should only increment by one level at a time",
    tags: &["headings"],
    needs_tokens: true,
    fixable: false,
};

impl Rule for Md001 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let has_title =
            front_matter_has_title(ctx.front_matter, ctx.config.get("front_matter_title"));
        // 원본의 `Number.MAX_SAFE_INTEGER` 자리. 레벨은 최대 6 이라 비교 결과는 같다.
        let mut prev_level = if has_title { 1 } else { usize::MAX };
        for id in ctx.tokens.filter_by_types(&["atxHeading", "setextHeading"]) {
            let level = ctx.tokens.heading_level(id);
            if level > prev_level {
                out.add_error_detail_if(
                    ctx.tokens.get(id).start_line,
                    format!("h{}", prev_level + 1),
                    format!("h{level}"),
                    None,
                    None,
                    None,
                    None,
                );
            }
            prev_level = level;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::{LintOptions, lint_content};
    use crate::rules::lint_rule;
    use serde_json::json;

    fn lint_with(params: serde_json::Value, content: &str) -> Vec<crate::error::LintError> {
        let config = json!({ "default": false, "MD001": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md001_skipped_level() {
        let errs = lint_rule("MD001", "# h1\n\n### h3\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: h2; Actual: h3")
        );
    }

    #[test]
    fn md001_first_heading_any_level() {
        assert!(lint_rule("MD001", "### h3\n\n#### h4\n").is_empty());
    }

    #[test]
    fn md001_setext_heading_levels() {
        let errs = lint_rule("MD001", "h1\n==\n\n### h3\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 4);
        assert!(lint_rule("MD001", "h1\n==\n\nh2\n--\n").is_empty());
    }

    #[test]
    fn md001_front_matter_title_counts_as_h1() {
        let content = "---\ntitle: t\n---\n\n### h3\n";
        let errs = lint_rule("MD001", content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 5);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: h2; Actual: h3")
        );
    }

    #[test]
    fn md001_front_matter_title_pattern() {
        let content = "---\nalternate: t\n---\n\n### h3\n";
        assert!(lint_rule("MD001", content).is_empty());
        assert_eq!(
            lint_with(
                json!({ "front_matter_title": "^\\s*alternate\\s*:" }),
                content
            )
            .len(),
            1
        );
        // falsy 패턴은 front matter 를 무시한다
        let titled = "---\ntitle: t\n---\n\n### h3\n";
        assert!(lint_with(json!({ "front_matter_title": "" }), titled).is_empty());
    }
}
