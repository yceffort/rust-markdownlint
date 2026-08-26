use super::{LintContext, Rule, RuleMeta};
use crate::error::{ErrorSink, FixInfo};
use crate::parser::TokenId;

pub(crate) struct Md019;

static META: RuleMeta = RuleMeta {
    names: &["MD019", "no-multiple-space-atx"],
    description: "Multiple spaces after hash on atx style heading",
    tags: &["headings", "atx", "spaces"],
    needs_tokens: true,
    fixable: true,
};

/// 원본 `validateHeadingSpaces`: heading 의 시작(`delta > 0`) 또는 끝(`delta < 0`) 쪽
/// hash 시퀀스에 붙은 공백 길이를 검사한다. MD019 와 MD021 이 공유한다.
pub(super) fn validate_heading_spaces(
    ctx: &LintContext,
    out: &mut ErrorSink,
    heading: TokenId,
    delta: isize,
) {
    let tokens = ctx.tokens;
    let heading = tokens.get(heading);
    let children = &heading.children;
    let child_at = |index: isize| {
        usize::try_from(index)
            .ok()
            .and_then(|index| children.get(index))
            .map(|&id| tokens.get(id))
    };
    let mut index: isize = if delta > 0 {
        0
    } else {
        children.len() as isize - 1
    };
    while child_at(index).is_some_and(|child| child.kind != "atxHeadingSequence") {
        index += delta;
    }
    let heading_sequence = child_at(index);
    let whitespace = child_at(index + delta);
    if heading_sequence.is_some_and(|token| token.kind == "atxHeadingSequence")
        && let Some(whitespace) = whitespace
        && whitespace.kind == "whitespace"
        && whitespace.text.chars().count() > 1
    {
        let column = whitespace.start_column + 1;
        let length = whitespace.end_column - column;
        out.add_error_context(
            heading.start_line,
            heading.text.trim(),
            delta > 0,
            delta < 0,
            Some((column, length)),
            Some(FixInfo {
                edit_column: Some(column),
                delete_count: Some(length as isize),
                ..Default::default()
            }),
        );
    }
}

impl Rule for Md019 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let atx_headings = ctx
            .tokens
            .filter_by_types(&["atxHeading"])
            .into_iter()
            .filter(|&heading| ctx.tokens.heading_style(heading) == "atx");
        for atx_heading in atx_headings {
            validate_heading_spaces(ctx, out, atx_heading, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::lint_rule;

    #[test]
    fn md019_multiple_spaces() {
        let errs = lint_rule("MD019", "##   Heading\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("##   Heading"));
        assert_eq!(errs[0].error_range, Some((4, 2)));
        let fix = errs[0].fix_info.as_ref().unwrap();
        assert_eq!((fix.edit_column, fix.delete_count), (Some(4), Some(2)));
    }

    #[test]
    fn md019_single_space_is_ok() {
        assert!(lint_rule("MD019", "## Heading\n").is_empty());
    }

    #[test]
    fn md019_ignores_closed_atx_and_setext() {
        assert!(lint_rule("MD019", "##  Heading  ##\n").is_empty());
        assert!(lint_rule("MD019", "Heading\n=======\n").is_empty());
    }

    #[test]
    fn md019_inside_block_quote() {
        let errs = lint_rule("MD019", "> #  Heading\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("#  Heading"));
        assert_eq!(errs[0].error_range, Some((5, 1)));
    }

    #[test]
    fn md019_fixture() {
        let content = include_str!("../../tests/fixtures/rules/atx_heading_spacing.md");
        let errs = lint_rule("MD019", content);
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(
            errs[0].error_context.as_deref(),
            Some("##  Heading 2 {MD019}")
        );
    }
}
