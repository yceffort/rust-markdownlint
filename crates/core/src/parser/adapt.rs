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
        "GfmTableDelimiterCellValue" => "tableContent",
        "GfmAutolinkLiteralProtocol" => "literalAutolinkHttp",
        "GfmAutolinkLiteralWww" => "literalAutolinkWww",
        "GfmAutolinkLiteralEmail" => "literalAutolinkEmail",
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

pub(super) fn adapt(nodes: Vec<Node>, text: &str, line_delta: usize) -> Vec<Node> {
    let mut nodes = nodes;
    rename_all(&mut nodes);
    demote_autolinks(&mut nodes, text, false);
    let mut nodes = restructure(nodes, "root", false);
    fix_code_text(&mut nodes);
    classify_whitespace(&mut nodes, "root", &[]);
    extend_line_endings(&mut nodes, "root", false);
    fix_list_spans(&mut nodes);
    merge_content(&mut nodes);
    reparse_html_flow(&mut nodes, text, line_delta);
    nodes
}

/// micromark `resolveCodeText`: 인접 codeTextData 병합 후, 양끝이 공백/줄바꿈이고
/// 사이에 내용이 있으면 한 글자씩 codeTextPadding 으로 분리한다.
fn fix_code_text(nodes: &mut [Node]) {
    for n in nodes.iter_mut() {
        fix_code_text(&mut n.children);
    }
    for n in nodes.iter_mut() {
        if n.kind != "codeText" {
            continue;
        }
        let kids = &mut n.children;
        // 인접 codeTextData 병합
        let mut k = 0;
        while k + 1 < kids.len() {
            if kids[k].kind == "codeTextData"
                && kids[k + 1].kind == "codeTextData"
                && kids[k].end == kids[k + 1].start
            {
                let nx = kids.remove(k + 1);
                let cur = &mut kids[k];
                cur.end_line = nx.end_line;
                cur.end_column = nx.end_column;
                cur.end = nx.end;
                cur.text.push_str(&nx.text);
            } else {
                k += 1;
            }
        }
        // 패딩: 여는/닫는 시퀀스 안쪽 첫/끝 토큰 검사
        if kids.len() < 3 {
            continue;
        }
        let (h, t) = (1, kids.len() - 2);
        let head_pad = kids[h].kind == "lineEnding"
            || (kids[h].kind == "codeTextData" && kids[h].text.starts_with(' '));
        let tail_pad = kids[t].kind == "lineEnding"
            || (kids[t].kind == "codeTextData" && kids[t].text.ends_with(' '));
        if !head_pad || !tail_pad {
            continue;
        }
        // 패딩 1 글자씩 제외한 내부에 공백 아닌 내용이 있어야 한다
        let inner: String = kids[h..=t].iter().map(|c| c.text.as_str()).collect();
        if inner.len() < 2
            || inner[1..inner.len() - 1]
                .trim_matches([' ', '\n', '\r'])
                .is_empty()
        {
            continue;
        }
        // 꼬리 먼저 처리 (인덱스 보존)
        if kids[t].kind == "lineEnding" || kids[t].text == " " {
            kids[t].kind = "codeTextPadding".into();
        } else {
            let d = &mut kids[t];
            d.text.pop();
            d.end -= 1;
            d.end_column -= 1;
            let pad = Node {
                kind: "codeTextPadding".into(),
                start_line: d.end_line,
                start_column: d.end_column,
                end_line: d.end_line,
                end_column: d.end_column + 1,
                start: d.end,
                end: d.end + 1,
                text: " ".into(),
                children: Vec::new(),
            };
            kids.insert(t + 1, pad);
        }
        if kids[h].kind == "lineEnding" || kids[h].text == " " {
            kids[h].kind = "codeTextPadding".into();
        } else {
            let d = &mut kids[h];
            d.text.remove(0);
            let pad = Node {
                kind: "codeTextPadding".into(),
                start_line: d.start_line,
                start_column: d.start_column,
                end_line: d.start_line,
                end_column: d.start_column + 1,
                start: d.start,
                end: d.start + 1,
                text: " ".into(),
                children: Vec::new(),
            };
            d.start += 1;
            d.start_column += 1;
            kids.insert(h, pad);
        }
    }
}

