use std::sync::LazyLock;

use regex::Regex;

use super::{LintContext, Rule, RuleMeta};
use crate::error::{ErrorSink, FixInfo};
use crate::parser::{JS_WHITESPACE, is_js_whitespace};

pub(crate) struct Md038;

static META: RuleMeta = RuleMeta {
    names: &["MD038", "no-space-in-code"],
    description: "Spaces inside code span elements",
    tags: &["whitespace", "code"],
    needs_tokens: true,
    fixable: true,
};

/// 원본 `/^(\s+)(\S)/`: 코드 스팬 시작의 공백과 그 뒤 첫 글자.
static START_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("^([{JS_WHITESPACE}]+)([^{JS_WHITESPACE}])")).expect("md038 start regex")
});

/// 원본 `/(\S)(\s+)$/`: 코드 스팬 끝의 마지막 글자와 그 뒤 공백.
static END_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("([^{JS_WHITESPACE}])([{JS_WHITESPACE}]+)$")).expect("md038 end regex")
});

/// 매치 그룹 텍스트. 매치가 없거나 그룹이 비었으면 빈 문자열 (JS `match?.[i] ?? ""`).
fn group<'h>(captures: &Option<regex::Captures<'h>>, i: usize) -> &'h str {
    captures
        .as_ref()
        .and_then(|c| c.get(i))
        .map_or("", |m| m.as_str())
}

impl Rule for Md038 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        // 원본 `filterByTypesCached([ "codeText" ])`.
        for code_text in ctx.tokens.filter_by_types(&["codeText"]) {
            let datas = ctx
                .tokens
                .descendants_by_type(code_text, &[&["codeTextData"]]);
            if datas.is_empty() {
                continue;
            }
            let paddings = ctx
                .tokens
                .descendants_by_type(code_text, &[&["codeTextPadding"]]);
            // 코드 시작의 여분 공백 확인
            let start_padding = paddings.first().map(|&id| ctx.tokens.get(id));
            let start_data = ctx.tokens.get(datas[0]);
            // 첫 글자가 공백이 아니면 정규식은 매치하지 않는다
            let start_text = ctx.tokens.text(datas[0]);
            let start_match = start_text
                .starts_with(is_js_whitespace)
                .then(|| START_RE.captures(start_text))
                .flatten();
            let (start_ws, start_first) = (group(&start_match, 1), group(&start_match, 2));
            let start_backtick = start_first == "`";
            let start_count = start_ws.chars().count() as isize
                - isize::from(start_backtick && start_padding.is_none());
            let start_spaces = start_count > 0;
            // 코드 끝의 여분 공백 확인
            let end_padding = paddings.last().map(|&id| ctx.tokens.get(id));
            let end_data = ctx.tokens.get(datas[datas.len() - 1]);
            let end_text = ctx.tokens.text(datas[datas.len() - 1]);
            let end_match = end_text
                .ends_with(is_js_whitespace)
                .then(|| END_RE.captures(end_text))
                .flatten();
            let (end_last, end_ws) = (group(&end_match, 1), group(&end_match, 2));
            let end_backtick = end_last == "`";
            let end_count = end_ws.chars().count() as isize
                - isize::from(end_backtick && end_padding.is_none());
            let end_spaces = end_count > 0;
            // 1칸 padding 을 지워도 안전한지 확인
            let remove_padding = start_spaces
                && end_spaces
                && start_padding.is_some()
                && end_padding.is_some()
                && !start_backtick
                && !end_backtick;
            let context = ctx.tokens.text(code_text);
            // 시작에 여분 공백이 있으면 위반 보고
            if start_spaces {
                let padding = start_padding.filter(|_| remove_padding);
                let start_column = padding.unwrap_or(start_data).start_column;
                let length = start_count as usize
                    + padding.map_or(0, |p| ctx.tokens.text_of(p).chars().count());
                out.add_error_context(
                    start_data.start_line,
                    context,
                    true,
                    false,
                    Some((start_column, length)),
                    Some(FixInfo {
                        edit_column: Some(start_column),
                        delete_count: Some(length as isize),
                        ..Default::default()
                    }),
                );
            }
            // 끝에 여분 공백이 있으면 위반 보고
            if end_spaces {
                let padding = end_padding.filter(|_| remove_padding);
                let end_column = padding.unwrap_or(end_data).end_column;
                let length = end_count as usize
                    + padding.map_or(0, |p| ctx.tokens.text_of(p).chars().count());
                out.add_error_context(
                    end_data.end_line,
                    context,
                    false,
                    true,
                    Some((end_column - length, length)),
                    Some(FixInfo {
                        edit_column: Some(end_column - length),
                        delete_count: Some(length as isize),
                        ..Default::default()
                    }),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::lint_rule;

    #[test]
    fn md038_space_at_start_and_end() {
        let errs = lint_rule("MD038", "text `  code  ` text\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("`  code  `"));
        // 양쪽 모두 여분 공백이면 padding 까지 함께 지운다.
        assert_eq!(errs[0].error_range, Some((7, 2)));
        assert_eq!(errs[1].error_range, Some((13, 2)));
    }

    #[test]
    fn md038_no_error_for_code_or_single_padding() {
        assert!(lint_rule("MD038", "text `code` text\n").is_empty());
        // CommonMark 가 지우는 padding 1칸은 위반이 아니다.
        assert!(lint_rule("MD038", "text ` code ` text\n").is_empty());
        assert!(lint_rule("MD038", "a `` `x`` b\n").is_empty());
    }

    #[test]
    fn md038_fix_info_deletes_the_spaces() {
        let errs = lint_rule("MD038", "a `code ` b\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_range, Some((8, 1)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!((f.edit_column, f.delete_count), (Some(8), Some(1)));
    }

    #[test]
    fn md038_backtick_content_keeps_one_space() {
        // 내용이 backtick 으로 시작/끝나고 padding 이 없으면 공백 1칸은 남긴다.
        let errs = lint_rule("MD038", "a ``  `x`` b\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_range, Some((5, 1)));
        let errs = lint_rule("MD038", "a ``x`  `` b\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_range, Some((8, 1)));
    }

    #[test]
    fn md038_padding_kept_when_only_one_side_has_spaces() {
        let errs = lint_rule("MD038", "a `` `code`  `` b\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("`` `code`  ``"));
        // padding 은 남기고 여분 공백 1칸만 지운다.
        assert_eq!(errs[0].error_range, Some((12, 1)));
    }

    #[test]
    fn md038_multiline_code_span() {
        let errs = lint_rule("MD038", "a `code\nmore ` b\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 2);
        assert_eq!(errs[0].error_range, Some((5, 1)));
    }
}
