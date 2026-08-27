use std::sync::LazyLock;

use regex::Regex;

use super::{LineSet, LintContext, Rule, RuleMeta, add_range_to_set};
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md020;

static META: RuleMeta = RuleMeta {
    names: &["MD020", "no-missing-space-closed-atx"],
    description: "No space inside hashes on closed atx style heading",
    tags: &["headings", "atx_closed", "spaces"],
    needs_tokens: true,
    fixable: true,
};

static CLOSED_ATX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(#+)([ \t]*)([^# \t\\]|[^# \t][^#]*?[^# \t\\])([ \t]*)((?:\\#)?)(#+)(\s*)$")
        .unwrap()
});

impl Rule for Md020 {
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
            if !line.starts_with('#') || ignore_block_line_numbers.contains(line_index + 1) {
                continue;
            }
            let Some(caps) = CLOSED_ATX_RE.captures(line) else {
                continue;
            };
            let left_hash = &caps[1];
            let left_space_length = caps[2].chars().count();
            let content = &caps[3];
            let right_space_length = caps[4].chars().count();
            let right_escape = &caps[5];
            let right_hash = &caps[6];
            let trail_space_length = caps[7].chars().count();

            let left_hash_length = left_hash.chars().count();
            let right_hash_length = right_hash.chars().count();
            let left = left_space_length == 0;
            let right = right_space_length == 0 || !right_escape.is_empty();
            let right_escape_replacement = if right_escape.is_empty() {
                String::new()
            } else {
                format!("{right_escape} ")
            };

            if left || right {
                let line_length = line.chars().count();
                let range = if left {
                    (1usize, left_hash_length + 1)
                } else {
                    (
                        line_length - trail_space_length - right_hash_length,
                        right_hash_length + 1,
                    )
                };
                out.add_error_context(
                    line_index + 1,
                    line.trim(),
                    left,
                    right,
                    Some(range),
                    Some(FixInfo {
                        edit_column: Some(1),
                        delete_count: Some(line_length as isize),
                        insert_text: Some(format!(
                            "{left_hash} {content} {right_escape_replacement}{right_hash}"
                        )),
                        ..Default::default()
                    }),
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
        let config = json!({ "default": false, "MD020": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md020_missing_left_space() {
        let errs = lint_rule("MD020", "#Heading #\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("#Heading #"));
        assert_eq!(errs[0].error_range, Some((1, 2)));
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some("# Heading #")
        );
    }

    #[test]
    fn md020_missing_right_space() {
        let errs = lint_rule("MD020", "# Heading#\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_range, Some((9, 2)));
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some("# Heading #")
        );
    }

    #[test]
    fn md020_valid_closed_atx_ok() {
        assert!(lint_rule("MD020", "# Heading #\n").is_empty());
    }

    #[test]
    fn md020_ignores_fenced_code() {
        assert!(lint_rule("MD020", "```\n#nope#\n```\n").is_empty());
    }

    #[test]
    fn md020_escaped_right_hash_flagged() {
        let errs = lint_with(json!({}), "# Heading \\##\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_range, Some((12, 2)));
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some("# Heading \\# #")
        );
    }
}
