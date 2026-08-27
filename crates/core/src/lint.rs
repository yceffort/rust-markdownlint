use std::sync::LazyLock;

use regex::Regex;

use crate::config::{ConfigValue, effective_config};
use crate::error::{ErrorSink, LintError};
use crate::fix::split_lines;
use crate::front_matter::strip_front_matter;
use crate::inline::apply_inline_config;
use crate::parser::{TokenTree, parse};
use crate::rules::{LintContext, registry};

#[derive(Default)]
pub struct LintOptions<'a> {
    pub config: Option<&'a ConfigValue>,
    pub front_matter: Option<&'a str>,
    pub no_inline_config: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum LintFailure {
    #[error("invalid front matter pattern: {0}")]
    InvalidFrontMatter(#[from] Box<fancy_regex::Error>),
}

/// helpers.cjs `clearHtmlCommentText`: 올바른 HTML 주석의 본문을 "." 로 치환해
/// 규칙이 주석 내용을 보지 못하게 하되 줄/열 정보는 유지한다.
pub(crate) fn clear_html_comment_text(text: &str) -> String {
    const BEGIN: &str = "<!--";
    const END: &str = "-->";
    static TRAILING_SPACE_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r" +[\r\n]").expect("trailing space regex"));

    let mut text = text.to_string();
    let mut i = 0;
    while let Some(found) = text[i..].find(BEGIN) {
        i += found;
        let Some(j) = text[i + 2..].find(END).map(|j| i + 2 + j) else {
            break;
        };
        if j > i + BEGIN.len() {
            let content = &text[i + BEGIN.len()..j];
            let last_lf = text[..=i].rfind('\n').map_or(0, |p| p + 1);
            let pre_text = &text[last_lf..i];
            let is_block = pre_text.trim().is_empty();
            let could_be_table = pre_text.trim_start_matches(' ').starts_with('|');
            let spans_table_cells = could_be_table && content.contains('\n');
            let is_valid = is_block
                || !(spans_table_cells
                    || content.starts_with('>')
                    || content.starts_with("->")
                    || content.ends_with('-')
                    || content.contains("--"));
            if is_valid {
                let cleared: String = content
                    .chars()
                    .map(|c| {
                        if c == ' ' || c == '\r' || c == '\n' {
                            c
                        } else {
                            '.'
                        }
                    })
                    .collect();
                let cleared = TRAILING_SPACE_RE.replace_all(&cleared, |caps: &regex::Captures| {
                    caps[0]
                        .chars()
                        .map(|c| if c == '\r' || c == '\n' { c } else { '.' })
                        .collect::<String>()
                });
                text = format!("{}{}{}", &text[..i + BEGIN.len()], cleared, &text[j..]);
                // 비 ASCII 문자가 "." 로 바뀌면 바이트 길이가 줄어드므로 치환 결과 기준으로 재계산
                i += BEGIN.len() + cleared.len() + END.len();
                continue;
            }
        }
        i = j + END.len();
    }
    text
}

/// markdownlint.mjs `lintContent` 포팅: BOM → front matter → 인라인 주석 →
/// 토큰 파싱(필요 시) → HTML 주석 치환 → 규칙 실행 → 비활성 줄 드롭 → 정렬.
pub fn lint_content(
    name: &str,
    content: &str,
    opts: &LintOptions,
) -> Result<Vec<LintError>, LintFailure> {
    let content = content.strip_prefix('\u{FEFF}').unwrap_or(content);

    let user_pattern = opts
        .front_matter
        .map(fancy_regex::Regex::new)
        .transpose()
        .map_err(Box::new)?;
    let (content, front_matter) = strip_front_matter(content, user_pattern.as_ref());
    let front_matter_lines = front_matter.len();

    let raw_lines = split_lines(content);
    let empty_config = ConfigValue::Object(serde_json::Map::new());
    let base = opts.config.unwrap_or(&empty_config);
    let inline = apply_inline_config(&raw_lines, base, opts.no_inline_config);
    let effective = effective_config(&inline.config);

    // 어떤 줄에서든 한 번이라도 활성인 규칙만 실행 대상
    let enabled_rules: Vec<_> = registry::all_rules()
        .iter()
        .copied()
        .filter(|rule| {
            let rule_name = rule.meta().names[0];
            inline
                .enabled_per_line
                .iter()
                .any(|line| line.contains(rule_name))
        })
        .collect();

    let need_tokens = enabled_rules.iter().any(|rule| rule.meta().needs_tokens);
    let tokens = if need_tokens {
        parse(content)
    } else {
        TokenTree::default()
    };

    let cleared = clear_html_comment_text(content);
    let lines = split_lines(&cleared);

    let mut results = Vec::new();
    for rule in enabled_rules {
        let meta = rule.meta();
        let rule_name = meta.names[0];
        let (_, severity, params) = effective.get(rule_name);
        let ctx = LintContext {
            name,
            lines: &lines,
            tokens: &tokens,
            front_matter: &front_matter,
            front_matter_lines,
            config: &params,
        };
        let mut sink = ErrorSink::new(name, &lines, meta, front_matter_lines, severity);
        rule.check(&ctx, &mut sink);
        results.extend(
            sink.errors()
                .iter()
                .filter(|e| {
                    inline
                        .enabled_per_line
                        .get(e.line_number - front_matter_lines - 1)
                        .is_some_and(|enabled| enabled.contains(rule_name))
                })
                .cloned(),
        );
    }

    results.sort_by(|a, b| {
        a.rule_names[0]
            .cmp(b.rule_names[0])
            .then(a.line_number.cmp(&b.line_number))
    });
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::clear_html_comment_text;

    /// 기대값은 원본 helpers.cjs `clearHtmlCommentText` 를 Node 로 실행해 얻었다.
    #[test]
    fn clear_html_comment_cases() {
        // 블록 주석: 본문을 "." 로, 후행 공백도 "." 로
        assert_eq!(clear_html_comment_text("<!-- text -->"), "<!-- .... -->");
        assert_eq!(clear_html_comment_text("<!-- a \nb -->"), "<!-- ..\n. -->");
        // 닫히지 않은 주석은 그대로
        assert_eq!(clear_html_comment_text("<!-- open"), "<!-- open");
        // 인라인 주석에서 "--" 포함은 유효하지 않아 그대로
        assert_eq!(
            clear_html_comment_text("a <!-- x--y -->"),
            "a <!-- x--y -->"
        );
        // 인라인 주석의 일반 본문은 치환
        assert_eq!(clear_html_comment_text("a <!-- xy -->"), "a <!-- .. -->");
        // 테이블 셀에 걸친 주석은 그대로
        assert_eq!(
            clear_html_comment_text("| a | <!-- x\ny --> |"),
            "| a | <!-- x\ny --> |"
        );
        // ">" 판정은 앞 공백을 포함한 원문 기준
        assert_eq!(clear_html_comment_text("a <!-- >x -->"), "a <!-- .. -->");
        // 비 ASCII 본문은 문자당 "." 하나로 줄어들어도 뒤따르는 주석과 본문을 건드리지 않는다
        assert_eq!(
            clear_html_comment_text("<!-- 한 -->\n한\n<!-- x -->"),
            "<!-- . -->\n한\n<!-- . -->"
        );
    }
}
