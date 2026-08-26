use super::{LintContext, Rule, RuleMeta};
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md023;

static META: RuleMeta = RuleMeta {
    names: &["MD023", "heading-start-left"],
    description: "Headings must start at the beginning of the line",
    tags: &["headings", "spaces"],
    needs_tokens: true,
    fixable: true,
};

impl Rule for Md023 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let headings = ctx
            .tokens
            .filter_by_types(&["atxHeading", "linePrefix", "setextHeading"]);
        for i in 0..headings.len().saturating_sub(1) {
            let current = ctx.tokens.get(headings[i]);
            let next = ctx.tokens.get(headings[i + 1]);
            if current.kind == "linePrefix"
                && next.kind != "linePrefix"
                && current.start_line == next.start_line
            {
                let start_line = current.start_line;
                let start_column = current.start_column;
                let length = current.end_column - current.start_column;
                out.add_error_context(
                    start_line,
                    ctx.lines[start_line - 1],
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
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::lint_rule;

    #[test]
    fn md023_atx_heading_indented() {
        let errs = lint_rule("MD023", " # Heading\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_context.as_deref(), Some(" # Heading"));
        assert_eq!(errs[0].error_range, Some((1, 1)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!((f.edit_column, f.delete_count), (Some(1), Some(1)));
    }

    #[test]
    fn md023_setext_heading_indented() {
        let errs = lint_rule("MD023", "  Heading\n  =======\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_range, Some((1, 2)));
    }

    #[test]
    fn md023_no_indent_no_error() {
        assert!(lint_rule("MD023", "# Heading\n").is_empty());
    }

    #[test]
    fn md023_multiple_indent_widths() {
        let errs = lint_rule("MD023", "  # One\n\nSome text\n\n   # Two\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].error_range, Some((1, 2)));
        assert_eq!(errs[1].error_range, Some((1, 3)));
    }

    #[test]
    fn md023_indented_paragraph_not_flagged() {
        assert!(lint_rule("MD023", "  Just a paragraph.\n").is_empty());
    }
}
