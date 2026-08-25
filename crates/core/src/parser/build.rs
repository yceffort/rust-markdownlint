use markdown::ParseOptions;
use markdown::event::{Event, Kind, Point};

use super::adapt::{self, Node};
use super::token::{Token, TokenTree};

fn parse_options(html_flow: bool) -> ParseOptions {
    let mut opts = ParseOptions::gfm();
    opts.constructs.frontmatter = false;
    opts.constructs.gfm_strikethrough = false;
    opts.constructs.gfm_task_list_item = false;
    opts.constructs.math_flow = true;
    opts.constructs.math_text = true;
    // 원본 htmlFlow 재파싱: codeIndented, htmlFlow 비활성
    opts.constructs.html_flow = html_flow;
    opts.constructs.code_indented = html_flow;
    opts
}

/// 줄 시작 바이트부터 `index` 까지의 UTF-16 단위 수 + 1 (micromark JS 와 동일).
fn column_at(text: &str, index: usize) -> usize {
    let line_start = text[..index].rfind(['\n', '\r']).map_or(0, |i| i + 1);
    text[line_start..index]
        .chars()
        .map(char::len_utf16)
        .sum::<usize>()
        + 1
}

fn codepoint_column(text: &str, point: &Point) -> usize {
    column_at(text, point.index)
}

pub fn parse(text: &str) -> TokenTree {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let nodes = parse_nodes(text, true, 0);
    let mut tree = TokenTree::default();
    for n in nodes {
        let id = flatten(&mut tree, n, None);
        tree.roots.push(id);
    }
    tree
}

/// markdown-rs 이벤트 → 중첩 노드 → micromark 형태로 변환.
pub(super) fn parse_nodes(text: &str, html_flow: bool, line_delta: usize) -> Vec<Node> {
    let opts = parse_options(html_flow);
    let (events, _) = markdown::parser::parse(text, &opts).expect("markdown-rs parse");
    let refs = markdown::undefined_refs::take();
    let nodes = nest(text, &events, line_delta);
    let mut nodes = adapt::adapt(nodes, text, line_delta);
    append_undefined_references(&mut nodes, refs, text, line_delta);
    nodes
}

/// 원본 micromark-parse.mjs: labelEnd nok 마다 undefinedReference* 인공 토큰을 문서 끝에 붙인다.
fn append_undefined_references(
    nodes: &mut Vec<Node>,
    refs: Vec<markdown::undefined_refs::UndefinedRef>,
    text: &str,
    line_delta: usize,
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
        let node_at = |kind: &str, s: (usize, usize), e: (usize, usize)| Node {
            kind: kind.to_string(),
            start_line: s.0 + line_delta,
            start_column: column_at(text, s.1),
            end_line: e.0 + line_delta,
            end_column: column_at(text, e.1),
            start: s.1,
            end: e.1,
            text: text[s.1..e.1].to_string(),
            children: Vec::new(),
        };
        r.data.retain(|d| d.1.1 < d.2.1);
        let mut outer = node_at("undefinedReferenceShortcut", r.start, r.end);
        // 직전 인공 토큰과 맞닿아 있으면 collapsed/full 로 병합
        if r.data.is_empty() {
            if let Some(p) = arts.last_mut().filter(|p| p.end == r.start.1) {
                p.kind = "undefinedReferenceCollapsed".into();
                p.end_line = outer.end_line;
                p.end_column = outer.end_column;
                p.end = outer.end;
                p.text = text[p.start..p.end].to_string();
            }
        } else if let Some(p) = arts.pop_if(|p| p.end == r.start.1) {
            outer.kind = "undefinedReferenceFull".into();
            outer.start_line = p.start_line;
            outer.start_column = p.start_column;
            outer.start = p.start;
            outer.text = text[outer.start..outer.end].to_string();
        }
        let joined: String = r.data.iter().map(|(_, s, e)| &text[s.1..e.1]).collect();
        let joined = joined.trim();
        if joined.is_empty() || joined.contains(']') {
            continue;
        }
        let mut inner = node_at("undefinedReference", r.start, r.end);
        inner.children = r
            .data
            .iter()
            .map(|&(le, s, e)| node_at(if le { "lineEnding" } else { "data" }, s, e))
            .collect();
        outer.children.push(inner);
        arts.push(outer);
    }
    nodes.extend(arts);
}

fn nest(text: &str, events: &[Event], line_delta: usize) -> Vec<Node> {
    let mut roots = Vec::new();
    let mut stack: Vec<Node> = Vec::new();
    for ev in events {
        match ev.kind {
            Kind::Enter => stack.push(Node {
                kind: format!("{:?}", ev.name),
                start_line: ev.point.line + line_delta,
                start_column: codepoint_column(text, &ev.point),
                end_line: 0,
                end_column: 0,
                start: ev.point.index,
                end: ev.point.index,
                text: String::new(),
                children: Vec::new(),
            }),
            Kind::Exit => {
                let mut n = stack.pop().expect("unbalanced exit");
                n.end_line = ev.point.line + line_delta;
                n.end_column = codepoint_column(text, &ev.point);
                n.end = ev.point.index;
                n.text = text[n.start..n.end].to_string();
                match stack.last_mut() {
                    Some(p) => p.children.push(n),
                    None => roots.push(n),
                }
            }
        }
    }
    roots
}

fn flatten(tree: &mut TokenTree, n: Node, parent: Option<usize>) -> usize {
    let id = tree.tokens.len();
    tree.tokens.push(Token {
        kind: n.kind,
        start_line: n.start_line,
        start_column: n.start_column,
        end_line: n.end_line,
        end_column: n.end_column,
        text: n.text,
        parent,
        children: Vec::new(),
    });
    let children: Vec<usize> = n
        .children
        .into_iter()
        .map(|c| flatten(tree, c, Some(id)))
        .collect();
    tree.tokens[id].children = children;
    id
}
