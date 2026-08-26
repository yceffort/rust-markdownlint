use super::md019::validate_heading_spaces;
use super::{LintContext, Rule, RuleMeta};
use crate::error::ErrorSink;

pub(crate) struct Md021;

static META: RuleMeta = RuleMeta {
    names: &["MD021", "no-multiple-space-closed-atx"],
    description: "Multiple spaces inside hashes on closed atx style heading",
    tags: &["headings", "atx_closed", "spaces"],
    needs_tokens: true,
    fixable: true,
};

impl Rule for Md021 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let atx_closed_headings = ctx
            .tokens
            .filter_by_types(&["atxHeading"])
            .into_iter()
            .filter(|&heading| ctx.tokens.heading_style(heading) == "atx_closed");
        for atx_closed_heading in atx_closed_headings {
            validate_heading_spaces(ctx, out, atx_closed_heading, 1);
            validate_heading_spaces(ctx, out, atx_closed_heading, -1);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::lint_rule;

    #[test]
    fn md021_multiple_spaces_after_hashes() {
        let errs = lint_rule("MD021", "##  Heading ##\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("##  Heading ##"));
        assert_eq!(errs[0].error_range, Some((4, 1)));
        let fix = errs[0].fix_info.as_ref().unwrap();
        assert_eq!((fix.edit_column, fix.delete_count), (Some(4), Some(1)));
    }

    #[test]
    fn md021_multiple_spaces_before_hashes() {
        let errs = lint_rule("MD021", "## Heading  ##\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_range, Some((12, 1)));
    }

    #[test]
    fn md021_both_sides() {
        let errs = lint_rule("MD021", "##  Heading  ##\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].error_range, Some((4, 1)));
        assert_eq!(errs[1].error_range, Some((13, 1)));
    }

    #[test]
    fn md021_single_spaces_are_ok() {
        assert!(lint_rule("MD021", "## Heading ##\n").is_empty());
    }

    #[test]
    fn md021_ignores_open_atx() {
        assert!(lint_rule("MD021", "##  Heading\n").is_empty());
    }
}
