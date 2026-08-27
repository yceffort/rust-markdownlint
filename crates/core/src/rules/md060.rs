use std::collections::HashSet;

use unicode_width::UnicodeWidthStr;

use super::{LintContext, Rule, RuleMeta};
use crate::config::{js_string, truthy};
use crate::error::ErrorSink;
use crate::parser::{TokenId, TokenTree};

pub(crate) struct Md060;

static META: RuleMeta = RuleMeta {
    names: &["MD060", "table-column-style"],
    description: "Table column style",
    tags: &["table"],
    needs_tokens: true,
    // 원본 md060.mjs 의 `addError` 는 fixInfo 를 만들지 않는다 (자동 수정 없음).
    fixable: false,
};

/// npm `string-width` 상당의 표시 폭 (ANSI 제거는 마크다운에서 무의미해 생략한다).
/// East Asian Width 로 Wide/Fullwidth 는 2, 결합 문자/zero-width 는 0, 나머지는 1 이고
/// (ambiguous 는 원본 기본값대로 1), 이모지 표현 시퀀스(VS16), 피부색 수정자, ZWJ
/// 시퀀스는 `unicode-width` 의 문자열 단위 계산이 원본과 같이 2 로 묶어 센다.
fn string_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// JS `line.slice(0, end)`: micromark 의 column 과 마찬가지로 UTF-16 code unit 기준이라
/// 서러게이트 쌍(BMP 밖 이모지) 앞에서도 원본과 같은 자리에서 자른다. 쌍 가운데가
/// 잘리면 원본은 짝 없는 서러게이트를 남기고 `string-width` 는 이를 폭 0 으로 세므로
/// 그 문자 앞에서 자른 접두 부분 문자열과 폭이 같다.
fn js_slice(s: &str, end: usize) -> &str {
    let mut units = 0;
    for (i, c) in s.char_indices() {
        if units + c.len_utf16() > end {
            return &s[..i];
        }
        units += c.len_utf16();
    }
    s
}

/// 원본 `filterByTypes(token.children, types)`: 토큰의 자식부터 전위 순회로 타입을 거른다.
fn filter_children_by_types(tokens: &TokenTree, id: TokenId, kinds: &[&str]) -> Vec<TokenId> {
    tokens.filter_by_predicate(
        &tokens.get(id).children,
        |t, id| kinds.contains(&t.get(id).kind) && !t.get(id).in_html_flow,
        |t, id, out| out.extend_from_slice(&t.get(id).children),
    )
}

/// 원본 `RuleOnErrorInfo` 중 이 규칙이 쓰는 부분.
#[derive(Clone)]
struct ErrorInfo {
    line_number: usize,
    column: usize,
    detail: &'static str,
}

/// 원본 `addError(errors, lineNumber, column, detail)`.
fn add_error(errors: &mut Vec<ErrorInfo>, line_number: usize, column: usize, detail: &'static str) {
    errors.push(ErrorInfo {
        line_number,
        column,
        detail,
    });
}

/// 원본 `Column`: 실제 열(1 기반)과 표시 폭 기준 열(1 기반).
struct Column {
    actual: usize,
    effective: usize,
}

/// 원본 `getTableDividerColumns`.
fn get_table_divider_columns(lines: &[&str], tokens: &TokenTree, row: TokenId) -> Vec<Column> {
    let start_line = tokens.get(row).start_line;
    filter_children_by_types(tokens, row, &["tableCellDivider"])
        .into_iter()
        .map(|divider| {
            let start_column = tokens.get(divider).start_column;
            Column {
                actual: start_column,
                effective: string_width(js_slice(lines[start_line - 1], start_column - 1)),
            }
        })
        .collect()
}

