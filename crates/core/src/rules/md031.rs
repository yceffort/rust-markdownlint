use std::sync::LazyLock;

use regex::Regex;

use super::{LintContext, Rule, RuleMeta, is_blank_line};
use crate::config::truthy;
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md031;

static META: RuleMeta = RuleMeta {
    names: &["MD031", "blanks-around-fences"],
    description: "Fenced code blocks should be surrounded by blank lines",
    tags: &["code", "blank_lines"],
    needs_tokens: true,
    fixable: true,
};

/// 원본 `codeFencePrefixRe`.
static CODE_FENCE_PREFIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*?)[`~]").expect("code fence prefix regex"));

/// 원본 `prefix.replace(/[^>]/g, " ")`.
static NOT_BLOCK_QUOTE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^>]").expect("not block quote regex"));

/// JS `lines[index]`: 범위 밖이면 `undefined` 이고 `isBlankLine(undefined)` 는 참이다.
fn is_blank_line_at(lines: &[&str], index: isize) -> bool {
    if index < 0 {
        return true;
    }
    match lines.get(index as usize) {
        Some(line) => is_blank_line(line),
        None => true,
    }
}

/// 원본 `addError`: 코드 펜스의 위/아래에 에러를 추가한다.
fn add_error(out: &mut ErrorSink, lines: &[&str], line_number: usize, top: bool) {
    let line = lines[line_number - 1];
    let prefix = CODE_FENCE_PREFIX_RE
        .captures(line)
        .map(|caps| caps[1].to_string());
    let fix_info = prefix.map(|prefix| FixInfo {
        line_number: Some(line_number + usize::from(!top)),
        insert_text: Some(format!(
            "{}\n",
            NOT_BLOCK_QUOTE_RE.replace_all(&prefix, " ").trim()
        )),
        ..Default::default()
    });
    out.add_error_context(line_number, line.trim(), false, false, None, fix_info);
}

impl Rule for Md031 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let list_items = ctx.config.get("list_items");
        let include_list_items = list_items.is_none_or(truthy);
        let lines = ctx.lines;
        for id in ctx.tokens.filter_by_types(&["codeFenced"]) {
            let code_block = ctx.tokens.get(id);
            if include_list_items
                || ctx
                    .tokens
                    .parent_of_type(id, &["listOrdered", "listUnordered"])
                    .is_none()
            {
                if !is_blank_line_at(lines, code_block.start_line as isize - 2) {
                    add_error(out, lines, code_block.start_line, true);
                }
                if !is_blank_line_at(lines, code_block.end_line as isize)
                    && !is_blank_line_at(lines, code_block.end_line as isize - 1)
                {
                    add_error(out, lines, code_block.end_line, false);
                }
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
        let config = json!({ "default": false, "MD031": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md031_blank_lines_around_fence_pass() {
        assert!(lint_rule("MD031", "Text\n\n```\ncode\n```\n\nText\n").is_empty());
    }

    #[test]
    fn md031_missing_blanks_reported_with_fix() {
        let errs = lint_rule("MD031", "Text\n```\ncode\n```\nText\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line_number, 2);
        assert_eq!(errs[0].error_context.as_deref(), Some("```"));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(f.line_number, Some(2));
        assert_eq!(f.insert_text.as_deref(), Some("\n"));
        assert_eq!(errs[1].line_number, 4);
        let f = errs[1].fix_info.as_ref().unwrap();
        assert_eq!(f.line_number, Some(5));
        assert_eq!(f.insert_text.as_deref(), Some("\n"));
    }

    #[test]
    fn md031_block_quote_prefix_kept_in_fix() {
        let errs = lint_rule("MD031", "> Text\n> ```\n> code\n> ```\n> Text\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].error_context.as_deref(), Some("> ```"));
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some(">\n")
        );
    }

    #[test]
    fn md031_document_edges_are_blank() {
        assert!(lint_rule("MD031", "```\ncode\n```\n").is_empty());
    }

    #[test]
    fn md031_list_items_option() {
        let content = "1. Item\n\n   ```\n   code\n   ```\n   Text\n";
        assert_eq!(lint_rule("MD031", content).len(), 1);
        assert!(lint_with(json!({ "list_items": false }), content).is_empty());
    }
}
