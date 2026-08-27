use std::collections::HashSet;
use std::sync::LazyLock;

use regex::{Captures, Regex};

use super::{FileRange, LintContext, Rule, RuleMeta, add_range_to_set, has_overlap};
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md011;

static META: RuleMeta = RuleMeta {
    names: &["MD011", "no-reversed-links"],
    description: "Reversed link syntax",
    tags: &["links"],
    needs_tokens: true,
    fixable: true,
};

/// 원본 `reversedLinkRe` 에서 끝의 부정 룩어헤드 `(?!\()` 를 뺀 것. 룩어헤드는
/// `reversed_links` 가 직접 검사한다 (fancy_regex 백트래킹 VM 보다 훨씬 빠르다).
static REVERSED_LINK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(^|[^\\])\(([^()]+)\)\[([^\]^][^\]]*)\]").expect("reversed link regex")
});

/// 원본 `line.matchAll(reversedLinkRe)` 와 같은 매치 열. 매치 뒤가 `(` 면 룩어헤드 실패이므로
/// 백트래킹처럼 한 문자 뒤에서 다시 찾는다 (`[^()]+`, `[^\]]*` 는 끝이 유일해 같은 시작점의 다른
/// 매치는 없다).
fn reversed_links(line: &str) -> Vec<Captures<'_>> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(captures) = REVERSED_LINK_RE.captures_at(line, pos) {
        let m = captures.get(0).expect("full match");
        if line[m.end()..].starts_with('(') {
            pos = m.start() + line[m.start()..].chars().next().map_or(1, char::len_utf8);
            continue;
        }
        pos = m.end();
        out.push(captures);
    }
    out
}

impl Rule for Md011 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let tokens = ctx.tokens;
        let mut ignore_block_line_numbers: HashSet<usize> = HashSet::new();
        for id in tokens.filter_by_types(&["codeFenced", "codeIndented", "mathFlow"]) {
            let ignore_block = tokens.get(id);
            add_range_to_set(
                &mut ignore_block_line_numbers,
                ignore_block.start_line,
                ignore_block.end_line,
            );
        }
        let ignore_texts: Vec<FileRange> = tokens
            .filter_by_types(&["codeText", "mathText"])
            .into_iter()
            .map(|id| {
                let token = tokens.get(id);
                FileRange {
                    start_line: token.start_line,
                    start_column: token.start_column,
                    end_line: token.end_line,
                    end_column: token.end_column,
                }
            })
            .collect();
        for (line_index, line) in ctx.lines.iter().enumerate() {
            let line_number = line_index + 1;
            if ignore_block_line_numbers.contains(&line_number) {
                continue;
            }
            for captures in reversed_links(line) {
                let reversed_link = captures.get(0).expect("full match");
                let pre_char = captures.get(1).expect("preChar").as_str();
                let link_text = captures.get(2).expect("linkText").as_str();
                let link_destination = captures.get(3).expect("linkDestination").as_str();
                if link_text.ends_with('\\') || link_destination.ends_with('\\') {
                    continue;
                }
                let pre_char_length = pre_char.chars().count();
                let column = line[..reversed_link.start()].chars().count() + pre_char_length + 1;
                let length = reversed_link.as_str().chars().count() - pre_char_length;
                let range = FileRange {
                    start_line: line_number,
                    start_column: column,
                    end_line: line_number,
                    end_column: column + length - 1,
                };
                if ignore_texts
                    .iter()
                    .any(|ignore_text| has_overlap(ignore_text, &range))
                {
                    continue;
                }
                out.add_error(
                    line_number,
                    Some(&reversed_link.as_str()[pre_char.len()..]),
                    None,
                    Some((column, length)),
                    Some(FixInfo {
                        edit_column: Some(column),
                        delete_count: Some(length as isize),
                        insert_text: Some(format!("[{link_text}]({link_destination})")),
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

    #[test]
    fn md011_reversed_link() {
        let errs = lint_rule("MD011", "See (this website)[https://example.com] here.\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("(this website)[https://example.com]")
        );
        assert_eq!(errs[0].error_range, Some((5, 35)));
    }

    #[test]
    fn md011_fix_info() {
        let errs = lint_rule("MD011", "(reversed)[link]\n");
        assert_eq!(errs.len(), 1);
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(f.edit_column, Some(1));
        assert_eq!(f.delete_count, Some(16));
        assert_eq!(f.insert_text.as_deref(), Some("[reversed](link)"));
    }

    #[test]
    fn md011_correct_link_is_not_reported() {
        assert!(lint_rule("MD011", "A [text](link) and (parens) [ref][id].\n").is_empty());
    }

    #[test]
    fn md011_footnote_and_escaped_are_skipped() {
        // `[^...]` 는 각주라 무시하고, `\(` 로 이스케이프된 것도 무시한다.
        assert!(lint_rule("MD011", "Text (note)[^1] here.\n").is_empty());
        assert!(lint_rule("MD011", "Text \\(reversed)[link] here.\n").is_empty());
    }

    #[test]
    fn md011_ignores_code_blocks_and_code_spans() {
        let content = "```text\n(reversed)[link]\n```\n\nA `(reversed)[link]` span.\n";
        assert!(lint_rule("MD011", content).is_empty());
    }
}
