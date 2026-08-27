use std::collections::HashMap;

pub type TokenId = usize;

/// micromark(JS) 와 동형인 토큰. `kind` 는 micromark 토큰 타입 문자열 (예: `atxHeading`).
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: String,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub text: String,
    pub parent: Option<TokenId>,
    pub children: Vec<TokenId>,
    /// 원본 `htmlFlowSymbol`: htmlFlow 재파싱으로 생긴 토큰인지 (`inHtmlFlow`).
    pub in_html_flow: bool,
}

#[derive(Debug, Default)]
pub struct TokenTree {
    pub tokens: Vec<Token>,
    pub roots: Vec<TokenId>,
    /// 종류별 토큰 id (오름차순 = 문서 순서). `filter_by_types` 가 트리를 다시 걷지 않게 한다.
    pub(crate) by_kind: HashMap<String, Vec<TokenId>>,
}

impl TokenTree {
    pub fn get(&self, id: TokenId) -> &Token {
        &self.tokens[id]
    }

    /// `tokens` 가 확정된 뒤 한 번 호출. id 는 깊이 우선 선행 순서로 매겨져 있다.
    pub(crate) fn index_kinds(&mut self) {
        self.by_kind.clear();
        for (id, token) in self.tokens.iter().enumerate() {
            match self.by_kind.get_mut(&token.kind) {
                Some(ids) => ids.push(id),
                None => {
                    self.by_kind.insert(token.kind.clone(), vec![id]);
                }
            }
        }
    }
}
