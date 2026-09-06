use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use fancy_regex::Regex;

use crate::parser::JS_WHITESPACE;

/// helpers.cjs `frontMatterRe`. JS 의 multiline `$` 는 개행을 소비하지 않고 줄 끝에
/// 매치하므로 lookahead `(?=[\r\n\x{2028}\x{2029}]|\z)` 로 옮겼다.
static DEFAULT_FRONT_MATTER_RE: LazyLock<Regex> = LazyLock::new(|| {
    const EOL: &str = r"[^\S\r\n\u{2028}\u{2029}]*(?=[\r\n\u{2028}\u{2029}]|\z)";
    Regex::new(&format!(
        r"(?m)((^---{EOL}[\s\S]+?^---\s*)|(^\+\+\+{EOL}[\s\S]+?^(\+\+\+|\.\.\.)\s*)|(^\{{{EOL}[\s\S]+?^\}}\s*))(\r\n|\r|\n|\z)"
    ))
    .expect("front matter regex")
});

/// ECMAScript 의 `\w` (`u` 플래그와 무관하게 ASCII). `(?i)` 아래에서 Rust 가 `K`/`ſ` 를
/// `[a-z]` 로 접지 않도록 `(?-i:` 로 감싼다.
const JS_WORD: &str = "(?-i:[A-Za-z0-9_])";
/// JS `.`: `u` 와 무관하게 LF, CR, U+2028, U+2029 만 제외한다 (Rust 는 LF 만 제외).
const JS_DOT: &str = r"[^\n\r\x{2028}\x{2029}]";

/// JS `\b`/`\B` 는 ASCII `\w` 기준 경계다. fancy_regex 는 `(?-u:\b)` 를 받지 않으므로 lookaround 로 쓴다.
fn js_word_boundary(negated: bool) -> String {
    if negated {
        format!("(?:(?<!{JS_WORD})(?!{JS_WORD})|(?<={JS_WORD})(?={JS_WORD}))")
    } else {
        format!("(?:(?<!{JS_WORD})(?={JS_WORD})|(?<={JS_WORD})(?!{JS_WORD}))")
    }
}

/// JavaScript RegExp 소스를 fancy_regex 문법으로 옮긴다.
///
/// - `\w`/`\d`/`\b` 는 `u` 플래그와 관계없이 ASCII 이고, `\s` 는 Rust 의 White_Space 와 집합이 다르다.
/// - `.` 은 CR, U+2028, U+2029 도 제외한다.
/// - `[^]` 는 모든 문자, `[]` 는 아무것도 매치하지 않는 클래스다.
/// - 클래스 안의 `[`, `&`, `~`, 범위가 아닌 `-` 는 Rust 에서 메타문자라 이스케이프한다.
/// - `u` 없는 모드에서 정의되지 않은 영숫자 이스케이프(`\z`, `\q`, `\p`)는 그 글자 자체다.
///
/// V8 만 거부하는 `{2,1}` 같은 수량자는 Err 로 V8 의 사유를 돌려준다.
pub(crate) fn translate_js_pattern(pattern: &str, unicode: bool) -> Result<String, String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len() + 16);
    let mut i = 0;
    // V8 는 수량자 뒤의 수량자(`x{2}{3}`, `a*+`)를 거부한다 (`?` 한 번은 lazy 표시).
    let mut after_quantifier: Option<bool> = None;
    while i < chars.len() {
        let c = chars[i];
        let mut quantifier = false;
        match c {
            '\\' => {
                let Some(&escaped) = chars.get(i + 1) else {
                    out.push('\\');
                    break;
                };
                i += 2;
                i = translate_escape_at(&chars, i, escaped, unicode, false, &mut out);
                after_quantifier = None;
                continue;
            }
            '[' => {
                i = translate_class(&chars, i + 1, unicode, &mut out);
                after_quantifier = None;
                continue;
            }
            '.' => out.push_str(JS_DOT),
            '{' => match quantifier_bounds(&chars, i) {
                Some((min, max, end)) => {
                    if max.is_some_and(|max| min > max) {
                        return Err("numbers out of order in {} quantifier".to_string());
                    }
                    quantifier = true;
                    out.extend(&chars[i..end]);
                    i = end - 1;
                }
                // Annex B: 수량자가 아닌 `{` 는 리터럴이다 (Rust 는 오류).
                None if !unicode => out.push_str(r"\{"),
                None => out.push('{'),
            },
            '*' | '+' | '?' => {
                quantifier = true;
                out.push(c);
            }
            _ => out.push(c),
        }
        i += 1;
        if quantifier {
            match after_quantifier {
                Some(false) if c == '?' => after_quantifier = Some(true),
                Some(_) => return Err("Nothing to repeat".to_string()),
                None => after_quantifier = Some(false),
            }
        } else {
            after_quantifier = None;
        }
    }
    Ok(out)
}

