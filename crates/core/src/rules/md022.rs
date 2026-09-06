use std::cell::OnceCell;

use serde_json::Value;

use super::{LintContext, Rule, RuleMeta, is_blank_line};
use crate::config::{js_number_string, js_repeat, js_string, to_number};
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

#[derive(Clone)]
enum LinesValue {
    /// 비배열 설정은 `Number(...)` 강제 후의 값을 반환한다.
    Number(f64),
    /// 배열 설정은 원소를 그대로 반환해 strict equality와 문자열화에 원 타입이 남는다.
    Raw(Value),
}

impl LinesValue {
    fn number(&self) -> f64 {
        match self {
            LinesValue::Number(value) => *value,
            LinesValue::Raw(value) => to_number(value),
        }
    }

    fn display(&self) -> String {
        match self {
            LinesValue::Number(value) => js_number_string(*value),
            LinesValue::Raw(value) => js_string(value),
        }
    }

    fn strictly_equals(&self, actual: usize) -> bool {
        match self {
            LinesValue::Number(value) => *value == actual as f64,
            LinesValue::Raw(Value::Number(value)) => {
                value.as_f64().is_some_and(|value| value == actual as f64)
            }
            LinesValue::Raw(_) => false,
        }
    }
}

/// 원본 `getLinesFunction`: 배열이면 heading 레벨별 값, 아니면 고정 값을 돌려준다.
enum LinesFunction {
    PerLevel(Box<[LinesValue; 6]>),
    Fixed(LinesValue),
}

impl LinesFunction {
    fn new(lines_param: Option<&Value>) -> Self {
        if let Some(Value::Array(array)) = lines_param {
            let mut lines_array = std::array::from_fn(|_| LinesValue::Number(DEFAULT_LINES));
            for (index, value) in array.iter().enumerate().take(6) {
                lines_array[index] = LinesValue::Raw(value.clone());
            }
            return LinesFunction::PerLevel(Box::new(lines_array));
        }
        let lines = match lines_param {
            None => DEFAULT_LINES,
            Some(value) => to_number(value),
        };
        LinesFunction::Fixed(LinesValue::Number(lines))
    }

    fn get(&self, level: usize) -> &LinesValue {
        match self {
            LinesFunction::PerLevel(array) => &array[level - 1],
            LinesFunction::Fixed(lines) => lines,
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
            let lines_above_number = lines_above.number();
            if lines_above_number >= 0.0 {
                let mut actual_above = 0usize;
                let mut i = 0usize;
                while (i as f64) < lines_above_number
                    && blank_at(start_line as isize - 2 - i as isize)
                {
                    actual_above += 1;
                    i += 1;
                }
                if !lines_above.strictly_equals(actual_above) {
                    let detail = format!(
                        "Expected: {}; Actual: {actual_above}; Above",
                        lines_above.display()
                    );
                    // 원본 `getBlockQuotePrefixText(...).repeat(count)`: Infinity 나 한도 초과는 규칙이
                    // 예외로 끝난다.
                    let insert_text = match js_repeat(
                        &tokens.block_quote_prefix_text(block_quote_prefixes(), start_line - 1, 1),
                        lines_above_number - actual_above as f64,
                    ) {
                        Ok(text) => text,
                        Err(message) => return out.add_rule_failure(&message),
                    };
                    out.add_error(
                        start_line,
                        Some(&detail),
                        Some(line),
                        None,
                        Some(FixInfo {
                            insert_text: Some(insert_text),
                            ..Default::default()
                        }),
                    );
                }
            }

            // Check lines below
            let lines_below = get_lines_below.get(level);
            let lines_below_number = lines_below.number();
            if lines_below_number >= 0.0 {
                let mut actual_below = 0usize;
                let mut i = 0usize;
                while (i as f64) < lines_below_number && blank_at((end_line + i) as isize) {
                    actual_below += 1;
                    i += 1;
                }
                if !lines_below.strictly_equals(actual_below) {
                    let detail = format!(
                        "Expected: {}; Actual: {actual_below}; Below",
                        lines_below.display()
                    );
                    let insert_text = match js_repeat(
                        &tokens.block_quote_prefix_text(block_quote_prefixes(), end_line + 1, 1),
                        lines_below_number - actual_below as f64,
                    ) {
                        Ok(text) => text,
                        Err(message) => return out.add_rule_failure(&message),
                    };
                    out.add_error(
                        start_line,
                        Some(&detail),
                        Some(line),
                        None,
                        Some(FixInfo {
                            line_number: Some(end_line + 1),
                            insert_text: Some(insert_text),
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
    fn md022_array_values_keep_javascript_types() {
        let errs = lint_with(
            json!({ "lines_above": ["1"], "lines_below": [null] }),
            "text\n\n# h1\n",
        );
        assert_eq!(errs.len(), 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 1; Actual: 1; Above")
        );
        assert_eq!(
            errs[1].error_detail.as_deref(),
            Some("Expected: null; Actual: 0; Below")
        );
    }

    #[test]
    fn md022_infinite_or_huge_lines_are_rule_failures() {
        let errs = lint_with(json!({ "lines_above": "Infinity" }), "text\n\n# h1\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("This rule threw an exception: Invalid count value: Infinity")
        );
        let errs = lint_with(json!({ "lines_above": 1e9 }), "text\n\n# h1\n");
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("This rule threw an exception: Invalid string length")
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
