use std::collections::HashMap;
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

/// JS 정규식의 `\s` 문자 집합 (문자 클래스 안에 넣어 쓴다). Rust 의 `\s` (Unicode
/// White_Space) 와 달리 U+0085 를 빼고 U+FEFF 를 넣는다.
pub const JS_WHITESPACE: &str =
    r"\t\n\x0B\f\r \u{a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}";

/// `JS_WHITESPACE` 와 같은 집합의 문자 판정 (정규식을 돌리기 전 첫/끝 글자 사전 검사용).
pub fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// 원본 `getHtmlAttributeRe(name)`: `/\s{name}\s*=\s*['"]?([^'"\s>]*)/iu`.
pub fn html_attribute_re(name: &str) -> Regex {
    Regex::new(&format!(
        r#"(?i)[{JS_WHITESPACE}]{name}[{JS_WHITESPACE}]*=[{JS_WHITESPACE}]*['"]?([^'"{JS_WHITESPACE}>]*)"#
    ))
    .expect("html attribute regex")
}

/// 원본 `getHtmlTagInfo` 의 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlTagInfo {
    pub close: bool,
    pub name: String,
}

/// 원본 `getReferenceLinkImageData` 의 참조 위치 `[lineIndex, index, length]` (0 기반).
pub type ReferenceDatum = [usize; 3];

/// 삽입 순서를 유지하는 label → 값 맵 (JS `Map` 의 순회 순서를 재현한다).
#[derive(Debug, Default)]
pub struct OrderedMap<V> {
    entries: Vec<(String, V)>,
    index: HashMap<String, usize>,
}

impl<V> OrderedMap<V> {
    pub fn get(&self, key: &str) -> Option<&V> {
        self.index.get(key).map(|&i| &self.entries[i].1)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn entries(&self) -> &[(String, V)] {
        &self.entries
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(k, _)| k.as_str())
    }

    /// JS `map.set(key, value)`: 기존 키면 값만 바꾸고 순서는 유지한다.
    pub fn set(&mut self, key: String, value: V) {
        match self.index.get(&key) {
            Some(&i) => self.entries[i].1 = value,
            None => {
                self.index.insert(key.clone(), self.entries.len());
                self.entries.push((key, value));
            }
        }
    }
}

impl<V: Default> OrderedMap<V> {
    pub fn entry(&mut self, key: String) -> &mut V {
        if !self.index.contains_key(&key) {
            self.set(key.clone(), V::default());
        }
        let i = self.index[&key];
        &mut self.entries[i].1
    }
}

/// 원본 `getReferenceLinkImageData` 의 결과.
#[derive(Debug, Default)]
pub struct ReferenceLinkImageData {
    /// full/collapsed 참조: 정규화한 label → 위치 목록.
    pub references: OrderedMap<Vec<ReferenceDatum>>,
    /// shortcut 참조 (`[text]`): 정규화한 label → 위치 목록.
    pub shortcuts: OrderedMap<Vec<ReferenceDatum>>,
    /// 정의: 정규화한 label → `(lineIndex, destination)`.
    pub definitions: OrderedMap<(usize, String)>,
    /// 중복 정의: `(정규화한 label, lineIndex)`.
    pub duplicate_definitions: Vec<(String, usize)>,
    /// 정의가 차지하는 줄의 lineIndex (0 기반).
    pub definition_line_indices: Vec<usize>,
}

/// 원본 `normalizeReference`: 소문자화, trim, 연속 공백을 한 칸으로.
fn normalize_reference(s: &str) -> String {
    static WS_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(&format!("[{JS_WHITESPACE}]+")).expect("whitespace regex"));
    WS_RE.replace_all(s.to_lowercase().trim(), " ").into_owned()
}

impl TokenTree {
    /// 원본 `filterByPredicate(tokens, allowed, transformChildren)`: `roots` 부터 전위 순회로
    /// `allowed` 를 만족하는 토큰을 모은다. 자식이 있는 토큰은 `transform_children` 이 `out` 에
    /// 채운 목록으로 계속 내려간다 (원본 기본값은 `token.children`). 토큰마다 Vec 을 새로
    /// 만들지 않도록 출력 버퍼를 넘긴다.
    pub fn filter_by_predicate(
        &self,
        roots: &[TokenId],
        allowed: impl Fn(&TokenTree, TokenId) -> bool,
        transform_children: impl Fn(&TokenTree, TokenId, &mut Vec<TokenId>),
    ) -> Vec<TokenId> {
        let mut result = Vec::new();
        let mut stack: Vec<TokenId> = roots.iter().rev().copied().collect();
        let mut children = Vec::new();
        while let Some(id) = stack.pop() {
            if allowed(self, id) {
                result.push(id);
            }
            if !self.tokens[id].children.is_empty() {
                children.clear();
                transform_children(self, id, &mut children);
                stack.extend(children.iter().rev());
            }
        }
        result
    }

