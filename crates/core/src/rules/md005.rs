use super::{LintContext, Rule, RuleMeta};
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md005;

static META: RuleMeta = RuleMeta {
    names: &["MD005", "list-indent"],
    description: "Inconsistent indentation for list items at the same level",
    tags: &["bullet", "ul", "indentation"],
    needs_tokens: true,
    fixable: true,
};

impl Rule for Md005 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let tokens = ctx.tokens;
        for list_id in tokens.filter_by_types(&["listOrdered", "listUnordered"]) {
            let list = tokens.get(list_id);
            let expected_indent = list.start_column - 1;
            let mut expected_end = 0usize;
            let mut end_matching = false;
            let list_item_prefixes: Vec<_> = list
                .children
                .iter()
                .copied()
                .filter(|&id| tokens.get(id).kind == "listItemPrefix")
                .collect();
            for prefix_id in list_item_prefixes {
                let list_item_prefix = tokens.get(prefix_id);
                let line_number = list_item_prefix.start_line;
                let actual_indent = list_item_prefix.start_column - 1;
                let range = Some((1, list_item_prefix.end_column - 1));
                if list.kind == "listUnordered" {
                    out.add_error_detail_if(
                        line_number,
                        expected_indent,
                        actual_indent,
                        None,
                        None,
                        range,
                        None,
                        // No fixInfo; MD007 handles this scenario better
                    );
                } else {
                    let marker_length = list_item_prefix.text.trim().chars().count();
                    let actual_end = list_item_prefix.start_column + marker_length - 1;
                    if expected_end == 0 {
                        expected_end = actual_end;
                    }
                    if (expected_indent != actual_indent) || end_matching {
                        if expected_end == actual_end {
                            end_matching = true;
                        } else {
                            let detail = if end_matching {
                                format!("Expected: ({expected_end}); Actual: ({actual_end})")
                            } else {
                                format!("Expected: {expected_indent}; Actual: {actual_indent}")
                            };
                            let expected = if end_matching {
                                expected_end - marker_length
                            } else {
                                expected_indent
                            };
                            let actual = if end_matching {
                                actual_end - marker_length
                            } else {
                                actual_indent
                            };
                            out.add_error(
                                line_number,
                                Some(&detail),
                                None,
                                range,
                                Some(FixInfo {
                                    edit_column: Some(actual.min(expected) + 1),
                                    delete_count: Some(actual.saturating_sub(expected) as isize),
                                    insert_text: Some(" ".repeat(expected.saturating_sub(actual))),
                                    ..Default::default()
                                }),
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::lint_rule;

    #[test]
    fn md005_consistent_lists_pass() {
        assert!(lint_rule("MD005", "* one\n* two\n* three\n").is_empty());
        assert!(lint_rule("MD005", "1. one\n1. two\n1. three\n").is_empty());
    }

    #[test]
    fn md005_unordered_inconsistent_indent() {
        let errs = lint_rule("MD005", "* one\n* two\n\n  * nested\n   * nested2\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 5);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 2; Actual: 3")
        );
        assert_eq!(errs[0].error_range, Some((1, 5)));
        assert!(errs[0].fix_info.is_none());
    }

    #[test]
    fn md005_ordered_inconsistent_indent_has_fix() {
        let errs = lint_rule("MD005", "1. one\n 2. two\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 0; Actual: 1")
        );
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(f.edit_column, Some(1));
        assert_eq!(f.delete_count, Some(1));
        assert_eq!(f.insert_text.as_deref(), Some(""));
    }

    #[test]
    fn md005_ordered_right_aligned_markers_allowed() {
        assert!(lint_rule("MD005", " 8. eight\n 9. nine\n10. ten\n").is_empty());
    }

    #[test]
    fn md005_ordered_end_matching_mismatch() {
        let errs = lint_rule("MD005", " 8. eight\n 9. nine\n10. ten\n 11. eleven\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 4);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: (3); Actual: (4)")
        );
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(f.edit_column, Some(1));
        assert_eq!(f.delete_count, Some(1));
        assert_eq!(f.insert_text.as_deref(), Some(""));
    }
}
