use super::{LintContext, Rule, RuleMeta, front_matter_has_title};
use crate::config::{to_number, truthy};
use crate::error::ErrorSink;
use crate::parser::NON_CONTENT_TOKENS;

pub(crate) struct Md025;

static META: RuleMeta = RuleMeta {
    names: &["MD025", "single-title", "single-h1"],
    description: "Multiple top-level headings in the same document",
    tags: &["headings"],
    needs_tokens: true,
    fixable: false,
};

impl Rule for Md025 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let level = ctx
            .config
            .get("level")
            .filter(|v| truthy(v))
            .map_or(1.0, to_number);
        let matching_headings: Vec<_> = ctx
            .tokens
            .filter_by_types(&["atxHeading", "setextHeading"])
            .into_iter()
            .filter(|&id| {
                level == ctx.tokens.heading_level(id) as f64 && !ctx.tokens.is_docfx_tab(id)
            })
            .collect();
        if matching_headings.is_empty() {
            return;
        }
        let found_front_matter_title =
            front_matter_has_title(ctx.front_matter, ctx.config.get("front_matter_title"));
        // front matter 의 title 도 최상위 heading 으로 센다
        let mut has_top_level_heading = found_front_matter_title;
        if !has_top_level_heading {
            let roots = &ctx.tokens.roots;
            // JS `indexOf` 가 -1 이면 `slice(0, -1)` 이라 마지막 하나만 빠진다
            let end = roots
                .iter()
                .position(|&id| id == matching_headings[0])
                .unwrap_or_else(|| roots.len().saturating_sub(1));
            has_top_level_heading = roots[..end].iter().all(|&id| {
                NON_CONTENT_TOKENS.contains(&ctx.tokens.get(id).kind)
                    || ctx.tokens.is_html_flow_comment(id)
            });
        }
        if has_top_level_heading {
            let skip = if found_front_matter_title { 0 } else { 1 };
            for &id in &matching_headings[skip..] {
                out.add_error_context(
                    ctx.tokens.get(id).start_line,
                    &ctx.tokens.heading_text(id),
                    false,
                    false,
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
        let config = json!({ "default": false, "MD025": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md025_multiple_top_level_headings() {
        let errs = lint_rule("MD025", "# One\n\n# Two\n\n# Three\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(errs[0].error_context.as_deref(), Some("Two"));
        assert_eq!(errs[1].line_number, 5);
    }

    #[test]
    fn md025_no_error_when_document_does_not_start_with_h1() {
        // 첫 h1 앞에 내용이 있으면 최상위 heading 문서가 아니라 보고하지 않는다
        assert!(lint_rule("MD025", "Text\n\n# One\n\n# Two\n").is_empty());
        // 빈 줄과 HTML 주석은 내용으로 치지 않는다
        let errs = lint_rule("MD025", "<!-- comment -->\n\n# One\n\n# Two\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 5);
    }

    #[test]
    fn md025_level_parameter() {
        let content = "## One\n\n## Two\n";
        assert!(lint_rule("MD025", content).is_empty());
        let errs = lint_with(json!({ "level": 2 }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
    }

    #[test]
    fn md025_front_matter_title_counts_as_top_level() {
        let content = "---\ntitle: t\n---\n\n# One\n";
        let errs = lint_rule("MD025", content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 5);
        // falsy 패턴은 front matter 를 무시한다
        assert!(lint_with(json!({ "front_matter_title": "" }), content).is_empty());
    }
}
