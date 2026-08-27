use rust_markdownlint::parser::parse;

#[test]
fn atx_heading_tokens() {
    let tree = parse("# Hello\n");
    let heads = tree.filter_by_types(&["atxHeading"]);
    assert_eq!(heads.len(), 1);
    let h = tree.get(heads[0]);
    assert_eq!(
        (h.start_line, h.start_column, h.end_line, h.end_column),
        (1, 1, 1, 8)
    );
    let seq = tree.descendants_by_type(heads[0], &[&["atxHeadingSequence"]]);
    assert_eq!(tree.text(seq[0]), "#");
    assert_eq!(tree.parent_of_type(seq[0], &["atxHeading"]), Some(heads[0]));
}

#[test]
fn column_is_codepoint_based() {
    let tree = parse("# 한글\n");
    let h = tree.get(tree.filter_by_types(&["atxHeading"])[0]);
    assert_eq!(h.end_column, 5);
}

#[test]
fn html_flow_reparse_positions_same_for_crlf() {
    // htmlFlow 재파싱은 줄 범위로 잘라내므로 CRLF 를 줄바꿈 하나로 세야 한다
    let lf = "<p>\n\nblock <em>b</em> block\n\n</p>\n\n<details>\n\n\t<details>\n";
    let crlf = lf.replace('\n', "\r\n");
    let positions = |text: &str| -> Vec<(usize, usize, usize, usize)> {
        let tree = parse(text);
        tree.filter_by_types_html_flow(&["htmlText"], true)
            .into_iter()
            .map(|id| {
                let t = tree.get(id);
                (t.start_line, t.start_column, t.end_line, t.end_column)
            })
            .collect()
    };
    let expected = positions(lf);
    // `<em>` on line 3
    assert!(expected.contains(&(3, 7, 3, 11)), "{expected:?}");
    assert_eq!(positions(&crlf), expected);
}

#[test]
fn nested_list_matches_inside_match() {
    let tree = parse("- a\n  - b\n");
    assert_eq!(tree.filter_by_types(&["listUnordered"]).len(), 2);
}

/// 원본 markdownlint(micromark JS) 토큰 덤프와 대조.
/// 기대값은 `scripts/compare-tokens.py` 오라클의 `dump-tokens.mjs` 로 생성했다.
#[test]
fn matches_micromark_token_dumps() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/tokens");
    for name in [
        "atx_heading_spacing",
        "bulleted_list_not_at_beginning_of_line",
        "code-blocks-and-spans",
        "simple-table",
        "empty-links",
        "list-syntax-in-paragraph-text",
        "prefix-whitespace-in-containers",
    ] {
        let md = std::fs::read_to_string(format!("{dir}/{name}.md")).unwrap();
        let expected: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(format!("{dir}/{name}.md.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(parse(&md).to_json(), expected, "{name}");
    }
}
