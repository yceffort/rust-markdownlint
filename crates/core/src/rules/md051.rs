use std::sync::LazyLock;

use regex::Regex;

use super::{LintContext, Rule, RuleMeta};
use crate::config::{js_string, truthy};
use crate::error::{ErrorSink, FixInfo};
use crate::parser::{OrderedMap, TokenId, TokenTree, html_attribute_re};

pub(crate) struct Md051;

static META: RuleMeta = RuleMeta {
    names: &["MD051", "link-fragments"],
    description: "Link fragments should be valid",
    tags: &["links"],
    needs_tokens: true,
    fixable: true,
};

// HTML anchor 이름을 찾는 정규식
static ID_RE: LazyLock<Regex> = LazyLock::new(|| html_attribute_re("id"));
static NAME_RE: LazyLock<Regex> = LazyLock::new(|| html_attribute_re("name"));
/// 원본 `anchorRe`: `/\{(#[a-z\d]+(?:[-_][a-z\d]+)*)\}/gu`.
static ANCHOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{(#[a-z0-9]+(?:[-_][a-z0-9]+)*)\}").expect("md051 anchor regex")
});
/// 원본 `lineFragmentRe`: `/^#(?:L\d+(?:C\d+)?-L\d+(?:C\d+)?|L\d+)$/`.
static LINE_FRAGMENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^#(?:L[0-9]+(?:C[0-9]+)?-L[0-9]+(?:C[0-9]+)?|L[0-9]+)$")
        .expect("md051 line fragment regex")
});
/// Ruby 의 `\p{Word}` 를 General Category 로 풀어 쓴 정규식 (원본 주석 참고:
/// html-pipeline 의 toc_filter.rb). `/[^\p{Letter}\p{Mark}\p{Number}\p{Connector_Punctuation}\- ]/gu`.
static NON_WORD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^\p{L}\p{M}\p{N}\p{Pc}\- ]").expect("md051 non-word regex"));

// 변환 중 heading 토큰을 거르는 집합
const CHILDREN_EXCLUDE: &[&str] = &["image", "reference", "resource"];
const TOKENS_INCLUDE: &[&str] = &[
    "characterEscapeValue",
    "codeTextData",
    "data",
    "mathTextData",
];

/// JS `encodeURIComponent`: UTF-8 바이트 단위 percent-encoding. 예외 문자는
/// `A-Z a-z 0-9 - _ . ! ~ * ' ( )`, 16진수는 대문자.
fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// 원본 `convertHeadingToHTMLFragment`: GitHub 규칙대로 Markdown heading 을 HTML fragment 로 바꾼다.
fn convert_heading_to_html_fragment(tokens: &TokenTree, heading_text: TokenId) -> String {
    let inline_text: String = tokens
        .filter_by_predicate(
            &tokens.get(heading_text).children,
            |t, id| TOKENS_INCLUDE.contains(&t.get(id).kind),
            |t, id| {
                if CHILDREN_EXCLUDE.contains(&t.get(id).kind) {
                    vec![]
                } else {
                    t.get(id).children.clone()
                }
            },
        )
        .into_iter()
        .map(|id| tokens.text(id))
        .collect();
    format!(
        "#{}",
        encode_uri_component(
            &NON_WORD_RE
                .replace_all(&inline_text.to_lowercase(), "")
                .replace(' ', "-")
        )
    )
}

/// 원본 `filterByTypes(token.children, types)`: 자식 이하를 전위 순회하며 타입이 맞고
/// htmlFlow 재파싱으로 생기지 않은 토큰을 모은다.
fn filter_children_by_types(tokens: &TokenTree, token: TokenId, types: &[&str]) -> Vec<TokenId> {
    tokens.filter_by_predicate(
        &tokens.get(token).children,
        |t, id| types.contains(&t.get(id).kind) && !t.get(id).in_html_flow,
        |t, id| t.get(id).children.clone(),
    )
}

