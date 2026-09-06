use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

use regex::Regex;
use serde_json::Value;

use super::{FileRange, LintContext, Rule, RuleMeta, has_overlap};
use crate::config::truthy;
use crate::error::{ErrorSink, FixInfo, utf16_len};
use crate::parser::TokenId;

pub(crate) struct Md044;

static META: RuleMeta = RuleMeta {
    names: &["MD044", "proper-names"],
    description: "Proper names should have the correct capitalization",
    tags: &["spelling"],
    needs_tokens: true,
    fixable: true,
};

/// 원본 `ignoredChildTypes`
const IGNORED_CHILD_TYPES: &[&str] = &["codeFencedFence", "definition", "reference", "resource"];

/// JS `\W` (ASCII 의미) 인지.
fn is_non_word(c: char) -> bool {
    !(c.is_ascii_alphanumeric() || c == '_')
}

/// JS `a.localeCompare(b)` 의 근사: 소문자화한 문자열을 먼저 비교하고, 같으면 앞에서부터
/// 소문자를 대문자보다 앞에 둔다 (ICU 기본 collation 의 tertiary 순서).
fn locale_compare(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| {
        for (ca, cb) in a.chars().zip(b.chars()) {
            if ca != cb {
                return ca.is_uppercase().cmp(&cb.is_uppercase());
            }
        }
        Ordering::Equal
    })
}

/// ECMAScript의 Unicode 플래그 없는 ignore-case `Canonicalize` (코드 유닛 기준).
/// 대문자화가 여러 유닛이면 원본을 쓰고, non-ASCII가 ASCII로 바뀌는
/// Kelvin sign/long s 같은 경우도 원본을 쓴다.
fn canonicalize_js_no_u(unit: u16) -> u16 {
    let Some(ch) = char::from_u32(u32::from(unit)) else {
        return unit;
    };
    let mut upper = ch.to_uppercase();
    let (Some(first), None) = (upper.next(), upper.next()) else {
        return unit;
    };
    if first.len_utf16() != 1 {
        return unit;
    }
    let upper_unit = first as u32 as u16;
    if unit >= 0x80 && upper_unit < 0x80 {
        unit
    } else {
        upper_unit
    }
}

/// JS `gi` (u 없음) 의 대소문자 무시 비교. 매치마다 불리므로 ASCII 는 할당 없이 비교한다.
fn js_no_u_ignore_case_eq(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    if a.is_ascii() && b.is_ascii() {
        return a.eq_ignore_ascii_case(b);
    }
    a.encode_utf16()
        .map(canonicalize_js_no_u)
        .eq(b.encode_utf16().map(canonicalize_js_no_u))
}

/// 원본 comparator `(b.length - a.length) || a.localeCompare(b)` 를 JS 값 위에서 평가한다.
/// 문자열이 아닌 원소가 있으면 V8 이 던지는 TypeError 문구가 Err 다.
fn js_names_compare(a: &Value, b: &Value) -> Result<f64, &'static str> {
    let js_length = |value: &Value| match value {
        Value::Null => Err("Cannot read properties of null (reading 'length')"),
        Value::String(value) => Ok(Some(value.encode_utf16().count())),
        Value::Array(value) => Ok(Some(value.len())),
        _ => Ok(None),
    };
    let b_length = js_length(b)?;
    let a_length = js_length(a)?;
    if let (Some(b_length), Some(a_length)) = (b_length, a_length)
        && b_length != a_length
    {
        return Ok(b_length as f64 - a_length as f64);
    }
    match a {
        Value::String(a) => Ok(match locale_compare(a, &crate::config::js_string(b)) {
            Ordering::Less => -1.0,
            Ordering::Equal => 0.0,
            Ordering::Greater => 1.0,
        }),
        _ => Err("a.localeCompare is not a function"),
    }
}

