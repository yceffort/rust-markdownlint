//! 토큰 종류. micromark(JS) 이름 문자열과 1:1 인 판별값으로, 어댑터 변환 패스와 종류별 인덱스가
//! 문자열 비교와 해시 대신 정수 비교와 배열 접근을 쓰게 한다. `name()` 이 규칙이 보는 이름이다.
use markdown::event::Name;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Kind(u16);

impl Kind {
    pub(crate) const NONE: Kind = Kind(0);
    pub(crate) const ATTENTION_SEQUENCE: Kind = Kind(1);
    pub(crate) const AUTOLINK: Kind = Kind(2);
    pub(crate) const AUTOLINK_EMAIL: Kind = Kind(3);
    pub(crate) const AUTOLINK_MARKER: Kind = Kind(4);
    pub(crate) const AUTOLINK_PROTOCOL: Kind = Kind(5);
    pub(crate) const LINE_ENDING_BLANK: Kind = Kind(6);
    pub(crate) const BLOCK_QUOTE: Kind = Kind(7);
    pub(crate) const BLOCK_QUOTE_MARKER: Kind = Kind(8);
    pub(crate) const BLOCK_QUOTE_PREFIX: Kind = Kind(9);
    pub(crate) const BYTE_ORDER_MARK: Kind = Kind(10);
    pub(crate) const CHARACTER_ESCAPE: Kind = Kind(11);
    pub(crate) const ESCAPE_MARKER: Kind = Kind(12);
    pub(crate) const CHARACTER_ESCAPE_VALUE: Kind = Kind(13);
    pub(crate) const CHARACTER_REFERENCE: Kind = Kind(14);
    pub(crate) const CHARACTER_REFERENCE_MARKER: Kind = Kind(15);
    pub(crate) const CHARACTER_REFERENCE_MARKER_HEXADECIMAL: Kind = Kind(16);
    pub(crate) const CHARACTER_REFERENCE_MARKER_NUMERIC: Kind = Kind(17);
    pub(crate) const CHARACTER_REFERENCE_VALUE: Kind = Kind(18);
    pub(crate) const CODE_FENCED: Kind = Kind(19);
    pub(crate) const CODE_FENCED_FENCE: Kind = Kind(20);
    pub(crate) const CODE_FENCED_FENCE_INFO: Kind = Kind(21);
    pub(crate) const CODE_FENCED_FENCE_META: Kind = Kind(22);
    pub(crate) const CODE_FENCED_FENCE_SEQUENCE: Kind = Kind(23);
    pub(crate) const CODE_FLOW_VALUE: Kind = Kind(24);
    pub(crate) const CODE_INDENTED: Kind = Kind(25);
    pub(crate) const CODE_TEXT: Kind = Kind(26);
    pub(crate) const CODE_TEXT_DATA: Kind = Kind(27);
    pub(crate) const CODE_TEXT_SEQUENCE: Kind = Kind(28);
    pub(crate) const CONTENT: Kind = Kind(29);
    pub(crate) const DATA: Kind = Kind(30);
    pub(crate) const DEFINITION: Kind = Kind(31);
    pub(crate) const DEFINITION_DESTINATION: Kind = Kind(32);
    pub(crate) const DEFINITION_DESTINATION_LITERAL: Kind = Kind(33);
    pub(crate) const DEFINITION_DESTINATION_LITERAL_MARKER: Kind = Kind(34);
    pub(crate) const DEFINITION_DESTINATION_RAW: Kind = Kind(35);
    pub(crate) const DEFINITION_DESTINATION_STRING: Kind = Kind(36);
    pub(crate) const DEFINITION_LABEL: Kind = Kind(37);
    pub(crate) const DEFINITION_LABEL_MARKER: Kind = Kind(38);
    pub(crate) const DEFINITION_LABEL_STRING: Kind = Kind(39);
    pub(crate) const DEFINITION_MARKER: Kind = Kind(40);
    pub(crate) const DEFINITION_TITLE: Kind = Kind(41);
    pub(crate) const DEFINITION_TITLE_MARKER: Kind = Kind(42);
    pub(crate) const DEFINITION_TITLE_STRING: Kind = Kind(43);
    pub(crate) const DIRECTIVE_CONTAINER: Kind = Kind(44);
    pub(crate) const DIRECTIVE_CONTAINER_ATTRIBUTES: Kind = Kind(45);
    pub(crate) const DIRECTIVE_CONTAINER_ATTRIBUTES_MARKER: Kind = Kind(46);
    pub(crate) const DIRECTIVE_CONTAINER_CHUNK: Kind = Kind(47);
    pub(crate) const DIRECTIVE_CONTAINER_CONTENT: Kind = Kind(48);
    pub(crate) const DIRECTIVE_CONTAINER_FENCE: Kind = Kind(49);
    pub(crate) const DIRECTIVE_CONTAINER_LABEL: Kind = Kind(50);
    pub(crate) const DIRECTIVE_CONTAINER_LABEL_MARKER: Kind = Kind(51);
    pub(crate) const DIRECTIVE_CONTAINER_LABEL_STRING: Kind = Kind(52);
    pub(crate) const DIRECTIVE_CONTAINER_NAME: Kind = Kind(53);
    pub(crate) const DIRECTIVE_CONTAINER_SEQUENCE: Kind = Kind(54);
    pub(crate) const EMPHASIS: Kind = Kind(55);
    pub(crate) const EMPHASIS_SEQUENCE: Kind = Kind(56);
    pub(crate) const EMPHASIS_TEXT: Kind = Kind(57);
    pub(crate) const FRONTMATTER: Kind = Kind(58);
    pub(crate) const FRONTMATTER_CHUNK: Kind = Kind(59);
    pub(crate) const FRONTMATTER_FENCE: Kind = Kind(60);
    pub(crate) const FRONTMATTER_SEQUENCE: Kind = Kind(61);
    pub(crate) const LITERAL_AUTOLINK_EMAIL: Kind = Kind(62);
    pub(crate) const GFM_AUTOLINK_LITERAL_MAILTO: Kind = Kind(63);
    pub(crate) const LITERAL_AUTOLINK_HTTP: Kind = Kind(64);
    pub(crate) const LITERAL_AUTOLINK_WWW: Kind = Kind(65);
    pub(crate) const GFM_AUTOLINK_LITERAL_XMPP: Kind = Kind(66);
    pub(crate) const GFM_FOOTNOTE_CALL: Kind = Kind(67);
    pub(crate) const GFM_FOOTNOTE_CALL_LABEL: Kind = Kind(68);
    pub(crate) const GFM_FOOTNOTE_CALL_MARKER: Kind = Kind(69);
    pub(crate) const GFM_FOOTNOTE_DEFINITION: Kind = Kind(70);
    pub(crate) const GFM_FOOTNOTE_DEFINITION_PREFIX: Kind = Kind(71);
    pub(crate) const GFM_FOOTNOTE_DEFINITION_LABEL: Kind = Kind(72);
    pub(crate) const GFM_FOOTNOTE_DEFINITION_LABEL_MARKER: Kind = Kind(73);
    pub(crate) const GFM_FOOTNOTE_DEFINITION_LABEL_STRING: Kind = Kind(74);
    pub(crate) const GFM_FOOTNOTE_DEFINITION_MARKER: Kind = Kind(75);
    pub(crate) const GFM_STRIKETHROUGH: Kind = Kind(76);
    pub(crate) const GFM_STRIKETHROUGH_SEQUENCE: Kind = Kind(77);
    pub(crate) const GFM_STRIKETHROUGH_TEXT: Kind = Kind(78);
    pub(crate) const TABLE: Kind = Kind(79);
    pub(crate) const TABLE_BODY: Kind = Kind(80);
    pub(crate) const TABLE_CELL: Kind = Kind(81);
    pub(crate) const TABLE_CONTENT: Kind = Kind(82);
    pub(crate) const TABLE_CELL_DIVIDER: Kind = Kind(83);
    pub(crate) const TABLE_DELIMITER_ROW: Kind = Kind(84);
    pub(crate) const TABLE_DELIMITER_MARKER: Kind = Kind(85);
    pub(crate) const TABLE_DELIMITER: Kind = Kind(86);
    pub(crate) const TABLE_DELIMITER_FILLER: Kind = Kind(87);
    pub(crate) const TABLE_HEAD: Kind = Kind(88);
    pub(crate) const TABLE_ROW: Kind = Kind(89);
    pub(crate) const GFM_TASK_LIST_ITEM_CHECK: Kind = Kind(90);
    pub(crate) const GFM_TASK_LIST_ITEM_MARKER: Kind = Kind(91);
    pub(crate) const GFM_TASK_LIST_ITEM_VALUE_CHECKED: Kind = Kind(92);
    pub(crate) const GFM_TASK_LIST_ITEM_VALUE_UNCHECKED: Kind = Kind(93);
    pub(crate) const HARD_BREAK_ESCAPE: Kind = Kind(94);
    pub(crate) const HARD_BREAK_TRAILING: Kind = Kind(95);
    pub(crate) const ATX_HEADING: Kind = Kind(96);
    pub(crate) const ATX_HEADING_SEQUENCE: Kind = Kind(97);
    pub(crate) const ATX_HEADING_TEXT: Kind = Kind(98);
    pub(crate) const SETEXT_HEADING: Kind = Kind(99);
    pub(crate) const SETEXT_HEADING_TEXT: Kind = Kind(100);
    pub(crate) const SETEXT_HEADING_LINE: Kind = Kind(101);
    pub(crate) const SETEXT_HEADING_LINE_SEQUENCE: Kind = Kind(102);
    pub(crate) const HTML_FLOW: Kind = Kind(103);
    pub(crate) const HTML_FLOW_DATA: Kind = Kind(104);
    pub(crate) const HTML_TEXT: Kind = Kind(105);
    pub(crate) const HTML_TEXT_DATA: Kind = Kind(106);
    pub(crate) const IMAGE: Kind = Kind(107);
    pub(crate) const LABEL: Kind = Kind(108);
    pub(crate) const LABEL_END: Kind = Kind(109);
    pub(crate) const LABEL_IMAGE: Kind = Kind(110);
    pub(crate) const LABEL_IMAGE_MARKER: Kind = Kind(111);
    pub(crate) const LABEL_LINK: Kind = Kind(112);
    pub(crate) const LABEL_MARKER: Kind = Kind(113);
    pub(crate) const LABEL_TEXT: Kind = Kind(114);
    pub(crate) const LINE_ENDING: Kind = Kind(115);
    pub(crate) const LINK: Kind = Kind(116);
    pub(crate) const LIST_ITEM: Kind = Kind(117);
    pub(crate) const LIST_ITEM_MARKER: Kind = Kind(118);
    pub(crate) const LIST_ITEM_PREFIX: Kind = Kind(119);
    pub(crate) const LIST_ITEM_VALUE: Kind = Kind(120);
    pub(crate) const LIST_ORDERED: Kind = Kind(121);
    pub(crate) const LIST_UNORDERED: Kind = Kind(122);
    pub(crate) const MATH_FLOW: Kind = Kind(123);
    pub(crate) const MATH_FLOW_FENCE: Kind = Kind(124);
    pub(crate) const MATH_FLOW_FENCE_META: Kind = Kind(125);
    pub(crate) const MATH_FLOW_FENCE_SEQUENCE: Kind = Kind(126);
    pub(crate) const MATH_FLOW_VALUE: Kind = Kind(127);
    pub(crate) const MATH_TEXT: Kind = Kind(128);
    pub(crate) const MATH_TEXT_DATA: Kind = Kind(129);
    pub(crate) const MATH_TEXT_SEQUENCE: Kind = Kind(130);
    pub(crate) const MDX_ESM: Kind = Kind(131);
    pub(crate) const MDX_ESM_DATA: Kind = Kind(132);
    pub(crate) const MDX_EXPRESSION_MARKER: Kind = Kind(133);
    pub(crate) const MDX_EXPRESSION_DATA: Kind = Kind(134);
    pub(crate) const MDX_FLOW_EXPRESSION: Kind = Kind(135);
    pub(crate) const MDX_TEXT_EXPRESSION: Kind = Kind(136);
    pub(crate) const MDX_JSX_FLOW_TAG: Kind = Kind(137);
    pub(crate) const MDX_JSX_TEXT_TAG: Kind = Kind(138);
    pub(crate) const MDX_JSX_ES_WHITESPACE: Kind = Kind(139);
    pub(crate) const MDX_JSX_TAG_MARKER: Kind = Kind(140);
    pub(crate) const MDX_JSX_TAG_CLOSING_MARKER: Kind = Kind(141);
    pub(crate) const MDX_JSX_TAG_NAME: Kind = Kind(142);
    pub(crate) const MDX_JSX_TAG_NAME_PRIMARY: Kind = Kind(143);
    pub(crate) const MDX_JSX_TAG_NAME_MEMBER_MARKER: Kind = Kind(144);
    pub(crate) const MDX_JSX_TAG_NAME_PREFIX_MARKER: Kind = Kind(145);
    pub(crate) const MDX_JSX_TAG_NAME_MEMBER: Kind = Kind(146);
    pub(crate) const MDX_JSX_TAG_NAME_LOCAL: Kind = Kind(147);
    pub(crate) const MDX_JSX_TAG_ATTRIBUTE: Kind = Kind(148);
    pub(crate) const MDX_JSX_TAG_ATTRIBUTE_EXPRESSION: Kind = Kind(149);
    pub(crate) const MDX_JSX_TAG_ATTRIBUTE_NAME: Kind = Kind(150);
    pub(crate) const MDX_JSX_TAG_ATTRIBUTE_PRIMARY_NAME: Kind = Kind(151);
    pub(crate) const MDX_JSX_TAG_ATTRIBUTE_NAME_PREFIX_MARKER: Kind = Kind(152);
    pub(crate) const MDX_JSX_TAG_ATTRIBUTE_NAME_LOCAL: Kind = Kind(153);
    pub(crate) const MDX_JSX_TAG_ATTRIBUTE_INITIALIZER_MARKER: Kind = Kind(154);
    pub(crate) const MDX_JSX_TAG_ATTRIBUTE_VALUE_EXPRESSION: Kind = Kind(155);
    pub(crate) const MDX_JSX_TAG_ATTRIBUTE_VALUE_LITERAL: Kind = Kind(156);
    pub(crate) const MDX_JSX_TAG_ATTRIBUTE_VALUE_LITERAL_MARKER: Kind = Kind(157);
    pub(crate) const MDX_JSX_TAG_ATTRIBUTE_VALUE_LITERAL_VALUE: Kind = Kind(158);
    pub(crate) const MDX_JSX_TAG_SELF_CLOSING_MARKER: Kind = Kind(159);
    pub(crate) const PARAGRAPH: Kind = Kind(160);
    pub(crate) const REFERENCE: Kind = Kind(161);
    pub(crate) const REFERENCE_MARKER: Kind = Kind(162);
    pub(crate) const REFERENCE_STRING: Kind = Kind(163);
    pub(crate) const RESOURCE: Kind = Kind(164);
    pub(crate) const RESOURCE_DESTINATION: Kind = Kind(165);
    pub(crate) const RESOURCE_DESTINATION_LITERAL: Kind = Kind(166);
    pub(crate) const RESOURCE_DESTINATION_LITERAL_MARKER: Kind = Kind(167);
    pub(crate) const RESOURCE_DESTINATION_RAW: Kind = Kind(168);
    pub(crate) const RESOURCE_DESTINATION_STRING: Kind = Kind(169);
    pub(crate) const RESOURCE_MARKER: Kind = Kind(170);
    pub(crate) const RESOURCE_TITLE: Kind = Kind(171);
    pub(crate) const RESOURCE_TITLE_MARKER: Kind = Kind(172);
    pub(crate) const RESOURCE_TITLE_STRING: Kind = Kind(173);
    pub(crate) const SPACE_OR_TAB: Kind = Kind(174);
    pub(crate) const STRONG: Kind = Kind(175);
    pub(crate) const STRONG_SEQUENCE: Kind = Kind(176);
    pub(crate) const STRONG_TEXT: Kind = Kind(177);
    pub(crate) const THEMATIC_BREAK: Kind = Kind(178);
    pub(crate) const THEMATIC_BREAK_SEQUENCE: Kind = Kind(179);
    pub(crate) const LINE_PREFIX: Kind = Kind(180);
    pub(crate) const ROOT: Kind = Kind(181);
    pub(crate) const LINE_SUFFIX: Kind = Kind(182);
    pub(crate) const WHITESPACE: Kind = Kind(183);
    pub(crate) const LIST_ITEM_INDENT: Kind = Kind(184);
    pub(crate) const LIST_ITEM_PREFIX_WHITESPACE: Kind = Kind(185);
    pub(crate) const BLOCK_QUOTE_PREFIX_WHITESPACE: Kind = Kind(186);
    pub(crate) const GFM_FOOTNOTE_DEFINITION_WHITESPACE: Kind = Kind(187);
    pub(crate) const GFM_FOOTNOTE_DEFINITION_INDENT: Kind = Kind(188);
    pub(crate) const LITERAL_AUTOLINK: Kind = Kind(189);
    pub(crate) const TABLE_HEADER: Kind = Kind(190);
    pub(crate) const TABLE_DATA: Kind = Kind(191);
    pub(crate) const CODE_TEXT_PADDING: Kind = Kind(192);
    pub(crate) const UNDEFINED_REFERENCE: Kind = Kind(193);
    pub(crate) const UNDEFINED_REFERENCE_SHORTCUT: Kind = Kind(194);
    pub(crate) const UNDEFINED_REFERENCE_COLLAPSED: Kind = Kind(195);
    pub(crate) const UNDEFINED_REFERENCE_FULL: Kind = Kind(196);
    pub(crate) const GFM_FOOTNOTE_CALL_LABEL_MARKER: Kind = Kind(197);
    pub(crate) const GFM_FOOTNOTE_CALL_STRING: Kind = Kind(198);
    pub(crate) const COUNT: usize = 199;

