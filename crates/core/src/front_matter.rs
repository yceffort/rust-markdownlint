use std::sync::LazyLock;

use fancy_regex::Regex;

/// helpers.cjs `frontMatterRe`. JS 의 multiline `$` 는 개행을 소비하지 않고 줄 끝에
/// 매치하므로 lookahead `(?=[\r\n\x{2028}\x{2029}]|\z)` 로 옮겼다.
static DEFAULT_FRONT_MATTER_RE: LazyLock<Regex> = LazyLock::new(|| {
    const EOL: &str = r"[^\S\r\n\u{2028}\u{2029}]*(?=[\r\n\u{2028}\u{2029}]|\z)";
    Regex::new(&format!(
        r"(?m)((^---{EOL}[\s\S]+?^---\s*)|(^\+\+\+{EOL}[\s\S]+?^(\+\+\+|\.\.\.)\s*)|(^\{{{EOL}[\s\S]+?^\}}\s*))(\r\n|\r|\n|\z)"
    ))
    .expect("front matter regex")
});

/// 사용자 `frontMatter` 패턴(JS 정규식) 컴파일. JS 의 `[^]` (개행 포함 아무 문자) 는 Rust 에
/// 없으므로 `[\s\S]` 로 옮긴다.
pub fn compile_js_pattern(pattern: &str) -> Result<Regex, fancy_regex::Error> {
    let mut translated = String::with_capacity(pattern.len());
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            translated.push(c);
            translated.extend(chars.next());
        } else if c == '[' && chars.as_str().starts_with("^]") {
            translated.push_str(r"[\s\S]");
            chars.next();
            chars.next();
        } else {
            translated.push(c);
        }
    }
    Regex::new(&translated)
}

/// markdownlint.mjs `removeFrontMatter` 포팅. 매치가 문서 맨 앞일 때만 제거하고
/// front matter 줄 목록을 반환한다.
pub fn strip_front_matter<'a>(
    content: &'a str,
    pattern: Option<&Regex>,
) -> (&'a str, Vec<&'a str>) {
    let re = pattern.unwrap_or(&DEFAULT_FRONT_MATTER_RE);
    if let Ok(Some(m)) = re.find(content)
        && m.start() == 0
    {
        return (&content[m.end()..], split_front_matter_lines(m.as_str()));
    }
    (content, Vec::new())
}

/// `matched.split(newLineRe)` (`/\r\n?|\n/`) 후 마지막 빈 요소를 버린 결과.
fn split_front_matter_lines(matched: &str) -> Vec<&str> {
    let mut lines = crate::fix::split_lines(matched);
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_front_matter_counts_lines() {
        let (rest, lines) = strip_front_matter("---\ntitle: x\n---\n# h\n", None);
        assert_eq!(lines, ["---", "title: x", "---"]);
        assert_eq!(rest, "# h\n");
    }

    #[test]
    fn toml_front_matter() {
        let (rest, lines) = strip_front_matter("+++\ntitle = \"x\"\n+++\n# h\n", None);
        assert_eq!(lines, ["+++", "title = \"x\"", "+++"]);
        assert_eq!(rest, "# h\n");
    }

    #[test]
    fn no_front_matter_returns_content() {
        let (rest, lines) = strip_front_matter("# h\n---\nx\n---\n", None);
        assert!(lines.is_empty());
        assert_eq!(rest, "# h\n---\nx\n---\n");
    }

    #[test]
    fn crlf_front_matter() {
        let (rest, lines) = strip_front_matter("---\r\ntitle: x\r\n---\r\n# h\r\n", None);
        assert_eq!(lines, ["---", "title: x", "---"]);
        assert_eq!(rest, "# h\r\n");
    }

    /// 기대값은 원본 helpers.cjs `frontMatterRe` 를 Node 로 실행해 얻었다.
    #[test]
    fn matches_original_front_matter_re() {
        let cases: &[(&str, &str, usize)] = &[
            ("---\nt\n---\n\n# h\n", "# h\n", 4),
            ("---\nt\n---", "", 3),
            ("+++\nt\n...\n# h\n", "# h\n", 3),
            ("{\n \"a\": 1\n}\n# h\n", "# h\n", 3),
            ("---  \nt\n---   \n# h\n", "# h\n", 3),
            ("---\n---\n# h\n", "# h\n", 2),
        ];
        for (content, rest, n) in cases {
            let (got_rest, got_lines) = strip_front_matter(content, None);
            assert_eq!((got_rest, got_lines.len()), (*rest, *n), "{content:?}");
        }
    }

    #[test]
    fn user_pattern_with_lookahead() {
        let re = Regex::new(r"(?s)^<!--.*?-->\r?\n(?=#)").unwrap();
        let (rest, lines) = strip_front_matter("<!-- meta -->\n# h\n", Some(&re));
        assert_eq!(lines, ["<!-- meta -->"]);
        assert_eq!(rest, "# h\n");
    }
}
