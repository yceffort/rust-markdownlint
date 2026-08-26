use std::sync::LazyLock;

use regex::Regex;

use super::{LintContext, NEXT_LINES_RE, Rule, RuleMeta};
use crate::config::truthy;
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md054;

static META: RuleMeta = RuleMeta {
    names: &["MD054", "link-image-style"],
    description: "Link and image style",
    tags: &["images", "links"],
    needs_tokens: true,
    fixable: true,
};

/// 원본 `backslashEscapeRe`.
static BACKSLASH_ESCAPE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r##"\\([!"#$%&'()*+,\-./:;<=>?@\[\\\]^_`{|}~])"##).expect("backslash escape regex")
});

/// 원본 `removeBackslashEscapes`.
fn remove_backslash_escapes(text: &str) -> String {
    BACKSLASH_ESCAPE_RE.replace_all(text, "${1}").into_owned()
}

/// 원본 `autolinkDisallowedRe`.
static AUTOLINK_DISALLOWED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[ <>]").expect("autolink disallowed regex"));

/// WHATWG URL 파서의 scheme 부분: `[A-Za-z][A-Za-z0-9+\-.]*:`.
static URL_SCHEME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z][A-Za-z0-9+\-.]*:").expect("url scheme regex"));

/// 원본 `new URL(destination)` 이 성공하는지 (= 절대 URL 인지). `url` 크레이트를 쓰지 않고
/// WHATWG URL 파서의 실패 조건만 옮긴다: scheme 이 없으면 실패, special scheme
/// (ftp/http/https/ws/wss) 은 host 가 비었거나 금지 문자를 담거나 port 가 숫자가 아니면 실패,
/// 그 밖의 scheme 은 opaque path 라 항상 성공.
fn is_absolute_url(destination: &str) -> bool {
    // C0 제어 문자와 공백을 앞뒤에서 떼고, 안쪽의 탭/개행을 지운다.
    let trimmed = destination.trim_matches(|c: char| c <= ' ');
    let input: String = trimmed
        .chars()
        .filter(|&c| c != '\t' && c != '\n' && c != '\r')
        .collect();
    let Some(scheme_match) = URL_SCHEME_RE.find(&input) else {
        return false;
    };
    let scheme = input[..scheme_match.end() - 1].to_lowercase();
    let rest = &input[scheme_match.end()..];
    if scheme == "file" {
        return true;
    }
    if !matches!(scheme.as_str(), "ftp" | "http" | "https" | "ws" | "wss") {
        // opaque path (예: `mailto:`) 는 파싱이 실패하지 않는다
        return true;
    }
    let rest = rest.trim_start_matches(['/', '\\']);
    let authority_end = rest.find(['/', '\\', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let host_port = match authority.rfind('@') {
        Some(at) => &authority[at + 1..],
        None => authority,
    };
    // IPv6 리터럴은 대괄호를 허용한다
    if host_port.starts_with('[') {
        return host_port.contains(']');
    }
    let (host, port) = match host_port.find(':') {
        Some(colon) => (&host_port[..colon], &host_port[colon + 1..]),
        None => (host_port, ""),
    };
    if host.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    // forbidden host code points 중 위에서 걸러지지 않는 것들
    !host.contains([
        '\0', ' ', '<', '>', '[', ']', '^', '|', '#', '/', '?', '@', '\\', ':',
    ])
}

/// 원본 `autolinkAble`.
fn autolink_able(destination: &str) -> bool {
    is_absolute_url(destination) && !AUTOLINK_DISALLOWED_RE.is_match(destination)
}

/// JS `s.replace(/[[\]]/g, "\\$&")`.
fn escape_brackets(text: &str) -> String {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\[\]]").expect("brackets regex"));
    RE.replace_all(text, r"\${0}").into_owned()
}

/// JS `s.replace(/[()]/g, "\\$&")`.
fn escape_parens(text: &str) -> String {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[()]").expect("parens regex"));
    RE.replace_all(text, r"\${0}").into_owned()
}

impl Rule for Md054 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        // 원본 `(config.x === undefined) || !!config.x`
        let flag = |key: &str| ctx.config.get(key).is_none_or(truthy);
        let autolink = flag("autolink");
        let inline = flag("inline");
        let full = flag("full");
        let collapsed = flag("collapsed");
        let shortcut = flag("shortcut");
        let url_inline = flag("url_inline");
        if autolink && inline && full && collapsed && shortcut && url_inline {
            // Everything allowed, nothing to check
            return;
        }
        let data = ctx.tokens.reference_link_image_data();
        let definitions = &data.definitions;
        let text_of = |id| ctx.tokens.get(id).text.clone();
        // 원본 `filterByTypesCached([ "autolink", "image", "link" ])`
        let links = ctx.tokens.filter_by_types(&["autolink", "image", "link"]);
        for link in links {
            let token = ctx.tokens.get(link);
            let (start_line, end_line) = (token.start_line, token.end_line);
            let (start_column, end_column) = (token.start_column, token.end_column);
            let text = token.text.clone();
            let image = token.kind == "image";
            let is_autolink = token.kind == "autolink";
            let label;
            let mut destination;
            let is_error;
            if is_autolink {
                // link kind is an autolink
                destination = ctx
                    .tokens
                    .descendants_by_type(link, &[&["autolinkEmail", "autolinkProtocol"]])
                    .first()
                    .map(|&id| text_of(id))
                    .unwrap_or_default();
                label = destination.clone();
                is_error = !autolink && !destination.is_empty();
            } else {
                // link type is "image" or "link"
                // 원본은 labelText 가 없으면 예외를 던진다. 여기서는 그 토큰을 건너뛴다.
                let Some(&label_text) = ctx
                    .tokens
                    .descendants_by_type(link, &[&["label"], &["labelText"]])
                    .first()
                else {
                    continue;
                };
                label = text_of(label_text);
                destination = ctx
                    .tokens
                    .descendants_by_type(
                        link,
                        &[
                            &["resource"],
                            &["resourceDestination"],
                            &["resourceDestinationLiteral", "resourceDestinationRaw"],
                            &["resourceDestinationString"],
                        ],
                    )
                    .first()
                    .map(|&id| text_of(id))
                    .unwrap_or_default();
                if !destination.is_empty() {
                    // link kind is an inline link
                    let title = ctx
                        .tokens
                        .descendants_by_type(
                            link,
                            &[&["resource"], &["resourceTitle"], &["resourceTitleString"]],
                        )
                        .first()
                        .map(|&id| text_of(id))
                        .unwrap_or_default();
                    is_error = !inline
                        || (!url_inline
                            && autolink
                            && !image
                            && title.is_empty()
                            && (label == destination)
                            && autolink_able(&destination));
                } else {
                    // link kind is a full/collapsed/shortcut reference link
                    let is_shortcut = ctx
                        .tokens
                        .descendants_by_type(link, &[&["reference"]])
                        .is_empty();
                    let reference_string = ctx
                        .tokens
                        .descendants_by_type(link, &[&["reference"], &["referenceString"]])
                        .first()
                        .map(|&id| text_of(id));
                    let is_collapsed = reference_string.is_none();
                    // 원본과 같이 정규화하지 않은 라벨로 조회한다
                    let key = reference_string
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| label.clone());
                    destination = definitions
                        .get(&key)
                        .map(|(_, dest)| dest.clone())
                        .unwrap_or_default();
                    is_error = !destination.is_empty()
                        && if is_shortcut {
                            !shortcut
                        } else if is_collapsed {
                            !collapsed
                        } else {
                            !full
                        };
                }
            }
            if is_error {
                let mut range = None;
                let mut fix_info = None;
                if start_line == end_line {
                    let r = (start_column, end_column - start_column);
                    range = Some(r);
                    let mut insert_text = None;
                    let can_inline = inline && !label.is_empty();
                    let can_autolink = autolink && !image && autolink_able(&destination);
                    if can_inline && (url_inline || !can_autolink) {
                        // Most useful form
                        let prefix = if image { "!" } else { "" };
                        insert_text = Some(format!(
                            "{prefix}[{}]({})",
                            escape_brackets(&label),
                            escape_parens(&destination)
                        ));
                    } else if can_autolink {
                        // Simplest form
                        insert_text = Some(format!("<{}>", remove_backslash_escapes(&destination)));
                    }
                    if let Some(insert_text) = insert_text {
                        fix_info = Some(FixInfo {
                            line_number: None,
                            edit_column: Some(r.0),
                            delete_count: Some(r.1 as isize),
                            insert_text: Some(insert_text),
                        });
                    }
                }
                out.add_error_context(
                    start_line,
                    &NEXT_LINES_RE.replace(&text, ""),
                    false,
                    false,
                    range,
                    fix_info,
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
        let config = json!({ "default": false, "MD054": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md054_all_styles_allowed_reports_nothing() {
        let content = "[url](https://example.com)\n\n<https://example.com>\n\n[url][]\n\n[url]: https://example.com\n";
        assert!(lint_rule("MD054", content).is_empty());
    }

    #[test]
    fn md054_inline_disabled_reports_and_fixes_to_autolink() {
        let errs = lint_with(
            json!({ "inline": false }),
            "Text [url](https://example.com) x\n",
        );
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(
            errs[0].error_context.as_deref(),
            Some("[url](https://example.com)")
        );
        assert_eq!(errs[0].error_range, Some((6, 26)));
        let fix = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(fix.edit_column, Some(6));
        assert_eq!(fix.delete_count, Some(26));
        // inline 이 꺼져 있으니 inline 형태는 제안할 수 없고 autolink 로 바꾼다
        assert_eq!(fix.insert_text.as_deref(), Some("<https://example.com>"));
    }

    #[test]
    fn md054_autolink_disabled_fixes_to_inline() {
        let errs = lint_with(
            json!({ "autolink": false }),
            "Text <https://example.com> x\n",
        );
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_range, Some((6, 21)));
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some("[https://example.com](https://example.com)")
        );
    }

    #[test]
    fn md054_url_inline_disabled_prefers_autolink() {
        let errs = lint_with(
            json!({ "url_inline": false }),
            "Text [https://example.com](https://example.com) x\n",
        );
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some("<https://example.com>")
        );
        // 상대 경로는 절대 URL 이 아니라 autolink 로 바꿀 수 없어 보고하지 않는다
        assert!(lint_with(json!({ "url_inline": false }), "[file.md](file.md)\n").is_empty());
    }

    #[test]
    fn md054_reference_kinds_are_reported_separately() {
        let content = "[t][url] [url][] [url]\n\n[url]: https://example.com\n";
        let full = lint_with(json!({ "full": false }), content);
        assert_eq!(full.len(), 1);
        assert_eq!(full[0].error_context.as_deref(), Some("[t][url]"));
        let collapsed = lint_with(json!({ "collapsed": false }), content);
        assert_eq!(collapsed.len(), 1);
        assert_eq!(collapsed[0].error_context.as_deref(), Some("[url][]"));
        let shortcut = lint_with(json!({ "shortcut": false }), content);
        assert_eq!(shortcut.len(), 1);
        assert_eq!(shortcut[0].error_context.as_deref(), Some("[url]"));
        assert_eq!(
            shortcut[0]
                .fix_info
                .as_ref()
                .unwrap()
                .insert_text
                .as_deref(),
            Some("[url](https://example.com)")
        );
    }

    #[test]
    fn md054_multiline_link_has_no_range_or_fix() {
        let errs = lint_with(
            json!({ "inline": false }),
            "Text [url](https://example.com\n\"title\") x\n",
        );
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(
            errs[0].error_context.as_deref(),
            Some("[url](https://example.com")
        );
        assert_eq!(errs[0].error_range, None);
        assert!(errs[0].fix_info.is_none());
    }
}
