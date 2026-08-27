use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::{LintContext, Rule, RuleMeta};
use crate::config::{js_string, truthy};
use crate::error::ErrorSink;
use crate::parser::JS_WHITESPACE;

pub(crate) struct Md059;

static META: RuleMeta = RuleMeta {
    names: &["MD059", "descriptive-link-text"],
    description: "Link text should be descriptive",
    tags: &["accessibility", "links"],
    needs_tokens: true,
    fixable: false,
};

/// 원본 `allowedChildrenTypes`: labelText 의 직계 자식이 이 타입이면 검사에서 제외한다.
const ALLOWED_CHILDREN_TYPES: &[&str] = &["codeText", "htmlText"];

/// 원본 `defaultProhibitedTexts`.
const DEFAULT_PROHIBITED_TEXTS: &[&str] = &["click here", "here", "link", "more"];

/// 원본 `normalize(str)`:
/// `str.replace(/[\W_]+/g, " ").replace(/\s+/g, " ").toLowerCase().trim()`.
///
/// JS 정규식은 `u` 플래그가 없어 `\W` 가 ASCII 기준(`[^A-Za-z0-9_]`)이므로,
/// 유니코드 인식 클래스인 Rust 의 `\W` 대신 `[^0-9A-Za-z]` 를 쓴다 (`[\W_]` 와 같은 집합).
fn normalize(str: &str) -> String {
    static NON_WORD_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[^0-9A-Za-z]+").expect("non-word regex"));
    static WHITESPACE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(&format!("[{JS_WHITESPACE}]+")).expect("whitespace regex"));
    let replaced = NON_WORD_RE.replace_all(str, " ");
    let replaced = WHITESPACE_RE.replace_all(&replaced, " ");
    replaced.to_lowercase().trim().to_string()
}

/// 원본 `new Set((params.config.prohibited_texts || defaultProhibitedTexts).map(normalize))`.
/// falsy 면 기본값을 쓴다. 배열이 아닌 truthy 값은 원본이 `.map` 에서 TypeError 를 내므로
/// 빈 집합으로 두어 (`size > 0` 가드에 걸려) 아무것도 보고하지 않게 한다.
fn prohibited_texts(value: Option<&Value>) -> HashSet<String> {
    match value {
        Some(v) if truthy(v) => match v {
            Value::Array(items) => items
                .iter()
                .map(|item| normalize(&js_string(item)))
                .collect(),
            _ => HashSet::new(),
        },
        _ => DEFAULT_PROHIBITED_TEXTS
            .iter()
            .map(|text| normalize(text))
            .collect(),
    }
}

impl Rule for Md059 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let prohibited_texts = prohibited_texts(ctx.config.get("prohibited_texts"));
        if !prohibited_texts.is_empty() {
            // 원본 `filterByTypesCached([ "link" ])`.
            let links = ctx.tokens.filter_by_types(&["link"]);
            for link in links {
                let label_texts = ctx
                    .tokens
                    .descendants_by_type(link, &[&["label"], &["labelText"]]);
                for label_text in label_texts {
                    let token = ctx.tokens.get(label_text);
                    let has_allowed_child = token
                        .children
                        .iter()
                        .any(|&child| ALLOWED_CHILDREN_TYPES.contains(&ctx.tokens.get(child).kind));
                    if !has_allowed_child
                        && prohibited_texts.contains(&normalize(ctx.tokens.text(label_text)))
                    {
                        // 여러 줄에 걸친 링크 텍스트는 range 를 붙이지 않는다
                        let range = (token.start_line == token.end_line)
                            .then(|| (token.start_column, token.end_column - token.start_column));
                        let parent = ctx
                            .tokens
                            .get(token.parent.expect("labelText has a parent"));
                        out.add_error_context(
                            token.start_line,
                            ctx.tokens.text_of(parent),
                            false,
                            false,
                            range,
                            None,
                        );
                    }
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
        let config = json!({ "default": false, "MD059": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md059_default_prohibited_texts() {
        let errs = lint_rule(
            "MD059",
            "Go [here](https://example.com).\n\n[Click here](https://example.com).\n\nLearn [more](https://example.com).\n\nThis [link](https://example.com).\n",
        );
        assert_eq!(errs.len(), 4);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("[here]"));
        assert_eq!(errs[0].error_range, Some((5, 4)));
        assert!(errs[0].fix_info.is_none());
        assert!(errs[0].error_detail.is_none());
        assert_eq!(errs[1].error_context.as_deref(), Some("[Click here]"));
        assert_eq!(errs[2].error_context.as_deref(), Some("[more]"));
        assert_eq!(errs[3].error_context.as_deref(), Some("[link]"));
    }

    #[test]
    fn md059_descriptive_text_is_fine() {
        assert!(
            lint_rule(
                "MD059",
                "Learn about [our mission](https://example.com/mission).\n\n[Read the guide](https://example.com/guide).\n",
            )
            .is_empty()
        );
    }

    #[test]
    fn md059_punctuation_and_emphasis_are_normalized() {
        // normalize 가 `[\W_]+` 를 공백으로 바꾸므로 구두점과 강조 표시는 무시된다
        let errs = lint_rule(
            "MD059",
            "[here!](d)\n\n[click-here!!!!](d)\n\n[*link*](d)\n\n[~~link~~](d)\n",
        );
        assert_eq!(errs.len(), 4);
        assert_eq!(errs[0].error_context.as_deref(), Some("[here!]"));
        assert_eq!(errs[1].error_context.as_deref(), Some("[click-here!!!!]"));
        assert_eq!(errs[2].error_context.as_deref(), Some("[*link*]"));
        assert_eq!(errs[2].error_range, Some((2, 6)));
        assert_eq!(errs[3].error_context.as_deref(), Some("[~~link~~]"));
    }

    #[test]
    fn md059_code_and_html_children_are_allowed() {
        // 원본 `allowedChildrenTypes` (codeText, htmlText) 가 자식이면 건너뛴다
        assert!(lint_rule("MD059", "[`link`](d)\n\n[<link>](d)\n").is_empty());
    }

    #[test]
    fn md059_multiline_label_has_no_range() {
        let errs = lint_rule("MD059", "[click\nhere](https://example.com)\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("[click here]"));
        assert_eq!(errs[0].error_range, None);
    }

    #[test]
    fn md059_prohibited_texts_option() {
        let content = "[Go here](d)\n\n[link](d)\n\n[this](d)\n";
        let errs = lint_with(json!({ "prohibited_texts": ["go here", "THIS"] }), content);
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].error_context.as_deref(), Some("[Go here]"));
        assert_eq!(errs[1].error_context.as_deref(), Some("[this]"));
        // 빈 배열이면 아무것도 보고하지 않는다
        assert!(lint_with(json!({ "prohibited_texts": [] }), content).is_empty());
    }
}
