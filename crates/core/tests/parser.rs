use rust_markdownlint::parser::parse;

#[test]
fn atx_heading_tokens() {
    let tree = parse("# Hello\n");
    let heads = tree.filter_by_types(&["atxHeading"]);
    assert_eq!(heads.len(), 1);
    let h = tree.get(heads[0]);
    assert_eq!((h.start_line, h.start_column, h.end_line, h.end_column), (1, 1, 1, 8));
    let seq = tree.descendants_by_type(heads[0], &["atxHeadingSequence"]);
    assert_eq!(tree.get(seq[0]).text, "#");
    assert_eq!(tree.parent_of_type(seq[0], &["atxHeading"]), Some(heads[0]));
}

#[test]
fn column_is_codepoint_based() {
    let tree = parse("# 한글\n");
    let h = tree.get(tree.filter_by_types(&["atxHeading"])[0]);
    assert_eq!(h.end_column, 5);
}

#[test]
fn nested_list_matches_inside_match() {
    let tree = parse("- a\n  - b\n");
    assert_eq!(tree.filter_by_types(&["listUnordered"]).len(), 2);
}
