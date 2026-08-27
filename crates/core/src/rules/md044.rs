use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

use regex::Regex;
use serde_json::Value;

use super::{FileRange, LintContext, Rule, RuleMeta, has_overlap};
use crate::config::truthy;
use crate::error::{ErrorSink, FixInfo};
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
            Some(Value::Array(names)) => names
                .iter()
                .filter_map(|name| name.as_str().map(str::to_string))
                .collect(),
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
                for captures in name_re.captures_iter(tokens.text(id)) {
                    let full = captures.get(0).expect("full match");
                    let left_match = captures.get(1).expect("leftMatch").as_str();
                    let name_match = captures.get(2).expect("nameMatch").as_str();
                    let column = token.start_column
                        + tokens.text(id)[..full.start()].chars().count()
                        + left_match.chars().count();
                    let length = name_match.chars().count();
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
}