/// GFM autolink literal 은 `[` 바로 뒤나 링크 라벨 안에서 허용되지 않는다 (micromark 는
/// resolver 로 제거한다). markdown-rs 는 허용하므로 data 로 되돌린다.
fn demote_autolinks(nodes: &mut [Node], text: &str, in_label: bool) {
    for n in nodes.iter_mut() {
        if n.kind.starts_with("literalAutolink")
            && (in_label || (n.start > 0 && text.as_bytes()[n.start - 1] == b'['))
        {
            n.kind = "data".into();
            n.children.clear();
        }
        let label = in_label || n.kind == "label";
        demote_autolinks(&mut n.children, text, label);
    }
}

/// micromark: 플로우 토크나이저가 줄 경계를 넘어 계속되는 도중 소비한 lineEnding 은
/// 다음 줄의 컨테이너 접두(blockQuotePrefix, listItemIndent) 끝까지 이어진다 (defineSkip).
/// 블록이 그 줄에서 끝났으면(다음이 새 블록/컨테이너) 이어지지 않는다.
fn extend_line_endings(nodes: &mut [Node], parent: &str, interrupt: bool) {
    for i in 0..nodes.len() {
        let kind = nodes[i].kind.clone();
        // table 이 문단을 인터럽트하며 시작했는지: tableHead 내부 lineEnding 확장 여부를 가른다
        let child_interrupt = if kind == "table" {
            let mut k = i;
            let mut found = false;
            while k > 0 {
                k -= 1;
                match nodes[k].kind.as_str() {
                    "blockQuotePrefix" | "listItemIndent" | "linePrefix" | "lineEnding" => {}
                    other => {
                        found = other == "content";
                        break;
                    }
                }
            }
            found
        } else if kind == "tableHead" {
            interrupt
        } else {
            false
        };
        extend_line_endings(&mut nodes[i].children, &kind, child_interrupt);
    }
    for i in 0..nodes.len() {
        if nodes[i].kind != "lineEnding" {
            continue;
        }
        let mut j = i + 1;
        let mut last_container = None;
        while nodes.get(j).is_some_and(|c| {
            matches!(
                c.kind.as_str(),
                "blockQuotePrefix" | "listItemIndent" | "linePrefix"
            )
        }) {
            if nodes[j].kind != "linePrefix" {
                last_container = Some(j);
            }
            j += 1;
        }
        let next = nodes.get(j).map(|c| c.kind.as_str()).unwrap_or("");
        // 다음 줄이 빈 줄(또는 부모의 끝)이면 flow linePrefix 까지 포함해 이어지고,
        // 플로우 내용이 이어지면 컨테이너 접두까지만 이어진다.
        let blank_next = next == "lineEndingBlank" || next.is_empty();
        let end_idx = if blank_next {
            last_container.map(|_| j - 1)
        } else {
            last_container
        };
        let Some(end_idx) = end_idx else { continue };
        let extend = match parent {
            "codeFenced" | "htmlFlow" => !next.is_empty(),
            "tableHead" => interrupt,
            // 문단이 다음 줄로 이어지는지 확인하는 도중 소비된 lineEnding 만 확장된다.
            // 새 컨테이너나 새 아이템이 시작되면 문서 수준에서 소비되어 확장되지 않는다.
            "blockQuote" | "listUnordered" | "listOrdered" | "root" => {
                i > 0
                    && ends_with_content(&nodes[i - 1])
                    && if blank_next {
                        parent != "blockQuote" || next == "lineEndingBlank"
                    } else {
                        !matches!(
                            next,
                            "blockQuote" | "listUnordered" | "listOrdered" | "listItemPrefix"
                        )
                    }
            }
            _ => false,
        };
        if extend {
            let (el, ec, e) = (
                nodes[end_idx].end_line,
                nodes[end_idx].end_column,
                nodes[end_idx].end,
            );
            nodes[i].end_line = el;
            nodes[i].end_column = ec;
            nodes[i].end = e;
        }
    }
}

pub(super) fn is_html_flow_comment(n: &Node) -> bool {
    let t = n.text.as_str();
    if n.kind == "htmlFlow" && t.starts_with("<!--") && t.ends_with("-->") && t.len() >= 7 {
        let c = &t[4..t.len() - 3];
        return !c.starts_with('>') && !c.starts_with("->") && !c.ends_with('-');
    }
    false
}

