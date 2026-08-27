use std::sync::LazyLock;

use regex::Regex;

use super::{LineSet, LintContext, Rule, RuleMeta, add_range_to_set};
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md018;

static META: RuleMeta = RuleMeta {
    names: &["MD018", "no-missing-space-atx"],
    description: "No space after hash on atx style heading",
    tags: &["headings", "atx", "spaces"],
    needs_tokens: true,
    fixable: true,
};

static HASH_PREFIX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#+[^# \t]").unwrap());
static HASH_SUFFIX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#\s*$").unwrap());

impl Rule for Md018 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let mut ignore_block_line_numbers = LineSet::default();
        for id in ctx
            .tokens
            .filter_by_types(&["codeFenced", "codeIndented", "htmlFlow"])
        {
            let token = ctx.tokens.get(id);
            add_range_to_set(
                &mut ignore_block_line_numbers,
                token.start_line,
                token.end_line,
            );
        }
        for (line_index, line) in ctx.lines.iter().enumerate() {
            if line.starts_with('#')
                && !ignore_block_line_numbers.contains(line_index + 1)
                && HASH_PREFIX_RE.is_match(line)
                && !HASH_SUFFIX_RE.is_match(line)
                && !line.starts_with("#️⃣")
            {
                let hash_count = line.chars().take_while(|&c| c == '#').count();
                out.add_error_context(
                    line_index + 1,
                    line.trim(),
                    false,
                    false,
                    Some((1, hash_count + 1)),
                    Some(FixInfo {
                        edit_column: Some(hash_count + 1),
                        insert_text: Some(" ".to_string()),
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
    fn md018_no_space() {
        let errs = lint_rule("MD018", "#Heading\n");
        assert_eq!(errs[0].error_context.as_deref(), Some("#Heading"));
        assert_eq!(errs[0].error_range, Some((1, 2)));
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some(" ")
        );
    }

    #[test]
    fn md018_ignores_fenced_code() {
        assert!(lint_rule("MD018", "```\n#nope\n```\n").is_empty());
    }

    #[test]
    fn md018_fixture() {
        let content = include_str!("../../tests/fixtures/rules/atx_heading_spacing.md");
        let errs = lint_rule("MD018", content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("#Heading 1 {MD018}"));
        assert_eq!(errs[0].error_range, Some((1, 2)));

        let fenced = include_str!("../../tests/fixtures/rules/fenced_code_without_blank_lines.md");
        assert!(lint_rule("MD018", fenced).is_empty());
    }
}
