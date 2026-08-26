use std::collections::HashMap;

use super::{LintContext, Rule, RuleMeta};
use crate::config::truthy;
use crate::error::{ErrorSink, FixInfo};
use crate::parser::TokenId;

pub(crate) struct Md007;

static META: RuleMeta = RuleMeta {
    names: &["MD007", "ul-indent"],
    description: "Unordered list indentation",
    tags: &["bullet", "ul", "indentation"],
    needs_tokens: true,
    fixable: true,
};

const UNORDERED_LIST_TYPES: &[&str] = &["blockQuotePrefix", "listItemPrefix", "listUnordered"];
const UNORDERED_PARENT_TYPES: &[&str] = &["blockQuote", "listOrdered", "listUnordered"];

impl Rule for Md007 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let indent = ctx
            .config
            .get("indent")
            .filter(|v| truthy(v))
            .and_then(|v| v.as_i64())
            .unwrap_or(2);
        let start_indented = ctx.config.get("start_indented").is_some_and(truthy);
        let start_indent = ctx
            .config
            .get("start_indent")
            .filter(|v| truthy(v))
            .and_then(|v| v.as_i64())
            .unwrap_or(indent);

        let tokens = ctx.tokens;
        let mut unordered_list_nesting: HashMap<TokenId, i64> = HashMap::new();
        let mut last_block_quote_prefix: Option<TokenId> = None;
        for id in tokens.filter_by_types(UNORDERED_LIST_TYPES) {
            let token = tokens.get(id);
            match token.kind.as_str() {
                "blockQuotePrefix" => last_block_quote_prefix = Some(id),
                "listUnordered" => {
                    let mut nesting = 0i64;
                    let mut current = id;
                    while let Some(parent) = tokens.parent_of_type(current, UNORDERED_PARENT_TYPES)
                    {
                        current = parent;
                        match tokens.get(parent).kind.as_str() {
                            "listUnordered" => {
                                nesting += 1;
                                continue;
                            }
                            "listOrdered" => nesting = -1,
                            _ => {}
                        }
                        break;
                    }
                    if nesting >= 0 {
                        unordered_list_nesting.insert(id, nesting);
                    }
                }
                _ => {
                    // listItemPrefix
                    let nesting = token.parent.and_then(|p| unordered_list_nesting.get(&p));
                    if let Some(&nesting) = nesting {
                        // listItemPrefix for listUnordered
                        let base_indent = if tokens
                            .parent_of_type(id, &["gfmFootnoteDefinition"])
                            .is_some()
                        {
                            4
                        } else {
                            0
                        };
                        let expected_indent = base_indent
                            + (if start_indented { start_indent } else { 0 })
                            + (nesting * indent);
                        let block_quote_adjustment = last_block_quote_prefix
                            .map(|p| tokens.get(p))
                            .filter(|p| p.end_line == token.start_line)
                            .map_or(0, |p| p.end_column as i64 - 1);
                        let actual_indent = token.start_column as i64 - 1 - block_quote_adjustment;
                        let range = (1, token.end_column - 1);
                        let fix_info = FixInfo {
                            edit_column: Some((token.start_column as i64 - actual_indent) as usize),
                            delete_count: Some((actual_indent - expected_indent).max(0) as isize),
                            insert_text: Some(
                                " ".repeat((expected_indent - actual_indent).max(0) as usize),
                            ),
                            ..Default::default()
                        };
                        out.add_error_detail_if(
                            token.start_line,
                            expected_indent,
                            actual_indent,
                            None,
                            None,
                            Some(range),
                            Some(fix_info),
                        );
                    }
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
        let config = json!({ "default": false, "MD007": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md007_default_indent_two() {
        assert!(lint_rule("MD007", "* a\n  * b\n    * c\n").is_empty());
        let errs = lint_rule("MD007", "* a\n   * b\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 2; Actual: 3")
        );
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!((f.edit_column, f.delete_count), (Some(1), Some(1)));
        assert_eq!(f.insert_text.as_deref(), Some(""));
    }

    #[test]
    fn md007_indent_four() {
        assert!(lint_with(json!({ "indent": 4 }), "* a\n    * b\n").is_empty());
        let errs = lint_with(json!({ "indent": 4 }), "* a\n  * b\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 4; Actual: 2")
        );
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!((f.edit_column, f.delete_count), (Some(1), Some(0)));
        assert_eq!(f.insert_text.as_deref(), Some("  "));
    }

    #[test]
    fn md007_start_indented() {
        let content = "* a\n  * b\n";
        assert!(lint_rule("MD007", content).is_empty());
        let errs = lint_with(json!({ "start_indented": true }), content);
        assert_eq!(errs.len(), 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 2; Actual: 0")
        );
        assert_eq!(
            errs[1].error_detail.as_deref(),
            Some("Expected: 4; Actual: 2")
        );
    }

    #[test]
    fn md007_start_indent_overrides_first_level() {
        let errs = lint_with(
            json!({ "start_indented": true, "start_indent": 1 }),
            "* a\n   * b\n",
        );
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 1; Actual: 0")
        );
    }

    #[test]
    fn md007_ordered_parent_is_skipped() {
        assert!(lint_rule("MD007", "1. a\n   * b\n     * c\n").is_empty());
    }

    #[test]
    fn md007_block_quote_prefix_is_subtracted() {
        assert!(lint_rule("MD007", "> * a\n>   * b\n").is_empty());
        let errs = lint_rule("MD007", "> * a\n>    * b\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 2; Actual: 3")
        );
    }
}