/// 원본 markdownlint: htmlFlow 는 codeIndented/htmlFlow 를 끈 채 해당 줄들을 재파싱해 자식으로 붙인다.
fn reparse_html_flow(nodes: &mut [Node], text: &str, line_delta: usize) {
    for n in nodes.iter_mut() {
        if n.kind == "htmlFlow" && !is_html_flow_comment(n) {
            // `\r\n` 을 줄바꿈 하나로 세야 CRLF 파일에서 줄 범위가 맞는다.
            let lines = crate::fix::split_lines(text);
            let lo = n.start_line - line_delta - 1;
            let hi = n.end_line - line_delta;
            let sub = lines[lo..hi.min(lines.len())].join("\n");
            n.children = super::build::parse_nodes(&sub, false, n.start_line - 1);
        } else {
            reparse_html_flow(&mut n.children, text, line_delta);
        }
    }
}

const FLOW_WITH_PREFIX: &[&str] = &[
    "atxHeading",
    "thematicBreak",
    "codeFenced",
    "htmlFlow",
    "definition",
    "paragraph",
    "setextHeading",
    "table",
    "mathFlow",
    "blockQuote",
];

/// 첫 자식 체인을 따라 내려가며 노드 시작의 spaceOrTab 을 꺼내고 시작 위치를 조정한다.
fn take_leading_ws(n: &mut Node) -> Option<Node> {
    let first = n.children.first_mut()?;
    if first.start != n.start {
        return None;
    }
    let ws = if first.kind == "spaceOrTab" {
        n.children.remove(0)
    } else {
        take_leading_ws(first)?
    };
    n.start_line = ws.end_line;
    n.start_column = ws.end_column;
    n.text = n.text.split_off(ws.end - n.start);
    n.start = ws.end;
    Some(ws)
}

fn rename_all(nodes: &mut [Node]) {
    for n in nodes.iter_mut() {
        let r = rename(&n.kind);
        n.kind = if r.is_empty() {
            camel(&n.kind)
        } else {
            r.to_string()
        };
        rename_all(&mut n.children);
    }
}

