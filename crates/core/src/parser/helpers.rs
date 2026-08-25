use super::token::{TokenId, TokenTree};

impl TokenTree {
    /// 원본 `filterByTypes`: 깊이 우선, 문서 순서. 매치된 토큰의 자식도 계속 탐색한다.
    pub fn filter_by_types(&self, kinds: &[&str]) -> Vec<TokenId> {
        let mut out = Vec::new();
        for &r in &self.roots {
            self.collect(r, kinds, &mut out);
        }
        out
    }

    /// 원본 `getDescendantsByType`: `id` 자신은 제외한 후손 중 매치.
    pub fn descendants_by_type(&self, id: TokenId, kinds: &[&str]) -> Vec<TokenId> {
        let mut out = Vec::new();
        for &c in &self.tokens[id].children {
            self.collect(c, kinds, &mut out);
        }
        out
    }

    /// 원본 `getParentOfType`: 가장 가까운 조상 중 매치.
    pub fn parent_of_type(&self, id: TokenId, kinds: &[&str]) -> Option<TokenId> {
        let mut cur = self.tokens[id].parent;
        while let Some(p) = cur {
            if kinds.contains(&self.tokens[p].kind.as_str()) {
                return Some(p);
            }
            cur = self.tokens[p].parent;
        }
        None
    }

    fn collect(&self, id: TokenId, kinds: &[&str], out: &mut Vec<TokenId>) {
        if kinds.contains(&self.tokens[id].kind.as_str()) {
            out.push(id);
        }
        for &c in &self.tokens[id].children {
            self.collect(c, kinds, out);
        }
    }
}
