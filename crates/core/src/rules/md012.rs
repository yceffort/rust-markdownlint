use std::collections::HashSet;

use super::{LintContext, Rule, RuleMeta, add_range_to_set};
use crate::config::truthy;
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md012;

static META: RuleMeta = RuleMeta {
    names: &["MD012", "no-multiple-blanks"],
    description: "Multiple consecutive blank lines",
    tags: &["whitespace", "blank_lines"],
    needs_tokens: true,
    fixable: true,
};

impl Rule for Md012 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        // 원본: `Number(params.config.maximum || 1)`
        let maximum = ctx
            .config
            .get("maximum")
            .filter(|v| truthy(v))
            .and_then(|v| v.as_i64())
            .unwrap_or(1);

        let tokens = ctx.tokens;
        let mut code_block_line_numbers = HashSet::new();
        for id in tokens.filter_by_types(&["codeFenced", "codeIndented"]) {
            let code_block = tokens.get(id);
            add_range_to_set(
                &mut code_block_line_numbers,
                code_block.start_line,
                code_block.end_line,
            );
        }

        let mut count: i64 = 0;
        for (line_index, line) in ctx.lines.iter().enumerate() {
            let line_number = line_index + 1;
            let in_code = code_block_line_numbers.contains(&line_number);
            count = if in_code || !line.trim().is_empty() {
                0
            } else {
                count + 1
            };
            if maximum < count {
                out.add_error_detail_if(
                    line_number,
                    maximum,
                    count,
                    None,
                    None,
                    None,
                    Some(FixInfo {
                        delete_count: Some(-1),
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
        let config = json!({ "default": false, "MD012": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md012_no_blanks_ok() {
        assert!(lint_rule("MD012", "a\n\nb\n").is_empty());
    }

    #[test]
    fn md012_two_blanks_flags_second() {
        let errs = lint_rule("MD012", "a\n\n\nb\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 1; Actual: 2")
        );
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(f.delete_count, Some(-1));
    }

    #[test]
    fn md012_ignores_blanks_in_fenced_code() {
        let content = "a\n\n```\nx\n\n\n\ny\n```\n\nb\n";
        assert!(lint_rule("MD012", content).is_empty());
    }

    #[test]
    fn md012_maximum_param() {
        let content = "a\n\n\n\nb\n";
        assert_eq!(lint_rule("MD012", content).len(), 2);
        assert!(lint_with(json!({ "maximum": 3 }), content).is_empty());
    }

    #[test]
    fn md012_maximum_zero_falls_back_to_default() {
        // 원본: `params.config.maximum || 1` — 0 은 falsy 라 기본값 1 로 대체된다.
        let content = "a\n\n\nb\n";
        let errs = lint_with(json!({ "maximum": 0 }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 1; Actual: 2")
        );
    }
}