/// 래퍼 제거(label*, listItem), content 래핑, 테이블 셀 분류, 리스트 범위 조정.
fn restructure(nodes: Vec<Node>, parent: &str, in_head: bool) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::new();
    for mut n in nodes {
        let kind = n.kind.clone();
        n.children = restructure(
            std::mem::take(&mut n.children),
            &kind,
            in_head || kind == "tableHead",
        );
        // 플로우 구성요소 앞 들여쓰기는 linePrefix 로 밖에 둔다 (중첩 첫 자식 체인 포함)
        if FLOW_WITH_PREFIX.contains(&kind.as_str())
            && let Some(mut ws) = take_leading_ws(&mut n)
        {
            ws.kind = "linePrefix".into();
            out.push(ws);
        }
        match kind.as_str() {
            "literalAutolinkHttp" | "literalAutolinkWww" | "literalAutolinkEmail" => {
                out.push(Node::wrapper("literalAutolink", n))
            }
            "definition" => {
                for k in 0..n.children.len() {
                    if n.children[k].kind == "spaceOrTab" {
                        let after_le = k > 0 && n.children[k - 1].kind == "lineEnding";
                        n.children[k].kind =
                            if after_le { "linePrefix" } else { "lineSuffix" }.into();
                    }
                }
                out.push(Node::wrapper("content", n));
            }
            "atxHeading" => {
                // 텍스트와 붙어 있는 시퀀스(#️⃣ 같은 이모지의 `#`)는 텍스트의 일부다
                let mut k = 0;
                while k + 1 < n.children.len() {
                    if n.children[k].kind == "atxHeadingSequence"
                        && n.children[k + 1].kind == "atxHeadingText"
                        && n.children[k].end == n.children[k + 1].start
                    {
                        let seq = n.children.remove(k);
                        let t = &mut n.children[k];
                        t.start_line = seq.start_line;
                        t.start_column = seq.start_column;
                        t.start = seq.start;
                        t.text = format!("{}{}", seq.text, t.text);
                        if let Some(d) = t.children.first_mut()
                            && d.kind == "data"
                        {
                            d.start_line = seq.start_line;
                            d.start_column = seq.start_column;
                            d.start = seq.start;
                            d.text = format!("{}{}", seq.text, d.text);
                        }
                    } else {
                        k += 1;
                    }
                }
                out.push(n);
            }
            "setextHeading" | "tableHead" | "tableBody" => {
                // 두 번째 이후 줄(밑줄, 구분자 행, 본문 행)의 들여쓰기는 행 밖 linePrefix.
                // 테이블 첫 줄은 table 자체의 linePrefix 로 빠지지만 본문 첫 행은 아니다.
                let mut k = 0;
                while k < n.children.len() {
                    if matches!(
                        n.children[k].kind.as_str(),
                        "setextHeadingLine" | "tableDelimiterRow" | "tableRow"
                    ) && (kind == "tableBody" || n.children[k].start != n.start)
                        && let Some(mut ws) = take_leading_ws(&mut n.children[k])
                    {
                        ws.kind = "linePrefix".into();
                        n.children.insert(k, ws);
                        k += 1;
                    }
                    k += 1;
                }
                // 첫 행에서 나온 linePrefix 는 tableBody 밖으로
                if kind == "tableBody"
                    && n.children
                        .first()
                        .is_some_and(|c| c.kind == "linePrefix" && c.start == n.start)
                {
                    let ws = n.children.remove(0);
                    if let Some(f) = n.children.first() {
                        n.start_line = f.start_line;
                        n.start_column = f.start_column;
                        n.start = f.start;
                    }
                    out.push(ws);
                }
                out.push(n);
            }
            "gfmFootnoteCall" => {
                fn flatten_call(children: Vec<Node>, out: &mut Vec<Node>) {
                    for mut c in children {
                        match c.kind.as_str() {
                            "label" | "gfmFootnoteCallLabel" => {
                                flatten_call(std::mem::take(&mut c.children), out)
                            }
                            "labelMarker" => {
                                c.kind = "gfmFootnoteCallLabelMarker".into();
                                out.push(c);
                            }
                            "labelText" => {
                                c.kind = "gfmFootnoteCallString".into();
                                out.push(c);
                            }
                            _ => out.push(c),
                        }
                    }
                }
                let mut kids = Vec::new();
                flatten_call(std::mem::take(&mut n.children), &mut kids);
                n.children = kids;
                out.push(n);
            }
            "gfmFootnoteDefinitionPrefix" => {
                for c in n.children.iter_mut() {
                    if c.kind == "spaceOrTab" {
                        c.kind = "gfmFootnoteDefinitionWhitespace".into();
                    }
                }
                out.extend(n.children);
            }
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
                if let Some(p) = pos
                    && p > 0
                    && n.children[p - 1].kind == "listItemIndent"
                {
                    n.children[p - 1].kind = "linePrefix".into();
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
                out.extend(leading);
                out.push(n);
            }
            "paragraph" if parent != "setextHeading" => out.push(Node::wrapper("content", n)),
            "tableCell" => {
                n.kind = if in_head {
                    "tableHeader".into()
                } else {
                    "tableData".into()
                };
                out.push(n);
            }
            _ => out.push(n),
        }
    }
    out
}

/// 줄머리 접두를 소비하는 컨테이너 (조상 체인, 바깥→안쪽).
#[derive(Clone, Copy, PartialEq)]
enum Container {
    BlockQuote,
    List,
    Footnote,
}

fn container_of(kind: &str) -> Option<Container> {
    match kind {
        "blockQuote" => Some(Container::BlockQuote),
        "listUnordered" | "listOrdered" => Some(Container::List),
        "gfmFootnoteDefinition" => Some(Container::Footnote),
        _ => None,
    }
}

/// 줄머리 공백이 어느 컨테이너의 continuation 에 소비됐는지 조상 체인 순서대로 배정한다.
/// micromark 는 컨테이너를 바깥부터 순서대로 이어가며, blockQuote 는 `linePrefix` 와
/// `blockQuotePrefix` 를, 리스트 아이템은 `listItemIndent` 를, footnote 는
/// `gfmFootnoteDefinitionIndent` 를 소비한다. 체인이 끝나면 flow 의 `linePrefix` 다.
fn line_start_kind(nodes: &[Node], i: usize, chain: &[Container]) -> &'static str {
    let mut k = i;
    while k > 0
        && nodes[k].start_column != 1
        && matches!(
            nodes[k - 1].kind.as_str(),
            "spaceOrTab" | "listItemIndent" | "linePrefix" | "blockQuotePrefix"
        )
    {
        k -= 1;
    }
    let mut ptr = 0;
    for node in &nodes[k..i] {
        if node.kind == "blockQuotePrefix" {
            while ptr < chain.len() && chain[ptr] != Container::BlockQuote {
                ptr += 1;
            }
            ptr = (ptr + 1).min(chain.len());
        } else if !matches!(chain.get(ptr), Some(Container::BlockQuote) | None) {
            ptr += 1;
        }
    }
    match chain.get(ptr) {
        Some(Container::List) => "listItemIndent",
        Some(Container::Footnote) if nodes[i].end - nodes[i].start == 4 => {
            "gfmFootnoteDefinitionIndent"
        }
        _ => "linePrefix",
    }
}

