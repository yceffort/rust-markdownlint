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
}

#[derive(Debug, Default)]
pub struct TokenTree {
    pub tokens: Vec<Token>,
    pub roots: Vec<TokenId>,
}

impl TokenTree {
    pub fn get(&self, id: TokenId) -> &Token {
        &self.tokens[id]
    }
}
