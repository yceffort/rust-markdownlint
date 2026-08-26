use std::sync::LazyLock;

use regex::Regex;

use super::{LintContext, Rule, RuleMeta};
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md014;

static META: RuleMeta = RuleMeta {
    names: &["MD014", "commands-show-output"],
    description: "Dollar signs used before commands without showing output",
    tags: &["code"],
    needs_tokens: true,
    fixable: true,
};

/// JS 정규식의 `\s`. Rust 의 `\s` (Unicode White_Space) 와 달리 U+0085 를 빼고 U+FEFF 를 넣는다.
const JS_WHITESPACE: &str = r"[\t\n\x0B\f\r \u{a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}]";

/// 원본 `dollarCommandRe`: `/^(\s*)(\$\s+)/`.
static DOLLAR_COMMAND_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("^({JS_WHITESPACE}*)(\\${JS_WHITESPACE}+)")).expect("dollar command regex")
});

impl Rule for Md014 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        for code_block in ctx.tokens.filter_by_types(&["codeFenced", "codeIndented"]) {
            let code_flow_values: Vec<_> = ctx
                .tokens
                .get(code_block)
                .children
                .iter()
                .map(|id| ctx.tokens.get(*id))
                .filter(|child| child.kind == "codeFlowValue")
                .collect();
            // 원본 `dollarMatches`: `dollarCommandRe` 에 매치되는 것만 남긴다.
            let dollar_matches: Vec<_> = code_flow_values
                .iter()
                .filter_map(|code_flow_value| {
                    DOLLAR_COMMAND_RE
                        .captures(&code_flow_value.text)
                        .map(|result| (result, *code_flow_value))
                })
                .collect();
            // 코드 블록의 모든 줄이 `$` 로 시작할 때만 보고한다.
            if dollar_matches.len() == code_flow_values.len() {
                for (result, code_flow_value) in &dollar_matches {
                    let column = code_flow_value.start_column
                        + result.get(1).map_or(0, |m| m.as_str().chars().count());
                    let length = result.get(2).map_or(0, |m| m.as_str().chars().count());
                    out.add_error_context(
                        code_flow_value.start_line,
                        &code_flow_value.text,
                        false,
                        false,
                        Some((column, length)),
                        Some(FixInfo {
                            edit_column: Some(column),
                            delete_count: Some(length as isize),
                            ..Default::default()
                        }),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::lint_rule;

    #[test]
    fn md014_fenced_block_all_dollar_commands() {
        let errs = lint_rule("MD014", "```bash\n$ ls\n$ cat file\n```\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line_number, 2);
        assert_eq!(errs[0].error_context.as_deref(), Some("$ ls"));
        assert_eq!(errs[0].error_range, Some((1, 2)));
        assert_eq!(errs[1].line_number, 3);
    }

    #[test]
    fn md014_fix_info_deletes_dollar_and_space() {
        let errs = lint_rule("MD014", "```\n$   ls\n```\n");
        assert_eq!(errs.len(), 1);
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!((f.edit_column, f.delete_count), (Some(1), Some(4)));
    }

    #[test]
    fn md014_mixed_output_lines_are_ignored() {
        assert!(lint_rule("MD014", "```bash\n$ ls\nfile.txt\n```\n").is_empty());
    }

    #[test]
    fn md014_indented_code_block_offsets_column() {
        let errs = lint_rule("MD014", "text\n\n    $ ls\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(errs[0].error_range, Some((5, 2)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!((f.edit_column, f.delete_count), (Some(5), Some(2)));
    }

    #[test]
    fn md014_dollar_without_following_space_is_not_a_command() {
        assert!(lint_rule("MD014", "```\n$ls\n```\n").is_empty());
    }

    #[test]
    fn md014_empty_code_block_reports_nothing() {
        assert!(lint_rule("MD014", "```bash\n```\n").is_empty());
    }
}
