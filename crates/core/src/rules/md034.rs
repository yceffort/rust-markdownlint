use super::{LintContext, Rule, RuleMeta};
use crate::error::{ErrorSink, FixInfo};
use crate::parser::{TokenId, TokenTree};

pub(crate) struct Md034;

static META: RuleMeta = RuleMeta {
    names: &["MD034", "no-bare-urls"],
    description: "Bare URL used",
    tags: &["links", "url"],
    needs_tokens: true,
    fixable: true,
};

/// 원본 `literalAutolinks` 의 `allowed` 조건.
/// micromark 의 <https://github.com/micromark/micromark/issues/164> 를 우회하려고
/// `<`, `>` 로 감싼 data 형제 사이에 낀 literalAutolink 는 무시한다.
fn allowed(tree: &TokenTree, id: TokenId) -> bool {
    let token = tree.get(id);
    if token.kind != "literalAutolink" || token.in_html_flow {
        return false;
    }
    let Some(parent) = token.parent else {
        // JS 는 siblings 가 undefined 라 prev/next 도 undefined 가 되어 통과한다.
        return true;
    };
    let siblings = &tree.get(parent).children;
    let index = siblings
        .iter()
        .position(|&s| s == id)
        .expect("child of parent");
    // JS `at(index - 1)`: index 가 0 이면 마지막 원소를 돌려준다.
    let prev = if index == 0 {
        siblings.last().copied()
    } else {
        siblings.get(index - 1).copied()
    };
    let next = siblings.get(index + 1).copied();
    let (Some(prev), Some(next)) = (prev, next) else {
        return true;
    };
    let prev = tree.get(prev);
    let next = tree.get(next);
    !(prev.kind == "data"
        && next.kind == "data"
        && prev.text.ends_with('<')
        && next.text.starts_with('>'))
}

/// 원본 `literalAutolinks` 의 `transformChildren`: 인라인 HTML 태그 안의 내용은 건너뛴다.
fn transform_children(tree: &TokenTree, id: TokenId) -> Vec<TokenId> {
    let children = &tree.get(id).children;
    let mut result = Vec::new();
    let mut i = 0;
    while i < children.len() {
        let current = children[i];
        let open_tag_info = tree.html_tag_info(current);
        match open_tag_info {
            Some(open_tag_info) if !open_tag_info.close => {
                let mut count = 1;
                for (j, &candidate) in children.iter().enumerate().skip(i + 1) {
                    if let Some(close_tag_info) = tree.html_tag_info(candidate)
                        && open_tag_info.name == close_tag_info.name
                    {
                        if close_tag_info.close {
                            count -= 1;
                            if count == 0 {
                                i = j;
                                break;
                            }
                        } else {
                            count += 1;
                        }
                    }
                }
            }
            _ => result.push(current),
        }
        i += 1;
    }
    result
}

/// 원본 `filterByPredicate(tokens, allowed, transformChildren)`: 전위 순회.
fn filter_by_predicate(tree: &TokenTree, tokens: &[TokenId], result: &mut Vec<TokenId>) {
    for &id in tokens {
        if allowed(tree, id) {
            result.push(id);
        }
        if !tree.get(id).children.is_empty() {
            let transformed = transform_children(tree, id);
            filter_by_predicate(tree, &transformed, result);
        }
    }
}

impl Rule for Md034 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let mut literal_autolinks = Vec::new();
        filter_by_predicate(ctx.tokens, &ctx.tokens.roots, &mut literal_autolinks);
        for id in literal_autolinks {
            let token = ctx.tokens.get(id);
            let range = (token.start_column, token.end_column - token.start_column);
            out.add_error_context(
                token.start_line,
                &token.text,
                false,
                false,
                Some(range),
                Some(FixInfo {
                    edit_column: Some(range.0),
                    delete_count: Some(range.1 as isize),
                    insert_text: Some(format!("<{}>", token.text)),
                    ..Default::default()
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::lint_rule;

    #[test]
    fn md034_bare_url_reported_with_fix() {
        let errs = lint_rule("MD034", "For more info, see https://example.com/x here.\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(
            errs[0].error_context.as_deref(),
            Some("https://example.com/x")
        );
        assert_eq!(errs[0].error_range, Some((20, 21)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(f.edit_column, Some(20));
        assert_eq!(f.delete_count, Some(21));
        assert_eq!(f.insert_text.as_deref(), Some("<https://example.com/x>"));
    }

    #[test]
    fn md034_angle_bracket_and_markdown_links_are_fine() {
        assert!(lint_rule("MD034", "<https://example.com>\n").is_empty());
        assert!(lint_rule("MD034", "[text](https://example.com)\n").is_empty());
    }

    #[test]
    fn md034_code_spans_and_fences_are_fine() {
        assert!(lint_rule("MD034", "`https://example.com`\n").is_empty());
        assert!(lint_rule("MD034", "```\nhttps://example.com\n```\n").is_empty());
    }

    #[test]
    fn md034_inline_html_tag_content_ignored() {
        // 여는 태그와 닫는 태그 사이의 내용은 transformChildren 이 건너뛴다.
        assert!(lint_rule("MD034", "<a href=\"x\">https://example.com</a>\n").is_empty());
    }

    #[test]
    fn md034_www_and_email_autolinks() {
        let errs = lint_rule("MD034", "www.example.com and user@example.com\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].error_context.as_deref(), Some("www.example.com"));
        assert_eq!(errs[1].error_context.as_deref(), Some("user@example.com"));
    }

    #[test]
    fn md034_html_flow_is_ignored() {
        assert!(lint_rule("MD034", "<div>\nhttps://example.com\n</div>\n").is_empty());
    }
}