    pub(crate) fn name(self) -> &'static str {
        NAMES[self.0 as usize]
    }

    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }

    pub(crate) fn from_name(name: &str) -> Option<Kind> {
        match name {
            "attentionSequence" => Some(Self::ATTENTION_SEQUENCE),
            "autolink" => Some(Self::AUTOLINK),
            "autolinkEmail" => Some(Self::AUTOLINK_EMAIL),
            "autolinkMarker" => Some(Self::AUTOLINK_MARKER),
            "autolinkProtocol" => Some(Self::AUTOLINK_PROTOCOL),
            "lineEndingBlank" => Some(Self::LINE_ENDING_BLANK),
            "blockQuote" => Some(Self::BLOCK_QUOTE),
            "blockQuoteMarker" => Some(Self::BLOCK_QUOTE_MARKER),
            "blockQuotePrefix" => Some(Self::BLOCK_QUOTE_PREFIX),
            "byteOrderMark" => Some(Self::BYTE_ORDER_MARK),
            "characterEscape" => Some(Self::CHARACTER_ESCAPE),
            "escapeMarker" => Some(Self::ESCAPE_MARKER),
            "characterEscapeValue" => Some(Self::CHARACTER_ESCAPE_VALUE),
            "characterReference" => Some(Self::CHARACTER_REFERENCE),
            "characterReferenceMarker" => Some(Self::CHARACTER_REFERENCE_MARKER),
            "characterReferenceMarkerHexadecimal" => {
                Some(Self::CHARACTER_REFERENCE_MARKER_HEXADECIMAL)
            }
            "characterReferenceMarkerNumeric" => Some(Self::CHARACTER_REFERENCE_MARKER_NUMERIC),
            "characterReferenceValue" => Some(Self::CHARACTER_REFERENCE_VALUE),
            "codeFenced" => Some(Self::CODE_FENCED),
            "codeFencedFence" => Some(Self::CODE_FENCED_FENCE),
            "codeFencedFenceInfo" => Some(Self::CODE_FENCED_FENCE_INFO),
            "codeFencedFenceMeta" => Some(Self::CODE_FENCED_FENCE_META),
            "codeFencedFenceSequence" => Some(Self::CODE_FENCED_FENCE_SEQUENCE),
            "codeFlowValue" => Some(Self::CODE_FLOW_VALUE),
            "codeIndented" => Some(Self::CODE_INDENTED),
            "codeText" => Some(Self::CODE_TEXT),
            "codeTextData" => Some(Self::CODE_TEXT_DATA),
            "codeTextSequence" => Some(Self::CODE_TEXT_SEQUENCE),
            "content" => Some(Self::CONTENT),
            "data" => Some(Self::DATA),
            "definition" => Some(Self::DEFINITION),
            "definitionDestination" => Some(Self::DEFINITION_DESTINATION),
            "definitionDestinationLiteral" => Some(Self::DEFINITION_DESTINATION_LITERAL),
            "definitionDestinationLiteralMarker" => {
                Some(Self::DEFINITION_DESTINATION_LITERAL_MARKER)
            }
            "definitionDestinationRaw" => Some(Self::DEFINITION_DESTINATION_RAW),
            "definitionDestinationString" => Some(Self::DEFINITION_DESTINATION_STRING),
            "definitionLabel" => Some(Self::DEFINITION_LABEL),
            "definitionLabelMarker" => Some(Self::DEFINITION_LABEL_MARKER),
            "definitionLabelString" => Some(Self::DEFINITION_LABEL_STRING),
            "definitionMarker" => Some(Self::DEFINITION_MARKER),
            "definitionTitle" => Some(Self::DEFINITION_TITLE),
            "definitionTitleMarker" => Some(Self::DEFINITION_TITLE_MARKER),
            "definitionTitleString" => Some(Self::DEFINITION_TITLE_STRING),
            "directiveContainer" => Some(Self::DIRECTIVE_CONTAINER),
            "directiveContainerAttributes" => Some(Self::DIRECTIVE_CONTAINER_ATTRIBUTES),
            "directiveContainerAttributesMarker" => {
                Some(Self::DIRECTIVE_CONTAINER_ATTRIBUTES_MARKER)
            }
            "directiveContainerChunk" => Some(Self::DIRECTIVE_CONTAINER_CHUNK),
            "directiveContainerContent" => Some(Self::DIRECTIVE_CONTAINER_CONTENT),
            "directiveContainerFence" => Some(Self::DIRECTIVE_CONTAINER_FENCE),
            "directiveContainerLabel" => Some(Self::DIRECTIVE_CONTAINER_LABEL),
            "directiveContainerLabelMarker" => Some(Self::DIRECTIVE_CONTAINER_LABEL_MARKER),
            "directiveContainerLabelString" => Some(Self::DIRECTIVE_CONTAINER_LABEL_STRING),
            "directiveContainerName" => Some(Self::DIRECTIVE_CONTAINER_NAME),
            "directiveContainerSequence" => Some(Self::DIRECTIVE_CONTAINER_SEQUENCE),
            "emphasis" => Some(Self::EMPHASIS),
            "emphasisSequence" => Some(Self::EMPHASIS_SEQUENCE),
            "emphasisText" => Some(Self::EMPHASIS_TEXT),
            "frontmatter" => Some(Self::FRONTMATTER),
            "frontmatterChunk" => Some(Self::FRONTMATTER_CHUNK),
            "frontmatterFence" => Some(Self::FRONTMATTER_FENCE),
            "frontmatterSequence" => Some(Self::FRONTMATTER_SEQUENCE),
            "literalAutolinkEmail" => Some(Self::LITERAL_AUTOLINK_EMAIL),
            "gfmAutolinkLiteralMailto" => Some(Self::GFM_AUTOLINK_LITERAL_MAILTO),
            "literalAutolinkHttp" => Some(Self::LITERAL_AUTOLINK_HTTP),
            "literalAutolinkWww" => Some(Self::LITERAL_AUTOLINK_WWW),
            "gfmAutolinkLiteralXmpp" => Some(Self::GFM_AUTOLINK_LITERAL_XMPP),
            "gfmFootnoteCall" => Some(Self::GFM_FOOTNOTE_CALL),
            "gfmFootnoteCallLabel" => Some(Self::GFM_FOOTNOTE_CALL_LABEL),
            "gfmFootnoteCallMarker" => Some(Self::GFM_FOOTNOTE_CALL_MARKER),
            "gfmFootnoteDefinition" => Some(Self::GFM_FOOTNOTE_DEFINITION),
            "gfmFootnoteDefinitionPrefix" => Some(Self::GFM_FOOTNOTE_DEFINITION_PREFIX),
            "gfmFootnoteDefinitionLabel" => Some(Self::GFM_FOOTNOTE_DEFINITION_LABEL),
            "gfmFootnoteDefinitionLabelMarker" => Some(Self::GFM_FOOTNOTE_DEFINITION_LABEL_MARKER),
            "gfmFootnoteDefinitionLabelString" => Some(Self::GFM_FOOTNOTE_DEFINITION_LABEL_STRING),
            "gfmFootnoteDefinitionMarker" => Some(Self::GFM_FOOTNOTE_DEFINITION_MARKER),
            "gfmStrikethrough" => Some(Self::GFM_STRIKETHROUGH),
            "gfmStrikethroughSequence" => Some(Self::GFM_STRIKETHROUGH_SEQUENCE),
            "gfmStrikethroughText" => Some(Self::GFM_STRIKETHROUGH_TEXT),
            "table" => Some(Self::TABLE),
            "tableBody" => Some(Self::TABLE_BODY),
            "tableCell" => Some(Self::TABLE_CELL),
            "tableContent" => Some(Self::TABLE_CONTENT),
            "tableCellDivider" => Some(Self::TABLE_CELL_DIVIDER),
            "tableDelimiterRow" => Some(Self::TABLE_DELIMITER_ROW),
            "tableDelimiterMarker" => Some(Self::TABLE_DELIMITER_MARKER),
            "tableDelimiter" => Some(Self::TABLE_DELIMITER),
            "tableDelimiterFiller" => Some(Self::TABLE_DELIMITER_FILLER),
            "tableHead" => Some(Self::TABLE_HEAD),
            "tableRow" => Some(Self::TABLE_ROW),
            "gfmTaskListItemCheck" => Some(Self::GFM_TASK_LIST_ITEM_CHECK),
            "gfmTaskListItemMarker" => Some(Self::GFM_TASK_LIST_ITEM_MARKER),
            "gfmTaskListItemValueChecked" => Some(Self::GFM_TASK_LIST_ITEM_VALUE_CHECKED),
            "gfmTaskListItemValueUnchecked" => Some(Self::GFM_TASK_LIST_ITEM_VALUE_UNCHECKED),
            "hardBreakEscape" => Some(Self::HARD_BREAK_ESCAPE),
            "hardBreakTrailing" => Some(Self::HARD_BREAK_TRAILING),
            "atxHeading" => Some(Self::ATX_HEADING),
            "atxHeadingSequence" => Some(Self::ATX_HEADING_SEQUENCE),
            "atxHeadingText" => Some(Self::ATX_HEADING_TEXT),
            "setextHeading" => Some(Self::SETEXT_HEADING),
            "setextHeadingText" => Some(Self::SETEXT_HEADING_TEXT),
            "setextHeadingLine" => Some(Self::SETEXT_HEADING_LINE),
            "setextHeadingLineSequence" => Some(Self::SETEXT_HEADING_LINE_SEQUENCE),
            "htmlFlow" => Some(Self::HTML_FLOW),
            "htmlFlowData" => Some(Self::HTML_FLOW_DATA),
            "htmlText" => Some(Self::HTML_TEXT),
            "htmlTextData" => Some(Self::HTML_TEXT_DATA),
            "image" => Some(Self::IMAGE),
            "label" => Some(Self::LABEL),
            "labelEnd" => Some(Self::LABEL_END),
            "labelImage" => Some(Self::LABEL_IMAGE),
            "labelImageMarker" => Some(Self::LABEL_IMAGE_MARKER),
            "labelLink" => Some(Self::LABEL_LINK),
            "labelMarker" => Some(Self::LABEL_MARKER),
            "labelText" => Some(Self::LABEL_TEXT),
            "lineEnding" => Some(Self::LINE_ENDING),
            "link" => Some(Self::LINK),
            "listItem" => Some(Self::LIST_ITEM),
            "listItemMarker" => Some(Self::LIST_ITEM_MARKER),
            "listItemPrefix" => Some(Self::LIST_ITEM_PREFIX),
            "listItemValue" => Some(Self::LIST_ITEM_VALUE),
            "listOrdered" => Some(Self::LIST_ORDERED),
            "listUnordered" => Some(Self::LIST_UNORDERED),
            "mathFlow" => Some(Self::MATH_FLOW),
            "mathFlowFence" => Some(Self::MATH_FLOW_FENCE),
            "mathFlowFenceMeta" => Some(Self::MATH_FLOW_FENCE_META),
            "mathFlowFenceSequence" => Some(Self::MATH_FLOW_FENCE_SEQUENCE),
            "mathFlowValue" => Some(Self::MATH_FLOW_VALUE),
            "mathText" => Some(Self::MATH_TEXT),
            "mathTextData" => Some(Self::MATH_TEXT_DATA),
            "mathTextSequence" => Some(Self::MATH_TEXT_SEQUENCE),
            "mdxEsm" => Some(Self::MDX_ESM),
            "mdxEsmData" => Some(Self::MDX_ESM_DATA),
            "mdxExpressionMarker" => Some(Self::MDX_EXPRESSION_MARKER),
            "mdxExpressionData" => Some(Self::MDX_EXPRESSION_DATA),
            "mdxFlowExpression" => Some(Self::MDX_FLOW_EXPRESSION),
            "mdxTextExpression" => Some(Self::MDX_TEXT_EXPRESSION),
            "mdxJsxFlowTag" => Some(Self::MDX_JSX_FLOW_TAG),
            "mdxJsxTextTag" => Some(Self::MDX_JSX_TEXT_TAG),
            "mdxJsxEsWhitespace" => Some(Self::MDX_JSX_ES_WHITESPACE),
            "mdxJsxTagMarker" => Some(Self::MDX_JSX_TAG_MARKER),
            "mdxJsxTagClosingMarker" => Some(Self::MDX_JSX_TAG_CLOSING_MARKER),
            "mdxJsxTagName" => Some(Self::MDX_JSX_TAG_NAME),
            "mdxJsxTagNamePrimary" => Some(Self::MDX_JSX_TAG_NAME_PRIMARY),
            "mdxJsxTagNameMemberMarker" => Some(Self::MDX_JSX_TAG_NAME_MEMBER_MARKER),
            "mdxJsxTagNamePrefixMarker" => Some(Self::MDX_JSX_TAG_NAME_PREFIX_MARKER),
            "mdxJsxTagNameMember" => Some(Self::MDX_JSX_TAG_NAME_MEMBER),
            "mdxJsxTagNameLocal" => Some(Self::MDX_JSX_TAG_NAME_LOCAL),
            "mdxJsxTagAttribute" => Some(Self::MDX_JSX_TAG_ATTRIBUTE),
            "mdxJsxTagAttributeExpression" => Some(Self::MDX_JSX_TAG_ATTRIBUTE_EXPRESSION),
            "mdxJsxTagAttributeName" => Some(Self::MDX_JSX_TAG_ATTRIBUTE_NAME),
            "mdxJsxTagAttributePrimaryName" => Some(Self::MDX_JSX_TAG_ATTRIBUTE_PRIMARY_NAME),
            "mdxJsxTagAttributeNamePrefixMarker" => {
                Some(Self::MDX_JSX_TAG_ATTRIBUTE_NAME_PREFIX_MARKER)
            }
            "mdxJsxTagAttributeNameLocal" => Some(Self::MDX_JSX_TAG_ATTRIBUTE_NAME_LOCAL),
            "mdxJsxTagAttributeInitializerMarker" => {
                Some(Self::MDX_JSX_TAG_ATTRIBUTE_INITIALIZER_MARKER)
            }
            "mdxJsxTagAttributeValueExpression" => {
                Some(Self::MDX_JSX_TAG_ATTRIBUTE_VALUE_EXPRESSION)
            }
            "mdxJsxTagAttributeValueLiteral" => Some(Self::MDX_JSX_TAG_ATTRIBUTE_VALUE_LITERAL),
            "mdxJsxTagAttributeValueLiteralMarker" => {
                Some(Self::MDX_JSX_TAG_ATTRIBUTE_VALUE_LITERAL_MARKER)
            }
            "mdxJsxTagAttributeValueLiteralValue" => {
                Some(Self::MDX_JSX_TAG_ATTRIBUTE_VALUE_LITERAL_VALUE)
            }
            "mdxJsxTagSelfClosingMarker" => Some(Self::MDX_JSX_TAG_SELF_CLOSING_MARKER),
            "paragraph" => Some(Self::PARAGRAPH),
            "reference" => Some(Self::REFERENCE),
            "referenceMarker" => Some(Self::REFERENCE_MARKER),
            "referenceString" => Some(Self::REFERENCE_STRING),
            "resource" => Some(Self::RESOURCE),
            "resourceDestination" => Some(Self::RESOURCE_DESTINATION),
            "resourceDestinationLiteral" => Some(Self::RESOURCE_DESTINATION_LITERAL),
            "resourceDestinationLiteralMarker" => Some(Self::RESOURCE_DESTINATION_LITERAL_MARKER),
            "resourceDestinationRaw" => Some(Self::RESOURCE_DESTINATION_RAW),
            "resourceDestinationString" => Some(Self::RESOURCE_DESTINATION_STRING),
            "resourceMarker" => Some(Self::RESOURCE_MARKER),
            "resourceTitle" => Some(Self::RESOURCE_TITLE),
            "resourceTitleMarker" => Some(Self::RESOURCE_TITLE_MARKER),
            "resourceTitleString" => Some(Self::RESOURCE_TITLE_STRING),
            "spaceOrTab" => Some(Self::SPACE_OR_TAB),
            "strong" => Some(Self::STRONG),
            "strongSequence" => Some(Self::STRONG_SEQUENCE),
            "strongText" => Some(Self::STRONG_TEXT),
            "thematicBreak" => Some(Self::THEMATIC_BREAK),
            "thematicBreakSequence" => Some(Self::THEMATIC_BREAK_SEQUENCE),
            "linePrefix" => Some(Self::LINE_PREFIX),
            "root" => Some(Self::ROOT),
            "lineSuffix" => Some(Self::LINE_SUFFIX),
            "whitespace" => Some(Self::WHITESPACE),
            "listItemIndent" => Some(Self::LIST_ITEM_INDENT),
            "listItemPrefixWhitespace" => Some(Self::LIST_ITEM_PREFIX_WHITESPACE),
            "blockQuotePrefixWhitespace" => Some(Self::BLOCK_QUOTE_PREFIX_WHITESPACE),
            "gfmFootnoteDefinitionWhitespace" => Some(Self::GFM_FOOTNOTE_DEFINITION_WHITESPACE),
            "gfmFootnoteDefinitionIndent" => Some(Self::GFM_FOOTNOTE_DEFINITION_INDENT),
            "literalAutolink" => Some(Self::LITERAL_AUTOLINK),
            "tableHeader" => Some(Self::TABLE_HEADER),
            "tableData" => Some(Self::TABLE_DATA),
            "codeTextPadding" => Some(Self::CODE_TEXT_PADDING),
            "undefinedReference" => Some(Self::UNDEFINED_REFERENCE),
            "undefinedReferenceShortcut" => Some(Self::UNDEFINED_REFERENCE_SHORTCUT),
            "undefinedReferenceCollapsed" => Some(Self::UNDEFINED_REFERENCE_COLLAPSED),
            "undefinedReferenceFull" => Some(Self::UNDEFINED_REFERENCE_FULL),
            "gfmFootnoteCallLabelMarker" => Some(Self::GFM_FOOTNOTE_CALL_LABEL_MARKER),
            "gfmFootnoteCallString" => Some(Self::GFM_FOOTNOTE_CALL_STRING),
            _ => None,
        }
    }
}

