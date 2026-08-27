use super::md049::style_config;
use super::{LintContext, Rule, RuleMeta};
use crate::error::ErrorSink;
use crate::parser::{TokenId, TokenTree};

pub(crate) struct Md055;

static META: RuleMeta = RuleMeta {
    names: &["MD055", "table-pipe-style"],
    description: "Table pipe style",
    tags: &["table"],
    needs_tokens: true,
    fixable: false,
};

/// 원본 `whitespaceTypes`.
const WHITESPACE_TYPES: [&str; 2] = ["linePrefix", "whitespace"];

/// 원본 `ignoreWhitespace`: 공백 토큰을 걸러낸다.
fn ignore_whitespace(tokens: &TokenTree, children: &[TokenId]) -> Vec<TokenId> {
    children
        .iter()
        .copied()
        .filter(|&id| !WHITESPACE_TYPES.contains(&tokens.get(id).kind))
        .collect()
}

/// 원본 `firstOrNothing`.
fn first_or_nothing(items: &[TokenId]) -> Option<TokenId> {
    items.first().copied()
}

/// 원본 `lastOrNothing`.
fn last_or_nothing(items: &[TokenId]) -> Option<TokenId> {
    items.last().copied()
}

/// 원본 `makeRange(start, end)`: `[start, end - start + 1]`.
fn make_range(start: usize, end: usize) -> (usize, usize) {
    (start, end - start + 1)
}

impl Rule for Md055 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let tokens = ctx.tokens;
        // 원본 `String(params.config.style || "consistent")`.
        let mut expected_style = style_config(ctx);
        let mut expected_leading_pipe =
            (expected_style != "no_leading_or_trailing") && (expected_style != "trailing_only");
        let mut expected_trailing_pipe =
            (expected_style != "no_leading_or_trailing") && (expected_style != "leading_only");
        let rows = tokens.filter_by_types(&["tableDelimiterRow", "tableRow"]);
        for row_id in rows {
            let row = tokens.get(row_id);
            // 원본은 first/lastOrNothing 의 fallback 을 두지 않는다 (불가능한 경우라 0% coverage).
            // 여기서는 예외 대신 해당 행을 건너뛴다.
            let (Some(first_cell), Some(last_cell)) = (
                first_or_nothing(&row.children),
                last_or_nothing(&row.children),
            ) else {
                continue;
            };
            let first_cell = tokens.get(first_cell);
            let last_cell = tokens.get(last_cell);
            let (Some(leading_token), Some(trailing_token)) = (
                first_or_nothing(&ignore_whitespace(tokens, &first_cell.children)),
                last_or_nothing(&ignore_whitespace(tokens, &last_cell.children)),
            ) else {
                continue;
            };
            let actual_leading_pipe = tokens.get(leading_token).kind == "tableCellDivider";
            let actual_trailing_pipe = tokens.get(trailing_token).kind == "tableCellDivider";
            let actual_style = if actual_leading_pipe {
                if actual_trailing_pipe {
                    "leading_and_trailing"
                } else {
                    "leading_only"
                }
            } else if actual_trailing_pipe {
                "trailing_only"
            } else {
                "no_leading_or_trailing"
            };
            if expected_style == "consistent" {
                expected_style = actual_style.to_string();
                expected_leading_pipe = actual_leading_pipe;
                expected_trailing_pipe = actual_trailing_pipe;
            }
            if actual_leading_pipe != expected_leading_pipe {
                out.add_error_detail_if(
                    first_cell.start_line,
                    &expected_style,
                    actual_style,
                    Some(&format!(
                        "{} leading pipe",
                        if expected_leading_pipe {
                            "Missing"
                        } else {
                            "Unexpected"
                        }
                    )),
                    None,
                    Some(make_range(row.start_column, first_cell.start_column)),
                    None,
                );
            }
            if actual_trailing_pipe != expected_trailing_pipe {
                out.add_error_detail_if(
                    last_cell.end_line,
                    &expected_style,
                    actual_style,
                    Some(&format!(
                        "{} trailing pipe",
                        if expected_trailing_pipe {
                            "Missing"
                        } else {
                            "Unexpected"
                        }
                    )),
                    None,
                    Some(make_range(last_cell.end_column - 1, row.end_column - 1)),
                    None,
                );
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
        let config = json!({ "default": false, "MD055": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    const BOTH: &str = "| a | b |\n| - | - |\n| c | d |\n";
    const NONE: &str = "a | b\n- | -\nc | d\n";

    #[test]
    fn md055_consistent_accepts_uniform_tables() {
        assert!(lint_rule("MD055", BOTH).is_empty());
        assert!(lint_rule("MD055", NONE).is_empty());
    }

    #[test]
    fn md055_consistent_reports_deviating_row() {
        let errs = lint_rule("MD055", "| a | b |\n| - | - |\nc | d\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some(
                "Expected: leading_and_trailing; Actual: no_leading_or_trailing; Missing leading pipe"
            )
        );
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(errs[0].error_range, Some((1, 1)));
        assert_eq!(
            errs[1].error_detail.as_deref(),
            Some(
                "Expected: leading_and_trailing; Actual: no_leading_or_trailing; Missing trailing pipe"
            )
        );
        assert_eq!(errs[1].error_range, Some((5, 1)));
        assert!(errs[0].fix_info.is_none());
    }

    #[test]
    fn md055_explicit_style_no_leading_or_trailing() {
        let errs = lint_with(json!({ "style": "no_leading_or_trailing" }), BOTH);
        assert_eq!(errs.len(), 6);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some(
                "Expected: no_leading_or_trailing; Actual: leading_and_trailing; Unexpected leading pipe"
            )
        );
        assert!(lint_with(json!({ "style": "no_leading_or_trailing" }), NONE).is_empty());
    }

    #[test]
    fn md055_explicit_style_leading_only() {
        let content = "| a | b\n| - | -\n| c | d\n";
        assert!(lint_with(json!({ "style": "leading_only" }), content).is_empty());
        let errs = lint_with(json!({ "style": "trailing_only" }), content);
        assert_eq!(errs.len(), 6);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: trailing_only; Actual: leading_only; Unexpected leading pipe")
        );
        assert_eq!(
            errs[1].error_detail.as_deref(),
            Some("Expected: trailing_only; Actual: leading_only; Missing trailing pipe")
        );
    }

    #[test]
    fn md055_delimiter_row_is_checked() {
        let errs = lint_rule("MD055", "| a | b |\n| - | -\n| c | d |\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: leading_and_trailing; Actual: leading_only; Missing trailing pipe")
        );
    }

    #[test]
    fn md055_indented_table_range_starts_at_row() {
        // blockquote 안이라 row.startColumn 이 1 이 아니다.
        let errs = lint_rule("MD055", "> | a | b |\n> | - | - |\n> c | d\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].error_range, Some((3, 1)));
    }
}
