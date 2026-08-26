use std::sync::LazyLock;

use regex::Regex;

use super::token::{TokenId, TokenTree};

/// 원본 `nonContentTokens`: 내용을 담지 않는 토큰 타입.
pub const NON_CONTENT_TOKENS: &[&str] = &[
    "blockQuoteMarker",
    "blockQuotePrefix",
    "blockQuotePrefixWhitespace",
    "gfmFootnoteDefinitionIndent",
    "lineEnding",
    "lineEndingBlank",
    "linePrefix",
    "listItemIndent",
    "undefinedReference",
    "undefinedReferenceCollapsed",
    "undefinedReferenceFull",
    "undefinedReferenceShortcut",
];

/// 원본 `getHtmlTagInfo` 의 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlTagInfo {
    pub close: bool,
    pub name: String,
}

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

    /// 원본 `getHeadingText`: heading 텍스트 토큰의 자식 중 htmlText 를 제외한 텍스트를
    /// 이어 붙이고 개행을 공백으로 바꾼다.
    pub fn heading_text(&self, id: TokenId) -> String {
        static NEW_LINE_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"\r\n?|\n").expect("newline regex"));
        let mut text = String::new();
        for t in self.descendants_by_type(id, &[&["atxHeadingText", "setextHeadingText"]]) {
            for &c in &self.tokens[t].children {
                if self.tokens[c].kind != "htmlText" {
                    text.push_str(&self.tokens[c].text);
                }
            }
        }
        NEW_LINE_RE.replace_all(&text, " ").into_owned()
    }

    /// 원본 `isHtmlFlowComment`: HTML 주석을 담은 htmlFlow 토큰인지.
    pub fn is_html_flow_comment(&self, id: TokenId) -> bool {
        let token = &self.tokens[id];
        let text = token.text.as_str();
        if token.kind == "htmlFlow" && text.starts_with("<!--") && text.ends_with("-->") {
            // JS `slice(4, -3)` 은 짧은 문자열에서 빈 문자열을 준다.
            let comment = if text.len() >= 7 {
                &text[4..text.len() - 3]
            } else {
                ""
            };
            return !comment.starts_with('>')
                && !comment.starts_with("->")
                && !comment.ends_with('-');
        }
        false
    }

    /// 원본 `getHtmlTagInfo`: htmlText 토큰의 태그 이름과 닫는 태그 여부.
    pub fn html_tag_info(&self, id: TokenId) -> Option<HtmlTagInfo> {
        static HTML_TAG_NAME_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"^<([^!>][^/\s>]*)").expect("html tag name regex"));
        let token = &self.tokens[id];
        if token.kind != "htmlText" {
            return None;
        }
        let name = HTML_TAG_NAME_RE.captures(&token.text)?[1].to_string();
        let close = name.starts_with('/');
        Some(HtmlTagInfo {
            close,
            name: if close { name[1..].to_string() } else { name },
        })
    }

    /// 원본 `isDocfxTab`: `# [Text](#tab/id)` 꼴의 Docfx 탭 heading 인지.
    pub fn is_docfx_tab(&self, id: TokenId) -> bool {
        if self.tokens[id].kind != "atxHeading" {
            return false;
        }
        let heading_texts = self.descendants_by_type(id, &[&["atxHeadingText"]]);
        if heading_texts.len() != 1 {
            return false;
        }
        let children = &self.tokens[heading_texts[0]].children;
        if children.len() != 1 || self.tokens[children[0]].kind != "link" {
            return false;
        }
        let mut destinations = Vec::new();
        for &c in &self.tokens[children[0]].children {
            self.collect(c, &["resourceDestinationString"], &mut destinations);
        }
        destinations.len() == 1 && self.tokens[destinations[0]].text.starts_with("#tab/")
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

#[cfg(test)]
mod tests {
    use super::super::parse;

    #[test]
    fn heading_text_joins_children_and_skips_html() {
        let tree = parse("# Hello <b>x</b> world\n\nSet\next\n===\n");
        let headings = tree.filter_by_types(&["atxHeading", "setextHeading"]);
        assert_eq!(tree.heading_text(headings[0]), "Hello x world");
        assert_eq!(tree.heading_text(headings[1]), "Set ext");
    }

    #[test]
    fn html_flow_comment_and_tag_info() {
        let tree = parse("<!-- c -->\n\n<div>\n\n<!-->\n\ntext <span>\n");
        let flows = tree.filter_by_types(&["htmlFlow"]);
        assert!(tree.is_html_flow_comment(flows[0]));
        assert!(!tree.is_html_flow_comment(flows[1]));
        assert!(tree.is_html_flow_comment(flows[2]));
        let texts = tree.filter_by_types(&["htmlText"]);
        let info = tree.html_tag_info(texts[0]).unwrap();
        assert_eq!((info.close, info.name.as_str()), (false, "span"));
        assert_eq!(tree.html_tag_info(flows[0]), None);
    }

    #[test]
    fn docfx_tab() {
        let tree = parse("# [Linux](#tab/linux)\n\n# [Other](#other)\n\n# Plain\n");
        let headings = tree.filter_by_types(&["atxHeading"]);
        assert!(tree.is_docfx_tab(headings[0]));
        assert!(!tree.is_docfx_tab(headings[1]));
        assert!(!tree.is_docfx_tab(headings[2]));
    }
}
