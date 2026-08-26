use super::{LintContext, Rule, RuleMeta};
use crate::config::truthy;
use crate::error::ErrorSink;

pub(crate) struct Md024;

static META: RuleMeta = RuleMeta {
    names: &["MD024", "no-duplicate-heading"],
    description: "Multiple headings with the same content",
    tags: &["headings"],
    needs_tokens: true,
    fixable: false,
};

impl Rule for Md024 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let siblings_only = ctx.config.get("siblings_only").is_some_and(truthy);
        // 원본 `knownContents`: 인덱스 0 은 사용하지 않고 레벨(1~6)별로 이미 본 heading 텍스트를 담는다.
        let mut known_contents: [Vec<String>; 7] = std::array::from_fn(|_| Vec::new());
        let mut last_level = 1usize;
        for heading in ctx.tokens.filter_by_types(&["atxHeading", "setextHeading"]) {
            let heading_text = ctx.tokens.heading_text(heading);
            if siblings_only {
                let new_level = ctx.tokens.heading_level(heading);
                while last_level < new_level {
                    last_level += 1;
                    known_contents[last_level] = Vec::new();
                }
                while last_level > new_level {
                    known_contents[last_level] = Vec::new();
                    last_level -= 1;
                }
            }
            let level = if siblings_only { last_level } else { 1 };
            if known_contents[level].contains(&heading_text) {
                out.add_error_context(
                    ctx.tokens.get(heading).start_line,
                    heading_text.trim(),
                    false,
                    false,
                    None,
                    None,
                );
            } else {
                known_contents[level].push(heading_text);
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
        let config = json!({ "default": false, "MD024": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md024_duplicate_heading() {
        let errs = lint_rule("MD024", "# Foo\n\n## Foo\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(errs[0].error_context.as_deref(), Some("Foo"));
    }

    #[test]
    fn md024_distinct_headings_ok() {
        assert!(lint_rule("MD024", "# Foo\n\n## Bar\n").is_empty());
    }

    #[test]
    fn md024_siblings_only_allows_different_parents() {
        let content = "# Change log\n\n## 1.0.0\n\n### Features\n\n## 2.0.0\n\n### Features\n";
        assert_eq!(lint_rule("MD024", content).len(), 1);
        assert!(lint_with(json!({ "siblings_only": true }), content).is_empty());
    }

    #[test]
    fn md024_siblings_only_still_flags_same_parent_duplicate() {
        let content = "# A\n\n## B\n\n## B\n";
        let errs = lint_with(json!({ "siblings_only": true }), content);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 5);
    }
}