static NAMES: [&str; 199] = [
    "",
    "attentionSequence",
    "autolink",
    "autolinkEmail",
    "autolinkMarker",
    "autolinkProtocol",
    "lineEndingBlank",
    "blockQuote",
    "blockQuoteMarker",
    "blockQuotePrefix",
    "byteOrderMark",
    "characterEscape",
    "escapeMarker",
    "characterEscapeValue",
    "characterReference",
    "characterReferenceMarker",
    "characterReferenceMarkerHexadecimal",
    "characterReferenceMarkerNumeric",
    "characterReferenceValue",
    "codeFenced",
    "codeFencedFence",
    "codeFencedFenceInfo",
    "codeFencedFenceMeta",
    "codeFencedFenceSequence",
    "codeFlowValue",
    "codeIndented",
    "codeText",
    "codeTextData",
    "codeTextSequence",
    "content",
    "data",
    "definition",
    "definitionDestination",
    "definitionDestinationLiteral",
    "definitionDestinationLiteralMarker",
    "definitionDestinationRaw",
    "definitionDestinationString",
    "definitionLabel",
    "definitionLabelMarker",
    "definitionLabelString",
    "definitionMarker",
    "definitionTitle",
    "definitionTitleMarker",
    "definitionTitleString",
    "directiveContainer",
    "directiveContainerAttributes",
    "directiveContainerAttributesMarker",
    "directiveContainerChunk",
    "directiveContainerContent",
    "directiveContainerFence",
    "directiveContainerLabel",
    "directiveContainerLabelMarker",
    "directiveContainerLabelString",
    "directiveContainerName",
    "directiveContainerSequence",
    "emphasis",
    "emphasisSequence",
    "emphasisText",
    "frontmatter",
    "frontmatterChunk",
    "frontmatterFence",
    "frontmatterSequence",
    "literalAutolinkEmail",
    "gfmAutolinkLiteralMailto",
    "literalAutolinkHttp",
    "literalAutolinkWww",
    "gfmAutolinkLiteralXmpp",
    "gfmFootnoteCall",
    "gfmFootnoteCallLabel",
    "gfmFootnoteCallMarker",
    "gfmFootnoteDefinition",
    "gfmFootnoteDefinitionPrefix",
    "gfmFootnoteDefinitionLabel",
    "gfmFootnoteDefinitionLabelMarker",
    "gfmFootnoteDefinitionLabelString",
    "gfmFootnoteDefinitionMarker",
    "gfmStrikethrough",
    "gfmStrikethroughSequence",
    "gfmStrikethroughText",
    "table",
    "tableBody",
    "tableCell",
    "tableContent",
    "tableCellDivider",
    "tableDelimiterRow",
    "tableDelimiterMarker",
    "tableDelimiter",
    "tableDelimiterFiller",
    "tableHead",
    "tableRow",
    "gfmTaskListItemCheck",
    "gfmTaskListItemMarker",
    "gfmTaskListItemValueChecked",
    "gfmTaskListItemValueUnchecked",
    "hardBreakEscape",
    "hardBreakTrailing",
    "atxHeading",
    "atxHeadingSequence",
    "atxHeadingText",
    "setextHeading",
    "setextHeadingText",
    "setextHeadingLine",
    "setextHeadingLineSequence",
    "htmlFlow",
    "htmlFlowData",
    "htmlText",
    "htmlTextData",
    "image",
    "label",
    "labelEnd",
    "labelImage",
    "labelImageMarker",
    "labelLink",
    "labelMarker",
    "labelText",
    "lineEnding",
    "link",
    "listItem",
    "listItemMarker",
    "listItemPrefix",
    "listItemValue",
    "listOrdered",
    "listUnordered",
    "mathFlow",
    "mathFlowFence",
    "mathFlowFenceMeta",
    "mathFlowFenceSequence",
    "mathFlowValue",
    "mathText",
    "mathTextData",
    "mathTextSequence",
    "mdxEsm",
    "mdxEsmData",
    "mdxExpressionMarker",
    "mdxExpressionData",
    "mdxFlowExpression",
    "mdxTextExpression",
    "mdxJsxFlowTag",
    "mdxJsxTextTag",
    "mdxJsxEsWhitespace",
    "mdxJsxTagMarker",
    "mdxJsxTagClosingMarker",
    "mdxJsxTagName",
    "mdxJsxTagNamePrimary",
    "mdxJsxTagNameMemberMarker",
    "mdxJsxTagNamePrefixMarker",
    "mdxJsxTagNameMember",
    "mdxJsxTagNameLocal",
    "mdxJsxTagAttribute",
    "mdxJsxTagAttributeExpression",
    "mdxJsxTagAttributeName",
    "mdxJsxTagAttributePrimaryName",
    "mdxJsxTagAttributeNamePrefixMarker",
    "mdxJsxTagAttributeNameLocal",
    "mdxJsxTagAttributeInitializerMarker",
    "mdxJsxTagAttributeValueExpression",
    "mdxJsxTagAttributeValueLiteral",
    "mdxJsxTagAttributeValueLiteralMarker",
    "mdxJsxTagAttributeValueLiteralValue",
    "mdxJsxTagSelfClosingMarker",
    "paragraph",
    "reference",
    "referenceMarker",
    "referenceString",
    "resource",
    "resourceDestination",
    "resourceDestinationLiteral",
    "resourceDestinationLiteralMarker",
    "resourceDestinationRaw",
    "resourceDestinationString",
    "resourceMarker",
    "resourceTitle",
    "resourceTitleMarker",
    "resourceTitleString",
    "spaceOrTab",
    "strong",
    "strongSequence",
    "strongText",
    "thematicBreak",
    "thematicBreakSequence",
    "linePrefix",
    "root",
    "lineSuffix",
    "whitespace",
    "listItemIndent",
    "listItemPrefixWhitespace",
    "blockQuotePrefixWhitespace",
    "gfmFootnoteDefinitionWhitespace",
    "gfmFootnoteDefinitionIndent",
    "literalAutolink",
    "tableHeader",
    "tableData",
    "codeTextPadding",
    "undefinedReference",
    "undefinedReferenceShortcut",
    "undefinedReferenceCollapsed",
    "undefinedReferenceFull",
    "gfmFootnoteCallLabelMarker",
    "gfmFootnoteCallString",
];

