use markdown::ParseOptions;
use markdown::event::{Event, Kind as EventKind};

use super::adapt::{self, Node};
use super::kinds::{Kind, kind_of};
use super::token::{Token, TokenTree};

fn parse_options(html_flow: bool) -> ParseOptions {
    let mut opts = ParseOptions::gfm();
    opts.constructs.frontmatter = false;
    opts.constructs.gfm_strikethrough = false;
    opts.constructs.gfm_task_list_item = false;
    opts.constructs.math_flow = true;
    opts.constructs.math_text = true;
    // 원본 micromark-parse.mjs 는 directive() 확장을 켠다 (container directive 만 지원)
    opts.constructs.directive_container = true;
    // 원본 htmlFlow 재파싱: codeIndented, htmlFlow 비활성
    opts.constructs.html_flow = html_flow;
    opts.constructs.code_indented = html_flow;
    opts
}

/// 바이트 인덱스 → 줄 안 UTF-16 컬럼(1 기준, micromark JS 와 동일). 이벤트는 거의 문서
/// 순서라 앞으로만 진행하는 커서로 세고, 뒤로 가야 할 때만 그 줄 시작부터 다시 센다.
struct ColumnCursor<'a> {
    bytes: &'a [u8],
    /// 지금까지 센 위치. `units` 는 그 줄 시작부터 `pos` 직전까지의 UTF-16 단위 수.
    pos: usize,
    units: usize,
}

impl<'a> ColumnCursor<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            bytes: text.as_bytes(),
            pos: 0,
            units: 0,
        }
    }

    fn column(&mut self, index: usize) -> usize {
        if index < self.pos {
            // 뒤로 이동: 줄 시작(마지막 `\n`/`\r` 다음)을 찾아 그 줄만 다시 센다
            let line_start = self.bytes[..index]
                .iter()
                .rposition(|&b| b == b'\n' || b == b'\r')
                .map_or(0, |i| i + 1);
            self.pos = line_start;
            self.units = 0;
        }
        for &b in &self.bytes[self.pos..index] {
            // 문자 시작 바이트에서만 증가: 4 바이트 문자(서로게이트 쌍)는 2 단위
            if b & 0xC0 != 0x80 {
                self.units += if b >= 0xF0 { 2 } else { 1 };
            }
            if b == b'\n' || b == b'\r' {
                self.units = 0;
            }
        }
        self.pos = index;
        self.units + 1
    }
}

pub fn parse(text: &str) -> TokenTree {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut sources = vec![text.to_string()];
    let nodes = parse_nodes(text, true, 0, 0, &mut sources);
    fn count(nodes: &[Node]) -> usize {
        nodes.iter().map(|n| 1 + count(&n.children)).sum()
    }
    let mut tree = TokenTree {
        sources,
        tokens: Vec::with_capacity(count(&nodes)),
        ..TokenTree::default()
    };
    for n in nodes {
        let id = flatten(&mut tree, n, None, false);
        tree.roots.push(id);
    }
    tree.index_kinds();
    tree
}

/// markdown-rs 이벤트 → 중첩 노드 → micromark 형태로 변환. `src` 는 `text` 의 `sources` 인덱스.
pub(super) fn parse_nodes(
    text: &str,
    html_flow: bool,
    line_delta: usize,
    src: usize,
    sources: &mut Vec<String>,
) -> Vec<Node> {
    let opts = parse_options(html_flow);
    let (events, _) = markdown::parser::parse(text, &opts).expect("markdown-rs parse");
    let refs = markdown::undefined_refs::take();
    let mut columns = ColumnCursor::new(text);
    let nodes = nest(&events, line_delta, src, &mut columns);
    drop(events);
    let mut nodes = adapt::adapt(nodes, text, line_delta, sources);
    append_undefined_references(&mut nodes, refs, text, line_delta, src, &mut columns);
    nodes
}

