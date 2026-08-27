use super::{LintContext, Rule, RuleMeta};
use crate::error::{ErrorSink, FixInfo};
use crate::parser::TokenId;

pub(crate) struct Md027;

static META: RuleMeta = RuleMeta {
    names: &["MD027", "no-multiple-space-blockquote"],
    description: "Multiple spaces after blockquote symbol",
    tags: &["blockquote", "whitespace", "indentation"],
    needs_tokens: true,
    fixable: true,
};

const LIST_TYPES: &[&str] = &["listOrdered", "listUnordered"];

impl Rule for Md027 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let list_items = ctx.config.get("list_items");
        let include_list_items = list_items.is_none_or(crate::config::truthy);
        let tokens = ctx.tokens;

        for token_id in tokens.filter_by_types(&["linePrefix"]) {
            let token = tokens.get(token_id);
            let parent = token.parent;
            let code_indented = parent.is_some_and(|p| tokens.get(p).kind == "codeIndented");
            let siblings: &[TokenId] = match parent {
                Some(p) => &tokens.get(p).children,
                None => &tokens.roots,
            };
            // JS `indexOf` 는 못 찾으면 -1. 음수 인덱스 접근은 undefined 가 된다.
            let index = siblings
                .iter()
                .position(|&id| id == token_id)
                .map(|i| i as isize)
                .unwrap_or(-1);
            let sibling_at = |offset: isize| -> Option<TokenId> {
                usize::try_from(index + offset)
                    .ok()
                    .and_then(|i| siblings.get(i).copied())
            };
            let prev_is_block_quote_prefix =
                sibling_at(-1).is_some_and(|id| tokens.get(id).kind == "blockQuotePrefix");

            if code_indented || !prev_is_block_quote_prefix {
                continue;
            }

            let allowed_by_list_items = include_list_items || {
                let next_is_list =
                    sibling_at(1).is_some_and(|id| LIST_TYPES.contains(&tokens.get(id).kind));
                !next_is_list && tokens.parent_of_type(token_id, LIST_TYPES).is_none()
            };

            if !allowed_by_list_items {
                continue;
            }

            let start_column = token.start_column;
            let start_line = token.start_line;
            let length = tokens.text_of(token).chars().count();
            let line = ctx.lines[start_line - 1];
            out.add_error_context(
                start_line,
                line,
                false,
                false,
                Some((start_column, length)),
                Some(FixInfo {
                    edit_column: Some(start_column),
                    delete_count: Some(length as isize),
                    ..Default::default()
                }),
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
        let config = json!({ "default": false, "MD027": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md027_multiple_spaces_after_marker() {
        let errs = lint_rule("MD027", ">  Multiple spaces\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_range, Some((3, 1)));
        let fix = errs[0].fix_info.as_ref().unwrap();
        assert_eq!((fix.edit_column, fix.delete_count), (Some(3), Some(1)));
    }

    #[test]
    fn md027_single_space_is_ok() {
        assert!(lint_rule("MD027", "> Single space\n").is_empty());
    }

    #[test]
    fn md027_list_items_default_reports() {
        let errs = lint_rule("MD027", ">  - item\n");
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn md027_list_items_false_skips_list() {
        assert!(lint_with(json!({ "list_items": false }), ">  - item\n").is_empty());
        // 리스트 항목이 아니면 여전히 보고한다.
        let errs = lint_with(json!({ "list_items": false }), ">  Text\n");
        assert_eq!(errs.len(), 1);
    }

    #[test]
    fn md027_indented_code_block_is_ignored() {
        // codeIndented 의 linePrefix 는 대상이 아니다.
        assert!(lint_rule("MD027", ">     code\n").is_empty());
    }
}