    /// 원본 `getReferenceLinkImageData`: 참조 링크/이미지/각주와 정의를 모은다.
    pub fn reference_link_image_data(&self) -> ReferenceLinkImageData {
        // 원본 `getText`: blockQuotePrefix 를 뺀 자식 텍스트를 이어 붙인다.
        let get_text = |id: Option<TokenId>| -> Option<String> {
            id.map(|id| {
                self.tokens[id]
                    .children
                    .iter()
                    .filter(|&&c| self.tokens[c].kind != "blockQuotePrefix")
                    .map(|&c| self.text(c))
                    .collect()
            })
        };
        let mut data = ReferenceLinkImageData::default();
        let add_reference_to_dictionary =
            |data: &mut ReferenceLinkImageData, id: TokenId, label: &str, is_shortcut: bool| {
                let token = &self.tokens[id];
                let datum = [
                    token.start_line - 1,
                    token.start_column - 1,
                    self.text(id).chars().count(),
                ];
                let dictionary = if is_shortcut {
                    &mut data.shortcuts
                } else {
                    &mut data.references
                };
                dictionary.entry(normalize_reference(label)).push(datum);
            };
        let filtered = self.filter_by_types(&[
            // definitionLineIndices
            "definition",
            "gfmFootnoteDefinition",
            // definitions and definitionLineIndices
            "definitionLabelString",
            "gfmFootnoteDefinitionLabelString",
            // references and shortcuts
            "gfmFootnoteCall",
            "image",
            "link",
            // undefined link labels
            "undefinedReferenceCollapsed",
            "undefinedReferenceFull",
            "undefinedReferenceShortcut",
        ]);
        for id in filtered {
            let token = &self.tokens[id];
            match token.kind {
                "definition" | "gfmFootnoteDefinition" => {
                    for line in token.start_line..=token.end_line {
                        data.definition_line_indices.push(line - 1);
                    }
                }
                kind @ ("definitionLabelString" | "gfmFootnoteDefinitionLabelString") => {
                    let label_prefix = if kind == "gfmFootnoteDefinitionLabelString" {
                        "^"
                    } else {
                        ""
                    };
                    let reference =
                        normalize_reference(&format!("{label_prefix}{}", self.text(id)));
                    if data.definitions.contains_key(&reference) {
                        data.duplicate_definitions
                            .push((reference, token.start_line - 1));
                    } else {
                        let destination = self
                            .parent_of_type(id, &["definition"])
                            .and_then(|parent| {
                                self.descendants_by_type(
                                    parent,
                                    &[
                                        &["definitionDestination"],
                                        &["definitionDestinationRaw"],
                                        &["definitionDestinationString"],
                                    ],
                                )
                                .first()
                                .map(|&d| self.text(d).to_string())
                            })
                            .unwrap_or_default();
                        data.definitions
                            .set(reference, (token.start_line - 1, destination));
                    }
                }
                "gfmFootnoteCall" | "image" | "link" => {
                    // shortcut 인지, full/collapsed 인지 판별
                    let children = &token.children;
                    let mut is_shortcut = children.len() == 1;
                    let is_full_or_collapsed = children.len() == 2
                        && !children.iter().any(|&c| self.tokens[c].kind == "resource");
                    let label_text = self
                        .descendants_by_type(id, &[&["label"], &["labelText"]])
                        .first()
                        .copied();
                    let reference_string = self
                        .descendants_by_type(id, &[&["reference"], &["referenceString"]])
                        .first()
                        .copied();
                    let mut label = get_text(label_text).unwrap_or_default();
                    // 각주인지 판별
                    if !is_shortcut && !is_full_or_collapsed {
                        let mut call = children.iter().filter(|&&c| {
                            matches!(
                                self.tokens[c].kind,
                                "gfmFootnoteCallMarker" | "gfmFootnoteCallString"
                            )
                        });
                        if let (Some(&marker), Some(&string)) = (call.next(), call.next()) {
                            label = format!("{}{}", self.text(marker), self.text(string));
                            is_shortcut = true;
                        }
                    }
                    // 링크 추적 (shortcut 은 "text [text] text" 의 모호함 때문에 따로 둔다)
                    if is_shortcut || is_full_or_collapsed {
                        let label = get_text(reference_string)
                            .filter(|s| !s.is_empty())
                            .unwrap_or(label);
                        add_reference_to_dictionary(&mut data, id, &label, is_shortcut);
                    }
                }
                kind @ ("undefinedReferenceCollapsed"
                | "undefinedReferenceFull"
                | "undefinedReferenceShortcut") => {
                    let undefined_reference =
                        self.descendants_by_type(id, &[&["undefinedReference"]])[0];
                    let label: String = self.tokens[undefined_reference]
                        .children
                        .iter()
                        .map(|&c| self.text(c))
                        .collect();
                    let is_shortcut = kind == "undefinedReferenceShortcut";
                    add_reference_to_dictionary(&mut data, id, &label, is_shortcut);
                }
                _ => {}
            }
        }
        data
    }

    /// 원본 `filterByTypes(tokens, types)`: htmlFlow 재파싱으로 생긴 토큰은 제외한다.
    pub fn filter_by_types(&self, kinds: &[&str]) -> Vec<TokenId> {
        self.filter_by_types_html_flow(kinds, false)
    }