/// 원본 micromark-parse.mjs: labelEnd nok 마다 undefinedReference* 인공 토큰을 문서 끝에 붙인다.
fn append_undefined_references(
    nodes: &mut Vec<Node>,
    refs: Vec<markdown::undefined_refs::UndefinedRef>,
    text: &str,
    line_delta: usize,
    src: usize,
    columns: &mut ColumnCursor,
) {
    let mut arts: Vec<Node> = Vec::new();
    let mut refs = refs;
    // micromark 는 텍스트 서브토크나이즈(문서 순서) 중 생성한다
    refs.sort_by_key(|r| (r.start.1, r.end.1));
    for mut r in refs {
        // 문단 연속 줄의 선행 들여쓰기는 micromark 에서 linePrefix 라 data 에 포함되지 않는다
        for d in r.data.iter_mut() {
            if d.0 {
                continue;
            }
            let (s, e) = (d.1.1, d.2.1);
            if s == 0
                || text.as_bytes().get(s.wrapping_sub(1)) == Some(&b'\n')
                || text.as_bytes().get(s.wrapping_sub(1)) == Some(&b'\r')
            {
                let mut ns = s;
                while ns < e && matches!(text.as_bytes()[ns], b' ' | b'\t') {
                    ns += 1;
                }
                d.1.1 = ns;
            }
        }
        let mut node_at = |kind: Kind, s: (usize, usize), e: (usize, usize)| Node {
            kind,
            start_line: (s.0 + line_delta) as u32,
            start_column: columns.column(s.1) as u32,
            end_line: (e.0 + line_delta) as u32,
            end_column: columns.column(e.1) as u32,
            src: src as u32,
            start: s.1 as u32,
            end: e.1 as u32,
            children: Vec::new(),
        };
        r.data.retain(|d| d.1.1 < d.2.1);
        let mut outer = node_at(Kind::UNDEFINED_REFERENCE_SHORTCUT, r.start, r.end);
        // 직전 인공 토큰과 맞닿아 있으면 collapsed/full 로 병합
        if r.data.is_empty() {
            if let Some(p) = arts.last_mut().filter(|p| p.end as usize == r.start.1) {
                p.kind = Kind::UNDEFINED_REFERENCE_COLLAPSED;
                p.end_line = outer.end_line;
                p.end_column = outer.end_column;
                p.end = outer.end;
            }
        } else if let Some(p) = arts.pop_if(|p| p.end as usize == r.start.1) {
            outer.kind = Kind::UNDEFINED_REFERENCE_FULL;
            outer.start_line = p.start_line;
            outer.start_column = p.start_column;
            outer.start = p.start;
        }
        let joined: String = r.data.iter().map(|(_, s, e)| &text[s.1..e.1]).collect();
        let joined = joined.trim();
        if joined.is_empty() || joined.contains(']') {
            continue;
        }
        let mut inner = node_at(Kind::UNDEFINED_REFERENCE, r.start, r.end);
        inner.children = r
            .data
            .iter()
            .map(|&(le, s, e)| node_at(if le { Kind::LINE_ENDING } else { Kind::DATA }, s, e))
            .collect();
        outer.children.push(inner);
        arts.push(outer);
    }
    nodes.extend(arts);
}

fn nest(events: &[Event], line_delta: usize, src: usize, columns: &mut ColumnCursor) -> Vec<Node> {
    // 자식 수를 먼저 세어 정확한 용량으로 만든다 (Node 는 커서 재할당 복사가 비싸다)
    let mut counts = vec![0u32; events.len()];
    let mut root_count = 0usize;
    let mut open: Vec<usize> = Vec::new();
    for (i, ev) in events.iter().enumerate() {
        match ev.kind {
            EventKind::Enter => open.push(i),
            EventKind::Exit => {
                open.pop();
                match open.last() {
                    Some(&p) => counts[p] += 1,
                    None => root_count += 1,
                }
            }
        }
    }
    let mut roots = Vec::with_capacity(root_count);
    let mut stack: Vec<Node> = Vec::new();
    for (i, ev) in events.iter().enumerate() {
        match ev.kind {
            EventKind::Enter => stack.push(Node {
                kind: kind_of(&ev.name),
                start_line: (ev.point.line + line_delta) as u32,
                start_column: columns.column(ev.point.index) as u32,
                end_line: 0,
                end_column: 0,
                src: src as u32,
                start: ev.point.index as u32,
                end: ev.point.index as u32,
                children: Vec::with_capacity(counts[i] as usize),
            }),
            EventKind::Exit => {
                let mut n = stack.pop().expect("unbalanced exit");
                n.end_line = (ev.point.line + line_delta) as u32;
                n.end_column = columns.column(ev.point.index) as u32;
                n.end = ev.point.index as u32;
                match stack.last_mut() {
                    Some(p) => p.children.push(n),
                    None => roots.push(n),
                }
            }
        }
    }
    roots
}

fn flatten(tree: &mut TokenTree, n: Node, parent: Option<usize>, in_html_flow: bool) -> usize {
    // 원본은 htmlFlow 재파싱으로 만든 토큰에만 htmlFlowSymbol 을 붙인다 (htmlFlow 자신은 제외).
    let children_in_html_flow = in_html_flow
        || (n.kind == Kind::HTML_FLOW
            && !adapt::is_html_flow_comment(&n, &tree.sources[n.src as usize]));
    let id = tree.tokens.len();
    tree.tokens.push(Token {
        kind: n.kind.name(),
        kind_id: n.kind,
        start_line: n.start_line as usize,
        start_column: n.start_column as usize,
        end_line: n.end_line as usize,
        end_column: n.end_column as usize,
        src: n.src as usize,
        start: n.start as usize,
        end: n.end as usize,
        parent,
        children: Vec::new(),
        in_html_flow,
    });
    let children: Vec<usize> = n
        .children
        .into_iter()
        .map(|c| flatten(tree, c, Some(id), children_in_html_flow))
        .collect();
    tree.tokens[id].children = children;
    id
}
