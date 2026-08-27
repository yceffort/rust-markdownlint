//! markdown-rs 토큰 트리를 micromark(JS, markdownlint 가 사용하는 형태) 토큰 트리로 변환.

use super::kinds::Kind;

#[derive(Debug, Clone)]
pub(super) struct Node {
    pub kind: Kind,
    // 노드는 변환 단계마다 통째로 옮겨지므로 u32 로 줄여 복사량을 아낀다
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    /// `TokenTree::sources` 인덱스. 본문은 `sources[src][start..end]`.
    pub src: u32,
    pub start: u32,
    pub end: u32,
    pub children: Vec<Node>,
}

impl Node {
    fn wrapper(kind: Kind, inner: Node) -> Node {
        Node {
            kind,
            start_line: inner.start_line,
            start_column: inner.start_column,
            end_line: inner.end_line,
            end_column: inner.end_column,
            src: inner.src,
            start: inner.start,
            end: inner.end,
            children: vec![inner],
        }
    }

    fn text<'a>(&self, text: &'a str) -> &'a str {
        &text[self.start as usize..self.end as usize]
    }
}

const HOIST: &[Kind] = &[Kind::LABEL_LINK, Kind::LABEL_IMAGE, Kind::LABEL_END];

pub(super) fn adapt(
    nodes: Vec<Node>,
    text: &str,
    line_delta: usize,
    sources: &mut Vec<String>,
) -> Vec<Node> {
    let mut nodes = nodes;
    demote_autolinks(&mut nodes, text, false);
    let mut nodes = restructure(nodes, Kind::ROOT, false);
    fix_code_text(&mut nodes, text);
    classify_whitespace(&mut nodes, Kind::ROOT, &mut Vec::new());
    extend_line_endings(&mut nodes, Kind::ROOT, false);
    fix_list_spans(&mut nodes);
    merge_content(&mut nodes);
    reparse_html_flow(
        &mut nodes,
        text,
        line_delta,
        sources,
        &std::cell::OnceCell::new(),
    );
    nodes
}

