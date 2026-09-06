use std::sync::LazyLock;

use regex::Regex;

use super::{LintContext, Rule, RuleMeta};
use crate::config::js_string;
use crate::error::ErrorSink;
use crate::front_matter::{compile_js_regex, js_regex_error_message};
use crate::parser::{TokenId, TokenTree};

pub(crate) struct Md036;

static META: RuleMeta = RuleMeta {
    names: &["MD036", "no-emphasis-as-heading"],
    description: "Emphasis used instead of a heading",
    tags: &["headings", "emphasis"],
    needs_tokens: true,
    fixable: false,
};

/// helpers.cjs `allPunctuation`.
const ALL_PUNCTUATION: &str = ".,;:!?。，；：！？";

/// 원본 `emphasisTypes`: 각 원소는 `getDescendantsByType` 에 넘길 타입 경로다.
const EMPHASIS_TYPES: [[&str; 2]; 2] = [["emphasis", "emphasisText"], ["strong", "strongText"]];

/// 원본 `isParagraphChildMeaningful`.
fn is_paragraph_child_meaningful(tokens: &TokenTree, id: TokenId) -> bool {
    let token = tokens.get(id);
    !((token.kind == "htmlText") || (token.kind == "data" && tokens.text(id).trim().is_empty()))
}

impl Rule for Md036 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        // 원본 `punctuationRe`: `[<punctuation>]$`. 원본은 파일마다 새로 만들지만
        // 기본값은 한 번만 컴파일한다.
        static DEFAULT_RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(&format!("[{ALL_PUNCTUATION}]$")).expect("default punctuation")
        });
        // 사용자 punctuation 은 JS 문자 클래스 본문이므로 `[...]$` 전체를 JS 정규식으로 옮긴다
        // (`[]` 는 아무것도, `[^]` 는 모든 문자를 매치하고, `[` 는 클래스 안에서 리터럴이다).
        let custom_re = match ctx.config.get("punctuation") {
            None => None,
            Some(value) => {
                let source = format!("[{}]$", js_string(value));
                match compile_js_regex(&source, false, false) {
                    Ok(re) => Some(re),
                    Err(error) => {
                        return out.add_rule_failure(&js_regex_error_message(&source, "", &error));
                    }
                }
            }
        };
        let ends_with_punctuation = |text: &str| match &custom_re {
            None => DEFAULT_RE.is_match(text),
            Some(re) => re.is_match(text).unwrap_or(false),
        };

        let tokens = ctx.tokens;
        let paragraph_tokens: Vec<TokenId> = tokens
            .filter_by_types_html_flow(&["paragraph"], true)
            .into_iter()
            .filter(|&id| {
                let Some(parent) = tokens.get(id).parent else {
                    return false;
                };
                if tokens.get(parent).kind != "content" {
                    return false;
                }
                let grandparent_ok = match tokens.get(parent).parent {
                    None => true,
                    Some(grandparent) => {
                        tokens.get(grandparent).kind == "htmlFlow"
                            && tokens.get(grandparent).parent.is_none()
                    }
                };
                grandparent_ok
                    && tokens
                        .get(id)
                        .children
                        .iter()
                        .filter(|&&child| is_paragraph_child_meaningful(tokens, child))
                        .count()
                        == 1
            })
            .collect();

        for emphasis_type in EMPHASIS_TYPES {
            let type_path: [&[&str]; 2] = [&emphasis_type[0..1], &emphasis_type[1..2]];
            let text_tokens = paragraph_tokens
                .iter()
                .flat_map(|&id| tokens.descendants_by_type(id, &type_path));
            for id in text_tokens {
                let text_token = tokens.get(id);
                if (text_token.children.len() == 1)
                    && (tokens.get(text_token.children[0]).kind == "data")
                    && !ends_with_punctuation(tokens.text(id))
                {
                    out.add_error_context(
                        text_token.start_line,
                        tokens.text(id),
                        false,
                        false,
                        None,
                        None,
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
        let config = json!({ "default": false, "MD036": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md036_emphasis_and_strong_paragraphs() {
        let errs = lint_rule("MD036", "*Section 1*\n\ntext\n\n**Section 2**\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("Section 1"));
        assert_eq!(errs[1].line_number, 5);
        assert_eq!(errs[1].error_context.as_deref(), Some("Section 2"));
        assert!(errs[0].fix_info.is_none());
    }

    #[test]
    fn md036_trailing_punctuation_and_multiline_are_allowed() {
        assert!(lint_rule("MD036", "**A heading.**\n").is_empty());
        assert!(lint_rule("MD036", "**A heading。**\n").is_empty());
        assert!(lint_rule("MD036", "**one\ntwo**\n").is_empty());
    }

    #[test]
    fn md036_non_data_child_and_extra_paragraph_content() {
        // 자식이 link 라 data 가 아니다.
        assert!(lint_rule("MD036", "**[link](https://example.com)**\n").is_empty());
        // 문단에 의미 있는 자식이 둘 이상이다.
        assert!(lint_rule("MD036", "text *emphasis*\n").is_empty());
    }

    #[test]
    fn md036_html_comment_and_blank_data_are_not_meaningful() {
        let errs = lint_rule("MD036", "*Section 4* <!-- comment -->\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("Section 4"));
    }

    /// #186: micromark 는 정의되지 않은 참조의 `[` 를 별도 data 로 남겨 강조의 자식이 여럿이 된다.
    #[test]
    fn md036_brackets_inside_emphasis_are_not_a_heading() {
        assert!(lint_rule("MD036", "**a [b] c**\n").is_empty());
        assert!(lint_rule("MD036", "*a [b] c*\n").is_empty());
        assert!(lint_rule("MD036", "**/pages/post/[id].tsx**\n").is_empty());
        // 정의된 참조와 인라인 링크는 link 자식이라 원래부터 잡히지 않는다.
        assert!(lint_rule("MD036", "**a [b] c**\n\n[b]: x\n").is_empty());
        assert!(lint_rule("MD036", "**a [b](x) c**\n").is_empty());
        // 대조군: 괄호가 없으면 잡힌다.
        assert_eq!(lint_rule("MD036", "**a b c**\n").len(), 1);
    }

    #[test]
    fn md036_leading_closing_bracket_makes_empty_js_class() {
        let errs = lint_with(json!({ "punctuation": "]" }), "**title]**\n");
        assert_eq!(errs.len(), 1);
    }

    /// JS 클래스 안의 `[` 는 리터럴, `[^]` 는 모든 문자, `[]` 는 없는 문자다.
    #[test]
    fn md036_punctuation_uses_javascript_class_syntax() {
        let errs = lint_with(json!({ "punctuation": "[" }), "**title[**\n\n**plain**\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        let errs = lint_with(json!({ "punctuation": "^]" }), "**title]**\n\n**titlea**\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        let errs = lint_with(json!({ "punctuation": "z-a" }), "**Title**\n");
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some(
                "This rule threw an exception: Invalid regular expression: /[z-a]$/: Range out of order in character class"
            )
        );
    }

    #[test]
    fn md036_invalid_character_class_is_a_rule_failure() {
        let errs = lint_with(json!({ "punctuation": "\\" }), "**title\\**\n");
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some(
                "This rule threw an exception: Invalid regular expression: /[\\]$/: Unterminated character class"
            )
        );
    }

    #[test]
    fn md036_paragraph_inside_list_is_ignored() {
        assert!(lint_rule("MD036", "* **Emphasized item**\n").is_empty());
    }

    #[test]
    fn md036_punctuation_config() {
        // 기본 구두점 목록에 없는 `-` 는 보고된다.
        assert_eq!(lint_rule("MD036", "**Heading-**\n").len(), 1);
        assert!(lint_with(json!({ "punctuation": ".-" }), "**Heading-**\n").is_empty());
        // 빈 문자열은 JS 의 빈 문자 클래스라 아무것도 제외하지 않는다.
        assert_eq!(
            lint_with(json!({ "punctuation": "" }), "**Heading.**\n").len(),
            1
        );
    }
}