/// 원본 `unescapeStringTokenText`: String 계열 micromark 토큰의 텍스트를 unescape 한다.
fn unescape_string_token_text(tokens: &TokenTree, token: TokenId) -> String {
    filter_children_by_types(tokens, token, &["characterEscapeValue", "data"])
        .into_iter()
        .map(|child| tokens.text(child))
        .collect()
}

impl Rule for Md051 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let ignore_case = ctx.config.get("ignore_case").is_some_and(truthy);
        let ignored_pattern = ctx
            .config
            .get("ignored_pattern")
            .filter(|value| truthy(value))
            .map(js_string)
            .unwrap_or_default();
        // 사용자 정규식이라 fancy_regex 로 컴파일한다. 컴파일 실패는 원본이 예외를 던지지만
        // 여기서는 매치 안 됨으로 본다. 기본값(빈 패턴)은 파일마다 다시 컴파일하지 않는다.
        static EMPTY_PATTERN_RE: LazyLock<fancy_regex::Regex> =
            LazyLock::new(|| fancy_regex::Regex::new("^$").expect("empty pattern regex"));
        let ignored_pattern_re = if ignored_pattern.is_empty() {
            Some(EMPTY_PATTERN_RE.clone())
        } else {
            fancy_regex::Regex::new(&ignored_pattern).ok()
        };
        let mut fragments: OrderedMap<usize> = OrderedMap::default();
        fragments.set("#top".to_string(), 0);

        // heading 처리
        let heading_texts = ctx
            .tokens
            .filter_by_types(&["atxHeadingText", "setextHeadingText"]);
        for heading_text in heading_texts {
            let fragment = convert_heading_to_html_fragment(ctx.tokens, heading_text);
            if fragment != "#" {
                let count = fragments.get(&fragment).copied().unwrap_or(0);
                if count > 0 {
                    fragments.set(format!("{fragment}-{count}"), 0);
                }
                fragments.set(fragment, count + 1);
                for m in ANCHOR_RE.captures_iter(ctx.tokens.text(heading_text)) {
                    let anchor = &m[1];
                    if !fragments.contains_key(anchor) {
                        fragments.set(anchor.to_string(), 1);
                    }
                }
            }
        }

        // HTML anchor 처리
        for token in ctx.tokens.filter_by_types_html_flow(&["htmlText"], true) {
            let Some(html_tag_info) = ctx.tokens.html_tag_info(token) else {
                continue;
            };
            if html_tag_info.close {
                continue;
            }
            let text = ctx.tokens.text(token);
            let anchor_match = ID_RE.captures(text).or_else(|| {
                if html_tag_info.name.to_lowercase() == "a" {
                    NAME_RE.captures(text)
                } else {
                    None
                }
            });
            if let Some(anchor_match) = anchor_match {
                fragments.set(format!("#{}", &anchor_match[1]), 0);
            }
        }

        // 링크와 정의의 fragment 처리
        let parent_childs = [
            ("link", "resourceDestinationString"),
            ("definition", "definitionDestinationString"),
        ];
        for (parent_type, definition_type) in parent_childs {
            let links = ctx
                .tokens
                .filter_by_types(&[parent_type])
                .into_iter()
                .filter(|&link| {
                    let parent = ctx.tokens.get(link).parent;
                    !(parent.is_some_and(|p| ctx.tokens.get(p).kind == "atxHeadingText")
                        && ctx
                            .tokens
                            .get(parent.unwrap())
                            .parent
                            .is_some_and(|grandparent| ctx.tokens.is_docfx_tab(grandparent)))
                });
            for link in links {
                let definitions = filter_children_by_types(ctx.tokens, link, &[definition_type]);
                for definition in definitions {
                    let (end_column, start_column) = {
                        let d = ctx.tokens.get(definition);
                        (d.end_column, d.start_column)
                    };
                    let text = unescape_string_token_text(ctx.tokens, definition);
                    let Some(text_slice_one) = text.strip_prefix('#') else {
                        continue;
                    };
                    let encoded_text = format!("#{}", encode_uri_component(text_slice_one));
                    if !text_slice_one.is_empty()
                        && !fragments.contains_key(&encoded_text)
                        && !LINE_FRAGMENT_RE.is_match(&encoded_text)
                        && !ignored_pattern_re
                            .as_ref()
                            .is_some_and(|re| re.is_match(text_slice_one).unwrap_or(false))
                    {
                        let link_token = ctx.tokens.get(link);
                        let mut context = None;
                        let mut range = None;
                        let mut fix_info = None;
                        if link_token.start_line == link_token.end_line {
                            context = Some(ctx.tokens.text(link));
                            range = Some((
                                link_token.start_column,
                                link_token.end_column - link_token.start_column,
                            ));
                            fix_info = Some(FixInfo {
                                edit_column: Some(start_column),
                                delete_count: Some((end_column - start_column) as isize),
                                ..Default::default()
                            });
                        }
                        let text_lower = text.to_lowercase();
                        let mixed_case_key = fragments
                            .keys()
                            .find(|key| text_lower == key.to_lowercase())
                            .map(str::to_string);
                        if let Some(mixed_case_key) = mixed_case_key {
                            if let Some(fix_info) = fix_info.as_mut() {
                                fix_info.insert_text = Some(mixed_case_key.clone());
                            }
                            if !ignore_case && mixed_case_key != text {
                                out.add_error(
                                    link_token.start_line,
                                    Some(&format!("Expected: {mixed_case_key}; Actual: {text}")),
                                    context,
                                    range,
                                    fix_info,
                                );
                            }
                        } else {
                            out.add_error(link_token.start_line, None, context, range, None);
                        }
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
        let config = json!({ "default": false, "MD051": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md051_valid_fragments() {
        let content = "# Heading Name\n\n## Ünïcödé & `code` *em*\n\n<a name=\"anchor\"></a>\n<div id=\"div-id\"></div>\n\n[a](#heading-name) [b](#%C3%BCn%C3%AFc%C3%B6d%C3%A9--code-em) [c](#anchor) [d](#div-id) [e](#top) [f](#L10-L20)\n\n[g]: #heading-name\n";
        assert!(lint_rule("MD051", content).is_empty());
    }

    #[test]
    fn md051_invalid_fragment_reports_link_range() {
        let errs = lint_rule("MD051", "# Heading\n\nText [link](#missing) here.\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(errs[0].error_detail, None);
        assert_eq!(errs[0].error_context.as_deref(), Some("[link](#missing)"));
        assert_eq!(errs[0].error_range, Some((6, 16)));
        assert!(errs[0].fix_info.is_none());
    }

    #[test]
    fn md051_mixed_case_is_fixable() {
        let errs = lint_rule("MD051", "# Heading\n\n[link](#HEADING)\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: #heading; Actual: #HEADING")
        );
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(
            (f.edit_column, f.delete_count, f.insert_text.as_deref()),
            (Some(8), Some(8), Some("#heading"))
        );
        assert!(
            lint_with(
                json!({ "ignore_case": true }),
                "# Heading\n\n[link](#HEADING)\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn md051_duplicate_headings_and_custom_anchor() {
        let content = "# Same\n\n# Same\n\n# Custom {#custom-id}\n\n[a](#same) [b](#same-1) [c](#same-2) [d](#custom-id)\n";
        let errs = lint_rule("MD051", content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("[c](#same-2)"));
    }

    #[test]
    fn md051_ignored_pattern() {
        let content = "[a](#figure-1) [b](#other)\n";
        assert_eq!(lint_rule("MD051", content).len(), 2);
        let errs = lint_with(json!({ "ignored_pattern": "^figure-\\d+$" }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("[b](#other)"));
    }

    #[test]
    fn md051_multiline_link_has_no_context() {
        let errs = lint_rule("MD051", "[link\ntext](#missing)\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_context, None);
        assert_eq!(errs[0].error_range, None);
    }
}
