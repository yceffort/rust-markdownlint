use crate::error::{FixInfo, LintError};

/// markdownlint.mjs `normalizeFixInfo`. JS 의 `||` 기본값 규칙(0 도 falsy)을 따른다.
#[derive(Clone, PartialEq)]
struct NormalizedFix {
    line_number: usize,
    edit_column: usize,
    delete_count: isize,
    insert_text: String,
}

fn normalize(fix: &FixInfo, line_number: usize) -> NormalizedFix {
    NormalizedFix {
        line_number: fix.line_number.filter(|n| *n != 0).unwrap_or(line_number),
        edit_column: fix.edit_column.filter(|n| *n != 0).unwrap_or(1),
        delete_count: fix.delete_count.unwrap_or(0),
        insert_text: fix.insert_text.clone().unwrap_or_default(),
    }
}

/// UTF-16 단위 인덱스(JS 문자열 인덱스)를 바이트 오프셋으로 (JS slice 처럼 줄 길이로 클램프).
/// 서로게이트 쌍 가운데를 가리키면 그 문자의 시작으로 본다.
fn byte_index(line: &str, utf16_index: usize) -> usize {
    let mut units = 0;
    line.char_indices()
        .find(|(_, c)| {
            if units >= utf16_index {
                return true;
            }
            units += c.len_utf16();
            false
        })
        .map_or(line.len(), |(i, _)| i)
}

fn apply_normalized(line: &str, fix: &NormalizedFix, line_ending: &str) -> Option<String> {
    if fix.delete_count == -1 {
        return None;
    }
    let edit_index = fix.edit_column - 1;
    let start = byte_index(line, edit_index);
    let end = byte_index(line, edit_index + fix.delete_count.max(0) as usize);
    Some(format!(
        "{}{}{}",
        &line[..start],
        fix.insert_text.replace('\n', line_ending),
        &line[end..]
    ))
}

/// markdownlint.mjs `applyFix` 포팅. `delete_count` 가 -1 이면 줄 삭제(None).
pub fn apply_fix(line: &str, fix: &FixInfo, line_ending: &str) -> Option<String> {
    apply_normalized(line, &normalize(fix, 0), line_ending)
}

/// helpers.cjs `getPreferredLineEnding` (줄 끝 다수결, 동수면 \n > \r\n > \r).
fn preferred_line_ending(input: &str) -> &'static str {
    let (mut cr, mut lf, mut crlf) = (0u32, 0u32, 0u32);
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                crlf += 1;
                i += 2;
            }
            b'\r' => {
                cr += 1;
                i += 1;
            }
            b'\n' => {
                lf += 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    if lf >= crlf && lf >= cr {
        "\n"
    } else if crlf >= cr {
        "\r\n"
    } else {
        "\r"
    }
}

/// helpers.cjs `newLineRe` (`/\r\n?|\n/`) 로 split.
pub(crate) fn split_lines(input: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let bytes = input.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' => {
                lines.push(&input[start..i]);
                i += if bytes.get(i + 1) == Some(&b'\n') {
                    2
                } else {
                    1
                };
                start = i;
            }
            b'\n' => {
                lines.push(&input[start..i]);
                i += 1;
                start = i;
            }
            _ => i += 1,
        }
    }
    lines.push(&input[start..]);
    lines
}

