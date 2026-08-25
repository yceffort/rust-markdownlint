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

/// markdownlint.mjs `removeFrontMatter` 포팅. 매치가 문서 맨 앞일 때만 제거하고
/// front matter 줄 수를 반환한다.
pub fn strip_front_matter<'a>(content: &'a str, pattern: Option<&Regex>) -> (&'a str, usize) {
    let re = pattern.unwrap_or(&DEFAULT_FRONT_MATTER_RE);
    if let Ok(Some(m)) = re.find(content)
        && m.start() == 0
    {
        return (&content[m.end()..], count_lines(m.as_str()));
    }
    (content, 0)
}

/// `matched.split(newLineRe)` (`/\r\n?|\n/`) 후 마지막 빈 요소를 버린 길이.
fn count_lines(matched: &str) -> usize {
    let bytes = matched.as_bytes();
    let mut terminators = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\r' => {
                terminators += 1;
                i += if bytes.get(i + 1) == Some(&b'\n') {
                    2
                } else {
                    1
                };
            }
            b'\n' => {
                terminators += 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    let ends_with_terminator = matches!(bytes.last(), Some(b'\r' | b'\n'));
    if matched.is_empty() || ends_with_terminator {
        terminators
    } else {
        terminators + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_front_matter_counts_lines() {
        let (rest, n) = strip_front_matter("---\ntitle: x\n---\n# h\n", None);
        assert_eq!(n, 3);
        assert_eq!(rest, "# h\n");
    }

    #[test]
    fn toml_front_matter() {
        let (rest, n) = strip_front_matter("+++\ntitle = \"x\"\n+++\n# h\n", None);
        assert_eq!(n, 3);
        assert_eq!(rest, "# h\n");
    }

    #[test]
    fn no_front_matter_returns_content() {
        let (rest, n) = strip_front_matter("# h\n---\nx\n---\n", None);
        assert_eq!(n, 0);
        assert_eq!(rest, "# h\n---\nx\n---\n");
    }

    #[test]
    fn crlf_front_matter() {
        let (rest, n) = strip_front_matter("---\r\ntitle: x\r\n---\r\n# h\r\n", None);
        assert_eq!(n, 3);
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
            assert_eq!(
                strip_front_matter(content, None),
                (*rest, *n),
                "{content:?}"
            );
        }
    }

    #[test]
    fn user_pattern_with_lookahead() {
        let re = Regex::new(r"(?s)^<!--.*?-->\r?\n(?=#)").unwrap();
        let (rest, n) = strip_front_matter("<!-- meta -->\n# h\n", Some(&re));
        assert_eq!(n, 1);
        assert_eq!(rest, "# h\n");
    }
}
