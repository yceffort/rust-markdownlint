use std::sync::LazyLock;

use regex::Regex;

use super::{LintContext, Rule, RuleMeta};
use crate::error::{ErrorSink, FixInfo};
use crate::parser::JS_WHITESPACE;

pub(crate) struct Md037;

static META: RuleMeta = RuleMeta {
    names: &["MD037", "no-space-in-emphasis"],
    description: "Spaces inside emphasis markers",
    tags: &["whitespace", "emphasis"],
    needs_tokens: true,
    fixable: true,
};

/// 원본의 emphasis 마커 종류 (`emphasisTokensByMarker` 의 키, 삽입 순서).
const MARKERS: [&str; 6] = ["_", "__", "___", "*", "**", "***"];

/// micromark 는 짝이 없는 `*`/`_` 시퀀스를 독립된 `data` 토큰으로 남기지만, markdown-rs 는
/// 인접한 data 를 하나로 합친다. 그래서 합쳐진 data 텍스트 안의 `*`/`_` 연속을 원본의 bare
/// 토큰으로 되살린다. 단 emphasis/strong/link label 안쪽은 micromark 도 인접 data 를 다시
/// 합치므로 (`insideSpan` resolver) 짝이 남지 않는다. 텍스트 컨텐츠의 최상위 컨테이너에서만
/// 되살린다.
const TEXT_PARENTS: [&str; 4] = [
    "paragraph",
    "atxHeadingText",
    "setextHeadingText",
    "tableContent",
];

/// data 텍스트 안의 `*` 또는 `_` 연속.
static MARKER_RUN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\*+|_+").expect("md037 marker run regex"));

/// 원본 `/^\s+\S/`: 시작 마커 뒤의 공백과 그 뒤 첫 글자.
static START_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("^[{JS_WHITESPACE}]+[^{JS_WHITESPACE}]")).expect("md037 start regex")
});

/// 원본 `/\S\s+$/`: 끝 마커 앞의 마지막 글자와 그 뒤 공백.
static END_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("[^{JS_WHITESPACE}][{JS_WHITESPACE}]+$")).expect("md037 end regex")
});

/// 원본의 bare 마커 data 토큰에 해당하는 위치 (startLine, startColumn, endColumn).
/// 컬럼은 토큰과 같은 1 기반 UTF-16 단위.
struct BareToken {
    start_line: usize,
    start_column: usize,
    end_column: usize,
}

/// JS `str.slice(from, to)` 를 UTF-16 단위로 흉내 낸다 (범위는 길이로 잘린다).
fn slice_utf16(text: &str, from: usize, to: Option<usize>) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let from = from.min(units.len());
    let to = to.map_or(units.len(), |t| t.clamp(from, units.len()));
    String::from_utf16_lossy(&units[from..to])
}

