use super::{LintContext, Rule, RuleMeta};
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md029;

static META: RuleMeta = RuleMeta {
    names: &["MD029", "ol-prefix"],
    description: "Ordered list item prefix",
    tags: &["ol"],
    needs_tokens: true,
    fixable: true,
};

impl Rule for Md029 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let tokens = ctx.tokens;
        // 원본 `String(params.config.style)`. 문자열이 아닌 값은 세 스타일 중 어느 것과도
        // 같아질 수 없으므로 같은 기본 분기로 떨어진다.
        let style = ctx
            .config
            .get("style")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        // 원본 `getOrderedListItemValue`. `Number(text)` 라 선행 0 은 사라진다.
        let ordered_list_item_value = |list_item_prefix| {
            let id = tokens.descendants_by_type(list_item_prefix, &[&["listItemValue"]])[0];
            let token = tokens.get(id);
            (token.start_column, tokens.text(id).parse::<i64>().unwrap())
        };
        for list_ordered in tokens.filter_by_types(&["listOrdered"]) {
            let list_item_prefixes =
                tokens.descendants_by_type(list_ordered, &[&["listItemPrefix"]]);
            let mut expected = 1i64;
            let mut incrementing = false;
            // Check for incrementing number pattern 1/2/3 or 0/1/2
            if list_item_prefixes.len() >= 2 {
                let first = ordered_list_item_value(list_item_prefixes[0]);
                let second = ordered_list_item_value(list_item_prefixes[1]);
                if (second.1 != 1) || (first.1 == 0) {
                    incrementing = true;
                    if first.1 == 0 {
                        expected = 0;
                    }
                }
            }
            // Determine effective style
            let list_style = if matches!(style, "one" | "ordered" | "zero") {
                style
            } else if incrementing {
                "ordered"
            } else {
                "one"
            };
            if list_style == "zero" {
                expected = 0;
            } else if list_style == "one" {
                expected = 1;
            }
            // 원본 `listStyleExamples`.
            let example = match list_style {
                "one" => "1/1/1",
                "ordered" => "1/2/3",
                _ => "0/0/0",
            };
            // Validate each list item marker
            for list_item_prefix in list_item_prefixes {
                let (column, actual) = ordered_list_item_value(list_item_prefix);
                let token = tokens.get(list_item_prefix);
                out.add_error_detail_if(
                    token.start_line,
                    expected,
                    actual,
                    Some(&format!("Style: {example}")),
                    None,
                    Some((token.start_column, token.end_column - token.start_column)),
                    Some(FixInfo {
                        edit_column: Some(column),
                        delete_count: Some(actual.to_string().chars().count() as isize),
                        insert_text: Some(expected.to_string()),
                        ..Default::default()
                    }),
                );
                if list_style == "ordered" {
                    expected += 1;
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
        let config = json!({ "default": false, "MD029": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md029_one_and_ordered_are_both_valid_by_default() {
        assert!(lint_rule("MD029", "1. a\n1. b\n1. c\n").is_empty());
        assert!(lint_rule("MD029", "1. a\n2. b\n3. c\n").is_empty());
        assert!(lint_rule("MD029", "0. a\n1. b\n2. c\n").is_empty());
    }

    #[test]
    fn md029_broken_sequence_reports_with_fix() {
        let errs = lint_rule("MD029", "1. a\n3. b\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 2; Actual: 3; Style: 1/2/3")
        );
        assert_eq!(errs[0].error_range, Some((1, 3)));
        let fix = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(fix.edit_column, Some(1));
        assert_eq!(fix.delete_count, Some(1));
        assert_eq!(fix.insert_text.as_deref(), Some("2"));
    }

    #[test]
    fn md029_style_parameter() {
        let one = "1. a\n2. b\n";
        assert_eq!(lint_with(json!({ "style": "one" }), one).len(), 1);
        assert!(lint_with(json!({ "style": "ordered" }), one).is_empty());
        let zero = "0. a\n0. b\n";
        assert!(lint_with(json!({ "style": "zero" }), zero).is_empty());
        assert_eq!(lint_with(json!({ "style": "one" }), zero).len(), 2);
        // 인식하지 못하는 값은 one_or_ordered 로 동작한다
        assert!(lint_with(json!({ "style": "nope" }), one).is_empty());
    }

    #[test]
    fn md029_zero_prefixed_values_use_numeric_value() {
        // 01/02/03 은 값 기준으로 1/2/3 이라 ordered 로 통과한다
        assert!(lint_rule("MD029", "01. a\n02. b\n03. c\n").is_empty());
        // `Number(...).toString().length` 라 deleteCount 는 선행 0 을 세지 않는다
        let errs = lint_with(json!({ "style": "zero" }), "01. a\n01. b\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].fix_info.as_ref().unwrap().delete_count, Some(1));
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some("0")
        );
    }
}