/// markdown-rs `Name` → 종류. 이름 변환표(rename) 를 여기에 흡수한다.
pub(super) fn kind_of(name: &Name) -> Kind {
    match name {
        Name::AttentionSequence => Kind::ATTENTION_SEQUENCE,
        Name::Autolink => Kind::AUTOLINK,
        Name::AutolinkEmail => Kind::AUTOLINK_EMAIL,
        Name::AutolinkMarker => Kind::AUTOLINK_MARKER,
        Name::AutolinkProtocol => Kind::AUTOLINK_PROTOCOL,
        Name::BlankLineEnding => Kind::LINE_ENDING_BLANK,
        Name::BlockQuote => Kind::BLOCK_QUOTE,
        Name::BlockQuoteMarker => Kind::BLOCK_QUOTE_MARKER,
        Name::BlockQuotePrefix => Kind::BLOCK_QUOTE_PREFIX,
        Name::ByteOrderMark => Kind::BYTE_ORDER_MARK,
        Name::CharacterEscape => Kind::CHARACTER_ESCAPE,
        Name::CharacterEscapeMarker => Kind::ESCAPE_MARKER,
        Name::CharacterEscapeValue => Kind::CHARACTER_ESCAPE_VALUE,
        Name::CharacterReference => Kind::CHARACTER_REFERENCE,
        Name::CharacterReferenceMarker => Kind::CHARACTER_REFERENCE_MARKER,
        Name::CharacterReferenceMarkerHexadecimal => Kind::CHARACTER_REFERENCE_MARKER_HEXADECIMAL,
        Name::CharacterReferenceMarkerNumeric => Kind::CHARACTER_REFERENCE_MARKER_NUMERIC,
        Name::CharacterReferenceMarkerSemi => Kind::CHARACTER_REFERENCE_MARKER,
        Name::CharacterReferenceValue => Kind::CHARACTER_REFERENCE_VALUE,
        Name::CodeFenced => Kind::CODE_FENCED,
        Name::CodeFencedFence => Kind::CODE_FENCED_FENCE,
        Name::CodeFencedFenceInfo => Kind::CODE_FENCED_FENCE_INFO,
        Name::CodeFencedFenceMeta => Kind::CODE_FENCED_FENCE_META,
        Name::CodeFencedFenceSequence => Kind::CODE_FENCED_FENCE_SEQUENCE,
        Name::CodeFlowChunk => Kind::CODE_FLOW_VALUE,
        Name::CodeIndented => Kind::CODE_INDENTED,
        Name::CodeText => Kind::CODE_TEXT,
        Name::CodeTextData => Kind::CODE_TEXT_DATA,
        Name::CodeTextSequence => Kind::CODE_TEXT_SEQUENCE,
        Name::Content => Kind::CONTENT,
        Name::Data => Kind::DATA,
        Name::Definition => Kind::DEFINITION,
        Name::DefinitionDestination => Kind::DEFINITION_DESTINATION,
        Name::DefinitionDestinationLiteral => Kind::DEFINITION_DESTINATION_LITERAL,
        Name::DefinitionDestinationLiteralMarker => Kind::DEFINITION_DESTINATION_LITERAL_MARKER,
        Name::DefinitionDestinationRaw => Kind::DEFINITION_DESTINATION_RAW,
        Name::DefinitionDestinationString => Kind::DEFINITION_DESTINATION_STRING,
        Name::DefinitionLabel => Kind::DEFINITION_LABEL,
        Name::DefinitionLabelMarker => Kind::DEFINITION_LABEL_MARKER,
        Name::DefinitionLabelString => Kind::DEFINITION_LABEL_STRING,
        Name::DefinitionMarker => Kind::DEFINITION_MARKER,
        Name::DefinitionTitle => Kind::DEFINITION_TITLE,
        Name::DefinitionTitleMarker => Kind::DEFINITION_TITLE_MARKER,
        Name::DefinitionTitleString => Kind::DEFINITION_TITLE_STRING,
        Name::DirectiveContainer => Kind::DIRECTIVE_CONTAINER,
        Name::DirectiveContainerAttributes => Kind::DIRECTIVE_CONTAINER_ATTRIBUTES,
        Name::DirectiveContainerAttributesMarker => Kind::DIRECTIVE_CONTAINER_ATTRIBUTES_MARKER,
        Name::DirectiveContainerChunk => Kind::DIRECTIVE_CONTAINER_CHUNK,
        Name::DirectiveContainerContent => Kind::DIRECTIVE_CONTAINER_CONTENT,
        Name::DirectiveContainerFence => Kind::DIRECTIVE_CONTAINER_FENCE,
        Name::DirectiveContainerLabel => Kind::DIRECTIVE_CONTAINER_LABEL,
        Name::DirectiveContainerLabelMarker => Kind::DIRECTIVE_CONTAINER_LABEL_MARKER,
        Name::DirectiveContainerLabelString => Kind::DIRECTIVE_CONTAINER_LABEL_STRING,
        Name::DirectiveContainerName => Kind::DIRECTIVE_CONTAINER_NAME,
        Name::DirectiveContainerSequence => Kind::DIRECTIVE_CONTAINER_SEQUENCE,
        Name::Emphasis => Kind::EMPHASIS,
        Name::EmphasisSequence => Kind::EMPHASIS_SEQUENCE,
        Name::EmphasisText => Kind::EMPHASIS_TEXT,
        Name::Frontmatter => Kind::FRONTMATTER,
        Name::FrontmatterChunk => Kind::FRONTMATTER_CHUNK,
        Name::FrontmatterFence => Kind::FRONTMATTER_FENCE,
        Name::FrontmatterSequence => Kind::FRONTMATTER_SEQUENCE,
        Name::GfmAutolinkLiteralEmail => Kind::LITERAL_AUTOLINK_EMAIL,
        Name::GfmAutolinkLiteralMailto => Kind::GFM_AUTOLINK_LITERAL_MAILTO,
        Name::GfmAutolinkLiteralProtocol => Kind::LITERAL_AUTOLINK_HTTP,
        Name::GfmAutolinkLiteralWww => Kind::LITERAL_AUTOLINK_WWW,
        Name::GfmAutolinkLiteralXmpp => Kind::GFM_AUTOLINK_LITERAL_XMPP,
        Name::GfmFootnoteCall => Kind::GFM_FOOTNOTE_CALL,
        Name::GfmFootnoteCallLabel => Kind::GFM_FOOTNOTE_CALL_LABEL,
        Name::GfmFootnoteCallMarker => Kind::GFM_FOOTNOTE_CALL_MARKER,
        Name::GfmFootnoteDefinition => Kind::GFM_FOOTNOTE_DEFINITION,
        Name::GfmFootnoteDefinitionPrefix => Kind::GFM_FOOTNOTE_DEFINITION_PREFIX,
        Name::GfmFootnoteDefinitionLabel => Kind::GFM_FOOTNOTE_DEFINITION_LABEL,
        Name::GfmFootnoteDefinitionLabelMarker => Kind::GFM_FOOTNOTE_DEFINITION_LABEL_MARKER,
        Name::GfmFootnoteDefinitionLabelString => Kind::GFM_FOOTNOTE_DEFINITION_LABEL_STRING,
        Name::GfmFootnoteDefinitionMarker => Kind::GFM_FOOTNOTE_DEFINITION_MARKER,
        Name::GfmStrikethrough => Kind::GFM_STRIKETHROUGH,
        Name::GfmStrikethroughSequence => Kind::GFM_STRIKETHROUGH_SEQUENCE,
        Name::GfmStrikethroughText => Kind::GFM_STRIKETHROUGH_TEXT,
        Name::GfmTable => Kind::TABLE,
        Name::GfmTableBody => Kind::TABLE_BODY,
        Name::GfmTableCell => Kind::TABLE_CELL,
        Name::GfmTableCellText => Kind::TABLE_CONTENT,
        Name::GfmTableCellDivider => Kind::TABLE_CELL_DIVIDER,
        Name::GfmTableDelimiterRow => Kind::TABLE_DELIMITER_ROW,
        Name::GfmTableDelimiterMarker => Kind::TABLE_DELIMITER_MARKER,
        Name::GfmTableDelimiterCell => Kind::TABLE_DELIMITER,
        Name::GfmTableDelimiterCellValue => Kind::TABLE_CONTENT,
        Name::GfmTableDelimiterFiller => Kind::TABLE_DELIMITER_FILLER,
        Name::GfmTableHead => Kind::TABLE_HEAD,
        Name::GfmTableRow => Kind::TABLE_ROW,
        Name::GfmTaskListItemCheck => Kind::GFM_TASK_LIST_ITEM_CHECK,
        Name::GfmTaskListItemMarker => Kind::GFM_TASK_LIST_ITEM_MARKER,
        Name::GfmTaskListItemValueChecked => Kind::GFM_TASK_LIST_ITEM_VALUE_CHECKED,
        Name::GfmTaskListItemValueUnchecked => Kind::GFM_TASK_LIST_ITEM_VALUE_UNCHECKED,
        Name::HardBreakEscape => Kind::HARD_BREAK_ESCAPE,
        Name::HardBreakTrailing => Kind::HARD_BREAK_TRAILING,
        Name::HeadingAtx => Kind::ATX_HEADING,
        Name::HeadingAtxSequence => Kind::ATX_HEADING_SEQUENCE,
        Name::HeadingAtxText => Kind::ATX_HEADING_TEXT,
        Name::HeadingSetext => Kind::SETEXT_HEADING,
        Name::HeadingSetextText => Kind::SETEXT_HEADING_TEXT,
        Name::HeadingSetextUnderline => Kind::SETEXT_HEADING_LINE,
        Name::HeadingSetextUnderlineSequence => Kind::SETEXT_HEADING_LINE_SEQUENCE,
        Name::HtmlFlow => Kind::HTML_FLOW,
        Name::HtmlFlowData => Kind::HTML_FLOW_DATA,
        Name::HtmlText => Kind::HTML_TEXT,
        Name::HtmlTextData => Kind::HTML_TEXT_DATA,
        Name::Image => Kind::IMAGE,
        Name::Label => Kind::LABEL,
        Name::LabelEnd => Kind::LABEL_END,
        Name::LabelImage => Kind::LABEL_IMAGE,
        Name::LabelImageMarker => Kind::LABEL_IMAGE_MARKER,
        Name::LabelLink => Kind::LABEL_LINK,
        Name::LabelMarker => Kind::LABEL_MARKER,
        Name::LabelText => Kind::LABEL_TEXT,
        Name::LineEnding => Kind::LINE_ENDING,
        Name::Link => Kind::LINK,
        Name::ListItem => Kind::LIST_ITEM,
        Name::ListItemMarker => Kind::LIST_ITEM_MARKER,
        Name::ListItemPrefix => Kind::LIST_ITEM_PREFIX,
        Name::ListItemValue => Kind::LIST_ITEM_VALUE,
        Name::ListOrdered => Kind::LIST_ORDERED,
        Name::ListUnordered => Kind::LIST_UNORDERED,
        Name::MathFlow => Kind::MATH_FLOW,
        Name::MathFlowFence => Kind::MATH_FLOW_FENCE,
        Name::MathFlowFenceMeta => Kind::MATH_FLOW_FENCE_META,
        Name::MathFlowFenceSequence => Kind::MATH_FLOW_FENCE_SEQUENCE,
        Name::MathFlowChunk => Kind::MATH_FLOW_VALUE,
        Name::MathText => Kind::MATH_TEXT,
        Name::MathTextData => Kind::MATH_TEXT_DATA,
        Name::MathTextSequence => Kind::MATH_TEXT_SEQUENCE,
        Name::MdxEsm => Kind::MDX_ESM,
        Name::MdxEsmData => Kind::MDX_ESM_DATA,
        Name::MdxExpressionMarker => Kind::MDX_EXPRESSION_MARKER,
        Name::MdxExpressionData => Kind::MDX_EXPRESSION_DATA,
        Name::MdxFlowExpression => Kind::MDX_FLOW_EXPRESSION,
        Name::MdxTextExpression => Kind::MDX_TEXT_EXPRESSION,
        Name::MdxJsxFlowTag => Kind::MDX_JSX_FLOW_TAG,
        Name::MdxJsxTextTag => Kind::MDX_JSX_TEXT_TAG,
        Name::MdxJsxEsWhitespace => Kind::MDX_JSX_ES_WHITESPACE,
        Name::MdxJsxTagMarker => Kind::MDX_JSX_TAG_MARKER,
        Name::MdxJsxTagClosingMarker => Kind::MDX_JSX_TAG_CLOSING_MARKER,
        Name::MdxJsxTagName => Kind::MDX_JSX_TAG_NAME,
        Name::MdxJsxTagNamePrimary => Kind::MDX_JSX_TAG_NAME_PRIMARY,
        Name::MdxJsxTagNameMemberMarker => Kind::MDX_JSX_TAG_NAME_MEMBER_MARKER,
        Name::MdxJsxTagNamePrefixMarker => Kind::MDX_JSX_TAG_NAME_PREFIX_MARKER,
        Name::MdxJsxTagNameMember => Kind::MDX_JSX_TAG_NAME_MEMBER,
        Name::MdxJsxTagNameLocal => Kind::MDX_JSX_TAG_NAME_LOCAL,
        Name::MdxJsxTagAttribute => Kind::MDX_JSX_TAG_ATTRIBUTE,
        Name::MdxJsxTagAttributeExpression => Kind::MDX_JSX_TAG_ATTRIBUTE_EXPRESSION,
        Name::MdxJsxTagAttributeName => Kind::MDX_JSX_TAG_ATTRIBUTE_NAME,
        Name::MdxJsxTagAttributePrimaryName => Kind::MDX_JSX_TAG_ATTRIBUTE_PRIMARY_NAME,
        Name::MdxJsxTagAttributeNamePrefixMarker => Kind::MDX_JSX_TAG_ATTRIBUTE_NAME_PREFIX_MARKER,
        Name::MdxJsxTagAttributeNameLocal => Kind::MDX_JSX_TAG_ATTRIBUTE_NAME_LOCAL,
        Name::MdxJsxTagAttributeInitializerMarker => Kind::MDX_JSX_TAG_ATTRIBUTE_INITIALIZER_MARKER,
        Name::MdxJsxTagAttributeValueExpression => Kind::MDX_JSX_TAG_ATTRIBUTE_VALUE_EXPRESSION,
        Name::MdxJsxTagAttributeValueLiteral => Kind::MDX_JSX_TAG_ATTRIBUTE_VALUE_LITERAL,
        Name::MdxJsxTagAttributeValueLiteralMarker => {
            Kind::MDX_JSX_TAG_ATTRIBUTE_VALUE_LITERAL_MARKER
        }
        Name::MdxJsxTagAttributeValueLiteralValue => {
            Kind::MDX_JSX_TAG_ATTRIBUTE_VALUE_LITERAL_VALUE
        }
        Name::MdxJsxTagSelfClosingMarker => Kind::MDX_JSX_TAG_SELF_CLOSING_MARKER,
        Name::Paragraph => Kind::PARAGRAPH,
        Name::Reference => Kind::REFERENCE,
        Name::ReferenceMarker => Kind::REFERENCE_MARKER,
        Name::ReferenceString => Kind::REFERENCE_STRING,
        Name::Resource => Kind::RESOURCE,
        Name::ResourceDestination => Kind::RESOURCE_DESTINATION,
        Name::ResourceDestinationLiteral => Kind::RESOURCE_DESTINATION_LITERAL,
        Name::ResourceDestinationLiteralMarker => Kind::RESOURCE_DESTINATION_LITERAL_MARKER,
        Name::ResourceDestinationRaw => Kind::RESOURCE_DESTINATION_RAW,
        Name::ResourceDestinationString => Kind::RESOURCE_DESTINATION_STRING,
        Name::ResourceMarker => Kind::RESOURCE_MARKER,
        Name::ResourceTitle => Kind::RESOURCE_TITLE,
        Name::ResourceTitleMarker => Kind::RESOURCE_TITLE_MARKER,
        Name::ResourceTitleString => Kind::RESOURCE_TITLE_STRING,
        Name::SpaceOrTab => Kind::SPACE_OR_TAB,
        Name::Strong => Kind::STRONG,
        Name::StrongSequence => Kind::STRONG_SEQUENCE,
        Name::StrongText => Kind::STRONG_TEXT,
        Name::ThematicBreak => Kind::THEMATIC_BREAK,
        Name::ThematicBreakSequence => Kind::THEMATIC_BREAK_SEQUENCE,
        Name::LinePrefix => Kind::LINE_PREFIX,
    }
}