/// 원본 `checkStyleAligned`: 첫 행의 파이프 위치(표시 폭 기준)와 어긋나는 파이프를 모은다.
fn check_style_aligned(
    lines: &[&str],
    tokens: &TokenTree,
    rows: &[TokenId],
    detail: &'static str,
) -> Vec<ErrorInfo> {
    let mut error_infos = Vec::new();
    let Some(&header_row) = rows.first() else {
        return error_infos;
    };
    let header_divider_columns = get_table_divider_columns(lines, tokens, header_row);
    for &row in &rows[1..] {
        let mut remaining_header_divider_columns: HashSet<usize> = header_divider_columns
            .iter()
            .map(|column| column.effective)
            .collect();
        let row_divider_columns = get_table_divider_columns(lines, tokens, row);
        for divider_column in row_divider_columns {
            if !remaining_header_divider_columns.is_empty()
                && !remaining_header_divider_columns.remove(&divider_column.effective)
            {
                add_error(
                    &mut error_infos,
                    tokens.get(row).start_line,
                    divider_column.actual,
                    detail,
                );
            }
        }
    }
    error_infos
}

impl Rule for Md060 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        // 원본 `String(params.config.style || "any")`
        let style = match ctx.config.get("style") {
            Some(value) if truthy(value) => js_string(value),
            _ => "any".to_string(),
        };
        let style_aligned_allowed = (style == "any") || (style == "aligned");
        let style_compact_allowed = (style == "any") || (style == "compact");
        let style_tight_allowed = (style == "any") || (style == "tight");
        let aligned_delimiter = ctx.config.get("aligned_delimiter").is_some_and(truthy);
        let lines = ctx.lines;
        let tokens = ctx.tokens;

        // 모든 표/행을 훑는다
        let tables = tokens.filter_by_types(&["table"]);
        for table in tables {
            let rows = filter_children_by_types(tokens, table, &["tableDelimiterRow", "tableRow"]);

            // "aligned" 스타일일 때의 오류
            let mut errors_if_aligned = Vec::new();
            if style_aligned_allowed {
                errors_if_aligned.extend(check_style_aligned(
                    lines,
                    tokens,
                    &rows,
                    "Table pipe does not align with header for style \"aligned\"",
                ));
            }

            // "compact", "tight" 스타일일 때의 오류
            let mut errors_if_compact = Vec::new();
            let mut errors_if_tight = Vec::new();
            if (style_compact_allowed || style_tight_allowed)
                && !(style_aligned_allowed && errors_if_aligned.is_empty())
            {
                if aligned_delimiter {
                    let error_infos = check_style_aligned(
                        lines,
                        tokens,
                        &rows[..rows.len().min(2)],
                        "Table pipe does not align with header for option \"aligned_delimiter\"",
                    );
                    errors_if_compact.extend(error_infos.iter().cloned());
                    errors_if_tight.extend(error_infos);
                }
                for &row in &rows {
                    let row_end_column = tokens.get(row).end_column;
                    let tokens_of_interest = filter_children_by_types(
                        tokens,
                        row,
                        &["tableCellDivider", "tableContent", "whitespace"],
                    );
                    for i in 0..tokens_of_interest.len() {
                        let token = tokens.get(tokens_of_interest[i]);
                        let (start_column, start_line) = (token.start_column, token.start_line);
                        if token.kind == "tableCellDivider" {
                            let previous = i
                                .checked_sub(1)
                                .map(|index| tokens.get(tokens_of_interest[index]));
                            if let Some(previous) = previous {
                                if previous.kind == "whitespace" {
                                    if tokens.text_of(previous).chars().count() != 1 {
                                        add_error(
                                            &mut errors_if_compact,
                                            start_line,
                                            start_column,
                                            "Table pipe has extra space to the left for style \"compact\"",
                                        );
                                    }
                                    add_error(
                                        &mut errors_if_tight,
                                        start_line,
                                        start_column,
                                        "Table pipe has space to the left for style \"tight\"",
                                    );
                                } else {
                                    add_error(
                                        &mut errors_if_compact,
                                        start_line,
                                        start_column,
                                        "Table pipe is missing space to the left for style \"compact\"",
                                    );
                                }
                            }
                            let next = tokens_of_interest.get(i + 1).map(|&id| tokens.get(id));
                            if let Some(next) = next {
                                if next.kind == "whitespace" {
                                    if next.end_column != row_end_column {
                                        if tokens.text_of(next).chars().count() != 1 {
                                            add_error(
                                                &mut errors_if_compact,
                                                start_line,
                                                start_column,
                                                "Table pipe has extra space to the right for style \"compact\"",
                                            );
                                        }
                                        add_error(
                                            &mut errors_if_tight,
                                            start_line,
                                            start_column,
                                            "Table pipe has space to the right for style \"tight\"",
                                        );
                                    }
                                } else {
                                    add_error(
                                        &mut errors_if_compact,
                                        start_line,
                                        start_column,
                                        "Table pipe is missing space to the right for style \"compact\"",
                                    );
                                }
                            }
                        }
                    }
                }
            }

            // 허용된 스타일 중 오류가 가장 적은 쪽을 보고한다
            let mut error_infos = &errors_if_aligned;
            if style_compact_allowed
                && ((errors_if_compact.len() < error_infos.len()) || !style_aligned_allowed)
            {
                error_infos = &errors_if_compact;
            }
            if style_tight_allowed
                && ((errors_if_tight.len() < error_infos.len())
                    || (!style_aligned_allowed && !style_compact_allowed))
            {
                error_infos = &errors_if_tight;
            }
            for error_info in error_infos {
                out.add_error(
                    error_info.line_number,
                    Some(error_info.detail),
                    None,
                    Some((error_info.column, 1)),
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
        let config = json!({ "default": false, "MD060": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md060_aligned_table_is_fine() {
        let content = "| Heading | Heading   |\n| ------- | --------- |\n| Text    | Text text |\n";
        assert!(lint_rule("MD060", content).is_empty());
    }

    #[test]
    fn md060_misaligned_pipe_reports_aligned_style() {
        let content = "| Heading | Heading   |\n| ------- | --------- |\n| Text     | Text tex |\n";
        let errs = lint_rule("MD060", content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Table pipe does not align with header for style \"aligned\"")
        );
        assert_eq!(errs[0].error_range, Some((12, 1)));
        assert!(errs[0].fix_info.is_none());
    }

    #[test]
    fn md060_compact_and_tight_tables_pass_by_default() {
        assert!(lint_rule("MD060", "| A | B |\n| - | - |\n| Text | T |\n").is_empty());
        assert!(lint_rule("MD060", "|A|B|\n|-|-|\n|Text|T|\n").is_empty());
    }

    #[test]
    fn md060_style_tight_rejects_spaces() {
        let content = "| A | B |\n| - | - |\n| C | D |\n";
        assert!(lint_rule("MD060", content).is_empty());
        let errs = lint_with(json!({ "style": "tight" }), content);
        assert_eq!(errs.len(), 12);
        assert!(
            errs.iter()
                .all(|e| e.error_detail.as_deref().unwrap().contains("\"tight\""))
        );
    }

    #[test]
    fn md060_wide_characters_count_as_two_columns() {
        // 한글과 이모지 표현 시퀀스(U+26A0 U+FE0F)는 폭 2 로 세므로 실제 열이 달라도 정렬이 맞는다
        assert!(
            lint_rule(
                "MD060",
                "| Response | Emoji |\n| -------- | ----- |\n| Yes      | ⚠️    |\n"
            )
            .is_empty()
        );
        assert!(
            lint_rule(
                "MD060",
                "| Response | Emoji |\n| -------- | ----- |\n| Yes      | 한글  |\n"
            )
            .is_empty()
        );
        // 공백 하나가 더 붙으면 표시 폭이 어긋난다
        let errs = lint_rule(
            "MD060",
            "| Response | Emoji |\n| -------- | ----- |\n| Yes      | ⚠️     |\n",
        );
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(errs[0].error_range, Some((21, 1)));
    }

    #[test]
    fn md060_aligned_delimiter_option() {
        // compact 로 통과하던 표에서 구분 행이 헤더와 어긋나면 옵션이 오류로 잡는다
        let content = "| Heading | Heading |\n| - | - |\n| Text | Text |\n";
        assert!(lint_rule("MD060", content).is_empty());
        let errs = lint_with(json!({ "aligned_delimiter": true }), content);
        assert_eq!(errs.len(), 2);
        assert!(errs.iter().all(|e| e.line_number == 2));
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Table pipe does not align with header for option \"aligned_delimiter\"")
        );
        assert_eq!(errs[0].error_range, Some((5, 1)));
        assert_eq!(errs[1].error_range, Some((9, 1)));
    }
}
