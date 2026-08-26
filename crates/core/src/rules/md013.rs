use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use super::{LintContext, Rule, RuleMeta, add_range_to_set};
use crate::config::{to_number, truthy};
use crate::error::ErrorSink;
use crate::parser::JS_WHITESPACE;

pub(crate) struct Md013;

static META: RuleMeta = RuleMeta {
    names: &["MD013", "line-length"],
    description: "Line length",
    tags: &["line_length"],
    needs_tokens: true,
    fixable: false,
};

fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// 원본 `notWrappableRe`: 줄바꿈으로 줄일 수 없는 줄.
static NOT_WRAPPABLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^(?:[#>{JS_WHITESPACE}]*[{JS_WHITESPACE}])?[^{JS_WHITESPACE}]*$"
    ))
    .expect("not wrappable regex")
});

/// JS `String(number)` 상당의 표기. 정수는 소수점 없이 찍는다.
fn number_to_string(n: f64) -> String {
    if n.is_finite() && n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// `Number(params.config.key || fallback)`
fn length_param(config: &super::RuleParams, key: &str, fallback: f64) -> f64 {
    config
        .get(key)
        .filter(|v| truthy(v))
        .map_or(fallback, to_number)
}

/// `(value === undefined) ? true : !!value`
fn include_param(config: &super::RuleParams, key: &str) -> bool {
    config.get(key).is_none_or(truthy)
}

impl Rule for Md013 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let config = ctx.config;
        let line_length = length_param(config, "line_length", 80.0);
        let heading_line_length = length_param(config, "heading_line_length", line_length);
        let code_line_length = length_param(config, "code_block_line_length", line_length);
        let strict = config.get("strict").is_some_and(truthy);
        let stern = config.get("stern").is_some_and(truthy);
        let include_code_blocks = include_param(config, "code_blocks");
        let include_tables = include_param(config, "tables");
        let include_headings = include_param(config, "headings");

        let tokens = ctx.tokens;
        let line_set = |kinds: &[&str]| {
            let mut set = HashSet::new();
            for id in tokens.filter_by_types(kinds) {
                let token = tokens.get(id);
                add_range_to_set(&mut set, token.start_line, token.end_line);
            }
            set
        };
        let heading_line_numbers = line_set(&["atxHeading", "setextHeading"]);
        let code_block_line_numbers = line_set(&["codeFenced", "codeIndented"]);
        let table_line_numbers = line_set(&["table"]);
        let link_line_numbers = line_set(&["autolink", "image", "link", "literalAutolink"]);
        let mut paragraph_data_line_numbers = HashSet::new();
        for paragraph in tokens.filter_by_types(&["paragraph"]) {
            for data in tokens.descendants_by_type(paragraph, &[&["data"]]) {
                let token = tokens.get(data);
                add_range_to_set(
                    &mut paragraph_data_line_numbers,
                    token.start_line,
                    token.end_line,
                );
            }
        }
        let link_only_line_numbers: HashSet<usize> = link_line_numbers
            .into_iter()
            .filter(|line_number| !paragraph_data_line_numbers.contains(line_number))
            .collect();
        // helpers.cjs `getReferenceLinkImageData().definitionLineIndices`
        let mut definition_line_indices = HashSet::new();
        for id in tokens.filter_by_types(&["definition", "gfmFootnoteDefinition"]) {
            let token = tokens.get(id);
            for i in token.start_line..=token.end_line {
                definition_line_indices.insert(i - 1);
            }
        }

        for (line_index, line) in ctx.lines.iter().enumerate() {
            let line_number = line_index + 1;
            let is_heading = heading_line_numbers.contains(&line_number);
            let in_code = code_block_line_numbers.contains(&line_number);
            let in_table = table_line_numbers.contains(&line_number);
            let max_length = if in_code {
                code_line_length
            } else if is_heading {
                heading_line_length
            } else {
                line_length
            };
            let length = line.chars().count();
            // If not strict/stern, the last run of non-whitespace is allowed to go
            // beyond the limit as long as it begins within the limit
            let text_length = if strict || stern {
                length
            } else {
                // `line.replace(/\S*$/u, "#")`
                let trailing = line
                    .chars()
                    .rev()
                    .take_while(|c| !is_js_whitespace(*c))
                    .count();
                length - trailing + 1
            };
            if max_length > 0.0
                && (include_code_blocks || !in_code)
                && (include_tables || !in_table)
                && (include_headings || !is_heading)
                && !definition_line_indices.contains(&line_index)
                && (strict
                    || !(link_only_line_numbers.contains(&line_number)
                        || (stern && NOT_WRAPPABLE_RE.is_match(line))))
                && (text_length as f64 > max_length)
            {
                let max_length_int = max_length as usize;
                out.add_error_detail_if(
                    line_number,
                    number_to_string(max_length),
                    length,
                    None,
                    None,
                    Some((max_length_int + 1, length - max_length_int)),
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
        let config = json!({ "default": false, "MD013": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md013_reports_long_line_with_range() {
        // 마지막 단어가 81열부터 시작하면 초과
        let content = format!("{}bb\n", "a ".repeat(41));
        let errs = lint_rule("MD013", &content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 80; Actual: 84")
        );
        assert_eq!(errs[0].error_range, Some((81, 4)));
    }

    #[test]
    fn md013_last_word_may_cross_limit_unless_strict() {
        // 마지막 단어가 79열에서 시작해 86열까지 이어지면 기본 모드에서는 허용
        let content = format!("{}bbbbbbbb\n", "a ".repeat(39));
        assert!(lint_rule("MD013", &content).is_empty());
        assert_eq!(lint_with(json!({ "strict": true }), &content).len(), 1);
        assert_eq!(lint_with(json!({ "stern": true }), &content).len(), 1);
        // stern 은 줄일 수 없는 줄(단어 하나)은 봐준다
        let word = format!("{}\n", "x".repeat(90));
        assert!(lint_rule("MD013", &word).is_empty());
        assert!(lint_with(json!({ "stern": true }), &word).is_empty());
        assert_eq!(lint_with(json!({ "strict": true }), &word).len(), 1);
    }

    #[test]
    fn md013_link_only_and_definition_lines_are_skipped() {
        let link = format!("[text](https://example.com/{})\n", "a".repeat(80));
        assert!(lint_rule("MD013", &link).is_empty());
        let definition = format!("[ref]: https://example.com/{}\n", "a".repeat(80));
        assert!(lint_rule("MD013", &definition).is_empty());
        let mixed = format!("see [text](https://example.com/{}) now\n", "a".repeat(80));
        assert_eq!(lint_rule("MD013", &mixed).len(), 1);
    }

    #[test]
    fn md013_separate_limits_and_toggles() {
        let content = format!(
            "# {}h\n\n```\n{}c\n```\n\n| {}|\n|---|\n",
            "h ".repeat(44),
            "c ".repeat(44),
            "t ".repeat(44)
        );
        assert_eq!(lint_rule("MD013", &content).len(), 3);
        assert!(
            lint_with(
                json!({ "headings": false, "code_blocks": false, "tables": false }),
                &content
            )
            .is_empty()
        );
        let errs = lint_with(
            json!({ "heading_line_length": 100, "code_block_line_length": "95" }),
            &content,
        );
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 7);
        // 0 은 falsy 라 기본값 80 으로 돌아가고, 음수여야 비활성이다
        assert_eq!(lint_with(json!({ "line_length": 0 }), &content).len(), 3);
        assert!(lint_with(json!({ "line_length": -1 }), &content).is_empty());
    }
}
