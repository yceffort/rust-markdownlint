use std::collections::HashSet;

use super::{FileRange, LintContext, Rule, RuleMeta, has_overlap};
use crate::config::truthy;
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md010;

static META: RuleMeta = RuleMeta {
    names: &["MD010", "no-hard-tabs"],
    description: "Hard tabs",
    tags: &["whitespace", "hard_tab"],
    needs_tokens: true,
    fixable: true,
};

impl Rule for Md010 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let include_code = ctx.config.get("code_blocks").is_none_or(truthy);
        let ignore_code_languages: HashSet<String> = ctx
            .config
            .get("ignore_code_languages")
            .and_then(|v| v.as_array())
            .map(|langs| {
                langs
                    .iter()
                    .map(|l| {
                        l.as_str()
                            .map_or_else(|| l.to_string(), str::to_string)
                            .to_lowercase()
                    })
                    .collect()
            })
            .unwrap_or_default();
        // 원본: `Math.max(0, Number(spacesPerTab))`, 숫자가 아니면 NaN 이라 padEnd 가 0 으로 처리
        let space_multiplier = ctx
            .config
            .get("spaces_per_tab")
            .map_or(1.0, |v| v.as_f64().map_or(0.0, |n| n.max(0.0)));

        let mut exclusion_types: Vec<&str> = Vec::new();
        if include_code {
            if !ignore_code_languages.is_empty() {
                exclusion_types.push("codeFenced");
            }
        } else {
            exclusion_types.extend(["codeFenced", "codeIndented", "codeText"]);
        }
        let tokens = ctx.tokens;
        let code_ranges: Vec<FileRange> = tokens
            .filter_by_types(&exclusion_types)
            .into_iter()
            .filter(|&id| {
                let token = tokens.get(id);
                if token.kind == "codeFenced" && !ignore_code_languages.is_empty() {
                    return tokens
                        .descendants_by_type(id, &[&["codeFencedFence"], &["codeFencedFenceInfo"]])
                        .iter()
                        .all(|&info| {
                            ignore_code_languages.contains(&tokens.text(info).to_lowercase())
                        });
                }
                true
            })
            .map(|id| {
                let token = tokens.get(id);
                let code_fenced = token.kind == "codeFenced";
                FileRange {
                    start_line: token.start_line + usize::from(code_fenced),
                    start_column: if code_fenced { 0 } else { token.start_column },
                    end_line: token.end_line - usize::from(code_fenced),
                    end_column: if code_fenced {
                        usize::MAX
                    } else {
                        token.end_column
                    },
                }
            })
            .collect();

        for (line_index, line) in ctx.lines.iter().enumerate() {
            let line_number = line_index + 1;
            if !line.contains('\t') {
                continue;
            }
            let mut chars = line.chars().enumerate().peekable();
            while let Some((index, c)) = chars.next() {
                if c != '\t' {
                    continue;
                }
                let mut length = 1;
                while chars.next_if(|&(_, c)| c == '\t').is_some() {
                    length += 1;
                }
                let column = index + 1;
                let range = FileRange {
                    start_line: line_number,
                    start_column: column,
                    end_line: line_number,
                    end_column: column + length - 1,
                };
                if !code_ranges.iter().any(|r| has_overlap(r, &range)) {
                    let width = (length as f64 * space_multiplier).floor() as usize;
                    out.add_error(
                        line_number,
                        Some(&format!("Column: {column}")),
                        None,
                        Some((column, length)),
                        Some(FixInfo {
                            edit_column: Some(column),
                            delete_count: Some(length as isize),
                            insert_text: Some(" ".repeat(width)),
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
    use crate::lint::{LintOptions, lint_content};
    use crate::rules::lint_rule;
    use serde_json::json;

    fn lint_with(params: serde_json::Value, content: &str) -> Vec<crate::error::LintError> {
        let config = json!({ "default": false, "MD010": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md010_tab_run() {
        let errs = lint_rule("MD010", "a\t\tb\tc\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].error_detail.as_deref(), Some("Column: 2"));
        assert_eq!(errs[0].error_range, Some((2, 2)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(f.edit_column, Some(2));
        assert_eq!(f.delete_count, Some(2));
        assert_eq!(f.insert_text.as_deref(), Some("  "));
        assert_eq!(errs[1].error_range, Some((5, 1)));
    }

    #[test]
    fn md010_spaces_per_tab() {
        let errs = lint_with(json!({ "spaces_per_tab": 4 }), "a\tb\n");
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some("    ")
        );
        let errs = lint_with(json!({ "spaces_per_tab": 0 }), "a\tb\n");
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some("")
        );
    }

    #[test]
    fn md010_code_blocks_false() {
        let content = "```\n\tcode\n```\n\n\tindented\n\ntext `a\tb`\n";
        assert_eq!(lint_rule("MD010", content).len(), 3);
        assert!(lint_with(json!({ "code_blocks": false }), content).is_empty());
    }

    #[test]
    fn md010_ignore_code_languages() {
        let content = "```Go\n\tcode\n```\n\n```js\n\tcode\n```\n";
        let errs = lint_with(json!({ "ignore_code_languages": ["go"] }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 6);
    }
}
