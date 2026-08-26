use std::fmt::Display;

use crate::rules::RuleMeta;

/// JS `String.length`: 컬럼과 같은 UTF-16 단위.
fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FixInfo {
    pub line_number: Option<usize>,
    pub edit_column: Option<usize>,
    pub delete_count: Option<isize>,
    pub insert_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LintError {
    pub line_number: usize,
    pub rule_names: Vec<String>,
    pub rule_description: String,
    pub rule_information: String,
    pub error_detail: Option<String>,
    pub error_context: Option<String>,
    pub error_range: Option<(usize, usize)>,
    pub fix_info: Option<FixInfo>,
    pub severity: Severity,
}

/// helpers.cjs `ellipsify`: 30자 초과 시 중요한 쪽을 남기고 "..." 처리.
pub fn ellipsify(text: &str, start: bool, end: bool) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= 30 {
        text.to_string()
    } else if start && end {
        let head: String = chars[..15].iter().collect();
        let tail: String = chars[chars.len() - 15..].iter().collect();
        format!("{head}...{tail}")
    } else if end {
        let tail: String = chars[chars.len() - 30..].iter().collect();
        format!("...{tail}")
    } else {
        let head: String = chars[..30].iter().collect();
        format!("{head}...")
    }
}

/// helpers.cjs `newLineRe` (`/\r\n?|\n/g`) 치환.
fn replace_newlines(text: &str, replacement: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', replacement)
}

pub struct ErrorSink<'a> {
    name: &'a str,
    lines: &'a [&'a str],
    meta: &'static RuleMeta,
    front_matter_lines: usize,
    severity: Severity,
    errors: Vec<LintError>,
}

impl<'a> ErrorSink<'a> {
    pub fn new(
        name: &'a str,
        lines: &'a [&'a str],
        meta: &'static RuleMeta,
        front_matter_lines: usize,
        severity: Severity,
    ) -> Self {
        Self {
            name,
            lines,
            meta,
            front_matter_lines,
            severity,
            errors: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(lines: &'a [&'a str]) -> Self {
        static TEST_META: RuleMeta = RuleMeta {
            names: &["MD000", "test-rule"],
            description: "Test rule",
            tags: &[],
            needs_tokens: false,
            fixable: false,
        };
        Self::new("test", lines, &TEST_META, 0, Severity::Error)
    }

    pub fn errors(&self) -> &[LintError] {
        &self.errors
    }

    fn fail(&self, property: &str) -> ! {
        panic!(
            "Value of '{}' passed to onError by '{}' is incorrect for '{}'.",
            property, self.meta.names[0], self.name
        );
    }

    /// markdownlint.mjs `onError` 검증과 수집.
    pub fn add_error(
        &mut self,
        line: usize,
        detail: Option<&str>,
        context: Option<&str>,
        range: Option<(usize, usize)>,
        fix: Option<FixInfo>,
    ) {
        if line < 1 || line > self.lines.len() {
            self.fail("lineNumber");
        }
        let line_number = line + self.front_matter_lines;
        if let Some((column, length)) = range
            && (column < 1 || length < 1 || column + length - 1 > utf16_len(self.lines[line - 1]))
        {
            self.fail("range");
        }
        let fix_info = fix.map(|fix| {
            let clean_line_number = fix.line_number.map(|n| {
                if n < 1 || n > self.lines.len() {
                    self.fail("fixInfo.lineNumber");
                }
                n + self.front_matter_lines
            });
            let effective_line = fix.line_number.unwrap_or(line);
            let line_len = utf16_len(self.lines[effective_line - 1]);
            if let Some(edit_column) = fix.edit_column
                && (edit_column < 1 || edit_column > line_len + 1)
            {
                self.fail("fixInfo.editColumn");
            }
            if let Some(delete_count) = fix.delete_count
                && (delete_count < -1 || delete_count > line_len as isize)
            {
                self.fail("fixInfo.deleteCount");
            }
            FixInfo {
                line_number: clean_line_number,
                ..fix
            }
        });
        self.errors.push(LintError {
            line_number,
            rule_names: self.meta.names.iter().map(|n| n.to_string()).collect(),
            rule_description: self.meta.description.to_string(),
            rule_information: format!(
                "https://github.com/DavidAnson/markdownlint/blob/v0.40.0/doc/{}.md",
                self.meta.names[0].to_lowercase()
            ),
            error_detail: detail
                .map(|d| replace_newlines(d, " "))
                .filter(|d| !d.is_empty()),
            error_context: context
                .map(|c| replace_newlines(c, " "))
                .filter(|c| !c.is_empty()),
            error_range: range,
            fix_info,
            severity: self.severity,
        });
    }

    /// helpers.cjs `addErrorDetailIf`.
    #[allow(clippy::too_many_arguments)]
    pub fn add_error_detail_if(
        &mut self,
        line: usize,
        expected: impl Display,
        actual: impl Display,
        detail: Option<&str>,
        context: Option<&str>,
        range: Option<(usize, usize)>,
        fix: Option<FixInfo>,
    ) {
        let expected = expected.to_string();
        let actual = actual.to_string();
        if expected != actual {
            let suffix = detail.map(|d| format!("; {d}")).unwrap_or_default();
            let detail = format!("Expected: {expected}; Actual: {actual}{suffix}");
            self.add_error(line, Some(&detail), context, range, fix);
        }
    }

    /// helpers.cjs `addErrorContext`.
    pub fn add_error_context(
        &mut self,
        line: usize,
        context: &str,
        start: bool,
        end: bool,
        range: Option<(usize, usize)>,
        fix: Option<FixInfo>,
    ) {
        let context = ellipsify(&replace_newlines(context, "\n"), start, end);
        self.add_error(line, None, Some(&context), range, fix);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ellipsify_rules() {
        let long = "abcdefghijklmnopqrstuvwxyz0123456789";
        assert_eq!(ellipsify("short", true, true), "short");
        assert_eq!(
            ellipsify(long, true, true),
            "abcdefghijklmno...vwxyz0123456789"
        );
        assert_eq!(
            ellipsify(long, false, true),
            "...ghijklmnopqrstuvwxyz0123456789"
        );
        assert_eq!(
            ellipsify(long, false, false),
            "abcdefghijklmnopqrstuvwxyz0123..."
        );
    }

    #[test]
    fn detail_if_only_when_different() {
        let mut sink = ErrorSink::for_test(&["a"]);
        sink.add_error_detail_if(1, 1, 1, None, None, None, None);
        assert!(sink.errors().is_empty());
        sink.add_error_detail_if(1, 1, 2, None, None, None, None);
        assert_eq!(
            sink.errors()[0].error_detail.as_deref(),
            Some("Expected: 1; Actual: 2")
        );
    }

    #[test]
    fn range_validation_panics_out_of_bounds() {
        let mut sink = ErrorSink::for_test(&["abc"]);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            sink.add_error(1, None, None, Some((3, 2)), None)
        }));
        assert!(r.is_err());
    }
}
