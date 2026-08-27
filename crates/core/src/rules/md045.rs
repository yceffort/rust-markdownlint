use std::sync::LazyLock;

use regex::Regex;

use super::{LintContext, NEXT_LINES_RE, Rule, RuleMeta};
use crate::error::{ErrorSink, utf16_len};
use crate::parser::html_attribute_re;

pub(crate) struct Md045;

static META: RuleMeta = RuleMeta {
    names: &["MD045", "no-alt-text"],
    description: "Images should have alternate text (alt text)",
    tags: &["accessibility", "images"],
    needs_tokens: true,
    fixable: false,
};

/// 원본 `altRe = getHtmlAttributeRe("alt")`.
static ALT_RE: LazyLock<Regex> = LazyLock::new(|| html_attribute_re("alt"));

/// 원본 `ariaHiddenRe = getHtmlAttributeRe("aria-hidden")`.
static ARIA_HIDDEN_RE: LazyLock<Regex> = LazyLock::new(|| html_attribute_re("aria-hidden"));

impl Rule for Md045 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        // Markdown 이미지 처리
        for id in ctx.tokens.filter_by_types(&["image"]) {
            let image = ctx.tokens.get(id);
            let label_texts = ctx
                .tokens
                .descendants_by_type(id, &[&["label"], &["labelText"]]);
            if label_texts
                .iter()
                .any(|&label_text| ctx.tokens.text(label_text).is_empty())
            {
                let range = (image.start_line == image.end_line)
                    .then(|| (image.start_column, image.end_column - image.start_column));
                out.add_error(image.start_line, None, None, range, None);
            }
        }

        // HTML 이미지 처리
        for id in ctx.tokens.filter_by_types_html_flow(&["htmlText"], true) {
            let html_text = ctx.tokens.get(id);
            let text = ctx.tokens.text(id);
            let Some(html_tag_info) = ctx.tokens.html_tag_info(id) else {
                continue;
            };
            if !html_tag_info.close
                && html_tag_info.name.to_lowercase() == "img"
                && !ALT_RE.is_match(text)
                && ARIA_HIDDEN_RE
                    .captures(text)
                    .is_none_or(|c| c[1].to_lowercase() != "true")
            {
                // 원본 `text.replace(nextLinesRe, "").length`: UTF-16 단위
                let range = (
                    html_text.start_column,
                    utf16_len(&NEXT_LINES_RE.replace(text, "")),
                );
                out.add_error(html_text.start_line, None, None, Some(range), None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::lint_rule;

    #[test]
    fn md045_markdown_image_without_alt() {
        let errs = lint_rule("MD045", "![](image.jpg)\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_range, Some((1, 14)));
        assert!(errs[0].fix_info.is_none());
    }

    #[test]
    fn md045_markdown_image_with_alt_ok() {
        assert!(lint_rule("MD045", "![Alt text](image.jpg)\n").is_empty());
        assert!(lint_rule("MD045", "![Alt text][ref]\n\n[ref]: image.jpg\n").is_empty());
    }

    #[test]
    fn md045_multiline_image_has_no_range() {
        let errs = lint_rule("MD045", "![](image.jpg\n\"title\")\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_range, None);
    }

    #[test]
    fn md045_html_img_without_alt() {
        let errs = lint_rule("MD045", "Text <img src=\"image.jpg\"> more\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_range, Some((6, 21)));
    }

    #[test]
    fn md045_html_img_with_alt_or_aria_hidden_ok() {
        assert!(lint_rule("MD045", "<img src=\"a.jpg\" alt=\"\">\n").is_empty());
        assert!(lint_rule("MD045", "<IMG SRC=\"a.jpg\" ALT=\"x\">\n").is_empty());
        assert!(lint_rule("MD045", "<img src=\"a.jpg\" aria-hidden=\"TRUE\">\n").is_empty());
        assert_eq!(
            lint_rule("MD045", "<img src=\"a.jpg\" aria-hidden=\"false\">\n").len(),
            1
        );
    }

    #[test]
    fn md045_html_img_multiline_range_stops_at_newline() {
        let errs = lint_rule("MD045", "Text <img\nsrc=\"image.jpg\"> more\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_range, Some((6, 4)));
    }

    #[test]
    fn md045_html_img_range_length_is_utf16() {
        // 기대값은 cli2 0.22.1 실행 결과 (`<img src="🎸.png">` 는 UTF-16 18단위)
        let errs = lint_rule("MD045", "<img src=\"🎸.png\">\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_range, Some((1, 18)));
    }
}
