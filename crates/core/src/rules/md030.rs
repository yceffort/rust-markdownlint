use serde_json::Value;

use super::{LintContext, Rule, RuleMeta};
use crate::config::{js_number_string, js_pad_end, to_number, truthy};
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md030;

static META: RuleMeta = RuleMeta {
    names: &["MD030", "list-marker-space"],
    description: "Spaces after list markers",
    tags: &["ol", "ul", "whitespace"],
    needs_tokens: true,
    fixable: true,
};

/// 원본 `Number(params.config.x || 1)`: falsy 면 1, 아니면 Number 변환.
fn config_spaces(config: &serde_json::Map<String, Value>, key: &str) -> f64 {
    match config.get(key) {
        Some(v) if truthy(v) => to_number(v),
        _ => 1.0,
    }
}

impl Rule for Md030 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let ul_single = config_spaces(ctx.config, "ul_single");
        let ol_single = config_spaces(ctx.config, "ol_single");
        let ul_multi = config_spaces(ctx.config, "ul_multi");
        let ol_multi = config_spaces(ctx.config, "ol_multi");

        let tokens = ctx.tokens;
        for list_id in tokens.filter_by_types(&["listOrdered", "listUnordered"]) {
            let list = tokens.get(list_id);
            let ordered = list.kind == "listOrdered";
            let list_item_prefixes: Vec<_> = list
                .children
                .iter()
                .copied()
                .filter(|&id| tokens.get(id).kind == "listItemPrefix")
                .collect();
            let all_single_line = (list.end_line - list.start_line + 1) == list_item_prefixes.len();
            let expected_spaces = if ordered {
                if all_single_line { ol_single } else { ol_multi }
            } else if all_single_line {
                ul_single
            } else {
                ul_multi
            };
            for prefix_id in list_item_prefixes {
                let list_item_prefix = tokens.get(prefix_id);
                let range = Some((
                    list_item_prefix.start_column,
                    list_item_prefix.end_column - list_item_prefix.start_column,
                ));
                let whitespaces: Vec<_> = list_item_prefix
                    .children
                    .iter()
                    .copied()
                    .filter(|&id| tokens.get(id).kind == "listItemPrefixWhitespace")
                    .collect();
                for whitespace_id in whitespaces {
                    let whitespace = tokens.get(whitespace_id);
                    let actual_spaces = whitespace.end_column - whitespace.start_column;
                    // 원본 `"".padEnd(expectedSpaces)`: 한도를 넘으면 규칙이 예외로 끝난다.
                    let insert_text = match js_pad_end(expected_spaces) {
                        Ok(text) => text,
                        Err(message) => return out.add_rule_failure(&message),
                    };
                    let fix_info = FixInfo {
                        edit_column: Some(whitespace.start_column),
                        delete_count: Some(actual_spaces as isize),
                        insert_text: Some(insert_text),
                        ..Default::default()
                    };
                    out.add_error_detail_if(
                        whitespace.start_line,
                        js_number_string(expected_spaces),
                        actual_spaces,
                        None,
                        None,
                        range,
                        Some(fix_info),
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
        let config = json!({ "default": false, "MD030": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md030_huge_spaces_are_a_rule_failure() {
        let errs = lint_with(json!({ "ul_single": "Infinity" }), "* a\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("This rule threw an exception: Invalid string length")
        );
    }

    #[test]
    fn md030_default_single_space_passes() {
        assert!(lint_rule("MD030", "* one\n* two\n\n1. one\n2. two\n").is_empty());
    }

    #[test]
    fn md030_extra_space_reported_with_fix() {
        let errs = lint_rule("MD030", "*  one\n*  two\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 1; Actual: 2")
        );
        assert_eq!(errs[0].error_range, Some((1, 3)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(f.edit_column, Some(2));
        assert_eq!(f.delete_count, Some(2));
        assert_eq!(f.insert_text.as_deref(), Some(" "));
    }

    #[test]
    fn md030_ul_multi_param_for_multi_line_items() {
        let content = "* one\n  continued\n* two\n  continued\n";
        assert!(lint_rule("MD030", content).is_empty());
        let errs = lint_with(json!({ "ul_multi": 3 }), content);
        assert_eq!(errs.len(), 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 3; Actual: 1")
        );
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some("   ")
        );
    }

    #[test]
    fn md030_ol_single_param_separate_from_ul() {
        let content = "1. one\n2. two\n";
        assert!(lint_rule("MD030", content).is_empty());
        let errs = lint_with(json!({ "ol_single": 2 }), content);
        assert_eq!(errs.len(), 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 2; Actual: 1")
        );
    }
}
