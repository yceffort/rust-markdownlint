use std::cell::OnceCell;

use serde_json::Value;

use super::{LintContext, Rule, RuleMeta, is_blank_line};
use crate::config::to_number;
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md022;

static META: RuleMeta = RuleMeta {
    names: &["MD022", "blanks-around-headings"],
    description: "Headings should be surrounded by blank lines",
    tags: &["headings", "blank_lines"],
    needs_tokens: true,
    fixable: true,
};

const DEFAULT_LINES: f64 = 1.0;

/// JS `String(number)` 상당의 표기. 정수는 소수점 없이 찍는다.
fn number_to_string(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// 원본 `getLinesFunction`: 배열이면 heading 레벨별 값, 아니면 고정 값을 돌려준다.
enum LinesFunction {
    PerLevel([f64; 6]),
    Fixed(f64),
}

impl LinesFunction {
    fn new(lines_param: Option<&Value>) -> Self {
        if let Some(Value::Array(array)) = lines_param {
            let mut lines_array = [DEFAULT_LINES; 6];
            for (index, value) in array.iter().enumerate().take(6) {
                lines_array[index] = to_number(value);
            }
            return LinesFunction::PerLevel(lines_array);
        }
        let lines = match lines_param {
            None => DEFAULT_LINES,
            Some(value) => to_number(value),
        };
        LinesFunction::Fixed(lines)
    }

    fn get(&self, level: usize) -> f64 {
        match self {
            LinesFunction::PerLevel(array) => array[level - 1],
            LinesFunction::Fixed(lines) => *lines,
        }
    }
}

impl Rule for Md022 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let get_lines_above = LinesFunction::new(ctx.config.get("lines_above"));
        let get_lines_below = LinesFunction::new(ctx.config.get("lines_below"));
        let lines = ctx.lines;
        // JS 는 범위 밖 인덱스가 undefined 라 isBlankLine 이 true 를 준다.
        let blank_at = |index: isize| -> bool {
            index < 0
                || lines
                    .get(index as usize)
                    .is_none_or(|line| is_blank_line(line))
        };

        let tokens = ctx.tokens;
        // linePrefix 는 매우 흔한 토큰이라 오류가 있을 때만 모으고, fixInfo 의 prefix 텍스트도
        // (전체 prefix 를 훑으므로) detail 이 다를 때만 만든다.
        let block_quote_prefixes = OnceCell::new();
        let block_quote_prefixes = || {
            block_quote_prefixes
                .get_or_init(|| tokens.filter_by_types(&["blockQuotePrefix", "linePrefix"]))
        };
        for heading_id in tokens.filter_by_types(&["atxHeading", "setextHeading"]) {
            let heading = tokens.get(heading_id);
            let (start_line, end_line) = (heading.start_line, heading.end_line);
            let line = lines[start_line - 1].trim();
            let level = tokens.heading_level(heading_id);

            // Check lines above
            let lines_above = get_lines_above.get(level);
            if lines_above >= 0.0 {
                let mut actual_above = 0usize;
                let mut i = 0usize;
                while (i as f64) < lines_above && blank_at(start_line as isize - 2 - i as isize) {
                    actual_above += 1;
                    i += 1;
                }
                let expected = number_to_string(lines_above);
                if expected != actual_above.to_string() {
                    out.add_error_detail_if(
                        start_line,
                        expected,
                        actual_above,
                        Some("Above"),
                        Some(line),
                        None,
                        Some(FixInfo {
                            insert_text: Some(tokens.block_quote_prefix_text(
                                block_quote_prefixes(),
                                start_line - 1,
                                (lines_above - actual_above as f64) as usize,
                            )),
                            ..Default::default()
                        }),
                    );
                }
            }

            // Check lines below
            let lines_below = get_lines_below.get(level);
            if lines_below >= 0.0 {
                let mut actual_below = 0usize;
                let mut i = 0usize;
                while (i as f64) < lines_below && blank_at((end_line + i) as isize) {
                    actual_below += 1;
                    i += 1;
                }
                let expected = number_to_string(lines_below);
                if expected != actual_below.to_string() {
                    out.add_error_detail_if(
                        start_line,
                        expected,
                        actual_below,
                        Some("Below"),
                        Some(line),
                        None,
                        Some(FixInfo {
                            line_number: Some(end_line + 1),
                            insert_text: Some(tokens.block_quote_prefix_text(
                                block_quote_prefixes(),
                                end_line + 1,
                                (lines_below - actual_below as f64) as usize,
                            )),
                            ..Default::default()
                        }),
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
        let config = json!({ "default": false, "MD022": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md022_default_reports_above_and_below() {
        let errs = lint_rule("MD022", "Text\n# Heading\nText\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line_number, 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 1; Actual: 0; Above")
        );
        assert_eq!(errs[0].error_context.as_deref(), Some("# Heading"));
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some("\n")
        );
        assert_eq!(
            errs[1].error_detail.as_deref(),
            Some("Expected: 1; Actual: 0; Below")
        );
        assert_eq!(errs[1].fix_info.as_ref().unwrap().line_number, Some(3));
    }

    #[test]
    fn md022_surrounded_heading_is_clean() {
        assert!(lint_rule("MD022", "Text\n\n# Heading\n\nText\n").is_empty());
    }

    #[test]
    fn md022_negative_disables_a_side() {
        let errs = lint_with(json!({ "lines_above": -1 }), "Text\n# Heading\nText\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 1; Actual: 0; Below")
        );
    }

    #[test]
    fn md022_counts_more_than_one_blank() {
        let errs = lint_with(
            json!({ "lines_above": 2, "lines_below": 2 }),
            "Text\n\n# Heading\n\nText\n",
        );
        assert_eq!(errs.len(), 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 2; Actual: 1; Above")
        );
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some("\n")
        );
    }

    #[test]
    fn md022_array_uses_heading_level() {
        let content = "Text\n# One\nText\n\n## Two\nText\n";
        let errs = lint_with(
            json!({ "lines_above": [-1, 2], "lines_below": [1, -1] }),
            content,
        );
        let detail: Vec<_> = errs
            .iter()
            .map(|e| (e.line_number, e.error_detail.clone().unwrap()))
            .collect();
        assert_eq!(
            detail,
            vec![
                (2, "Expected: 1; Actual: 0; Below".to_string()),
                (5, "Expected: 2; Actual: 1; Above".to_string()),
            ]
        );
    }

    #[test]
    fn md022_string_parameter_is_coerced() {
        let errs = lint_with(
            json!({ "lines_above": "1", "lines_below": "1" }),
            "# H\nText\n",
        );
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 1; Actual: 0; Below")
        );
    }
}
