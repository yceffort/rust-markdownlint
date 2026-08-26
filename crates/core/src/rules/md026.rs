use std::sync::LazyLock;

use regex::Regex;

use super::{LintContext, Rule, RuleMeta};
use crate::error::{ErrorSink, FixInfo};
use crate::parser::JS_WHITESPACE;

pub(crate) struct Md026;

static META: RuleMeta = RuleMeta {
    names: &["MD026", "no-trailing-punctuation"],
    description: "Trailing punctuation in heading",
    tags: &["headings"],
    needs_tokens: true,
    fixable: true,
};

/// helpers.cjs `allPunctuationNoQuestion`: `allPunctuation` 에서 `?` 와 `？` 를 뺀 것.
const ALL_PUNCTUATION_NO_QUESTION: &str = ".,;:!。，；：！";

/// helpers.cjs `endOfLineHtmlEntityRe`.
static END_OF_LINE_HTML_ENTITY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"&(?:#\d+|#[xX][\da-fA-F]+|[a-zA-Z]{2,31}|blk\d{2}|emsp1[34]|frac\d{2}|sup\d|there4);$",
    )
    .expect("end of line html entity regex")
});

/// helpers.cjs `endOfLineGemojiCodeRe`.
static END_OF_LINE_GEMOJI_CODE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r":(?:[abmovx]|[-+]1|100|1234|(?:1st|2nd|3rd)_place_medal|8ball|clock\d{1,4}|e-mail|non-potable_water|o2|t-rex|u5272|u5408|u55b6|u6307|u6708|u6709|u6e80|u7121|u7533|u7981|u7a7a|[a-z]{2,15}2?|[a-z]{1,14}(?:_[a-z\d]{1,16})+):$",
    )
    .expect("end of line gemoji code regex")
});

/// helpers.cjs `escapeForRegExp`.
fn escape_for_reg_exp(str: &str) -> String {
    let mut escaped = String::new();
    for c in str.chars() {
        if r"-/\^$*+?.()|[]{}".contains(c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// 원본 `trailingPunctuationRe`: `\s*[<punctuation>]+$`.
fn trailing_punctuation_re(punctuation: &str) -> Result<Regex, regex::Error> {
    Regex::new(&format!(
        "[{JS_WHITESPACE}]*[{}]+$",
        escape_for_reg_exp(punctuation)
    ))
}

impl Rule for Md026 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        // 원본은 파일마다 정규식을 새로 만든다. 기본값은 한 번만 컴파일한다.
        static DEFAULT_RE: LazyLock<Regex> = LazyLock::new(|| {
            trailing_punctuation_re(ALL_PUNCTUATION_NO_QUESTION).expect("default punctuation")
        });
        let custom_re = match ctx.config.get("punctuation") {
            None => None,
            Some(value) => {
                let punctuation = match value {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                // 원본은 잘못된 정규식이면 예외를 던진다. 여기서는 아무것도 보고하지 않는다.
                match trailing_punctuation_re(&punctuation) {
                    Ok(re) => Some(re),
                    Err(_) => return,
                }
            }
        };
        let trailing_punctuation_re = custom_re.as_ref().unwrap_or(&DEFAULT_RE);
        for id in ctx
            .tokens
            .filter_by_types(&["atxHeadingText", "setextHeadingText"])
        {
            let heading = ctx.tokens.get(id);
            let text = &heading.text;
            let Some(matched) = trailing_punctuation_re.find(text) else {
                continue;
            };
            if END_OF_LINE_HTML_ENTITY_RE.is_match(text)
                || END_OF_LINE_GEMOJI_CODE_RE.is_match(text)
            {
                continue;
            }
            let full_match = matched.as_str();
            let length = full_match.chars().count();
            let column = heading.end_column - length;
            out.add_error(
                heading.end_line,
                Some(&format!("Punctuation: '{full_match}'")),
                None,
                Some((column, length)),
                Some(FixInfo {
                    edit_column: Some(column),
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
        let config = json!({ "default": false, "MD026": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md026_atx_trailing_period() {
        let errs = lint_rule("MD026", "# Heading.\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_detail.as_deref(), Some("Punctuation: '.'"));
        assert_eq!(errs[0].error_range, Some((10, 1)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!((f.edit_column, f.delete_count), (Some(10), Some(1)));
    }

    #[test]
    fn md026_question_mark_allowed_by_default() {
        assert!(lint_rule("MD026", "# Heading?\n").is_empty());
        let errs = lint_with(json!({ "punctuation": ".?" }), "# Heading?\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_detail.as_deref(), Some("Punctuation: '?'"));
    }

    #[test]
    fn md026_setext_and_preceding_whitespace() {
        let errs = lint_rule("MD026", "Heading .\n=========\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_detail.as_deref(), Some("Punctuation: ' .'"));
        assert_eq!(errs[0].error_range, Some((8, 2)));
    }

    #[test]
    fn md026_html_entity_and_gemoji_endings_ignored() {
        assert!(lint_rule("MD026", "# Heading &copy;\n").is_empty());
        assert!(lint_rule("MD026", "# Heading :smile:\n").is_empty());
    }

    #[test]
    fn md026_empty_punctuation_disables_rule() {
        assert!(lint_with(json!({ "punctuation": "" }), "# Heading.\n").is_empty());
    }
}
