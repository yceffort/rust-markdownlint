//! 배너, 도움말, 결과 정렬과 기본 포매터 (`markdownlint-cli2-formatter-default`).
//! 배너를 제외한 문자열은 원본과 바이트 단위로 같다.

use std::cmp::Ordering;
use std::path::{Component, Path};

use rust_markdownlint::error::{LintError, Severity};

pub const BANNER: &str = concat!(
    "rust-markdownlint v",
    env!("CARGO_PKG_VERSION"),
    " (markdownlint-cli2 v0.22.1 / markdownlint v0.40.0 compatible)"
);

/// 원본 `showHelp` 본문 (배너 제외).
pub const HELP: &str = r##"https://github.com/DavidAnson/markdownlint-cli2

Syntax: markdownlint-cli2 glob0 [glob1] [...] [globN] [--config file] [--configPointer pointer] [--fix] [--format] [--help] [--no-globs]

Glob expressions (from the globby library):
- * matches any number of characters, but not /
- ? matches a single character, but not /
- ** matches any number of characters, including /
- {} allows for a comma-separated list of "or" expressions
- ! or # at the beginning of a pattern negate the match
- : at the beginning identifies a literal file path
- - as a glob represents standard input (stdin)

Dot-only glob:
- The command "markdownlint-cli2 ." would lint every file in the current directory tree which is probably not intended
- Instead, it is mapped to "markdownlint-cli2 *.{md,markdown}" which lints all Markdown files in the current directory
- To lint every file in the current directory tree, the command "markdownlint-cli2 **" can be used instead

Optional parameters:
- --config        specifies the path to a configuration file to define the base configuration
- --configPointer specifies a JSON Pointer to a configuration object within the --config file
- --fix           updates files to resolve fixable issues (can be overridden in configuration)
- --format        reads standard input (stdin), applies fixes, writes standard output (stdout)
- --help          writes this message to the console and exits without doing anything else
- --no-globs      ignores the "globs" property if present in the top-level options object

Configuration via:
- .markdownlint-cli2.jsonc
- .markdownlint-cli2.yaml
- .markdownlint-cli2.cjs or .markdownlint-cli2.mjs
- .markdownlint.jsonc or .markdownlint.json
- .markdownlint.yaml or .markdownlint.yml
- .markdownlint.cjs or .markdownlint.mjs

Cross-platform compatibility:
- UNIX and Windows shells expand globs according to different rules; quoting arguments is recommended
- Some Windows shells don't handle single-quoted (') arguments well; double-quote (") is recommended
- Shells that expand globs do not support negated patterns (!node_modules); quoting is required here
- Some UNIX shells parse exclamation (!) in double-quotes; hashtag (#) is recommended in these cases
- The path separator is forward slash (/) on all platforms; backslash (\) is automatically converted
- On any platform, passing the parameter "--" causes all remaining parameters to be treated literally

The most compatible syntax for cross-platform support:
$ markdownlint-cli2 "**/*.md" "#node_modules""##;

/// 원본 `createResults` 의 항목: base 기준 posix 상대 경로와 에러.
pub struct LintResult {
    pub file_name: String,
    pub error: LintError,
}

/// `path.posix.relative(base, path)`. base 밖의 경로는 `..` 로 시작한다.
pub fn relative_posix(base: &Path, path: &Path) -> String {
    let base: Vec<Component> = base.components().collect();
    let path: Vec<Component> = path.components().collect();
    let common = base.iter().zip(&path).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".to_string(); base.len() - common];
    parts.extend(
        path[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}

/// CLDR root collation 에서 ASCII 구두점의 1차 순서 (공백 다음, 숫자 앞).
const PUNCTUATION_ORDER: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";

fn primary_key(c: char) -> (u8, u32) {
    if c.is_whitespace() {
        (0, c as u32)
    } else if let Some(i) = PUNCTUATION_ORDER.find(c) {
        (1, i as u32)
    } else if c.is_ascii_digit() {
        (2, c as u32)
    } else if c.is_ascii_alphabetic() {
        (3, c.to_ascii_lowercase() as u32)
    } else {
        (4, c as u32)
    }
}

/// JS `String.prototype.localeCompare` (ICU root) 근사: 1차 키는 공백 < 구두점 < 숫자 < 문자
/// (대소문자 무시), 1차가 같으면 소문자 우선. ASCII 이외 문자는 코드 포인트 순.
pub fn locale_compare(a: &str, b: &str) -> Ordering {
    let primary = |s: &str| s.chars().map(primary_key).collect::<Vec<_>>();
    primary(a).cmp(&primary(b)).then_with(|| {
        let case = |s: &str| s.chars().map(|c| c.is_uppercase()).collect::<Vec<_>>();
        case(a).cmp(&case(b))
    })
}

/// 원본 `createResults` 정렬: 파일명 localeCompare → 줄 → 규칙명 (안정 정렬로 입력 순서 유지).
pub fn sort_results(results: &mut [LintResult]) {
    results.sort_by(|a, b| {
        locale_compare(&a.file_name, &b.file_name)
            .then(a.error.line_number.cmp(&b.error.line_number))
            .then_with(|| locale_compare(a.error.rule_names[0], b.error.rule_names[0]))
    });
}

/// `markdownlint-cli2-formatter-default` 한 줄.
pub fn format_result(result: &LintResult) -> String {
    let e = &result.error;
    let column = match e.error_range {
        Some((start, _)) if start > 0 => format!(":{start}"),
        _ => String::new(),
    };
    let severity = match e.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    let detail = e
        .error_detail
        .as_ref()
        .filter(|d| !d.is_empty())
        .map(|d| format!(" [{d}]"))
        .unwrap_or_default();
    let context = e
        .error_context
        .as_ref()
        .filter(|c| !c.is_empty())
        .map(|c| format!(" [Context: \"{c}\"]"))
        .unwrap_or_default();
    format!(
        "{}:{}{column} {severity} {} {}{detail}{context}",
        result.file_name,
        e.line_number,
        e.rule_names.join("/"),
        e.rule_description
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_compare_matches_icu_oracle() {
        // node 의 localeCompare 로 정렬한 결과
        let expected = [
            "0x",
            "9x",
            "a b",
            "a c",
            "a_b",
            "a-b",
            "a-c",
            "a.b/c.md",
            "a.md",
            "A.md",
            "a/b.md",
            "a1.md",
            "a10.md",
            "a2.md",
            "ab",
            "aB",
            "Ab",
            "AB",
            "ab.md",
            "B.md",
            "e.md",
            "f.md",
            "README.md",
            "sub/a.md",
            "Zed.md",
        ];
        let mut actual = expected;
        actual.reverse();
        actual.sort_by(|a, b| locale_compare(a, b));
        assert_eq!(actual, expected);
    }

    #[test]
    fn relative_paths() {
        let base = Path::new("/r/base");
        assert_eq!(relative_posix(base, Path::new("/r/base/a/b.md")), "a/b.md");
        assert_eq!(
            relative_posix(base, Path::new("/r/other/b.md")),
            "../other/b.md"
        );
        assert_eq!(relative_posix(base, Path::new("/r/base")), "");
    }
}
