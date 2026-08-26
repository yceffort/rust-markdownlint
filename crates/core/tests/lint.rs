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
        .map(|e| (e.rule_names[0].as_str(), e.line_number))
        .collect();
    assert_eq!(names, [("MD018", 1), ("MD018", 2), ("MD047", 3)]);
}

#[test]
fn bom_is_stripped() {
    let errs = lint_content("a.md", "\u{FEFF}# a\n\ntext\n", &LintOptions::default()).unwrap();
    assert!(errs.is_empty());
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
    let config = serde_json::json!({"MD047": false});
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
