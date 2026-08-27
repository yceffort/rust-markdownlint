use super::{LintContext, Rule, RuleMeta, is_blank_line};
use crate::error::{ErrorSink, FixInfo, utf16_len};

pub(crate) struct Md047;

static META: RuleMeta = RuleMeta {
    names: &["MD047", "single-trailing-newline"],
    description: "Files should end with a single newline character",
    tags: &["blank_lines"],
    needs_tokens: false,
    fixable: true,
};

impl Rule for Md047 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let last_line_number = ctx.lines.len();
        let last_line = ctx.lines[last_line_number - 1];
        if !is_blank_line(last_line) {
            // 원본 `lastLine.length`: UTF-16 단위
            let len = utf16_len(last_line);
            out.add_error(
                last_line_number,
                None,
                None,
                Some((len, 1)),
                Some(FixInfo {
                    edit_column: Some(len + 1),
                    insert_text: Some("\n".to_string()),
                    ..Default::default()
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::lint_rule;

    #[test]
    fn md047_missing_newline() {
        let errs = lint_rule("MD047", "# a\ntext");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 2);
        assert_eq!(errs[0].error_range, Some((4, 1)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(
            (f.edit_column, f.insert_text.as_deref()),
            (Some(5), Some("\n"))
        );
    }

    #[test]
    fn md047_ok() {
        assert!(lint_rule("MD047", "# a\ntext\n").is_empty());
    }

    #[test]
    fn md047_fixture_missing_newline() {
        let content = include_str!("../../tests/fixtures/rules/fenced_code_without_blank_lines.md");
        let errs = lint_rule("MD047", content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 47);
        assert_eq!(errs[0].error_range, Some((3, 1)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(
            (f.edit_column, f.insert_text.as_deref()),
            (Some(4), Some("\n"))
        );
    }

    #[test]
    fn md047_fixture_with_newline() {
        let content = include_str!("../../tests/fixtures/rules/atx_heading_spacing.md");
        assert!(lint_rule("MD047", content).is_empty());
    }

    #[test]
    fn md047_column_counts_utf16_units() {
        // 기대값은 cli2 0.22.1 실행 결과 (원본 `lastLine.length` 는 UTF-16 단위)
        let errs = lint_rule("MD047", "# a\n🎸 end");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_range, Some((6, 1)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(
            (f.edit_column, f.insert_text.as_deref()),
            (Some(7), Some("\n"))
        );
    }
}
