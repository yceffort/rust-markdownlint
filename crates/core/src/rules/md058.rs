use super::{LintContext, Rule, RuleMeta, is_blank_line};
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md058;

static META: RuleMeta = RuleMeta {
    names: &["MD058", "blanks-around-tables"],
    description: "Tables should be surrounded by blank lines",
    tags: &["table"],
    needs_tokens: true,
    fixable: true,
};

impl Rule for Md058 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let lines = ctx.lines;
        let tokens = ctx.tokens;
        // JS 는 범위 밖 인덱스가 undefined 라 isBlankLine 이 true 를 준다.
        let blank_at = |index: isize| -> bool {
            index < 0
                || lines
                    .get(index as usize)
                    .is_none_or(|line| is_blank_line(line))
        };
        let block_quote_prefixes = tokens.filter_by_types(&["blockQuotePrefix", "linePrefix"]);

        // For every table...
        for table in tokens.filter_by_types(&["table"]) {
            let table = tokens.get(table);

            // Look for a blank line above the table
            let first_line_number = table.start_line;
            if !blank_at(first_line_number as isize - 2) {
                out.add_error_context(
                    first_line_number,
                    lines[first_line_number - 1].trim(),
                    false,
                    false,
                    None,
                    Some(FixInfo {
                        insert_text: Some(tokens.block_quote_prefix_text(
                            &block_quote_prefixes,
                            first_line_number,
                            1,
                        )),
                        ..Default::default()
                    }),
                );
            }

            // Look for a blank line below the table
            let last_line_number = table.end_line;
            if !blank_at(last_line_number as isize) {
                out.add_error_context(
                    last_line_number,
                    lines[last_line_number - 1].trim(),
                    false,
                    false,
                    None,
                    Some(FixInfo {
                        line_number: Some(last_line_number + 1),
                        insert_text: Some(tokens.block_quote_prefix_text(
                            &block_quote_prefixes,
                            last_line_number,
                            1,
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
    use crate::rules::lint_rule;

    const TABLE: &str = "| a | b |\n| - | - |\n| 1 | 2 |";

    #[test]
    fn md058_surrounded_table_is_clean() {
        assert!(lint_rule("MD058", &format!("# H\n\n{TABLE}\n\nText\n")).is_empty());
    }

    #[test]
    fn md058_missing_blank_above_reported_with_fix() {
        let errs = lint_rule("MD058", &format!("Text\n{TABLE}\n\nText\n"));
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 2);
        assert_eq!(errs[0].error_context.as_deref(), Some("| a | b |"));
        let fix = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(fix.line_number, None);
        assert_eq!(fix.insert_text.as_deref(), Some("\n"));
    }

    #[test]
    fn md058_missing_blank_below_reported_with_fix() {
        let errs = lint_rule("MD058", &format!("Text\n\n{TABLE}\n> Quote\n"));
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 5);
        assert_eq!(errs[0].error_context.as_deref(), Some("| 1 | 2 |"));
        let fix = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(fix.line_number, Some(6));
        assert_eq!(fix.insert_text.as_deref(), Some("\n"));
    }

    #[test]
    fn md058_document_edges_are_blank() {
        assert!(lint_rule("MD058", &format!("{TABLE}\n")).is_empty());
    }

    #[test]
    fn md058_block_quote_prefix_is_inserted() {
        let content = "> Text\n> | a | b |\n> | - | - |\n> | 1 | 2 |\n> > Quote\n";
        let errs = lint_rule("MD058", content);
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line_number, 2);
        assert_eq!(errs[0].error_context.as_deref(), Some("> | a | b |"));
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some(">\n")
        );
        assert_eq!(errs[1].line_number, 4);
        assert_eq!(
            errs[1].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some(">\n")
        );
    }

    #[test]
    fn md058_html_comment_line_counts_as_blank() {
        // isBlankLine 은 주석만 있는 줄을 빈 줄로 본다.
        let content = format!("<!-- comment -->\n{TABLE}\n\nText\n");
        assert!(lint_rule("MD058", &content).is_empty());
    }
}