/// `\cX` (제어 문자) 처럼 뒤 글자를 더 읽는 이스케이프를 처리하고 다음 인덱스를 돌려준다.
fn translate_escape_at(
    chars: &[char],
    i: usize,
    escaped: char,
    unicode: bool,
    in_class: bool,
    out: &mut String,
) -> usize {
    if escaped == 'c' {
        return match chars.get(i) {
            Some(letter) if letter.is_ascii_alphabetic() => {
                out.push_str(&format!(r"\x{{{:02x}}}", (*letter as u32) % 32));
                i + 1
            }
            // `u` 없는 모드에서 `\c` 뒤에 글자가 없으면 백슬래시와 `c` 그대로다.
            _ => {
                out.push_str(r"\\c");
                i
            }
        };
    }
    translate_escape(escaped, unicode, in_class, out);
    i
}

/// `{n}`, `{n,}`, `{n,m}` 수량자의 경계와 닫는 `}` 다음 인덱스. 수량자가 아니면 None.
fn quantifier_bounds(chars: &[char], i: usize) -> Option<(u64, Option<u64>, usize)> {
    let rest: String = chars[i + 1..].iter().take_while(|c| **c != '}').collect();
    let end = i + 1 + rest.chars().count();
    if chars.get(end) != Some(&'}') {
        return None;
    }
    let (min, max) = match rest.split_once(',') {
        Some((min, max)) => (min, Some(max)),
        None => (rest.as_str(), None),
    };
    let parse = |digits: &str| {
        (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
            .then(|| digits.parse::<u64>().unwrap_or(u64::MAX))
    };
    let min = parse(min)?;
    let max = match max {
        Some("") => None,
        Some(max) => Some(parse(max)?),
        None => Some(min),
    };
    Some((min, max, end + 1))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClassAtom {
    /// 문자 하나 (범위의 끝점이 될 수 있다).
    Literal,
    /// `\w` 같은 집합 이스케이프 (범위 끝점이 될 수 없어 옆의 `-` 는 리터럴).
    Set,
    /// 방금 범위를 닫았다 (`a-b` 뒤의 `-` 는 리터럴).
    RangeEnd,
}

/// `[` 다음부터 클래스를 옮기고 닫는 `]` 다음 인덱스를 돌려준다. 닫히지 않은 클래스는 JS 도 오류이므로
/// 그대로 두어 컴파일 오류로 이어지게 한다.
fn translate_class(chars: &[char], mut i: usize, unicode: bool, out: &mut String) -> usize {
    let negated = chars.get(i) == Some(&'^');
    if negated {
        i += 1;
    }
    if chars.get(i) == Some(&']') {
        out.push_str(if negated { r"[\s\S]" } else { r"[^\s\S]" });
        return i + 1;
    }
    out.push('[');
    if negated {
        out.push('^');
    }
    let mut prev: Option<ClassAtom> = None;
    while i < chars.len() {
        let c = chars[i];
        if c == ']' {
            out.push(']');
            return i + 1;
        }
        if c == '-' {
            let range = prev == Some(ClassAtom::Literal)
                && class_atom_kind(chars, i + 1) == Some(ClassAtom::Literal);
            if range {
                out.push('-');
                i = push_class_atom(chars, i + 1, unicode, out);
                prev = Some(ClassAtom::RangeEnd);
            } else {
                out.push_str(r"\-");
                prev = Some(ClassAtom::Literal);
                i += 1;
            }
            continue;
        }
        prev = class_atom_kind(chars, i);
        i = push_class_atom(chars, i, unicode, out);
    }
    i
}

fn class_atom_kind(chars: &[char], i: usize) -> Option<ClassAtom> {
    match chars.get(i)? {
        ']' => None,
        '\\' => Some(match chars.get(i + 1) {
            Some('w' | 'W' | 'd' | 'D' | 's' | 'S') => ClassAtom::Set,
            _ => ClassAtom::Literal,
        }),
        _ => Some(ClassAtom::Literal),
    }
}

/// 클래스 안의 원자 하나를 옮기고 다음 인덱스를 돌려준다.
fn push_class_atom(chars: &[char], i: usize, unicode: bool, out: &mut String) -> usize {
    match chars[i] {
        '\\' => match chars.get(i + 1) {
            Some(&escaped) => translate_escape_at(chars, i + 2, escaped, unicode, true, out),
            None => {
                out.push('\\');
                i + 1
            }
        },
        c @ ('[' | '&' | '~' | '^' | '-') => {
            out.push('\\');
            out.push(c);
            i + 1
        }
        c => {
            out.push(c);
            i + 1
        }
    }
}

fn translate_escape(escaped: char, unicode: bool, in_class: bool, out: &mut String) {
    match escaped {
        'w' if in_class => out.push_str("A-Za-z0-9_"),
        'w' => out.push_str(JS_WORD),
        'W' if in_class => out.push_str("[^A-Za-z0-9_]"),
        'W' => out.push_str("(?-i:[^A-Za-z0-9_])"),
        'd' if in_class => out.push_str("0-9"),
        'd' => out.push_str("[0-9]"),
        'D' => out.push_str("[^0-9]"),
        's' if in_class => out.push_str(JS_WHITESPACE),
        's' => {
            out.push('[');
            out.push_str(JS_WHITESPACE);
            out.push(']');
        }
        'S' => {
            out.push_str("[^");
            out.push_str(JS_WHITESPACE);
            out.push(']');
        }
        // 클래스 안의 `\b` 는 backspace 다.
        'b' if in_class => out.push_str(r"\x08"),
        'b' => out.push_str(&js_word_boundary(false)),
        'B' if in_class => out.push('B'),
        'B' => out.push_str(&js_word_boundary(true)),
        '0' => out.push_str(r"\x00"),
        // Property escapes require `u` in JavaScript. Without it, the slash is an
        // identity escape and the expression starts with a literal p/P.
        'p' | 'P' if !unicode => out.push(escaped),
        'p' | 'P' | 'n' | 'r' | 't' | 'v' | 'f' | 'x' | 'u' | 'k' | '1'..='9' => {
            out.push('\\');
            out.push(escaped);
        }
        // 그 밖의 영숫자와 비 ASCII 문자는 identity escape 라 글자 자체다 (`\z`, `\A`, `\한`).
        c if c.is_ascii_alphanumeric() || !c.is_ascii() => out.push(c),
        c => {
            out.push('\\');
            out.push(c);
        }
    }
}

type JsRegexCache = Mutex<HashMap<(String, bool, bool), Regex>>;
static JS_REGEX_CACHE: LazyLock<JsRegexCache> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// JavaScript RegExp 소스를 옮겨 컴파일한다. 사용자 패턴은 파일마다 같으므로 소스와 플래그로 캐시한다.
/// 실패하면 V8 `SyntaxError` 의 사유 문구를 돌려준다.
pub(crate) fn compile_js_regex(
    source: &str,
    unicode: bool,
    ignore_case: bool,
) -> Result<Regex, String> {
    let key = (source.to_string(), unicode, ignore_case);
    if let Some(re) = JS_REGEX_CACHE.lock().expect("js regex cache").get(&key) {
        return Ok(re.clone());
    }
    let mut translated = translate_js_pattern(source, unicode)?;
    if ignore_case {
        translated.insert_str(0, "(?i)");
    }
    let re = Regex::new(&translated).map_err(|error| js_regex_error_reason(&error))?;
    JS_REGEX_CACHE
        .lock()
        .expect("js regex cache")
        .insert(key, re.clone());
    Ok(re)
}

/// 흔한 컴파일 오류는 V8 의 사유 문구로 옮기고, 나머지는 fancy_regex 의 설명을 그대로 쓴다.
fn js_regex_error_reason(error: &fancy_regex::Error) -> String {
    use fancy_regex::{CompileError, Error, ParseError};
    match error {
        Error::ParseError(_, ParseError::UnclosedOpenParen) => "Unterminated group".to_string(),
        Error::ParseError(_, ParseError::InvalidClass) => {
            "Unterminated character class".to_string()
        }
        Error::ParseError(_, ParseError::TargetNotRepeatable) => "Nothing to repeat".to_string(),
        Error::ParseError(_, ParseError::TrailingBackslash) => "\\ at end of pattern".to_string(),
        Error::ParseError(_, ParseError::GeneralParseError(message))
            if message == "end of string not reached" =>
        {
            "Unmatched ')'".to_string()
        }
        // regex 의 오류 종류는 Debug 표기로만 드러난다.
        Error::CompileError(compile_error)
            if matches!(**compile_error, CompileError::InnerError(ref inner)
                if format!("{inner:?}").contains("ClassRangeInvalid")) =>
        {
            "Range out of order in character class".to_string()
        }
        other => other.to_string(),
    }
}

/// V8 의 `SyntaxError` 문구: `Invalid regular expression: /src/flags: reason`.
pub(crate) fn js_regex_error_message(source: &str, flags: &str, reason: &str) -> String {
    format!("Invalid regular expression: /{source}/{flags}: {reason}")
}

/// 사용자 `frontMatter` 패턴은 cli2처럼 Unicode 플래그를 켠 JavaScript RegExp로 컴파일한다.
pub fn compile_js_pattern(pattern: &str) -> Result<Regex, String> {
    compile_js_regex(pattern, true, false)
        .map_err(|reason| js_regex_error_message(pattern, "u", &reason))
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

    #[test]
    fn javascript_ascii_character_classes() {
        let re = compile_js_pattern(r"^\w+\s+\d+$").unwrap();
        assert!(re.is_match("abc_ \u{feff}42").unwrap());
        assert!(!re.is_match("한글 42").unwrap());
        assert!(!re.is_match("abc \u{85}42").unwrap());

        let non_word = compile_js_pattern(r"^[\W]$").unwrap();
        assert!(non_word.is_match("한").unwrap());
        assert!(!non_word.is_match("a").unwrap());
        let non_digit = compile_js_pattern(r"^[\D]$").unwrap();
        assert!(non_digit.is_match("a").unwrap());
        assert!(!non_digit.is_match("1").unwrap());
        let non_space = compile_js_pattern(r"^[\S]$").unwrap();
        assert!(non_space.is_match("\u{85}").unwrap());
        assert!(!non_space.is_match("\u{feff}").unwrap());
    }

    #[test]
    fn javascript_any_character_class() {
        let re = compile_js_pattern(r"^[^]+$").unwrap();
        assert!(re.is_match("a\nb").unwrap());
    }

    #[test]
    fn javascript_dot_and_word_boundary_are_not_unicode() {
        let dot = compile_js_pattern(r"^a.b$").unwrap();
        assert!(dot.is_match("a b").unwrap());
        assert!(!dot.is_match("a\rb").unwrap());
        assert!(!dot.is_match("a\u{2028}b").unwrap());
        let boundary = compile_js_regex(r"\ba$", false, false).unwrap();
        assert!(boundary.is_match("한글a").unwrap());
        assert!(!boundary.is_match("xa").unwrap());
        let non_boundary = compile_js_regex(r"\Ba$", false, false).unwrap();
        assert!(non_boundary.is_match("xa").unwrap());
        assert!(!non_boundary.is_match("한글a").unwrap());
    }

    #[test]
    fn javascript_class_syntax_is_escaped_for_rust() {
        let assert_matches = |source: &str, yes: &[&str], no: &[&str]| {
            let re = compile_js_regex(source, false, false).unwrap();
            for text in yes {
                assert!(re.is_match(text).unwrap(), "{source} should match {text:?}");
            }
            for text in no {
                assert!(
                    !re.is_match(text).unwrap(),
                    "{source} should not match {text:?}"
                );
            }
        };
        assert_matches(r"^[[]$", &["["], &["]"]);
        assert_matches(r"^[]$", &[], &["", "a"]);
        assert_matches(r"^[^]]$", &["a]"], &["]"]);
        assert_matches(r"^[+--]$", &["+", ",", "-"], &["."]);
        assert_matches(r"^[a-b-c]$", &["a", "b", "-", "c"], &["d"]);
        assert_matches(r"^[\w-!]$", &["a", "-", "!"], &["한"]);
        assert_matches(r"^[&&]$", &["&"], &[]);
        assert_matches(r"^[\b]$", &["\u{8}"], &["b"]);
        assert_matches(r"^\z$", &["z"], &[]);
        assert_matches(r"^\p{L}$", &["p{L}"], &["a"]);
    }

    #[test]
    fn annex_b_braces_and_control_escapes() {
        let re = compile_js_regex(r"^a{$", false, false).unwrap();
        assert!(re.is_match("a{").unwrap());
        let re = compile_js_regex(r"^a{,2}$", false, false).unwrap();
        assert!(re.is_match("a{,2}").unwrap());
        let re = compile_js_regex(r"^\cA$", false, false).unwrap();
        assert!(re.is_match("\u{1}").unwrap());
        let re = compile_js_regex(r"^\c1$", false, false).unwrap();
        assert!(re.is_match("\\c1").unwrap());
        assert!(compile_js_regex(r"a*?", false, false).is_ok());
        assert!(compile_js_regex(r"(?:a)*b+?", false, false).is_ok());
        for source in ["x{2}{3}", "x{2}+", "a*??", "a{2}*"] {
            assert_eq!(
                compile_js_regex(source, false, false).unwrap_err(),
                "Nothing to repeat"
            );
        }
    }

    #[test]
    fn invalid_patterns_get_v8_reasons() {
        for (source, reason) in [
            ("(", "Unterminated group"),
            ("[", "Unterminated character class"),
            ("*", "Nothing to repeat"),
            (")", "Unmatched ')'"),
            ("\\", "\\ at end of pattern"),
            ("[b-a]", "Range out of order in character class"),
            ("a{2,1}", "numbers out of order in {} quantifier"),
        ] {
            assert_eq!(compile_js_regex(source, false, false).unwrap_err(), reason);
        }
        assert_eq!(
            js_regex_error_message("(", "i", "Unterminated group"),
            "Invalid regular expression: /(/i: Unterminated group"
        );
    }
}
