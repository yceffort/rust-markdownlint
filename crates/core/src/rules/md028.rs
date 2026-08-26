use super::{LintContext, Rule, RuleMeta};
use crate::error::ErrorSink;

pub(crate) struct Md028;

static META: RuleMeta = RuleMeta {
    names: &["MD028", "no-blanks-blockquote"],
    description: "Blank line inside blockquote",
    tags: &["blockquote", "whitespace"],
    needs_tokens: true,
    fixable: false,
};

/// 원본 `ignoreTypes`: 무시할 무형 포맷 토큰.
const IGNORE_TYPES: &[&str] = &["lineEnding", "listItemIndent", "linePrefix"];

impl Rule for Md028 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let tokens = ctx.tokens;
        for token_id in tokens.filter_by_types(&["blockQuote"]) {
            let mut error_line_numbers = Vec::new();
            let parent = tokens.get(token_id).parent;
            let siblings: &[usize] = match parent {
                Some(p) => &tokens.get(p).children,
                None => &tokens.roots,
            };
            let index = siblings.iter().position(|&s| s == token_id).unwrap();
            for &sibling_id in &siblings[index + 1..] {
                let sibling = tokens.get(sibling_id);
                if sibling.kind == "lineEndingBlank" {
                    // Possible blank between blockquotes
                    error_line_numbers.push(sibling.start_line);
                } else if IGNORE_TYPES.contains(&sibling.kind.as_str()) {
                    // Ignore invisible formatting
                } else if sibling.kind == "blockQuote" {
                    // Blockquote followed by blockquote
                    for &line_number in &error_line_numbers {
                        out.add_error(line_number, None, None, None, None);
                    }
                    break;
                } else {
                    // Blockquote not followed by blockquote
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::lint_rule;

    #[test]
    fn md028_blank_line_between_blockquotes_is_flagged() {
        let errs = lint_rule("MD028", "> a\n\n> b\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 2);
    }

    #[test]
    fn md028_blockquote_followed_by_text_is_clean() {
        assert!(lint_rule("MD028", "> a\n\ntext\n").is_empty());
    }

    #[test]
    fn md028_blank_line_inside_single_blockquote_is_clean() {
        assert!(lint_rule("MD028", "> a\n>\n> b\n").is_empty());
    }

    #[test]
    fn md028_multiple_blank_lines_between_blockquotes_flags_all() {
        let errs = lint_rule("MD028", "> a\n\n\n> b\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line_number, 2);
        assert_eq!(errs[1].line_number, 3);
    }

    #[test]
    fn md028_nested_blockquote_bare_marker_is_not_flagged() {
        // 중첩 블록인용 사이의 빈 ">" 줄은 blockQuotePrefix 형제로 나타나
        // lineEndingBlank 를 만나기 전에 순회가 멈춘다 (원본과 동일한 동작).
        assert!(lint_rule("MD028", "> > a\n>\n> > b\n").is_empty());
    }
}
