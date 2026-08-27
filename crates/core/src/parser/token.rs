use std::collections::HashMap;

pub type TokenId = usize;

/// micromark(JS) 와 동형인 토큰. `kind` 는 micromark 토큰 타입 문자열 (예: `atxHeading`).
/// 본문은 소유하지 않고 `TokenTree::sources[src]` 의 바이트 범위로 가리킨다 (`TokenTree::text`).
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: &'static str,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub(crate) src: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub parent: Option<TokenId>,
    pub children: Vec<TokenId>,
    /// 원본 `htmlFlowSymbol`: htmlFlow 재파싱으로 생긴 토큰인지 (`inHtmlFlow`).
    pub in_html_flow: bool,
}

#[derive(Debug, Default)]
pub struct TokenTree {
    pub tokens: Vec<Token>,
    pub roots: Vec<TokenId>,
    /// `sources[0]` 은 원문, 나머지는 htmlFlow 재파싱에 쓴 본문 (CRLF 파일에서는 원문과 다르다).
    pub(crate) sources: Vec<String>,
    /// 종류별 토큰 id (오름차순 = 문서 순서). `filter_by_types` 가 트리를 다시 걷지 않게 한다.
    pub(crate) by_kind: HashMap<&'static str, Vec<TokenId>>,
}

impl TokenTree {
    pub fn get(&self, id: TokenId) -> &Token {
        &self.tokens[id]
    }

    /// 토큰이 가리키는 본문 (원본 `token.text`).
    pub fn text(&self, id: TokenId) -> &str {
        self.text_of(&self.tokens[id])
    }

    /// `text` 와 같지만 id 없이 토큰 참조로 꺼낸다.
    pub fn text_of(&self, t: &Token) -> &str {
        &self.sources[t.src][t.start..t.end]
    }

    /// `tokens` 가 확정된 뒤 한 번 호출. id 는 깊이 우선 선행 순서로 매겨져 있다.
    pub(crate) fn index_kinds(&mut self) {
        self.by_kind.clear();
        for (id, token) in self.tokens.iter().enumerate() {
            self.by_kind.entry(token.kind).or_default().push(id);
        }
    }
}
