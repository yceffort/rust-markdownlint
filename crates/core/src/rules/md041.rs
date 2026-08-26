use super::{LintContext, Rule, RuleMeta, front_matter_has_title};
use crate::config::to_number;
use crate::error::ErrorSink;
use crate::parser::{NON_CONTENT_TOKENS, TokenId, TokenTree};

pub(crate) struct Md041;

static META: RuleMeta = RuleMeta {
    names: &["MD041", "first-line-heading", "first-line-h1"],
    description: "First line in a file should be a top-level heading",
    tags: &["headings"],
    needs_tokens: true,
    fixable: false,
};

/// JS `` `h${level}` `` 의 표기. 정수는 소수점 없이 찍는다.
fn heading_tag_name(level: f64) -> String {
    if level.is_nan() {
        "hNaN".to_string()
    } else if level.is_finite() && level.fract() == 0.0 {
        format!("h{}", level as i64)
    } else {
        format!("h{level}")
    }
}

/// 원본 `filterByTypes(children, [ "htmlText" ], true)[0]`.
fn first_html_text(tokens: &TokenTree, id: TokenId) -> Option<TokenId> {
    for &child in &tokens.get(id).children {
        if tokens.get(child).kind == "htmlText" {
            return Some(child);
        }
        if let Some(found) = first_html_text(tokens, child) {
            return Some(found);
        }
    }
    None
}

/// 원본 `getHtmlFlowTagName`: htmlFlow 토큰의 HTML 태그 이름 (소문자).
fn html_flow_tag_name(tokens: &TokenTree, id: TokenId) -> Option<String> {
    if tokens.get(id).kind != "htmlFlow" {
        return None;
    }
    let html_text = first_html_text(tokens, id)?;
    tokens
        .html_tag_info(html_text)
        .map(|info| info.name.to_lowercase())
}

/// 원본 `headingTagNameRe` (`/^h[1-6]$/`).
fn is_heading_tag_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() == 2 && bytes[0] == b'h' && (b'1'..=b'6').contains(&bytes[1])
}

impl Rule for Md041 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let allow_preamble = ctx
            .config
            .get("allow_preamble")
            .is_some_and(crate::config::truthy);
        let level = match ctx.config.get("level").filter(|v| crate::config::truthy(v)) {
            Some(value) => to_number(value),
            None => 1.0,
        };
        if front_matter_has_title(ctx.front_matter, ctx.config.get("front_matter_title")) {
            return;
        }
        let mut error_line_number = 0;
        for &id in &ctx.tokens.roots {
            let token = ctx.tokens.get(id);
            if NON_CONTENT_TOKENS.contains(&token.kind.as_str())
                || ctx.tokens.is_html_flow_comment(id)
            {
                continue;
            }
            if token.kind == "atxHeading" || token.kind == "setextHeading" {
                // 첫 heading 은 기대한 레벨이어야 한다
                if ctx.tokens.heading_level(id) as f64 != level {
                    error_line_number = token.start_line;
                }
                break;
            }
            let tag_name = html_flow_tag_name(ctx.tokens, id);
            if tag_name.as_deref().is_some_and(is_heading_tag_name) {
                // 첫 HTML 요소는 기대한 레벨의 <h?> 여야 한다
                if tag_name.as_deref() != Some(heading_tag_name(level).as_str()) {
                    error_line_number = token.start_line;
                }
                break;
            }
            if !allow_preamble {
                // 첫 내용은 기대한 레벨의 heading 이어야 한다
                error_line_number = token.start_line;
                break;
            }
        }
        if error_line_number > 0 {
            out.add_error_context(
                error_line_number,
                ctx.lines[error_line_number - 1],
                false,
                false,
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
        let config = json!({ "default": false, "MD041": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md041_first_line_heading() {
        assert!(lint_rule("MD041", "# Title\n\ntext\n").is_empty());
        let errs = lint_rule("MD041", "text\n\n# Title\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("text"));
    }

    #[test]
    fn md041_wrong_level() {
        let errs = lint_rule("MD041", "## Title\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert!(lint_with(json!({ "level": 2 }), "## Title\n").is_empty());
    }

    #[test]
    fn md041_html_heading() {
        assert!(lint_rule("MD041", "<h1>Title</h1>\n\ntext\n").is_empty());
        let errs = lint_rule("MD041", "<h3>Title</h3>\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        // heading 이 아닌 HTML 은 preamble 로 취급한다
        assert_eq!(lint_rule("MD041", "<div>x</div>\n\n# Title\n").len(), 1);
    }

    #[test]
    fn md041_allow_preamble_and_comments() {
        assert!(lint_with(json!({ "allow_preamble": true }), "text\n\n# Title\n").is_empty());
        let errs = lint_with(json!({ "allow_preamble": true }), "text\n\n## Title\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        // 선행 HTML 주석은 건너뛴다
        assert!(lint_rule("MD041", "<!-- comment -->\n\n# Title\n").is_empty());
    }

    #[test]
    fn md041_front_matter_title() {
        assert!(lint_rule("MD041", "---\ntitle: t\n---\n\ntext\n").is_empty());
        let errs = lint_rule("MD041", "---\nalternate: t\n---\n\ntext\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 5);
        assert_eq!(
            lint_with(
                json!({ "front_matter_title": "" }),
                "---\ntitle: t\n---\n\ntext\n"
            )
            .len(),
            1
        );
    }
}
