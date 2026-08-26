use super::{LintContext, Rule, RuleMeta};
use crate::error::ErrorSink;
use crate::parser::{TokenId, TokenTree};

pub(crate) struct Md042;

static META: RuleMeta = RuleMeta {
    names: &["MD042", "no-empty-links"],
    description: "No empty links",
    tags: &["links"],
    needs_tokens: true,
    fixable: false,
};

/// JS `String.prototype.trim` 이 지우는 문자 (`\s` 와 같은 집합, `JS_WHITESPACE`).
const JS_TRIM_CHARS: &[char] = &[
    '\t', '\n', '\x0B', '\x0C', '\r', ' ', '\u{a0}', '\u{1680}', '\u{2000}', '\u{2001}',
    '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}',
    '\u{200a}', '\u{2028}', '\u{2029}', '\u{202f}', '\u{205f}', '\u{3000}', '\u{feff}',
];

/// JS `text.trim()`.
fn js_trim(text: &str) -> &str {
    text.trim_matches(JS_TRIM_CHARS)
}

/// 원본 `getDescendantsByType(tokens, typePath)` 의 배열 입력 형태: 각 토큰에서 경로를 따라
/// 내려간 결과를 이어 붙인다.
fn descendants_of_all(tree: &TokenTree, tokens: &[TokenId], type_path: &[&[&str]]) -> Vec<TokenId> {
    tokens
        .iter()
        .flat_map(|&id| tree.descendants_by_type(id, type_path))
        .collect()
}

impl Rule for Md042 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let definitions = ctx.tokens.reference_link_image_data().definitions;
        // 원본 `isReferenceDefinitionHash`: 정의의 destination 이 정확히 "#" 인지.
        let is_reference_definition_hash = |token: TokenId| -> bool {
            let definition = definitions.get(js_trim(&ctx.tokens.get(token).text));
            definition.is_some_and(|definition| definition.1 == "#")
        };
        // 원본 `filterByTypesCached([ "link" ])`.
        let links = ctx.tokens.filter_by_types(&["link"]);
        for link in links {
            let label_text = ctx
                .tokens
                .descendants_by_type(link, &[&["label"], &["labelText"]]);
            let reference = ctx.tokens.descendants_by_type(link, &[&["reference"]]);
            let resource = ctx.tokens.descendants_by_type(link, &[&["resource"]]);
            let reference_string =
                descendants_of_all(ctx.tokens, &reference, &[&["referenceString"]]);
            let resource_destination_string = descendants_of_all(
                ctx.tokens,
                &resource,
                &[
                    &["resourceDestination"],
                    &["resourceDestinationLiteral", "resourceDestinationRaw"],
                    &["resourceDestinationString"],
                ],
            );
            let has_label_text = !label_text.is_empty();
            let has_reference = !reference.is_empty();
            let has_resource = !resource.is_empty();
            let has_reference_string = !reference_string.is_empty();
            let has_resource_destination_string = !resource_destination_string.is_empty();
            let mut error = false;
            if has_label_text
                && ((!has_reference && !has_resource) || (has_reference && !has_reference_string))
            {
                error = is_reference_definition_hash(label_text[0]);
            } else if has_reference_string && !has_resource_destination_string {
                error = is_reference_definition_hash(reference_string[0]);
            } else if !has_reference_string && has_resource_destination_string {
                error = js_trim(&ctx.tokens.get(resource_destination_string[0]).text) == "#";
            } else if !has_reference_string && !has_resource_destination_string {
                error = true;
            }
            if error {
                let token = ctx.tokens.get(link);
                out.add_error_context(
                    token.start_line,
                    &token.text,
                    false,
                    false,
                    Some((token.start_column, token.end_column - token.start_column)),
                    None,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::lint_rule;

    #[test]
    fn md042_empty_inline_links() {
        let errs = lint_rule("MD042", "a [text]() b\n\n[x](<>)\n\n[y](#)\n");
        assert_eq!(errs.len(), 3);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("[text]()"));
        assert_eq!(errs[0].error_range, Some((3, 8)));
        assert!(errs[0].fix_info.is_none());
        assert_eq!(errs[1].error_context.as_deref(), Some("[x](<>)"));
        assert_eq!(errs[2].error_context.as_deref(), Some("[y](#)"));
    }

    #[test]
    fn md042_non_empty_links_are_fine() {
        assert!(lint_rule("MD042", "[text](https://example.com)\n").is_empty());
        assert!(lint_rule("MD042", "[text](#fragment)\n").is_empty());
        assert!(lint_rule("MD042", "[text](<#a>)\n").is_empty());
    }

    #[test]
    fn md042_reference_links_with_hash_definition() {
        let content = "[full][ref] and [collapsed][] and [shortcut]\n\n[ref]: #\n[collapsed]: #\n[shortcut]: #\n";
        let errs = lint_rule("MD042", content);
        assert_eq!(errs.len(), 3);
        assert_eq!(errs[0].error_context.as_deref(), Some("[full][ref]"));
        assert_eq!(errs[1].error_context.as_deref(), Some("[collapsed][]"));
        assert_eq!(errs[2].error_context.as_deref(), Some("[shortcut]"));
    }

    #[test]
    fn md042_reference_links_with_real_definition() {
        let content = "[full][ref] and [collapsed][] and [shortcut]\n\n[ref]: #a\n[collapsed]: https://example.com\n[shortcut]: <#b>\n";
        assert!(lint_rule("MD042", content).is_empty());
    }

    #[test]
    fn md042_label_is_trimmed_before_lookup() {
        let errs = lint_rule("MD042", "[ text ][]\n\n[text]: #\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("[ text ][]"));
    }

    #[test]
    fn md042_multiline_link_context_and_range() {
        let errs = lint_rule("MD042", "see [empty\nlink]() here\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("[empty link]()"));
        // 원본과 같이 endColumn(2번째 줄 기준) - startColumn 을 그대로 쓴다.
        assert_eq!(errs[0].error_range, Some((5, 3)));
    }
}