    /// 원본 `filterByTypes(tokens, types, htmlFlow)`: 깊이 우선, 문서 순서.
    /// 매치된 토큰의 자식도 계속 탐색한다. `html_flow` 가 참이면 htmlFlow 안의 토큰도 포함한다.
    pub fn filter_by_types_html_flow(&self, kinds: &[&str], html_flow: bool) -> Vec<TokenId> {
        let mut out: Vec<TokenId> = kinds
            .iter()
            .flat_map(|kind| self.ids_of_kind(kind))
            .copied()
            .collect();
        if kinds.len() > 1 {
            out.sort_unstable();
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
                    if kinds.contains(&self.tokens[c].kind) {
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
            if kinds.contains(&self.tokens[p].kind) {
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
                    self.tokens[c].kind,
                    "atxHeadingSequence" | "setextHeadingLine"
                )
            })
            .expect("heading has a sequence child");
        let text = self.text(*sequence);
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
                    text.push_str(self.text(c));
                }
            }
        }
        NEW_LINE_RE.replace_all(&text, " ").into_owned()
    }

    /// 원본 `isHtmlFlowComment`: HTML 주석을 담은 htmlFlow 토큰인지.
    pub fn is_html_flow_comment(&self, id: TokenId) -> bool {
        let token = &self.tokens[id];
        let text = self.text(id);
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
        let name = HTML_TAG_NAME_RE.captures(self.text(id))?[1].to_string();
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
        destinations.len() == 1 && self.text(destinations[0]).starts_with("#tab/")
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
            .map(|&id| self.text(id))
            .collect();
        format!("{}\n", joined.trim_end()).repeat(count)
    }

    fn collect(&self, id: TokenId, kinds: &[&str], out: &mut Vec<TokenId>) {
        if kinds.contains(&self.tokens[id].kind) {
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
    fn filter_by_predicate_respects_transformed_children() {
        let tree = parse("# H *em*\n\n<div>\n*x*\n</div>\n");
        // htmlFlow 아래로 내려가지 않으면 본문의 emphasis 만 남는다.
        let ids = tree.filter_by_predicate(
            &tree.roots,
            |t, id| t.get(id).kind == "emphasis",
            |t, id, out| {
                if t.get(id).kind != "htmlFlow" {
                    out.extend_from_slice(&t.get(id).children);
                }
            },
        );
        assert_eq!(ids.len(), 1);
        assert_eq!(tree.get(ids[0]).start_line, 1);
    }

    /// 기대값은 원본 helpers.cjs `getReferenceLinkImageData` 를 Node 로 실행해 얻었다.
    #[test]
    fn reference_link_image_data_matches_original() {
        let text = "# Title\n\nA [full][Label One] and [collapsed][] and [shortcut] link.\nAn image ![alt][img] and footnote[^note] here.\n\n> Quoted [Label One][] text.\n\n[label one]: https://example.com/one \"One\"\n[collapsed]: <https://example.com/two>\n[img]: image.png\n[label one]: https://example.com/dup\n[^note]: The note\n    continues here.\n[unused]: #\n\nUndefined [missing] and [missing][] and [x][missing two].\n";
        let data = parse(text).reference_link_image_data();
        let entries = |m: &super::OrderedMap<Vec<[usize; 3]>>| -> Vec<(String, Vec<[usize; 3]>)> {
            m.entries().to_vec()
        };
        assert_eq!(
            entries(&data.references),
            vec![
                ("label one".into(), vec![[2, 2, 17], [5, 9, 13]]),
                ("collapsed".into(), vec![[2, 24, 13]]),
                ("img".into(), vec![[3, 9, 11]]),
                ("missing".into(), vec![[15, 24, 11]]),
                ("missing two".into(), vec![[15, 40, 16]]),
            ]
        );
        assert_eq!(
            entries(&data.shortcuts),
            vec![
                ("^note".into(), vec![[3, 33, 7]]),
                ("shortcut".into(), vec![[2, 42, 10]]),
                ("unused".into(), vec![[13, 0, 8]]),
                ("missing".into(), vec![[15, 10, 9]]),
            ]
        );
        assert_eq!(
            data.definitions.entries().to_vec(),
            vec![
                ("label one".into(), (7, "https://example.com/one".into())),
                ("collapsed".into(), (8, String::new())),
                ("img".into(), (9, "image.png".into())),
                ("^note".into(), (11, String::new())),
            ]
        );
        assert_eq!(data.duplicate_definitions, vec![("label one".into(), 10)]);
        assert_eq!(data.definition_line_indices, vec![7, 8, 9, 10, 11, 12, 13]);
    }

    #[test]
    fn html_attribute_re_captures_value() {
        let re = super::html_attribute_re("alt");
        assert_eq!(&re.captures("<img ALT=\"x y\" src=a>").unwrap()[1], "x");
        assert_eq!(&re.captures("<img alt = y>").unwrap()[1], "y");
        assert!(re.captures("<img data-alt=x>").is_none());
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