fn classify_whitespace(nodes: &mut [Node], parent: &str, chain: &[Container]) {
    for i in 0..nodes.len() {
        let kind = nodes[i].kind.clone();
        let mut child_chain = chain.to_vec();
        child_chain.extend(container_of(&kind));
        classify_whitespace(&mut nodes[i].children, &kind, &child_chain);
        if kind == "listItemIndent" && matches!(parent, "listUnordered" | "listOrdered") {
            // restructure 에서 일괄 변환된 리스트 직속 들여쓰기를 컨테이너 순서로 다시 배정
            nodes[i].kind = line_start_kind(nodes, i, chain).into();
            continue;
        }
        if kind != "spaceOrTab" {
            continue;
        }
        let prev = if i > 0 {
            nodes[i - 1].kind.as_str()
        } else {
            ""
        };
        let next = nodes.get(i + 1).map(|n| n.kind.as_str()).unwrap_or("");
        let at_line_start = nodes[i].start_column == 1
            || matches!(prev, "blockQuotePrefix" | "listItemIndent" | "linePrefix")
            || prev == "lineEnding";
        let new = match parent {
            "listItemPrefix" => "listItemPrefixWhitespace",
            "blockQuotePrefix" => "blockQuotePrefixWhitespace",
            "atxHeading" | "tableHeader" | "tableData" | "tableDelimiter" | "thematicBreak" => {
                "whitespace"
            }
            "resource" => "lineSuffix",
            "codeIndented" if at_line_start || prev.is_empty() => "linePrefix",
            "codeFencedFence" if prev.is_empty() => "linePrefix",
            _ if at_line_start => line_start_kind(nodes, i, chain),
            _ if next == "lineEnding" || next.is_empty() => "lineSuffix",
            _ => "whitespace",
        };
        nodes[i].kind = new.into();
    }
}

/// micromark 가 `_container` 로 표시하는 토큰: postprocess 의 exit 이동 대상.
fn is_list(n: &Node) -> bool {
    matches!(
        n.kind.as_str(),
        "listUnordered" | "listOrdered" | "gfmFootnoteDefinition"
    )
}

/// 노드가 (컨테이너를 파고들었을 때) 문단 content 로 끝나는가.
/// 그 경우 micromark 문단 lazy 연속 검사가 다음 줄 접두까지 lineEnding 을 소비한다.
fn ends_with_content(n: &Node) -> bool {
    match n.kind.as_str() {
        "content" | "codeIndented" => true,
        "blockQuote" | "listUnordered" | "listOrdered" => n
            .children
            .iter()
            .rev()
            .find(|c| {
                !matches!(
                    c.kind.as_str(),
                    "lineEnding"
                        | "lineEndingBlank"
                        | "linePrefix"
                        | "listItemIndent"
                        | "blockQuotePrefix"
                )
            })
            .is_some_and(ends_with_content),
        _ => false,
    }
}

fn set_end_from_last(n: &mut Node) {
    if let Some(l) = n.children.last() {
        n.end_line = l.end_line;
        n.end_column = l.end_column;
        n.end = l.end;
    }
}

