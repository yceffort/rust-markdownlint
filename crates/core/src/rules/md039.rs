use std::sync::LazyLock;

use regex::Regex;

use super::{LintContext, Rule, RuleMeta};
use crate::error::{ErrorSink, FixInfo};
use crate::parser::{JS_WHITESPACE, Token};

pub(crate) struct Md039;

static META: RuleMeta = RuleMeta {
    names: &["MD039", "no-space-in-links"],
    description: "Spaces inside link text",
    tags: &["whitespace", "links"],
    needs_tokens: true,
    fixable: true,
};

/// JS 의 `[^\S\r\n]`: `\s` 문자 집합에서 `\r` 과 `\n` 을 뺀 것.
static WHITESPACE_NO_NEWLINE: LazyLock<String> =
    LazyLock::new(|| JS_WHITESPACE.replace(r"\n", "").replace(r"\r", ""));

/// 원본 `/^[^\S\r\n]+/`: 링크 텍스트 시작의 줄바꿈 아닌 공백.
static START_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("^[{}]+", *WHITESPACE_NO_NEWLINE)).expect("md039 start regex")
});

/// 원본 `/[^\S\r\n]+$/`: 링크 텍스트 끝의 줄바꿈 아닌 공백.
static END_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("[{}]+$", *WHITESPACE_NO_NEWLINE)).expect("md039 end regex")
});

/// 원본 `text.trimStart().length !== text.length` 판정 (JS 공백 집합 기준).
static TRIM_START_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("^[{JS_WHITESPACE}]")).expect("md039 trim start regex"));

/// 원본 `text.trimEnd().length !== text.length` 판정 (JS 공백 집합 기준).
static TRIM_END_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("[{JS_WHITESPACE}]$")).expect("md039 trim end regex"));

/// 원본 `label.text.replace(/\s+/g, " ")` 의 `\s+`.
static WHITESPACE_RUN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("[{JS_WHITESPACE}]+")).expect("md039 whitespace run regex")
});

/// 원본 `addLabelSpaceError`: 링크 텍스트 공백 위반을 보고한다.
fn add_label_space_error(out: &mut ErrorSink, label: &Token, label_text: &Token, is_start: bool) {
    let re = if is_start { &START_RE } else { &END_RE };
    let matched = re
        .find(&label_text.text)
        .map(|m| m.as_str().chars().count());
    let range = matched.map(|length| {
        (
            if is_start {
                label_text.start_column
            } else {
                label_text.end_column - length
            },
            length,
        )
    });
    let line = if is_start {
        label_text.start_line + usize::from(matched.is_none())
    } else {
        label_text.end_line - usize::from(matched.is_none())
    };
    out.add_error_context(
        line,
        &WHITESPACE_RUN_RE.replace_all(&label.text, " "),
        is_start,
        !is_start,
        range,
        range.map(|(edit_column, delete_count)| FixInfo {
            edit_column: Some(edit_column),
            delete_count: Some(delete_count as isize),
            ..Default::default()
        }),
    );
}

impl Rule for Md039 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        // 원본 `filterByTypesCached([ "label" ]).filter((label) => label.parent?.type === "link")`.
        let labels = ctx
            .tokens
            .filter_by_types(&["label"])
            .into_iter()
            .filter(|&id| {
                ctx.tokens
                    .get(id)
                    .parent
                    .is_some_and(|p| ctx.tokens.get(p).kind == "link")
            });
        for label_id in labels {
            let label = ctx.tokens.get(label_id);
            let label_texts = label
                .children
                .iter()
                .map(|&c| ctx.tokens.get(c))
                .filter(|child| child.kind == "labelText");
            for label_text in label_texts {
                if TRIM_START_RE.is_match(&label_text.text) {
                    add_label_space_error(out, label, label_text, true);
                }
                if TRIM_END_RE.is_match(&label_text.text) {
                    add_label_space_error(out, label, label_text, false);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::lint_rule;

    #[test]
    fn md039_space_at_start_and_end() {
        let errs = lint_rule("MD039", "[ link ](url)\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("[ link ]"));
        assert_eq!(errs[0].error_range, Some((2, 1)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!((f.edit_column, f.delete_count), (Some(2), Some(1)));
        assert_eq!(errs[1].error_range, Some((7, 1)));
        let f = errs[1].fix_info.as_ref().unwrap();
        assert_eq!((f.edit_column, f.delete_count), (Some(7), Some(1)));
    }

    #[test]
    fn md039_no_error_for_clean_links_and_images() {
        assert!(lint_rule("MD039", "[link](url) and [ref][r]\n\n[r]: url\n").is_empty());
        // 이미지의 label 은 부모가 `image` 라 대상이 아니다.
        assert!(lint_rule("MD039", "![ alt ](image.png)\n").is_empty());
        // 링크 정의(definition)의 label 도 대상이 아니다.
        assert!(lint_rule("MD039", "[ def ]: url\n").is_empty());
    }

    #[test]
    fn md039_multiple_spaces_reported_as_one_range() {
        let errs = lint_rule("MD039", "[link   ](url)\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_range, Some((6, 3)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!((f.edit_column, f.delete_count), (Some(6), Some(3)));
    }

    #[test]
    fn md039_newline_only_has_no_range_and_shifts_line() {
        // 시작 공백이 줄바꿈뿐이면 range 없이 다음 줄에 보고한다.
        let errs = lint_rule("MD039", "[\nlink](url)\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 2);
        assert_eq!(errs[0].error_context.as_deref(), Some("[ link]"));
        assert_eq!(errs[0].error_range, None);
        assert!(errs[0].fix_info.is_none());
        // 끝 공백이 줄바꿈뿐이면 range 없이 이전 줄에 보고한다.
        let errs = lint_rule("MD039", "[link\n](url)\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_range, None);
    }

    #[test]
    fn md039_space_around_newline() {
        // 공백 뒤에 줄바꿈이 오면 텍스트 끝은 줄바꿈이라 range 없이 이전 줄에 보고한다.
        let errs = lint_rule("MD039", "[link \n](url)\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("[link ]"));
        assert_eq!(errs[0].error_range, None);
        // 줄바꿈 뒤의 공백은 그 줄의 1열에 range 를 잡는다.
        let errs = lint_rule("MD039", "[link\n ](url)\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 2);
        assert_eq!(errs[0].error_range, Some((1, 1)));
    }

    #[test]
    fn md039_non_breaking_space_counts_as_whitespace() {
        let errs = lint_rule("MD039", "[\u{a0}link](url)\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_range, Some((2, 1)));
    }
}
