use super::{LintContext, Rule, RuleMeta};
use crate::error::ErrorSink;

pub(crate) struct Md056;

static META: RuleMeta = RuleMeta {
    names: &["MD056", "table-column-count"],
    description: "Table column count",
    tags: &["table"],
    needs_tokens: true,
    fixable: false,
};

/// 원본 `makeRange(start, end)`: 시작 열과 끝 열로 `[column, length]` 를 만든다.
fn make_range(start: usize, end: usize) -> (usize, usize) {
    (start, end - start + 1)
}

impl Rule for Md056 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        // 원본 `filterByTypesCached([ "tableDelimiterRow", "tableRow" ])`.
        let rows = ctx
            .tokens
            .filter_by_types(&["tableDelimiterRow", "tableRow"]);
        let mut expected_count = 0usize;
        let mut current_table = None;
        for row in rows {
            // 원본 `getParentOfType(row, [ "table" ])`.
            let table = ctx.tokens.parent_of_type(row, &["table"]);
            if current_table != table {
                expected_count = 0;
                current_table = table;
            }
            let cells: Vec<_> = ctx
                .tokens
                .get(row)
                .children
                .iter()
                .copied()
                .filter(|&child| {
                    matches!(
                        ctx.tokens.get(child).kind.as_str(),
                        "tableData" | "tableDelimiter" | "tableHeader"
                    )
                })
                .collect();
            let actual_count = cells.len();
            // 원본 `expectedCount ||= actualCount`.
            if expected_count == 0 {
                expected_count = actual_count;
            }
            let row_token = ctx.tokens.get(row);
            let mut detail = None;
            let mut range = None;
            if actual_count < expected_count {
                detail = Some("Too few cells, row will be missing data");
                range = Some((row_token.end_column - 1, 1));
            } else if expected_count < actual_count {
                range = Some(make_range(
                    ctx.tokens.get(cells[expected_count]).start_column,
                    row_token.end_column - 1,
                ));
                detail = Some("Too many cells, extra data will be missing");
            }
            out.add_error_detail_if(
                row_token.end_line,
                expected_count,
                actual_count,
                detail,
                None,
                range,
                None,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::lint_rule;

    #[test]
    fn md056_matching_column_counts_are_fine() {
        let content = "| a | b |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |\n";
        assert!(lint_rule("MD056", content).is_empty());
    }

    #[test]
    fn md056_too_few_cells() {
        let content = "| a | b |\n| --- | --- |\n| 1 |\n";
        let errs = lint_rule("MD056", content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 2; Actual: 1; Too few cells, row will be missing data")
        );
        assert_eq!(errs[0].error_range, Some((5, 1)));
        assert!(errs[0].fix_info.is_none());
    }

    #[test]
    fn md056_too_many_cells() {
        let content = "| a | b |\n| --- | --- |\n| 1 | 2 | 3 |\n";
        let errs = lint_rule("MD056", content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 2; Actual: 3; Too many cells, extra data will be missing")
        );
        assert_eq!(errs[0].error_range, Some((9, 5)));
    }

    #[test]
    fn md056_pipeless_continuation_row() {
        // GFM 에서 파이프 없는 줄도 테이블 본문 행으로 이어진다 (셀 1개).
        let content = "| a | b |\n| --- | --- |\ntext\n";
        let errs = lint_rule("MD056", content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 2; Actual: 1; Too few cells, row will be missing data")
        );
        assert_eq!(errs[0].error_range, Some((4, 1)));
    }

    #[test]
    fn md056_expected_count_resets_per_table() {
        let content =
            "| a | b |\n| --- | --- |\n| 1 | 2 |\n\ntext\n\n| a |\n| --- |\n| 1 |\n| 1 | 2 |\n";
        let errs = lint_rule("MD056", content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 10);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 1; Actual: 2; Too many cells, extra data will be missing")
        );
    }
}
