use super::token::{TokenId, TokenTree};

impl TokenTree {
    /// `{ t, s:[line,col], e:[line,col], c:[...] }` 형태의 JSON (원본 대조용).
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::Value::Array(self.roots.iter().map(|&r| self.node_json(r)).collect())
    }

    fn node_json(&self, id: TokenId) -> serde_json::Value {
        let t = &self.tokens[id];
        serde_json::json!({
            "t": t.kind,
            "s": [t.start_line, t.start_column],
            "e": [t.end_line, t.end_column],
            "c": t.children.iter().map(|&c| self.node_json(c)).collect::<Vec<_>>(),
        })
    }
}