impl Rule for Md037 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        // Initialize variables
        let lines = ctx.lines;
        let mut emphasis_tokens_by_marker: [Vec<BareToken>; 6] = Default::default();
        // 원본은 전체 트리에서 data 자식이 있는 토큰을 모으지만, 아래에서 TEXT_PARENTS 만 쓰므로
        // 종류 인덱스로 바로 찾는다 (htmlFlow 안쪽 토큰은 자식의 in_html_flow 검사로 걸러진다).
        let tokens = ctx.tokens.filter_by_types_html_flow(&TEXT_PARENTS, true);
        for token in tokens {
            // Build lists of bare tokens for each emphasis marker type
            for emphasis_tokens in emphasis_tokens_by_marker.iter_mut() {
                emphasis_tokens.clear();
            }
            for &child in &ctx.tokens.get(token).children {
                let child_token = ctx.tokens.get(child);
                if child_token.kind != "data" || child_token.in_html_flow {
                    continue;
                }
                // 합쳐진 data 안의 마커 연속을 원본의 bare 토큰으로 되살린다
                let text = ctx.tokens.text(child);
                let (mut byte_pos, mut utf16_pos) = (0, 0);
                for run in MARKER_RUN_RE.find_iter(text) {
                    utf16_pos += text[byte_pos..run.start()].encode_utf16().count();
                    byte_pos = run.start();
                    let Some(index) = MARKERS.iter().position(|&m| m == run.as_str()) else {
                        continue;
                    };
                    emphasis_tokens_by_marker[index].push(BareToken {
                        start_line: child_token.start_line,
                        start_column: child_token.start_column + utf16_pos,
                        end_column: child_token.start_column + utf16_pos + run.len(),
                    });
                }
            }

            // Process bare tokens for each emphasis marker type
            for (marker, emphasis_tokens) in MARKERS.iter().zip(&emphasis_tokens_by_marker) {
                let mut i = 0;
                while i + 1 < emphasis_tokens.len() {
                    // Process start token of start/end pair
                    let start_token = &emphasis_tokens[i];
                    let start_line = lines[start_token.start_line - 1];
                    let start_slice = slice_utf16(start_line, start_token.end_column - 1, None);
                    if let Some(start_match) = START_RE.find(&start_slice) {
                        let start_space_character = start_match.as_str();
                        let start_context = format!("{marker}{start_space_character}");
                        let column = start_token.end_column;
                        let count = start_space_character.chars().count() - 1;
                        out.add_error(
                            start_token.start_line,
                            None,
                            Some(&start_context),
                            Some((column, count)),
                            Some(FixInfo {
                                edit_column: Some(column),
                                delete_count: Some(count as isize),
                                ..Default::default()
                            }),
                        );
                    }

                    // Process end token of start/end pair
                    let end_token = &emphasis_tokens[i + 1];
                    let end_line = lines[end_token.start_line - 1];
                    let end_slice = slice_utf16(end_line, 0, Some(end_token.start_column - 1));
                    if let Some(end_match) = END_RE.find(&end_slice) {
                        let end_space_character = end_match.as_str();
                        let end_context = format!("{end_space_character}{marker}");
                        let count = end_space_character.chars().count() - 1;
                        let column = end_token.start_column - count;
                        out.add_error(
                            end_token.start_line,
                            None,
                            Some(&end_context),
                            Some((column, count)),
                            Some(FixInfo {
                                edit_column: Some(column),
                                delete_count: Some(count as isize),
                                ..Default::default()
                            }),
                        );
                    }
                    i += 2;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::lint_rule;

    #[test]
    fn md037_space_after_start_and_before_end() {
        let errs = lint_rule("MD037", "text * emphasis * text\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("* e"));
        assert_eq!(errs[0].error_range, Some((7, 1)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!((f.edit_column, f.delete_count), (Some(7), Some(1)));
        assert_eq!(errs[1].error_context.as_deref(), Some("s *"));
        assert_eq!(errs[1].error_range, Some((16, 1)));
    }

    #[test]
    fn md037_no_error_for_valid_emphasis() {
        assert!(lint_rule("MD037", "text *emphasis* and __strong__ text\n").is_empty());
        // 짝이 없는 마커와 4개 이상의 연속은 무시한다.
        assert!(lint_rule("MD037", "text * alone in a sentence\n").is_empty());
        assert!(lint_rule("MD037", "text **** a **** b\n").is_empty());
        assert!(lint_rule("MD037", "snake_case_name and other_name\n").is_empty());
    }

    #[test]
    fn md037_strong_and_underscore_markers() {
        let errs = lint_rule("MD037", "a ** strong ** b\n\nc __ strong __ d\n");
        assert_eq!(errs.len(), 4);
        assert_eq!(errs[0].error_context.as_deref(), Some("** s"));
        assert_eq!(errs[0].error_range, Some((5, 1)));
        assert_eq!(errs[1].error_context.as_deref(), Some("g **"));
        assert_eq!(errs[1].error_range, Some((12, 1)));
        assert_eq!(errs[2].line_number, 3);
        assert_eq!(errs[2].error_context.as_deref(), Some("__ s"));
        assert_eq!(errs[3].error_context.as_deref(), Some("g __"));
    }

    #[test]
    fn md037_multiple_spaces_are_all_deleted() {
        let errs = lint_rule("MD037", "text *   emphasis* text\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_context.as_deref(), Some("*   e"));
        assert_eq!(errs[0].error_range, Some((7, 3)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!((f.edit_column, f.delete_count), (Some(7), Some(3)));
    }

    #[test]
    fn md037_markers_across_lines() {
        let errs = lint_rule("MD037", "text * emphasis\ncontinues * text\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(errs[0].error_range, Some((7, 1)));
        assert_eq!(errs[1].line_number, 2);
        assert_eq!(errs[1].error_context.as_deref(), Some("s *"));
        assert_eq!(errs[1].error_range, Some((10, 1)));
    }

    #[test]
    fn md037_ignores_spans_code_and_html_flow() {
        // emphasis/strong/link label 안의 bare 마커는 원본도 보고하지 않는다.
        assert!(lint_rule("MD037", "a **bold * x * bold** b\n").is_empty());
        assert!(lint_rule("MD037", "[* a *](b)\n").is_empty());
        assert!(lint_rule("MD037", "text `* code *` text\n").is_empty());
        assert!(lint_rule("MD037", "<div>\n* not emphasis *\n</div>\n").is_empty());
    }

    #[test]
    fn md037_columns_are_utf16_units() {
        // 기대값은 원본 markdownlint 를 Node 로 실행해 얻었다 (😀 은 UTF-16 2단위).
        let errs = lint_rule("MD037", "한글 * 강조 * 텍스트 😀 * b * c\n");
        let ranges: Vec<_> = errs.iter().map(|e| e.error_range.unwrap()).collect();
        assert_eq!(ranges, vec![(5, 1), (8, 1), (19, 1), (21, 1)]);
        assert_eq!(errs[0].error_context.as_deref(), Some("* 강"));
    }
}
