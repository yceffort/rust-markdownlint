use super::{LintContext, Rule, RuleMeta};
use crate::config::{js_string, truthy};
use crate::error::{ErrorSink, FixInfo};

pub(crate) struct Md049;

static META: RuleMeta = RuleMeta {
    names: &["MD049", "emphasis-style"],
    description: "Emphasis style",
    tags: &["emphasis"],
    needs_tokens: true,
    fixable: true,
};

/// 원본 `intrawordRe` (`/^\w$/`): 줄의 `index` 번째 UTF-16 단위가 ASCII 단어 문자인지.
/// JS 에서 범위 밖 인덱스는 `undefined` 라 false 가 된다.
fn is_intraword(line: &str, index: isize) -> bool {
    usize::try_from(index)
        .ok()
        .and_then(|index| line.encode_utf16().nth(index))
        .is_some_and(|unit| {
            u8::try_from(unit).is_ok_and(|b| b.is_ascii_alphanumeric() || b == b'_')
        })
}

/// 원본 `emphasisOrStrongStyleFor`: 강조 마크업 문자열의 스타일 이름.
fn emphasis_or_strong_style_for(markup: &str) -> &'static str {
    match markup.chars().next() {
        Some('*') => "asterisk",
        _ => "underscore",
    }
}

/// 원본 `params.config.style || undefined` 후 기본값 "consistent".
pub(super) fn style_config(ctx: &LintContext) -> String {
    match ctx.config.get("style") {
        Some(value) if truthy(value) => js_string(value),
        _ => "consistent".to_string(),
    }
}

/// 원본 `impl`: `kind` 토큰(emphasis/strong)의 시작과 끝 `sequence_kind` 마크업 스타일이
/// `style` 과 다르면 보고한다. MD049 와 MD050 이 공유한다.
pub(super) fn check_style(
    ctx: &LintContext,
    out: &mut ErrorSink,
    kind: &str,
    sequence_kind: &str,
    asterisk: &str,
    underline: &str,
    mut style: String,
) {
    let lines = ctx.lines;
    let tokens = ctx.tokens;
    let emphasis_tokens = tokens.filter_by_predicate(
        &tokens.roots,
        |tree, id| tree.get(id).kind == kind,
        |tree, id| {
            let token = tree.get(id);
            if token.kind == "htmlFlow" {
                Vec::new()
            } else {
                token.children.clone()
            }
        },
    );
    for token in emphasis_tokens {
        let sequences = tokens.descendants_by_type(token, &[&[sequence_kind]]);
        let start_sequence = sequences.first().map(|&id| tokens.get(id));
        let end_sequence = sequences.last().map(|&id| tokens.get(id));
        if let (Some(start_sequence), Some(end_sequence)) = (start_sequence, end_sequence) {
            let markup_style = emphasis_or_strong_style_for(tokens.text_of(start_sequence));
            if style == "consistent" {
                style = markup_style.to_string();
            }
            if style != markup_style {
                let underscore_intraword = style == "underscore"
                    && (is_intraword(
                        lines[start_sequence.start_line - 1],
                        start_sequence.start_column as isize - 2,
                    ) || is_intraword(
                        lines[end_sequence.end_line - 1],
                        end_sequence.end_column as isize - 1,
                    ));
                if !underscore_intraword {
                    for sequence in [start_sequence, end_sequence] {
                        let length = tokens.text_of(sequence).chars().count();
                        out.add_error(
                            sequence.start_line,
                            Some(&format!("Expected: {style}; Actual: {markup_style}")),
                            None,
                            Some((sequence.start_column, length)),
                            Some(FixInfo {
                                edit_column: Some(sequence.start_column),
                                delete_count: Some(length as isize),
                                insert_text: Some(
                                    if style == "asterisk" {
                                        asterisk
                                    } else {
                                        underline
                                    }
                                    .to_string(),
                                ),
                                ..Default::default()
                            }),
                        );
                    }
                }
            }
        }
    }
}

impl Rule for Md049 {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn check(&self, ctx: &LintContext, out: &mut ErrorSink) {
        check_style(
            ctx,
            out,
            "emphasis",
            "emphasisSequence",
            "*",
            "_",
            style_config(ctx),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::lint::{LintOptions, lint_content};
    use crate::rules::lint_rule;
    use serde_json::json;

    fn lint_with(params: serde_json::Value, content: &str) -> Vec<crate::error::LintError> {
        let config = json!({ "default": false, "MD049": params });
        let opts = LintOptions {
            config: Some(&config),
            ..Default::default()
        };
        lint_content("test.md", content, &opts).unwrap()
    }

    #[test]
    fn md049_consistent_reports_second_style() {
        let errs = lint_rule("MD049", "*one* and _two_\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: asterisk; Actual: underscore")
        );
        assert_eq!(errs[0].error_range, Some((11, 1)));
        assert_eq!(errs[1].error_range, Some((15, 1)));
        let fix = errs[0].fix_info.as_ref().unwrap();
        assert_eq!(fix.edit_column, Some(11));
        assert_eq!(fix.delete_count, Some(1));
        assert_eq!(fix.insert_text.as_deref(), Some("*"));
    }

    #[test]
    fn md049_consistent_same_style_is_ok() {
        assert!(lint_rule("MD049", "*one* and *two*\n").is_empty());
        assert!(lint_rule("MD049", "_one_ and _two_\n").is_empty());
    }

    #[test]
    fn md049_explicit_style() {
        let errs = lint_with(json!({ "style": "underscore" }), "*one*\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: underscore; Actual: asterisk")
        );
        assert_eq!(
            errs[0].fix_info.as_ref().unwrap().insert_text.as_deref(),
            Some("_")
        );
        assert!(lint_with(json!({ "style": "asterisk" }), "*one*\n").is_empty());
    }

    #[test]
    fn md049_underscore_intraword_is_skipped() {
        assert!(lint_with(json!({ "style": "underscore" }), "a*b*c\n").is_empty());
        assert!(lint_with(json!({ "style": "underscore" }), "a*b* c\n").is_empty());
        assert_eq!(
            lint_with(json!({ "style": "underscore" }), "a *b* c\n").len(),
            2
        );
    }

    #[test]
    fn md049_ignores_strong_and_code() {
        assert!(lint_rule("MD049", "*one* and __two__\n").is_empty());
        assert!(lint_rule("MD049", "*one* and `_two_`\n").is_empty());
    }

    /// 컬럼은 JS 와 같은 UTF-16 단위라 이모지(서로게이트 쌍) 뒤에서는 코드 포인트 수보다 크다.
    #[test]
    fn md049_column_after_astral_char_is_utf16() {
        let errs = lint_with(json!({ "style": "underscore" }), "👉 *a*\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(errs[0].error_range, Some((4, 1)));
        assert_eq!(errs[1].error_range, Some((6, 1)));
    }

    #[test]
    fn md049_non_string_style() {
        let errs = lint_with(json!({ "style": 1 }), "*one*\n");
        assert_eq!(errs.len(), 2);
        assert_eq!(
            errs[0].error_detail.as_deref(),
            Some("Expected: 1; Actual: asterisk")
        );
    }
}