/// `names` 에 문자열이 아닌 원소가 있을 때 원본이 던지는 첫 TypeError. V8 의 sort(TimSort: 첫 run 판정 뒤
/// binary insertion, 64개 미만이면 run 하나)가 comparator 를 부르는 순서를 그대로 밟고, 정렬이 끝나면
/// `escapeForRegExp` 의 `str.replace` 에서 첫 비문자열이 던진다.
fn invalid_names_failure(names: &[Value]) -> Option<&'static str> {
    let mut sorted: Vec<Value> = names.to_vec();
    let n = sorted.len();
    let mut run_length = n.min(1);
    if n >= 2 {
        let descending = match js_names_compare(&sorted[1], &sorted[0]) {
            Ok(order) => order < 0.0,
            Err(message) => return Some(message),
        };
        run_length = 2;
        while run_length < n {
            let order = match js_names_compare(&sorted[run_length], &sorted[run_length - 1]) {
                Ok(order) => order,
                Err(message) => return Some(message),
            };
            if descending == (order >= 0.0) {
                break;
            }
            run_length += 1;
        }
        if descending {
            sorted[..run_length].reverse();
        }
    }
    for start in run_length..n {
        let pivot = sorted[start].clone();
        let (mut left, mut right) = (0, start);
        while left < right {
            let mid = left + (right - left) / 2;
            match js_names_compare(&pivot, &sorted[mid]) {
                Ok(order) if order < 0.0 => right = mid,
                Ok(_) => left = mid + 1,
                Err(message) => return Some(message),
            }
        }
        sorted.remove(start);
        sorted.insert(left, pivot);
    }
    sorted.iter().find(|name| !name.is_string()).map(|name| {
        if name.is_null() {
            "Cannot read properties of null (reading 'replace')"
        } else {
            "str.replace is not a function"
        }
    })
}

/// 이름별 `nameRe` 는 설정에 따라 달라지므로 이름당 한 번만 컴파일해 캐시한다.
static NAME_RE_CACHE: LazyLock<Mutex<HashMap<String, Regex>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 원본 `namePattern` / `nameRe`: `(${startNamePattern})(${escapedName})${endNamePattern}` 에
/// `gi` 플래그. `\b` 는 ASCII 의미로 옮긴다.
fn name_re(name: &str) -> Regex {
    let mut cache = NAME_RE_CACHE.lock().expect("name regex cache");
    if let Some(re) = cache.get(name) {
        return re.clone();
    }
    let escaped_name = regex::escape(name);
    let start_name_pattern = if name.chars().next().is_some_and(is_non_word) {
        ""
    } else {
        r"(?-u:\b)_*"
    };
    let end_name_pattern = if name.chars().last().is_some_and(is_non_word) {
        ""
    } else {
        r"_*(?-u:\b)"
    };
    let name_pattern = format!("(?i)({start_name_pattern})({escaped_name}){end_name_pattern}");
    let re = Regex::new(&name_pattern).expect("proper name regex");
    cache.insert(name.to_string(), re.clone());
    re
}

