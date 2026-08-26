use std::collections::HashSet;

use super::{LintContext, Rule, RuleMeta};
use crate::config::truthy;
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md009;

static META: RuleMeta = RuleMeta {
    names: &["MD009", "no-trailing-spaces"],
    description: "Trailing spaces",
    tags: &["whitespace"],
    needs_tokens: true,
    fixable: true,
};

fn add_range_to_set(set: &mut HashSet<usize>, start: usize, end: usize) {
    for line in start..=end {
        set.insert(line);
    }
}

impl Rule for Md009 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let br_spaces = ctx
            .config
            .get("br_spaces")
            .and_then(|v| v.as_i64())
            .unwrap_or(2);
        let include_code = ctx.config.get("code_blocks").is_some_and(truthy);
        let list_item_empty_lines = ctx.config.get("list_item_empty_lines").is_some_and(truthy);
        let strict = ctx.config.get("strict").is_some_and(truthy);

        let tokens = ctx.tokens;
        let mut code_block_line_numbers = HashSet::new();
        if !include_code {
            for id in tokens.filter_by_types(&["codeFenced"]) {
                let t = tokens.get(id);
                add_range_to_set(
                    &mut code_block_line_numbers,
                    t.start_line + 1,
                    t.end_line - 1,
                );
            }
            for id in tokens.filter_by_types(&["codeIndented"]) {
                let t = tokens.get(id);
                add_range_to_set(&mut code_block_line_numbers, t.start_line, t.end_line);
            }
        }

        let mut list_item_line_numbers = HashSet::new();
        if list_item_empty_lines {
            for id in tokens.filter_by_types(&["listOrdered", "listUnordered"]) {
                let list_block = tokens.get(id);
                add_range_to_set(
                    &mut list_item_line_numbers,
                    list_block.start_line,
                    list_block.end_line,
                );
                let mut trailing_indent = true;
                for &child_id in list_block.children.iter().rev() {
                    let child = tokens.get(child_id);
                    match child.kind.as_str() {
                        "content" => trailing_indent = false,
                        "listItemIndent" if trailing_indent => {
                            list_item_line_numbers.remove(&child.start_line);
                        }
                        "listItemPrefix" => trailing_indent = true,
                        _ => {}
                    }
                }
            }
        }

        let mut paragraph_line_numbers = HashSet::new();
        let mut code_inline_line_numbers = HashSet::new();
        if strict {
            for id in tokens.filter_by_types(&["paragraph"]) {
                let t = tokens.get(id);
                add_range_to_set(&mut paragraph_line_numbers, t.start_line, t.end_line - 1);
            }
            for id in tokens.filter_by_types(&["codeText"]) {
                let t = tokens.get(id);
                add_range_to_set(&mut code_inline_line_numbers, t.start_line, t.end_line - 1);
            }
        }

        let expected = if br_spaces < 2 { 0 } else { br_spaces as usize };
        for (line_index, line) in ctx.lines.iter().enumerate() {
            let line_number = line_index + 1;
            let line_len = line.chars().count();
            let trailing_spaces = line_len - line.trim_end().chars().count();
            if trailing_spaces > 0
                && !code_block_line_numbers.contains(&line_number)
                && !list_item_line_numbers.contains(&line_number)
                && (expected != trailing_spaces
                    || (strict
                        && (!paragraph_line_numbers.contains(&line_number)
                            || code_inline_line_numbers.contains(&line_number))))
            {
                let column = line_len - trailing_spaces + 1;
                let detail = format!(
                    "Expected: {}{expected}; Actual: {trailing_spaces}",
                    if expected == 0 { "" } else { "0 or " }
                );
                out.add_error(
                    line_number,
                    Some(&detail),
                    None,
                    Some((column, trailing_spaces)),
                    Some(FixInfo {
                        edit_column: Some(column),
                        delete_count: Some(trailing_spaces as isize),
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
        let config = json!({ "default": false, "MD009": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md009_single_trailing_space() {
        let errs = lint_rule("MD009", "text \n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 0 or 2; Actual: 1")
        );
        assert_eq!(errs[0].error_range, Some((5, 1)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!((f.edit_column, f.delete_count), (Some(5), Some(1)));
    }

    #[test]
    fn md009_br_spaces_allows_hard_break() {
        assert!(lint_rule("MD009", "text  \nmore\n").is_empty());
        let errs = lint_with(json!({ "br_spaces": 0 }), "text  \nmore\n");
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 0; Actual: 2")
        );
    }

    #[test]
    fn md009_code_blocks() {
        let content = "```\ncode \n```\n";
        assert!(lint_rule("MD009", content).is_empty());
        let errs = lint_with(json!({ "code_blocks": true }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 2);
    }

    #[test]
    fn md009_list_item_empty_lines() {
        let content = "- item\n   \n  more\n";
        let errs = lint_rule("MD009", content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_range, Some((1, 3)));
        assert!(lint_with(json!({ "list_item_empty_lines": true }), content).is_empty());
    }

    #[test]
    fn md009_strict() {
        let content = "text  \nmore\n\n# Heading  \n";
        assert_eq!(lint_rule("MD009", content).len(), 0);
        let errs = lint_with(json!({ "strict": true }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 4);
        assert_eq!(errs[0].error_range, Some((10, 2)));
    }
}
