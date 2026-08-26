use super::token::{TokenId, TokenTree};

impl TokenTree {
    /// 원본 `filterByTypes(tokens, types)`: htmlFlow 재파싱으로 생긴 토큰은 제외한다.
    pub fn filter_by_types(&self, kinds: &[&str]) -> Vec<TokenId> {
        self.filter_by_types_html_flow(kinds, false)
    }

    /// 원본 `filterByTypes(tokens, types, htmlFlow)`: 깊이 우선, 문서 순서.
    /// 매치된 토큰의 자식도 계속 탐색한다. `html_flow` 가 참이면 htmlFlow 안의 토큰도 포함한다.
    pub fn filter_by_types_html_flow(&self, kinds: &[&str], html_flow: bool) -> Vec<TokenId> {
        let mut out = Vec::new();
        for &r in &self.roots {
            self.collect(r, kinds, &mut out);
        }
        if !html_flow {
            out.retain(|&id| !self.tokens[id].in_html_flow);
        }
        out
    }

    /// 원본 `getDescendantsByType`: 타입 경로(typePath)를 따라 한 단계씩 직계 자식만 걸러
    /// 내려간다. 경로의 각 원소는 그 단계에서 허용하는 타입 목록이다.
    pub fn descendants_by_type(&self, id: TokenId, type_path: &[&[&str]]) -> Vec<TokenId> {
        let mut tokens = vec![id];
        for kinds in type_path {
            let mut next = Vec::new();
            for t in tokens {
                for &c in &self.tokens[t].children {
                    if kinds.contains(&self.tokens[c].kind.as_str()) {
                        next.push(c);
                    }
                }
            }
            tokens = next;
        }
        tokens
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

    /// 원본 `getHeadingLevel`: heading 의 sequence 자식 텍스트로 레벨(1~6)을 구한다.
    pub fn heading_level(&self, id: TokenId) -> usize {
        let mut level = 1;
        let sequence = self.tokens[id]
            .children
            .iter()
            .find(|&&c| {
                matches!(
                    self.tokens[c].kind.as_str(),
                    "atxHeadingSequence" | "setextHeadingLine"
                )
            })
            .expect("heading has a sequence child");
        let text = &self.tokens[*sequence].text;
        if text.starts_with('#') {
            level = text.chars().count().min(6);
        } else if text.starts_with('-') {
            level = 2;
        }
        level
    }

    /// 원본 `getHeadingStyle`: "atx" | "atx_closed" | "setext".
    pub fn heading_style(&self, id: TokenId) -> &'static str {
        let heading = &self.tokens[id];
        if heading.kind == "setextHeading" {
            return "setext";
        }
        let atx_heading_sequence_length = heading
            .children
            .iter()
            .filter(|&&c| self.tokens[c].kind == "atxHeadingSequence")
            .count();
        if atx_heading_sequence_length == 1 {
            "atx"
        } else {
            "atx_closed"
        }
    }

    /// 원본 `getBlockQuotePrefixText`: 주어진 토큰들에서 해당 줄의 blockQuotePrefix,
    /// linePrefix 텍스트를 이어 붙이고 끝 공백을 지운 뒤 개행을 더해 `count` 번 반복한다.
    pub fn block_quote_prefix_text(
        &self,
        tokens: &[TokenId],
        line_number: usize,
        count: usize,
    ) -> String {
        let mut prefixes = Vec::new();
        for &id in tokens {
            self.collect(id, &["blockQuotePrefix", "linePrefix"], &mut prefixes);
        }
        let joined: String = prefixes
            .iter()
            .filter(|&&id| {
                !self.tokens[id].in_html_flow && self.tokens[id].start_line == line_number
            })
            .map(|&id| self.tokens[id].text.as_str())
            .collect();
        format!("{}\n", joined.trim_end()).repeat(count)
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