impl Rule for Md044 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        let tokens = ctx.tokens;
        let mut names: Vec<String> = match ctx.config.get("names") {
            Some(Value::Array(names)) if names.iter().all(Value::is_string) => names
                .iter()
                .map(|name| name.as_str().expect("checked string").to_string())
                .collect(),
            Some(Value::Array(names)) => {
                let message = invalid_names_failure(names).expect("array contains a non-string");
                out.add_rule_failure(message);
                return;
            }
            _ => Vec::new(),
        };
        names.sort_by(|a, b| {
            b.encode_utf16()
                .count()
                .cmp(&a.encode_utf16().count())
                .then_with(|| locale_compare(a, b))
        });
        if names.is_empty() {
            // Nothing to check; avoid doing any work
            return;
        }
        let include_code_blocks = ctx.config.get("code_blocks").is_none_or(truthy);
        let include_html_elements = ctx.config.get("html_elements").is_none_or(truthy);
        let mut scanned_types: Vec<&str> = vec!["data"];
        if include_code_blocks {
            scanned_types.push("codeFlowValue");
            scanned_types.push("codeTextData");
        }
        if include_html_elements {
            scanned_types.push("htmlFlowData");
            scanned_types.push("htmlTextData");
        }
        let content_tokens = tokens.filter_by_predicate(
            &tokens.roots,
            |t, id| scanned_types.contains(&t.get(id).kind),
            |t, id, out| {
                out.extend(
                    t.get(id)
                        .children
                        .iter()
                        .copied()
                        .filter(|&c| !IGNORED_CHILD_TYPES.contains(&t.get(c).kind)),
                );
            },
        );
        let mut exclusions: Vec<FileRange> = Vec::new();
        let mut scanned_tokens: HashSet<TokenId> = HashSet::new();
        for name in &names {
            let name_re = name_re(name);
            for &id in &content_tokens {
                let token = tokens.get(id);
                let text = tokens.text(id);
                let mut position = 0;
                while position <= text.len()
                    && let Some(captures) = name_re.captures_at(text, position)
                {
                    let full = captures.get(0).expect("full match");
                    let left_match = captures.get(1).expect("leftMatch").as_str();
                    let name_match = captures.get(2).expect("nameMatch").as_str();
                    let next_char_len = text[full.start()..]
                        .chars()
                        .next()
                        .map_or(1, char::len_utf8);
                    // Rust regex 의 Unicode case-fold 가 JS `gi` 보다 넓게 잡은 후보: JS 는 이 위치에서
                    // 실패하고 다음 코드 유닛부터 다시 찾는다.
                    if !js_no_u_ignore_case_eq(name, name_match) {
                        position = full.start() + next_char_len;
                        continue;
                    }
                    position = if full.is_empty() {
                        full.end() + next_char_len
                    } else {
                        full.end()
                    };
                    // 원본 `match.index`, `.length`: UTF-16 단위
                    let column = token.start_column
                        + utf16_len(&tokens.text(id)[..full.start()])
                        + utf16_len(left_match);
                    let length = utf16_len(name_match);
                    let line_number = token.start_line;
                    let name_range = FileRange {
                        start_line: line_number,
                        start_column: column,
                        end_line: line_number,
                        end_column: column + length - 1,
                    };
                    if !names.iter().any(|n| n == name_match)
                        && !exclusions
                            .iter()
                            .any(|exclusion| has_overlap(exclusion, &name_range))
                    {
                        let mut autolink_ranges: Vec<FileRange> = Vec::new();
                        if !scanned_tokens.contains(&id) {
                            let reparsed = crate::parser::parse(tokens.text(id));
                            autolink_ranges = reparsed
                                .filter_by_types(&["literalAutolink"])
                                .into_iter()
                                .map(|tok_id| {
                                    let tok = reparsed.get(tok_id);
                                    FileRange {
                                        start_line: line_number,
                                        start_column: token.start_column + tok.start_column - 1,
                                        end_line: line_number,
                                        end_column: token.end_column + tok.end_column - 1,
                                    }
                                })
                                .collect();
                            exclusions.extend(autolink_ranges.iter().cloned());
                            scanned_tokens.insert(id);
                        }
                        if !autolink_ranges
                            .iter()
                            .any(|autolink_range| has_overlap(autolink_range, &name_range))
                        {
                            out.add_error_detail_if(
                                token.start_line,
                                name,
                                name_match,
                                None,
                                None,
                                Some((column, length)),
                                Some(FixInfo {
                                    edit_column: Some(column),
                                    delete_count: Some(length as isize),
                                    insert_text: Some(name.clone()),
                                    ..Default::default()
                                }),
                            );
                        }
                    }
                    exclusions.push(name_range);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::{LintOptions, lint_content};
    use serde_json::json;

    fn lint_with(params: serde_json::Value, content: &str) -> Vec<crate::error::LintError> {
        let config = json!({ "default": false, "MD044": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md044_no_names_does_nothing() {
        assert!(lint_with(json!(true), "javascript and JAVASCRIPT\n").is_empty());
    }

    #[test]
    fn md044_uses_javascript_non_unicode_case_folding() {
        assert!(lint_with(json!({ "names": ["K", "S"] }), "AKB AſB\n").is_empty());
        assert_eq!(lint_with(json!({ "names": ["É"] }), "é\n").len(), 1);
    }

    /// Rust 의 case-fold 로 잡힌 가짜 후보(`sſ`)를 버린 뒤 그 안에서 시작하는 진짜 매치(`ſS`)를 찾는다.
    #[test]
    fn md044_retries_after_rejected_unicode_fold() {
        let errs = lint_with(json!({ "names": ["ſs"] }), "sſS\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: ſs; Actual: ſS")
        );
        assert_eq!(errs[0].error_range, Some((2, 2)));
    }

    /// V8 sort 는 첫 run 판정 뒤 binary insertion 이라 인접하지 않은 원소끼리도 비교한다.
    #[test]
    fn md044_sort_failure_follows_v8_comparison_order() {
        let errs = lint_with(json!({ "names": ["x", "ab", [1]] }), "x ab\n");
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("This rule threw an exception: a.localeCompare is not a function")
        );
        let errs = lint_with(json!({ "names": [7, "ab"] }), "ab\n");
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("This rule threw an exception: str.replace is not a function")
        );
    }

    #[test]
    fn md044_mixed_names_report_rule_failure() {
        let errs = lint_with(json!({ "names": ["GitHub", 7, null] }), "github 7 null\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("This rule threw an exception: a.localeCompare is not a function")
        );
    }

    #[test]
    fn md044_reports_wrong_case_with_fix() {
        let errs = lint_with(json!({ "names": ["JavaScript"] }), "Use javascript here.\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].line_number, 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: JavaScript; Actual: javascript")
        );
        assert_eq!(errs[0].error_range, Some((5, 10)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(f.edit_column, Some(5));
        assert_eq!(f.delete_count, Some(10));
        assert_eq!(f.insert_text.as_deref(), Some("JavaScript"));
    }

    #[test]
    fn md044_word_boundary_and_substring() {
        // 단어 경계가 없으면 매치하지 않는다
        assert!(lint_with(json!({ "names": ["Node"] }), "nodejs and Nodes\n").is_empty());
        // 긴 이름이 먼저 매치되어 짧은 이름의 제외 범위가 된다
        assert!(
            lint_with(
                json!({ "names": ["Node.js", "node"] }),
                "Node.js is great\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn md044_code_blocks_and_html_elements_options() {
        // 기대값은 원본을 node 로 실행해 얻었다. `<div>javascript</div>` 의 본문은 htmlFlow
        // 재파싱으로 `data` 가 되므로 html_elements 와 무관하고, 속성값만 htmlTextData 다.
        let content = "```\njavascript\n```\n\n<div>javascript</div>\n\n<span title=\"javascript\">x</span>\n";
        let names = json!(["JavaScript"]);
        let lines = |params: serde_json::Value| -> Vec<usize> {
            lint_with(params, content)
                .iter()
                .map(|e| e.line_number)
                .collect()
        };
        assert_eq!(lines(json!({ "names": names })), vec![2, 5, 7]);
        assert_eq!(
            lines(json!({ "names": names, "code_blocks": false })),
            vec![5, 7]
        );
        assert_eq!(
            lines(json!({ "names": names, "html_elements": false })),
            vec![2, 5]
        );
    }

    #[test]
    fn md044_ignores_link_destinations_and_autolinks() {
        let content = "[GitHub](https://github.com/x) and https://github.com/y\n";
        assert!(lint_with(json!({ "names": ["GitHub"] }), content).is_empty());
    }

    #[test]
    fn md044_non_word_boundaries() {
        let errs = lint_with(json!({ "names": [".NET"] }), "Use .net now\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].error_range, Some((5, 4)));
    }

    #[test]
    fn md044_column_and_length_are_utf16() {
        // 기대값은 cli2 0.22.1 실행 결과 (원본 `match.index`, `nameMatch.length` 는 UTF-16 단위)
        let errs = lint_with(json!({ "names": ["Rock🎸"] }), "🎸 rock🎸 here\n");
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: Rock🎸; Actual: rock🎸")
        );
        assert_eq!(errs[0].error_range, Some((4, 6)));
        let f = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(
            (f.edit_column, f.delete_count, f.insert_text.as_deref()),
            (Some(4), Some(6), Some("Rock🎸"))
        );
    }
}