/// micromark postprocess 의 컨테이너 exit 이동 재현.
///
/// 1. 컨테이너를 닫는 줄에서 소비된 접두들(lineEnding, blank, linePrefix, listItemIndent,
///    blockQuotePrefix)을 컨테이너 안으로 되돌린다 (micromark 의 이동 전 상태).
/// 2. exit 에서 뒤로 스캔하며 lineEnding/blank 와 linePrefix/listItemIndent 는 건너뛰고
///    (blockQuotePrefix 등 다른 토큰이면 중단), lineEnding 을 찾으면 exit 를 가장 이른
///    lineEnding 앞으로 옮긴다. 그 run 의 가장 이른 것은 lineEnding, 나머지는
///    lineEndingBlank 로 rename 된다.
fn fix_list_spans(nodes: &mut Vec<Node>) {
    for n in nodes.iter_mut() {
        fix_list_spans(&mut n.children);
    }
    let mut i = 0;
    while i < nodes.len() {
        // blockQuote exit 직후의 blank(빈 `>` 줄 끝)는 lineEnding 이 된다
        if nodes[i].kind == "blockQuote"
            && nodes
                .get(i + 1)
                .is_some_and(|c| c.kind == "lineEndingBlank" && c.start == nodes[i].end)
        {
            nodes[i + 1].kind = "lineEnding".into();
        }
        if !is_list(&nodes[i]) {
            i += 1;
            continue;
        }
        // 1. 이동 전 상태 복원: 뒤따르는 접두류 흡수
        while nodes.get(i + 1).is_some_and(|c| {
            matches!(
                c.kind.as_str(),
                "lineEnding" | "linePrefix" | "listItemIndent" | "blockQuotePrefix"
            )
        }) {
            let c = nodes.remove(i + 1);
            nodes[i].children.push(c);
        }
        // 2. 뒤로 스캔. 다음 형제가 새 컨테이너나 blank 가 아니면(lazy/_closeFlow 종료)
        // blockQuotePrefix 도 건너뛰어 백데이트한다.
        let allow_bq = nodes.get(i + 1).is_some_and(|c| {
            !matches!(
                c.kind.as_str(),
                "lineEndingBlank"
                    | "listUnordered"
                    | "listOrdered"
                    | "gfmFootnoteDefinition"
                    | "blockQuote"
            )
        });
        let children = &mut nodes[i].children;
        let mut line_index: Option<usize> = None;
        let mut k = children.len();
        while k > 0 {
            k -= 1;
            match children[k].kind.as_str() {
                "lineEnding" | "lineEndingBlank" => {
                    // 빈 인용 줄(`>` 만 있는 줄) 끝의 blank 에서는 멈춘다
                    let stop = children[k].kind == "lineEndingBlank"
                        && k > 0
                        && children[k - 1].kind == "blockQuotePrefix";
                    if let Some(prev) = line_index {
                        children[prev].kind = "lineEndingBlank".into();
                    }
                    children[k].kind = "lineEnding".into();
                    line_index = Some(k);
                    if stop {
                        break;
                    }
                }
                "linePrefix" | "listItemIndent" => {}
                "blockQuotePrefix" if allow_bq => {}
                _ => break,
            }
        }
        if let Some(li) = line_index {
            let moved: Vec<Node> = nodes[i].children.split_off(li);
            let (sl, sc, s) = (moved[0].start_line, moved[0].start_column, moved[0].start);
            nodes[i].end_line = sl;
            nodes[i].end_column = sc;
            nodes[i].end = s;
            for (k, m) in moved.into_iter().enumerate() {
                nodes.insert(i + 1 + k, m);
            }
        } else {
            set_end_from_last(&mut nodes[i]);
        }
        // 리스트 exit 직후의 blank(빈 아이템이나 빈 `>` 줄 뒤)는 lineEnding 이 된다
        if nodes
            .get(i + 1)
            .is_some_and(|c| c.kind == "lineEndingBlank" && c.start == nodes[i].end)
        {
            nodes[i + 1].kind = "lineEnding".into();
        }
        i += 1;
    }
}

/// `content` + lineEnding + `content` 연속을 하나의 content 로 병합 (micromark 의 content 청크).
fn merge_content(nodes: &mut Vec<Node>) {
    for n in nodes.iter_mut() {
        merge_content(&mut n.children);
    }
    let mut i = 0;
    while i + 2 < nodes.len() {
        if nodes[i].kind == "content"
            && nodes[i + 1].kind == "lineEnding"
            && nodes[i + 2].kind == "content"
        {
            let next = nodes.remove(i + 2);
            let le = nodes.remove(i + 1);
            let cur = &mut nodes[i];
            cur.children.push(le);
            cur.children.extend(next.children);
            cur.end_line = next.end_line;
            cur.end_column = next.end_column;
            cur.end = next.end;
        } else {
            i += 1;
        }
    }
}
