use rust_markdownlint::lint::{LintOptions, lint_content};

#[test]
fn front_matter_offsets_line_numbers() {
    let errs = lint_content("a.md", "---\nt: 1\n---\n#x", &LintOptions::default()).unwrap();
    let md018 = errs.iter().find(|e| e.rule_names[0] == "MD018").unwrap();
    assert_eq!(md018.line_number, 4);
}

#[test]
fn html_comment_body_cleared_but_tokens_from_raw() {
    let errs = lint_content("a.md", "<!-- #x -->\n", &LintOptions::default()).unwrap();
    assert!(errs.iter().all(|e| e.rule_names[0] != "MD018"));
}

#[test]
fn sorted_by_rule_then_line() {
    let errs = lint_content("a.md", "#a\n#b\ntext", &LintOptions::default()).unwrap();
    let names: Vec<_> = errs
        .iter()
        .map(|e| (e.rule_names[0], e.line_number))
        .collect();
    assert_eq!(
        names,
        [("MD018", 1), ("MD018", 2), ("MD041", 1), ("MD047", 3)]
    );
}

#[test]
fn bom_is_stripped() {
    let errs = lint_content("a.md", "\u{FEFF}# a\n\ntext\n", &LintOptions::default()).unwrap();
    assert!(errs.is_empty());
}

#[test]
fn nul_is_punctuation_for_emphasis_like_micromark() {
    // micromark preprocess 는 NUL 을 U+FFFD(구두점) 로 바꾼 뒤 토크나이즈하므로 `_` 뒤의 NUL 이 강조를 닫는다.
    // 컬럼은 원본 기준(NUL 도 U+FFFD 도 UTF-16 1단위)이다.
    let errs = lint_content("a.md", "# T\n\n*a* _y_\0 z\n", &LintOptions::default()).unwrap();
    let md049: Vec<_> = errs
        .iter()
        .filter(|e| e.rule_names[0] == "MD049")
        .map(|e| (e.line_number, e.error_range.map(|r| r.0)))
        .collect();
    assert_eq!(md049, [(3, Some(5)), (3, Some(7))]);
}

#[test]
fn nul_stays_in_token_text() {
    // 원본 token.text 는 markdown.slice(...) 라 NUL 이 그대로 남는다 (MD038 컨텍스트 `a^@ `)
    let errs = lint_content("a.md", "# T\n\n`a\0 `\n", &LintOptions::default()).unwrap();
    let md038: Vec<_> = errs
        .iter()
        .filter(|e| e.rule_names[0] == "MD038")
        .map(|e| e.error_context.as_deref())
        .collect();
    assert_eq!(md038, [Some("`a\0 `")]);
}

#[test]
fn inline_disable_drops_errors() {
    let errs = lint_content(
        "a.md",
        "<!-- markdownlint-disable MD018 -->\n#x\n",
        &LintOptions::default(),
    )
    .unwrap();
    assert!(errs.iter().all(|e| e.rule_names[0] != "MD018"));
}

#[test]
fn config_disables_rule() {
    let config = serde_json::json!({"MD041": false, "MD047": false});
    let opts = LintOptions {
        config: Some(&config),
        ..Default::default()
    };
    let errs = lint_content("a.md", "text", &opts).unwrap();
    assert!(errs.is_empty());
}

#[test]
fn no_inline_config_option() {
    let opts = LintOptions {
        no_inline_config: true,
        ..Default::default()
    };
    let errs = lint_content("a.md", "<!-- markdownlint-disable MD018 -->\n#x\n", &opts).unwrap();
    assert!(errs.iter().any(|e| e.rule_names[0] == "MD018"));
}

#[test]
fn user_front_matter_pattern() {
    let opts = LintOptions {
        front_matter: Some(r"(?s)^<!--.*?-->\n"),
        ..Default::default()
    };
    let errs = lint_content("a.md", "<!-- meta -->\n#x\n", &opts).unwrap();
    let md018 = errs.iter().find(|e| e.rule_names[0] == "MD018").unwrap();
    assert_eq!(md018.line_number, 2);
}

#[test]
fn invalid_front_matter_pattern_is_error() {
    let opts = LintOptions {
        front_matter: Some(r"(unclosed"),
        ..Default::default()
    };
    assert!(lint_content("a.md", "# a\n", &opts).is_err());
}