/// micromark `resolveCodeText`: 인접 codeTextData 병합 후, 양끝이 공백/줄바꿈이고
/// 사이에 내용이 있으면 한 글자씩 codeTextPadding 으로 분리한다.
fn fix_code_text(nodes: &mut [Node], text: &str) {
    for n in nodes.iter_mut() {
        fix_code_text(&mut n.children, text);
    }
    for n in nodes.iter_mut() {
        if n.kind != Kind::CODE_TEXT {
            continue;
        }
        let kids = &mut n.children;
        // 인접 codeTextData 병합
        let mut k = 0;
        while k + 1 < kids.len() {
            if kids[k].kind == Kind::CODE_TEXT_DATA
                && kids[k + 1].kind == Kind::CODE_TEXT_DATA
                && kids[k].end == kids[k + 1].start
            {
                let nx = kids.remove(k + 1);
                let cur = &mut kids[k];
                cur.end_line = nx.end_line;
                cur.end_column = nx.end_column;
                cur.end = nx.end;
            } else {
                k += 1;
            }
        }
        // 패딩: 여는/닫는 시퀀스 안쪽 첫/끝 토큰 검사
        if kids.len() < 3 {
            continue;
        }
        let (h, t) = (1, kids.len() - 2);
        let head_pad = kids[h].kind == Kind::LINE_ENDING
            || (kids[h].kind == Kind::CODE_TEXT_DATA && kids[h].text(text).starts_with(' '));
        let tail_pad = kids[t].kind == Kind::LINE_ENDING
            || (kids[t].kind == Kind::CODE_TEXT_DATA && kids[t].text(text).ends_with(' '));
        if !head_pad || !tail_pad {
            continue;
        }
        // 패딩 1 글자씩 제외한 내부에 공백 아닌 내용이 있어야 한다
        let inner: String = kids[h..=t].iter().map(|c| c.text(text)).collect();
        if inner.len() < 2
            || inner[1..inner.len() - 1]
                .trim_matches([' ', '\n', '\r'])
                .is_empty()
        {
            continue;
        }
        // 꼬리 먼저 처리 (인덱스 보존)
        if kids[t].kind == Kind::LINE_ENDING || kids[t].text(text) == " " {
            kids[t].kind = Kind::CODE_TEXT_PADDING;
        } else {
            let d = &mut kids[t];
            d.end -= 1;
            d.end_column -= 1;
            let pad = Node {
                kind: Kind::CODE_TEXT_PADDING,
                start_line: d.end_line,
                start_column: d.end_column,
                end_line: d.end_line,
                end_column: d.end_column + 1,
                src: d.src,
                start: d.end,
                end: d.end + 1,
                children: Vec::new(),
            };
            kids.insert(t + 1, pad);
        }
        if kids[h].kind == Kind::LINE_ENDING || kids[h].text(text) == " " {
            kids[h].kind = Kind::CODE_TEXT_PADDING;
        } else {
            let d = &mut kids[h];
            let pad = Node {
                kind: Kind::CODE_TEXT_PADDING,
                start_line: d.start_line,
                start_column: d.start_column,
                end_line: d.start_line,
                end_column: d.start_column + 1,
                src: d.src,
                start: d.start,
                end: d.start + 1,
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
        if matches!(
            n.kind,
            Kind::LITERAL_AUTOLINK_HTTP | Kind::LITERAL_AUTOLINK_WWW | Kind::LITERAL_AUTOLINK_EMAIL
        ) && (in_label || (n.start > 0 && text.as_bytes()[n.start as usize - 1] == b'['))
        {
            n.kind = Kind::DATA;
            n.children.clear();
        }
        let label = in_label || n.kind == Kind::LABEL;
        demote_autolinks(&mut n.children, text, label);
    }
}

/// micromark: 플로우 토크나이저가 줄 경계를 넘어 계속되는 도중 소비한 lineEnding 은
/// 다음 줄의 컨테이너 접두(blockQuotePrefix, listItemIndent) 끝까지 이어진다 (defineSkip).
/// 블록이 그 줄에서 끝났으면(다음이 새 블록/컨테이너) 이어지지 않는다.
fn extend_line_endings(nodes: &mut [Node], parent: Kind, interrupt: bool) {
    for i in 0..nodes.len() {
        let kind = nodes[i].kind;
        // table 이 문단을 인터럽트하며 시작했는지: tableHead 내부 lineEnding 확장 여부를 가른다
        let child_interrupt = if kind == Kind::TABLE {
            let mut k = i;
            let mut found = false;
            while k > 0 {
                k -= 1;
                match nodes[k].kind {
                    Kind::BLOCK_QUOTE_PREFIX
                    | Kind::LIST_ITEM_INDENT
                    | Kind::LINE_PREFIX
                    | Kind::LINE_ENDING => {}
                    other => {
                        found = other == Kind::CONTENT;
                        break;
                    }
                }
            }
            found
        } else if kind == Kind::TABLE_HEAD {
            interrupt
        } else {
            false
        };
        extend_line_endings(&mut nodes[i].children, kind, child_interrupt);
    }
    for i in 0..nodes.len() {
        if nodes[i].kind != Kind::LINE_ENDING {
            continue;
        }
        let mut j = i + 1;
        let mut last_container = None;
        while nodes.get(j).is_some_and(|c| {
            matches!(
                c.kind,
                Kind::BLOCK_QUOTE_PREFIX | Kind::LIST_ITEM_INDENT | Kind::LINE_PREFIX
            )
        }) {
            if nodes[j].kind != Kind::LINE_PREFIX {
                last_container = Some(j);
            }
            j += 1;
        }
        let next = nodes.get(j).map(|c| c.kind).unwrap_or(Kind::NONE);
        // 다음 줄이 빈 줄(또는 부모의 끝)이면 flow linePrefix 까지 포함해 이어지고,
        // 플로우 내용이 이어지면 컨테이너 접두까지만 이어진다.
        let blank_next = next == Kind::LINE_ENDING_BLANK || next == Kind::NONE;
        let end_idx = if blank_next {
            last_container.map(|_| j - 1)
        } else {
            last_container
        };
        let Some(end_idx) = end_idx else { continue };
        let extend = match parent {
            Kind::CODE_FENCED | Kind::HTML_FLOW => next != Kind::NONE,
            Kind::TABLE_HEAD => interrupt,
            // 문단이 다음 줄로 이어지는지 확인하는 도중 소비된 lineEnding 만 확장된다.
            // 새 컨테이너나 새 아이템이 시작되면 문서 수준에서 소비되어 확장되지 않는다.
            Kind::BLOCK_QUOTE | Kind::LIST_UNORDERED | Kind::LIST_ORDERED | Kind::ROOT => {
                i > 0
                    && ends_with_content(&nodes[i - 1])
                    && if blank_next {
                        parent != Kind::BLOCK_QUOTE || next == Kind::LINE_ENDING_BLANK
                    } else {
                        !matches!(
                            next,
                            Kind::BLOCK_QUOTE
                                | Kind::LIST_UNORDERED
                                | Kind::LIST_ORDERED
                                | Kind::LIST_ITEM_PREFIX
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

pub(super) fn is_html_flow_comment(n: &Node, text: &str) -> bool {
    let t = n.text(text);
    if n.kind == Kind::HTML_FLOW && t.starts_with("<!--") && t.ends_with("-->") && t.len() >= 7 {
        let c = &t[4..t.len() - 3];
        return !c.starts_with('>') && !c.starts_with("->") && !c.ends_with('-');
    }
    false
}

/// 원본 markdownlint: htmlFlow 는 codeIndented/htmlFlow 를 끈 채 해당 줄들을 재파싱해 자식으로 붙인다.
fn reparse_html_flow<'t>(
    nodes: &mut [Node],
    text: &'t str,
    line_delta: usize,
    sources: &mut Vec<String>,
    lines: &std::cell::OnceCell<Vec<&'t str>>,
) {
    for n in nodes.iter_mut() {
        if n.kind == Kind::HTML_FLOW && !is_html_flow_comment(n, text) {
            // `\r\n` 을 줄바꿈 하나로 세야 CRLF 파일에서 줄 범위가 맞는다. 파일당 한 번만 나눈다.
            let lines = lines.get_or_init(|| crate::fix::split_lines(text));
            let lo = n.start_line as usize - line_delta - 1;
            let hi = n.end_line as usize - line_delta;
            let sub = lines[lo..hi.min(lines.len())].join("\n");
            // 재파싱 본문은 원문과 다를 수 있으므로(CRLF) 별도 source 로 보관한다
            let src = sources.len();
            sources.push(String::new());
            n.children =
                super::build::parse_nodes(&sub, false, n.start_line as usize - 1, src, sources);
            sources[src] = sub;
        } else {
            reparse_html_flow(&mut n.children, text, line_delta, sources, lines);
        }
    }
}

const FLOW_WITH_PREFIX: &[Kind] = &[
    Kind::ATX_HEADING,
    Kind::THEMATIC_BREAK,
    Kind::CODE_FENCED,
    Kind::HTML_FLOW,
    Kind::DEFINITION,
    Kind::PARAGRAPH,
    Kind::SETEXT_HEADING,
    Kind::TABLE,
    Kind::MATH_FLOW,
    Kind::BLOCK_QUOTE,
    Kind::DIRECTIVE_CONTAINER,
];

/// 첫 자식 체인을 따라 내려가며 노드 시작의 spaceOrTab 을 꺼내고 시작 위치를 조정한다.
fn take_leading_ws(n: &mut Node) -> Option<Node> {
    let first = n.children.first_mut()?;
    if first.start != n.start {
        return None;
    }
    let ws = if first.kind == Kind::SPACE_OR_TAB {
        n.children.remove(0)
    } else {
        take_leading_ws(first)?
    };
    n.start_line = ws.end_line;
    n.start_column = ws.end_column;
    n.start = ws.end;
    Some(ws)
}

/// 첫 자식 체인의 선행 공백을 제자리에 둔 채 `linePrefix` 로 표기한다.
fn mark_leading_ws_line_prefix(n: &mut Node) {
    let start = n.start;
    let mut cur = n;
    while let Some(first) = cur.children.first_mut() {
        if first.start != start {
            return;
        }
        if first.kind == Kind::SPACE_OR_TAB {
            first.kind = Kind::LINE_PREFIX;
            return;
        }
        cur = first;
    }
}

/// 래퍼 제거(label*, listItem), content 래핑, 테이블 셀 분류, 리스트 범위 조정.
fn restructure(nodes: Vec<Node>, parent: Kind, in_head: bool) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::with_capacity(nodes.len());
    for mut n in nodes {
        let kind = n.kind;
        n.children = restructure(
            std::mem::take(&mut n.children),
            kind,
            in_head || kind == Kind::TABLE_HEAD,
        );
        // 플로우 구성요소 앞 들여쓰기는 linePrefix 로 밖에 둔다 (중첩 첫 자식 체인 포함)
        if FLOW_WITH_PREFIX.contains(&kind)
            && let Some(mut ws) = take_leading_ws(&mut n)
        {
            ws.kind = Kind::LINE_PREFIX;
            out.push(ws);
        }
        match kind {
            Kind::LITERAL_AUTOLINK_HTTP
            | Kind::LITERAL_AUTOLINK_WWW
            | Kind::LITERAL_AUTOLINK_EMAIL => out.push(Node::wrapper(Kind::LITERAL_AUTOLINK, n)),
            Kind::DEFINITION => {
                for k in 0..n.children.len() {
                    if n.children[k].kind == Kind::SPACE_OR_TAB {
                        let after_le = k > 0 && n.children[k - 1].kind == Kind::LINE_ENDING;
                        n.children[k].kind = if after_le {
                            Kind::LINE_PREFIX
                        } else {
                            Kind::LINE_SUFFIX
                        };
                    }
                }
                out.push(Node::wrapper(Kind::CONTENT, n));
            }
            Kind::ATX_HEADING => {
                // 텍스트와 붙어 있는 시퀀스(#️⃣ 같은 이모지의 `#`)는 텍스트의 일부다
                let mut k = 0;
                while k + 1 < n.children.len() {
                    if n.children[k].kind == Kind::ATX_HEADING_SEQUENCE
                        && n.children[k + 1].kind == Kind::ATX_HEADING_TEXT
                        && n.children[k].end == n.children[k + 1].start
                    {
                        let seq = n.children.remove(k);
                        let t = &mut n.children[k];
                        t.start_line = seq.start_line;
                        t.start_column = seq.start_column;
                        t.start = seq.start;
                        if let Some(d) = t.children.first_mut()
                            && d.kind == Kind::DATA
                        {
                            d.start_line = seq.start_line;
                            d.start_column = seq.start_column;
                            d.start = seq.start;
                        }
                    } else {
                        k += 1;
                    }
                }
                out.push(n);
            }
            Kind::SETEXT_HEADING | Kind::TABLE_HEAD | Kind::TABLE_BODY => {
                // 두 번째 이후 줄(밑줄, 본문 행)의 들여쓰기는 행 밖 linePrefix.
                // 테이블 첫 줄은 table 자체의 linePrefix 로 빠지지만 본문 첫 행은 아니다.
                // 구분자 행은 micromark 가 table 구성요소 안에서 이어 읽으므로 선행 공백이
                // 첫 tableDelimiter 셀 안에 남는다.
                let mut k = 0;
                while k < n.children.len() {
                    if matches!(
                        n.children[k].kind,
                        Kind::SETEXT_HEADING_LINE | Kind::TABLE_ROW
                    ) && (kind == Kind::TABLE_BODY || n.children[k].start != n.start)
                        && let Some(mut ws) = take_leading_ws(&mut n.children[k])
                    {
                        ws.kind = Kind::LINE_PREFIX;
                        n.children.insert(k, ws);
                        k += 1;
                    } else if n.children[k].kind == Kind::TABLE_DELIMITER_ROW {
                        mark_leading_ws_line_prefix(&mut n.children[k]);
                    }
                    k += 1;
                }
                // 첫 행에서 나온 linePrefix 는 tableBody 밖으로
                if kind == Kind::TABLE_BODY
                    && n.children
                        .first()
                        .is_some_and(|c| c.kind == Kind::LINE_PREFIX && c.start == n.start)
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
            Kind::GFM_FOOTNOTE_CALL => {
                fn flatten_call(children: Vec<Node>, out: &mut Vec<Node>) {
                    for mut c in children {
                        match c.kind {
                            Kind::LABEL | Kind::GFM_FOOTNOTE_CALL_LABEL => {
                                flatten_call(std::mem::take(&mut c.children), out)
                            }
                            Kind::LABEL_MARKER => {
                                c.kind = Kind::GFM_FOOTNOTE_CALL_LABEL_MARKER;
                                out.push(c);
                            }
                            Kind::LABEL_TEXT => {
                                c.kind = Kind::GFM_FOOTNOTE_CALL_STRING;
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
            Kind::GFM_FOOTNOTE_DEFINITION_PREFIX => {
                for c in n.children.iter_mut() {
                    if c.kind == Kind::SPACE_OR_TAB {
                        c.kind = Kind::GFM_FOOTNOTE_DEFINITION_WHITESPACE;
                    }
                }
                out.extend(n.children);
            }
            k if HOIST.contains(&k) => out.extend(n.children),
            Kind::LIST_ITEM => {
                // 첫 아이템의 prefix 앞 공백은 리스트 밖(앞)으로 나간다. 호출부(list)에서 처리.
                for c in n.children.iter_mut() {
                    if c.kind == Kind::SPACE_OR_TAB {
                        c.kind = Kind::LIST_ITEM_INDENT;
                    }
                }
                // prefix 바로 앞 공백은 linePrefix
                let pos = n
                    .children
                    .iter()
                    .position(|c| c.kind == Kind::LIST_ITEM_PREFIX);
                if let Some(p) = pos
                    && p > 0
                    && n.children[p - 1].kind == Kind::LIST_ITEM_INDENT
                {
                    n.children[p - 1].kind = Kind::LINE_PREFIX;
                }
                out.extend(n.children);
            }
            Kind::LIST_UNORDERED | Kind::LIST_ORDERED => {
                // 리스트 직속 공백(아이템 사이 continuation) → listItemIndent
                for c in n.children.iter_mut() {
                    if c.kind == Kind::SPACE_OR_TAB {
                        c.kind = Kind::LIST_ITEM_INDENT;
                    }
                }
                // 첫 아이템 prefix 앞 linePrefix 는 리스트 앞으로
                let mut leading = Vec::new();
                while n
                    .children
                    .first()
                    .is_some_and(|c| c.kind == Kind::LINE_PREFIX)
                {
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
            Kind::PARAGRAPH if parent != Kind::SETEXT_HEADING => {
                out.push(Node::wrapper(Kind::CONTENT, n))
            }
            Kind::TABLE_CELL => {
                n.kind = if in_head {
                    Kind::TABLE_HEADER
                } else {
                    Kind::TABLE_DATA
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

fn container_of(kind: Kind) -> Option<Container> {
    match kind {
        Kind::BLOCK_QUOTE => Some(Container::BlockQuote),
        Kind::LIST_UNORDERED | Kind::LIST_ORDERED => Some(Container::List),
        Kind::GFM_FOOTNOTE_DEFINITION => Some(Container::Footnote),
        _ => None,
    }
}

/// 줄머리 공백이 어느 컨테이너의 continuation 에 소비됐는지 조상 체인 순서대로 배정한다.
/// micromark 는 컨테이너를 바깥부터 순서대로 이어가며, blockQuote 는 `linePrefix` 와
/// `blockQuotePrefix` 를, 리스트 아이템은 `listItemIndent` 를, footnote 는
/// `gfmFootnoteDefinitionIndent` 를 소비한다. 체인이 끝나면 flow 의 `linePrefix` 다.
fn line_start_kind(nodes: &[Node], i: usize, chain: &[Container]) -> Kind {
    let mut k = i;
    while k > 0
        && nodes[k].start_column != 1
        && matches!(
            nodes[k - 1].kind,
            Kind::SPACE_OR_TAB
                | Kind::LIST_ITEM_INDENT
                | Kind::LINE_PREFIX
                | Kind::BLOCK_QUOTE_PREFIX
        )
    {
        k -= 1;
    }
    let mut ptr = 0;
    for node in &nodes[k..i] {
        if node.kind == Kind::BLOCK_QUOTE_PREFIX {
            while ptr < chain.len() && chain[ptr] != Container::BlockQuote {
                ptr += 1;
            }
            ptr = (ptr + 1).min(chain.len());
        } else if !matches!(chain.get(ptr), Some(Container::BlockQuote) | None) {
            ptr += 1;
        }
    }
    match chain.get(ptr) {
        Some(Container::List) => Kind::LIST_ITEM_INDENT,
        Some(Container::Footnote) if nodes[i].end - nodes[i].start == 4 => {
            Kind::GFM_FOOTNOTE_DEFINITION_INDENT
        }
        _ => Kind::LINE_PREFIX,
    }
}

fn classify_whitespace(nodes: &mut [Node], parent: Kind, chain: &mut Vec<Container>) {
    for i in 0..nodes.len() {
        let kind = nodes[i].kind;
        let container = container_of(kind);
        chain.extend(container);
        classify_whitespace(&mut nodes[i].children, kind, chain);
        if container.is_some() {
            chain.pop();
        }
        if kind == Kind::LIST_ITEM_INDENT
            && matches!(parent, Kind::LIST_UNORDERED | Kind::LIST_ORDERED)
        {
            // restructure 에서 일괄 변환된 리스트 직속 들여쓰기를 컨테이너 순서로 다시 배정
            nodes[i].kind = line_start_kind(nodes, i, chain);
            continue;
        }
        if kind != Kind::SPACE_OR_TAB {
            continue;
        }
        let prev = if i > 0 { nodes[i - 1].kind } else { Kind::NONE };
        let next = nodes.get(i + 1).map(|n| n.kind).unwrap_or(Kind::NONE);
        let at_line_start = nodes[i].start_column == 1
            || matches!(
                prev,
                Kind::BLOCK_QUOTE_PREFIX | Kind::LIST_ITEM_INDENT | Kind::LINE_PREFIX
            )
            || prev == Kind::LINE_ENDING;
        let new = match parent {
            Kind::LIST_ITEM_PREFIX => Kind::LIST_ITEM_PREFIX_WHITESPACE,
            Kind::BLOCK_QUOTE_PREFIX => Kind::BLOCK_QUOTE_PREFIX_WHITESPACE,
            Kind::ATX_HEADING
            | Kind::TABLE_HEADER
            | Kind::TABLE_DATA
            | Kind::TABLE_DELIMITER
            | Kind::THEMATIC_BREAK => Kind::WHITESPACE,
            Kind::RESOURCE => Kind::LINE_SUFFIX,
            Kind::CODE_INDENTED if at_line_start || prev == Kind::NONE => Kind::LINE_PREFIX,
            Kind::CODE_FENCED_FENCE if prev == Kind::NONE => Kind::LINE_PREFIX,
            Kind::DIRECTIVE_CONTAINER_FENCE => Kind::WHITESPACE,
            _ if at_line_start => line_start_kind(nodes, i, chain),
            _ if next == Kind::LINE_ENDING || next == Kind::NONE => Kind::LINE_SUFFIX,
            _ => Kind::WHITESPACE,
        };
        nodes[i].kind = new;
    }
}

/// micromark 가 `_container` 로 표시하는 토큰: postprocess 의 exit 이동 대상.
fn is_list(n: &Node) -> bool {
    matches!(
        n.kind,
        Kind::LIST_UNORDERED | Kind::LIST_ORDERED | Kind::GFM_FOOTNOTE_DEFINITION
    )
}

/// 노드가 (컨테이너를 파고들었을 때) 문단 content 로 끝나는가.
/// 그 경우 micromark 문단 lazy 연속 검사가 다음 줄 접두까지 lineEnding 을 소비한다.
fn ends_with_content(n: &Node) -> bool {
    match n.kind {
        Kind::CONTENT | Kind::CODE_INDENTED => true,
        Kind::BLOCK_QUOTE | Kind::LIST_UNORDERED | Kind::LIST_ORDERED => n
            .children
            .iter()
            .rev()
            .find(|c| {
                !matches!(
                    c.kind,
                    Kind::LINE_ENDING
                        | Kind::LINE_ENDING_BLANK
                        | Kind::LINE_PREFIX
                        | Kind::LIST_ITEM_INDENT
                        | Kind::BLOCK_QUOTE_PREFIX
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
        if nodes[i].kind == Kind::BLOCK_QUOTE
            && nodes
                .get(i + 1)
                .is_some_and(|c| c.kind == Kind::LINE_ENDING_BLANK && c.start == nodes[i].end)
        {
            nodes[i + 1].kind = Kind::LINE_ENDING;
        }
        if !is_list(&nodes[i]) {
            i += 1;
            continue;
        }
        // 1. 이동 전 상태 복원: 뒤따르는 접두류 흡수
        while nodes.get(i + 1).is_some_and(|c| {
            matches!(
                c.kind,
                Kind::LINE_ENDING
                    | Kind::LINE_PREFIX
                    | Kind::LIST_ITEM_INDENT
                    | Kind::BLOCK_QUOTE_PREFIX
            )
        }) {
            let c = nodes.remove(i + 1);
            nodes[i].children.push(c);
        }
        // 2. 뒤로 스캔. 다음 형제가 새 컨테이너나 blank 가 아니면(lazy/_closeFlow 종료)
        // blockQuotePrefix 도 건너뛰어 백데이트한다.
        let allow_bq = nodes.get(i + 1).is_some_and(|c| {
            !matches!(
                c.kind,
                Kind::LINE_ENDING_BLANK
                    | Kind::LIST_UNORDERED
                    | Kind::LIST_ORDERED
                    | Kind::GFM_FOOTNOTE_DEFINITION
                    | Kind::BLOCK_QUOTE
            )
        });
        let children = &mut nodes[i].children;
        let mut line_index: Option<usize> = None;
        let mut k = children.len();
        while k > 0 {
            k -= 1;
            match children[k].kind {
                Kind::LINE_ENDING | Kind::LINE_ENDING_BLANK => {
                    // 빈 인용 줄(`>` 만 있는 줄) 끝의 blank 에서는 멈춘다
                    let stop = children[k].kind == Kind::LINE_ENDING_BLANK
                        && k > 0
                        && children[k - 1].kind == Kind::BLOCK_QUOTE_PREFIX;
                    if let Some(prev) = line_index {
                        children[prev].kind = Kind::LINE_ENDING_BLANK;
                    }
                    children[k].kind = Kind::LINE_ENDING;
                    line_index = Some(k);
                    if stop {
                        break;
                    }
                }
                Kind::LINE_PREFIX | Kind::LIST_ITEM_INDENT => {}
                Kind::BLOCK_QUOTE_PREFIX if allow_bq => {}
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
            .is_some_and(|c| c.kind == Kind::LINE_ENDING_BLANK && c.start == nodes[i].end)
        {
            nodes[i + 1].kind = Kind::LINE_ENDING;
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
        if nodes[i].kind == Kind::CONTENT
            && nodes[i + 1].kind == Kind::LINE_ENDING
            && nodes[i + 2].kind == Kind::CONTENT
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
