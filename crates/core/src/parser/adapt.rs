//! markdown-rs 토큰 트리를 micromark(JS, markdownlint 가 사용하는 형태) 토큰 트리로 변환.

#[derive(Debug, Clone)]
pub(super) struct Node {
    pub kind: String,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub children: Vec<Node>,
}

impl Node {
    fn wrapper(kind: &str, inner: Node) -> Node {
        Node {
            kind: kind.to_string(),
            start_line: inner.start_line,
            start_column: inner.start_column,
            end_line: inner.end_line,
            end_column: inner.end_column,
            start: inner.start,
            end: inner.end,
            text: inner.text.clone(),
            children: vec![inner],
        }
    }
}

/// markdown-rs `Name` (Debug 문자열) → micromark 타입 이름
fn rename(name: &str) -> &str {
    match name {
        "HeadingAtx" => "atxHeading",
        "HeadingAtxSequence" => "atxHeadingSequence",
        "HeadingAtxText" => "atxHeadingText",
        "HeadingSetext" => "setextHeading",
        "HeadingSetextText" => "setextHeadingText",
        "HeadingSetextUnderline" => "setextHeadingLine",
        "HeadingSetextUnderlineSequence" => "setextHeadingLineSequence",
        "BlankLineEnding" => "lineEndingBlank",
        "CharacterEscapeMarker" => "escapeMarker",
        "CharacterReferenceMarkerSemi" => "characterReferenceMarker",
        "CodeFlowChunk" => "codeFlowValue",
        "MathFlowChunk" => "mathFlowValue",
        "GfmTable" => "table",
        "GfmTableBody" => "tableBody",
        "GfmTableHead" => "tableHead",
        "GfmTableRow" => "tableRow",
        "GfmTableCellDivider" => "tableCellDivider",
        "GfmTableCellText" => "tableContent",
        "GfmTableDelimiterRow" => "tableDelimiterRow",
        "GfmTableDelimiterCell" => "tableDelimiter",
        "GfmTableDelimiterFiller" => "tableDelimiterFiller",
        "GfmTableDelimiterMarker" => "tableDelimiterMarker",
        "GfmTableCell" => "tableCell",
        "SpaceOrTab" => "spaceOrTab",
        _ => "",
    }
}

fn camel(name: &str) -> String {
    let mut s = String::with_capacity(name.len());
    let mut chars = name.chars();
    if let Some(c) = chars.next() {
        s.extend(c.to_lowercase());
    }
    s.extend(chars);
    s
}

const HOIST: &[&str] = &["labelLink", "labelImage", "labelEnd"];

pub(super) fn adapt(nodes: Vec<Node>, _text: &str, _line_delta: usize) -> Vec<Node> {
    let mut nodes = nodes;
    rename_all(&mut nodes);
    let mut nodes = restructure(nodes, "root");
    classify_whitespace(&mut nodes, "root");
    nodes
}

fn rename_all(nodes: &mut [Node]) {
    for n in nodes.iter_mut() {
        let r = rename(&n.kind);
        n.kind = if r.is_empty() { camel(&n.kind) } else { r.to_string() };
        rename_all(&mut n.children);
    }
}

/// 래퍼 제거(label*, listItem), content 래핑, 테이블 셀 분류, 리스트 범위 조정.
fn restructure(nodes: Vec<Node>, parent: &str) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::new();
    for mut n in nodes {
        let kind = n.kind.clone();
        n.children = restructure(std::mem::take(&mut n.children), &kind);
        match kind.as_str() {
            k if HOIST.contains(&k) => out.extend(n.children),
            "listItem" => {
                // 첫 아이템의 prefix 앞 공백은 리스트 밖(앞)으로 나간다. 호출부(list)에서 처리.
                for c in n.children.iter_mut() {
                    if c.kind == "spaceOrTab" {
                        c.kind = "listItemIndent".into();
                    }
                }
                // prefix 바로 앞 공백은 linePrefix
                let pos = n.children.iter().position(|c| c.kind == "listItemPrefix");
                if let Some(p) = pos {
                    if p > 0 && n.children[p - 1].kind == "listItemIndent" {
                        n.children[p - 1].kind = "linePrefix".into();
                    }
                }
                out.extend(n.children);
            }
            "listUnordered" | "listOrdered" => {
                // 리스트 직속 공백(아이템 사이 continuation) → listItemIndent
                for c in n.children.iter_mut() {
                    if c.kind == "spaceOrTab" {
                        c.kind = "listItemIndent".into();
                    }
                }
                // 첫 아이템 prefix 앞 linePrefix 는 리스트 앞으로
                let mut leading = Vec::new();
                while n.children.first().is_some_and(|c| c.kind == "linePrefix") {
                    leading.push(n.children.remove(0));
                }
                if let Some(f) = n.children.first() {
                    n.start_line = f.start_line;
                    n.start_column = f.start_column;
                    n.start = f.start;
                }
                // 끝의 줄바꿈은 리스트 밖으로
                let mut trailing = Vec::new();
                while n.children.last().is_some_and(|c| c.kind == "lineEnding" || c.kind == "lineEndingBlank") {
                    trailing.push(n.children.pop().unwrap());
                }
                trailing.reverse();
                if let Some(l) = n.children.last() {
                    n.end_line = l.end_line;
                    n.end_column = l.end_column;
                    n.end = l.end;
                }
                out.extend(leading);
                out.push(n);
                out.extend(trailing);
            }
            "paragraph" | "definition" if parent != "setextHeading" => out.push(Node::wrapper("content", n)),
            "tableCell" => {
                n.kind = if parent == "tableHead" { "tableHeader".into() } else { "tableData".into() };
                out.push(n);
            }
            _ => out.push(n),
        }
    }
    out
}

fn classify_whitespace(nodes: &mut [Node], parent: &str) {
    for i in 0..nodes.len() {
        let kind = nodes[i].kind.clone();
        classify_whitespace(&mut nodes[i].children, &kind);
        if kind != "spaceOrTab" {
            continue;
        }
        let prev = if i > 0 { nodes[i - 1].kind.as_str() } else { "" };
        let next = nodes.get(i + 1).map(|n| n.kind.as_str()).unwrap_or("");
        let at_line_start = nodes[i].start_column == 1
            || matches!(prev, "blockQuotePrefix" | "listItemIndent" | "linePrefix")
            || prev == "lineEnding";
        let new = match parent {
            "listItemPrefix" => "listItemPrefixWhitespace",
            "blockQuotePrefix" => "blockQuotePrefixWhitespace",
            _ if at_line_start && next != "lineEnding" && next != "" => "linePrefix",
            _ if next == "lineEnding" || next == "" => "lineSuffix",
            _ => "whitespace",
        };
        nodes[i].kind = new.into();
    }
}
