use super::{LintContext, Rule, RuleMeta};
use crate::config::truthy;
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md004;

static META: RuleMeta = RuleMeta {
    names: &["MD004", "ul-style"],
    description: "Unordered list style",
    tags: &["bullet", "ul"],
    needs_tokens: true,
    fixable: true,
};

fn marker_to_style(marker: &str) -> &'static str {
    if marker == "-" {
        "dash"
    } else if marker == "+" {
        "plus"
    } else {
        "asterisk"
    }
}

fn style_to_marker(style: &str) -> &'static str {
    if style == "dash" {
        "-"
    } else if style == "plus" {
        "+"
    } else {
        "*"
    }
}

fn different_item_style(style: &str) -> &'static str {
    if style == "dash" {
        "plus"
    } else if style == "plus" {
        "asterisk"
    } else {
        "dash"
    }
}

const VALID_STYLES: [&str; 5] = ["asterisk", "consistent", "dash", "plus", "sublist"];

impl Rule for Md004 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let style = match ctx.config.get("style").filter(|v| truthy(v)) {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => "consistent".to_string(),
        };
        let mut expected_style: String = if VALID_STYLES.contains(&style.as_str()) {
            style.clone()
        } else {
            "dash".to_string()
        };
        let mut nesting_styles: Vec<Option<&'static str>> = Vec::new();
        let tokens = ctx.tokens;
        for list_unordered in tokens.filter_by_types(&["listUnordered"]) {
            let mut nesting = 0usize;
            if style == "sublist" {
                let mut parent = list_unordered;
                while let Some(p) = tokens.parent_of_type(parent, &["listOrdered", "listUnordered"])
                {
                    nesting += 1;
                    parent = p;
                }
            }
            let list_item_markers = tokens
                .descendants_by_type(list_unordered, &[&["listItemPrefix"], &["listItemMarker"]]);
            for list_item_marker in list_item_markers {
                let marker = tokens.get(list_item_marker);
                let item_style = marker_to_style(tokens.text_of(marker));
                if style == "sublist" {
                    if nesting >= nesting_styles.len() {
                        nesting_styles.resize(nesting + 1, None);
                    }
                    if nesting_styles[nesting].is_none() {
                        let previous = nesting.checked_sub(1).and_then(|i| nesting_styles[i]);
                        nesting_styles[nesting] = Some(if previous == Some(item_style) {
                            different_item_style(item_style)
                        } else {
                            item_style
                        });
                    }
                    expected_style = nesting_styles[nesting].unwrap().to_string();
                } else if expected_style == "consistent" {
                    expected_style = item_style.to_string();
                }
                let column = marker.start_column;
                let length = marker.end_column - marker.start_column;
                out.add_error_detail_if(
                    marker.start_line,
                    &expected_style,
                    item_style,
                    None,
                    None,
                    Some((column, length)),
                    Some(FixInfo {
                        edit_column: Some(column),
                        delete_count: Some(length as isize),
                        insert_text: Some(style_to_marker(&expected_style).to_string()),
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
        let config = json!({ "default": false, "MD004": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md004_consistent_uses_first_marker() {
        assert!(lint_rule("MD004", "* a\n* b\n").is_empty());
        let errs = lint_rule("MD004", "* a\n\n- b\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: asterisk; Actual: dash")
        );
        assert_eq!(errs[0].error_range, Some((1, 1)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(f.insert_text.as_deref(), Some("*"));
    }

    #[test]
    fn md004_explicit_style() {
        assert!(lint_with(json!({ "style": "dash" }), "- a\n- b\n").is_empty());
        let errs = lint_with(json!({ "style": "plus" }), "- a\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: plus; Actual: dash")
        );
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some("+")
        );
    }

    #[test]
    fn md004_invalid_style_falls_back_to_dash() {
        let errs = lint_with(json!({ "style": "bogus" }), "* a\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: dash; Actual: asterisk")
        );
    }

    #[test]
    fn md004_sublist_alternates_by_level() {
        assert!(lint_with(json!({ "style": "sublist" }), "* a\n  + b\n* c\n  + d\n").is_empty());
        let errs = lint_with(json!({ "style": "sublist" }), "* a\n  + b\n  * c\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 3);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: plus; Actual: asterisk")
        );
    }

    #[test]
    fn md004_sublist_same_marker_as_parent_is_reassigned() {
        let errs = lint_with(json!({ "style": "sublist" }), "* a\n  * b\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: dash; Actual: asterisk")
        );
    }
}