/// markdownlint.mjs `applyFixes` 포팅: 정렬(줄 내림차순, 줄 삭제 뒤로, 열 내림차순,
/// insert 길이 내림차순) → 중복 제거 → insert+delete 병합 → 겹침 스킵 적용.
pub fn apply_fixes(input: &str, errors: &[LintError]) -> String {
    let line_ending = preferred_line_ending(input);
    let mut lines: Vec<Option<String>> = split_lines(input)
        .into_iter()
        .map(|s| Some(s.to_string()))
        .collect();

    let mut fix_infos: Vec<NormalizedFix> = errors
        .iter()
        .filter_map(|e| e.fix_info.as_ref().map(|f| normalize(f, e.line_number)))
        .collect();
    fix_infos.sort_by(|a, b| {
        b.line_number
            .cmp(&a.line_number)
            .then_with(|| (a.delete_count == -1).cmp(&(b.delete_count == -1)))
            .then_with(|| b.edit_column.cmp(&a.edit_column))
            .then_with(|| {
                b.insert_text
                    .chars()
                    .count()
                    .cmp(&a.insert_text.chars().count())
            })
    });
    fix_infos.dedup();

    // 같은 줄/열의 insert(삭제 없음)와 delete(삽입 없음)를 하나로 합친다
    for i in 1..fix_infos.len() {
        let (prev, cur) = {
            let (a, b) = fix_infos.split_at_mut(i);
            (&mut a[i - 1], &mut b[0])
        };
        if cur.line_number == prev.line_number
            && cur.edit_column == prev.edit_column
            && cur.insert_text.is_empty()
            && cur.delete_count > 0
            && !prev.insert_text.is_empty()
            && prev.delete_count == 0
        {
            cur.insert_text = prev.insert_text.clone();
            prev.line_number = 0;
        }
    }
    fix_infos.retain(|f| f.line_number != 0);

    let mut last_line_index: isize = -1;
    let mut last_edit_index: isize = -1;
    for fix in &fix_infos {
        let line_index = fix.line_number as isize - 1;
        let edit_index = fix.edit_column as isize - 1;
        if line_index != last_line_index
            || fix.delete_count == -1
            || (edit_index + fix.delete_count)
                <= (last_edit_index - if fix.delete_count > 0 { 0 } else { 1 })
        {
            let idx = line_index as usize;
            lines[idx] =
                apply_normalized(lines[idx].as_deref().unwrap_or_default(), fix, line_ending);
        }
        last_line_index = line_index;
        last_edit_index = edit_index;
    }

    lines
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(line_ending)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Severity;

    fn err_fix(line: usize, col: usize, del: isize, text: &str) -> LintError {
        LintError {
            line_number: line,
            rule_names: &[],
            rule_description: "",
            rule_information: String::new(),
            error_detail: None,
            error_context: None,
            error_range: None,
            fix_info: Some(FixInfo {
                line_number: Some(line),
                edit_column: Some(col),
                delete_count: Some(del),
                insert_text: Some(text.to_string()),
            }),
            severity: Severity::Error,
        }
    }

    fn err_raw(line: usize, fix: FixInfo) -> LintError {
        LintError {
            fix_info: Some(fix),
            ..err_fix(line, 1, 0, "")
        }
    }

    /// editColumn 은 JS 문자열 인덱스(UTF-16)라 이모지 뒤에서는 코드 포인트보다 하나 더 크다.
    #[test]
    fn edit_column_is_utf16() {
        let f = FixInfo {
            line_number: Some(1),
            edit_column: Some(4),
            delete_count: Some(1),
            insert_text: Some("_".into()),
        };
        assert_eq!(apply_fix("👉 *a*", &f, "\n"), Some("👉 _a*".into()));
    }

    #[test]
    fn delete_line_with_minus_one() {
        let f = FixInfo {
            line_number: Some(1),
            edit_column: None,
            delete_count: Some(-1),
            insert_text: None,
        };
        assert_eq!(apply_fix("abc", &f, "\n"), None);
    }

    #[test]
    fn insert_and_replace() {
        let f = FixInfo {
            line_number: Some(1),
            edit_column: Some(2),
            delete_count: Some(1),
            insert_text: Some("X".into()),
        };
        assert_eq!(apply_fix("abc", &f, "\n").unwrap(), "aXc");
    }

    #[test]
    fn delete_count_clamps_to_line_end() {
        let f = FixInfo {
            line_number: Some(1),
            edit_column: Some(3),
            delete_count: Some(9),
            insert_text: Some("X".into()),
        };
        assert_eq!(apply_fix("abc", &f, "\n").unwrap(), "abX");
    }

    #[test]
    fn fixes_applied_descending_and_crlf_preserved() {
        let input = "a \r\nb \r\n";
        let errs = vec![err_fix(1, 2, 1, ""), err_fix(2, 2, 1, "")];
        assert_eq!(apply_fixes(input, &errs), "a\r\nb\r\n");
    }

    /// 계획 문서는 "X\n" 을 기대했지만 원본 applyFixes 실행 결과는 "aYc\n" 이다.
    /// (열 내림차순 정렬로 col2 가 먼저 적용되고 col1 은 겹침으로 스킵)
    #[test]
    fn overlapping_same_line_skipped() {
        let errs = vec![err_fix(1, 1, 3, "X"), err_fix(1, 2, 1, "Y")];
        assert_eq!(apply_fixes("abc\n", &errs), "aYc\n");
    }

    /// 기대값은 원본 markdownlint@0.40.0 `applyFixes` 를 Node 로 실행해 얻었다.
    #[test]
    fn matches_original_apply_fixes() {
        let insert_only = |line: usize, col: usize, text: &str| {
            err_raw(
                line,
                FixInfo {
                    line_number: None,
                    edit_column: Some(col),
                    delete_count: None,
                    insert_text: Some(text.to_string()),
                },
            )
        };
        let delete_only = |line: usize, col: usize, del: isize| {
            err_raw(
                line,
                FixInfo {
                    line_number: None,
                    edit_column: Some(col),
                    delete_count: Some(del),
                    insert_text: None,
                },
            )
        };
        let delete_line = |line: usize| {
            err_raw(
                line,
                FixInfo {
                    line_number: None,
                    edit_column: None,
                    delete_count: Some(-1),
                    insert_text: None,
                },
            )
        };

        // insert+delete 병합: 같은 줄/열의 insert 와 delete 가 하나로 합쳐진다
        assert_eq!(
            apply_fixes("abc\n", &[delete_only(1, 2, 1), insert_only(1, 2, "X")]),
            "aXc\n"
        );
        // 중복 제거
        assert_eq!(
            apply_fixes("abc\n", &[err_fix(1, 2, 1, "Y"), err_fix(1, 2, 1, "Y")]),
            "aYc\n"
        );
        // 줄 삭제
        assert_eq!(apply_fixes("a\nb\n", &[delete_line(1)]), "b\n");
        // 줄 끝 다수결: crlf 2 > lf 1 → 전체를 \r\n 으로
        assert_eq!(
            apply_fixes("a \nb \r\nc \r\n", &[err_fix(1, 2, 1, "")]),
            "a\r\nb \r\nc \r\n"
        );
        // 같은 줄에서 겹치지 않는 fix 는 모두 적용
        assert_eq!(
            apply_fixes("aabb\n", &[err_fix(1, 1, 2, "X"), err_fix(1, 3, 2, "Y")]),
            "XY\n"
        );
        // insert_text 의 \n 은 파일의 줄 끝으로 치환
        assert_eq!(
            apply_fixes("ab\r\n", &[err_fix(1, 2, 0, "\n")]),
            "a\r\nb\r\n"
        );
        // 같은 줄의 일반 fix 적용 후 줄 삭제
        assert_eq!(
            apply_fixes("abc\ndef\n", &[delete_line(1), err_fix(1, 2, 1, "Z")]),
            "def\n"
        );
    }
}
