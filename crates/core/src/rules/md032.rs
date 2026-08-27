use super::{LintContext, Rule, RuleMeta, is_blank_line};
use crate::error::{ErrorSink, FixInfo};
use crate::parser::{NON_CONTENT_TOKENS, TokenId, TokenTree};

pub(crate) struct Md032;

static META: RuleMeta = RuleMeta {
    names: &["MD032", "blanks-around-lists"],
    description: "Lists should be surrounded by blank lines",
    tags: &["bullet", "ul", "ol", "blank_lines"],
    needs_tokens: true,
    fixable: true,
};

/// 원본 `isList`: 목록 토큰인지.
fn is_list(tree: &TokenTree, id: TokenId) -> bool {
    matches!(tree.get(id).kind, "listOrdered" | "listUnordered")
}

fn is_non_content(tree: &TokenTree, id: TokenId) -> bool {
    NON_CONTENT_TOKENS.contains(&tree.get(id).kind)
}

impl Rule for Md032 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let lines = ctx.lines;
        let tokens = ctx.tokens;
        // JS 는 범위 밖 인덱스가 undefined 라 isBlankLine 이 true 를 준다.
        let blank_at = |index: isize| -> bool {
            index < 0
                || lines
                    .get(index as usize)
                    .is_none_or(|line| is_blank_line(line))
        };
        let block_quote_prefixes = tokens.filter_by_types(&["blockQuotePrefix", "linePrefix"]);

        // For every top-level list...
        let top_level_lists = tokens.filter_by_predicate(&tokens.roots, is_list, |tree, id| {
            // 목록과 htmlFlow 아래로는 내려가지 않는다
            if is_list(tree, id) || tree.get(id).kind == "htmlFlow" {
                Vec::new()
            } else {
                tree.get(id).children.clone()
            }
        });
        for list in top_level_lists {
            // Look for a blank line above the list
            let first_line_number = tokens.get(list).start_line;
            if !blank_at(first_line_number as isize - 2) {
                out.add_error_context(
                    first_line_number,
                    lines[first_line_number - 1].trim(),
                    false,
                    false,
                    None,
                    Some(FixInfo {
                        insert_text: Some(tokens.block_quote_prefix_text(
                            &block_quote_prefixes,
                            first_line_number,
                            1,
                        )),
                        ..Default::default()
                    }),
                );
            }

            // Find the "visual" end of the list
            let flattened_children = tokens.filter_by_predicate(
                &tokens.get(list).children,
                |tree, id| !is_non_content(tree, id),
                |tree, id| {
                    if is_non_content(tree, id) {
                        Vec::new()
                    } else {
                        tree.get(id).children.clone()
                    }
                },
            );
            let mut end_line = tokens.get(list).end_line;
            if let Some(&last) = flattened_children.last() {
                end_line = tokens.get(last).end_line;
            }

            // Look for a blank line below the list
            let last_line_number = end_line;
            if !blank_at(last_line_number as isize) {
                out.add_error_context(
                    last_line_number,
                    lines[last_line_number - 1].trim(),
                    false,
                    false,
                    None,
                    Some(FixInfo {
                        line_number: Some(last_line_number + 1),
                        insert_text: Some(tokens.block_quote_prefix_text(
                            &block_quote_prefixes,
                            last_line_number,
                            1,
                        )),
                        ..Default::default()
                    }),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::lint_rule;

    #[test]
    fn md032_reports_missing_blank_lines_around_list() {
        let errs = lint_rule("MD032", "Text\n* Item\n# Heading\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line_number, 2);
        assert_eq!(errs[0].error_context.as_deref(), Some("* Item"));
        let fix = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(fix.line_number, None);
        assert_eq!(fix.insert_text.as_deref(), Some("\n"));
        assert_eq!(errs[1].line_number, 2);
        assert_eq!(errs[1].fix_info.as_ref().unwrap().line_number, Some(3));
    }

    #[test]
    fn md032_lazy_continuation_belongs_to_the_list() {
        // 뒤따르는 줄이 목록 항목의 lazy continuation 이면 목록의 끝이 밀려 아래쪽 오류가 없다.
        let errs = lint_rule("MD032", "Text\n* Item\nText\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 2);
    }

    #[test]
    fn md032_surrounded_list_is_clean() {
        assert!(lint_rule("MD032", "Text\n\n* Item\n\nText\n").is_empty());
    }

    #[test]
    fn md032_nested_list_is_not_reported() {
        let errs = lint_rule("MD032", "Text\n\n* Item\n  * Nested\n\nText\n");
        assert!(errs.is_empty());
    }

    #[test]
    fn md032_uses_visual_end_of_list() {
        // 목록 뒤에 붙은 후행 공백 줄은 목록 토큰에 포함되지만 "시각적" 끝은 마지막 내용 줄이다.
        let errs = lint_rule("MD032", "Text\n\n* Item\n\n\nText\n");
        assert!(errs.is_empty());
    }

    #[test]
    fn md032_blockquote_prefix_is_inserted() {
        let errs = lint_rule("MD032", "> Text\n> * Item\n> # Heading\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].error_context.as_deref(), Some("> * Item"));
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some(">\n")
        );
        assert_eq!(
            errs[1].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some(">\n")
        );
    }
}
