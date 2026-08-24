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

/// 줄 시작 바이트부터 `point.index` 까지의 코드포인트 수 + 1.
fn codepoint_column(text: &str, point: &Point) -> usize {
    let line_start = text[..point.index].rfind('\n').map_or(0, |i| i + 1);
    text[line_start..point.index].chars().count() + 1
}

pub fn parse(text: &str) -> TokenTree {
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
    let nodes = nest(text, &events, line_delta);
    adapt::adapt(nodes, text, line_delta)
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
    let children: Vec<usize> = n.children.into_iter().map(|c| flatten(tree, c, Some(id))).collect();
    tree.tokens[id].children = children;
    id
}
