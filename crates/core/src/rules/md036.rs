use std::sync::LazyLock;

use regex::Regex;

use super::{LintContext, Rule, RuleMeta};
use crate::error::ErrorSink;
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
        // 바깥 Option 은 config 지정 여부, 안쪽 Option 은 "절대 매치하지 않음"(JS 의 빈 `[]`).
        let custom_re: Option<Option<Regex>> = match ctx.config.get("punctuation") {
            None => None,
            Some(value) => {
                let punctuation = match value {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                if punctuation.is_empty() {
                    // JS 의 빈 문자 클래스 `[]$` 는 아무것도 매치하지 않는다.
                    Some(None)
                } else {
                    // 원본은 잘못된 정규식이면 예외를 던진다. 여기서는 아무것도 보고하지 않는다.
                    match Regex::new(&format!("[{punctuation}]$")) {
                        Ok(re) => Some(Some(re)),
                        Err(_) => return,
                    }
                }
            }
        };
        let punctuation_re: Option<&Regex> = match &custom_re {
            None => Some(&DEFAULT_RE),
            Some(inner) => inner.as_ref(),
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
                    && !punctuation_re.is_some_and(|re| re.is_match(tokens.text(id)))
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
