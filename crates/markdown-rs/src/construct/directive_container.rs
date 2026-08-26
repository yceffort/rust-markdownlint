//! Directive (container) occurs in the [flow][] content type.
//!
//! 로컬 확장: `micromark-extension-directive` 의 container directive 를 옮긴 것이다.
//! markdownlint 는 항상 이 확장을 켠 채 파싱하므로 `:::name` ~ `:::` 블록이
//! 문단/표로 새지 않게 하려면 같은 토큰을 내야 한다.
//!
//! ## Grammar
//!
//! ```bnf
//! directive_container ::= fence_open *( eol *line ) [ eol fence_close ]
//!
//! fence_open ::= 3*':' name [ label ] [ attributes ] *space_or_tab
//! fence_close ::= 3*':' *space_or_tab
//! name ::= 1*( letter | digit | '-' | '_' )  ; 원본 `factoryName` 와 같이
//!                                           ; 구두점/공백이 아니면 이름 문자
//! label ::= '[' *( text ) ']'               ; 한 줄, `[`/`]` 균형, `\` 이스케이프
//! attributes ::= '{' *( not '{' '}' eol ) '}'
//! ```
//!
//! 본문 줄은 `chunkDocument` 처럼 [`Content::Document`][] 로 연결해 하위 토큰화한다.
//! 원본과 같이 concrete 구성요소라 lazy 줄은 본문이 될 수 없고, 닫는 fence 는
//! 여는 sequence 이상의 `:` 여야 한다. attributes 의 내부 문법(`#id`, `.class`,
//! `key=value`)은 토큰으로 나누지 않고 `Data` 하나로 둔다.
//!
//! ## Tokens
//!
//! * [`DirectiveContainer`][Name::DirectiveContainer]
//! * [`DirectiveContainerFence`][Name::DirectiveContainerFence]
//! * [`DirectiveContainerSequence`][Name::DirectiveContainerSequence]
//! * [`DirectiveContainerName`][Name::DirectiveContainerName]
//! * [`DirectiveContainerLabel`][Name::DirectiveContainerLabel]
//! * [`DirectiveContainerLabelMarker`][Name::DirectiveContainerLabelMarker]
//! * [`DirectiveContainerLabelString`][Name::DirectiveContainerLabelString]
//! * [`DirectiveContainerAttributes`][Name::DirectiveContainerAttributes]
//! * [`DirectiveContainerAttributesMarker`][Name::DirectiveContainerAttributesMarker]
//! * [`DirectiveContainerContent`][Name::DirectiveContainerContent]
//! * [`DirectiveContainerChunk`][Name::DirectiveContainerChunk]
//! * [`LineEnding`][Name::LineEnding]
//! * [`SpaceOrTab`][Name::SpaceOrTab]
//!
//! [flow]: crate::construct::flow

use crate::construct::partial_space_or_tab::{space_or_tab, space_or_tab_min_max};
use crate::event::{Content, Link, Name};
use crate::state::{Name as StateName, State};
use crate::tokenizer::Tokenizer;
use crate::util::{
    constant::TAB_SIZE,
    slice::{Position, Slice},
};

/// 여는/닫는 sequence 의 최소 길이 (`:::`).
const SEQUENCE_SIZE_MIN: usize = 3;

/// 원본 `unicodePunctuation`/`unicodeWhitespace` 를 ASCII 범위에서 흉내 낸다.
/// 이름 문자가 아닌 바이트: 제어 문자, 공백, ASCII 구두점. 비ASCII 바이트는 이름으로 본다.
fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte >= 0x80
}

/// Start of directive container.
///
/// ```markdown
/// > | :::note
///     ^
///   | a
///   | :::
/// ```
pub fn start(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.parse_state.options.constructs.directive_container {
        if matches!(tokenizer.current, Some(b'\t' | b' ')) {
            tokenizer.attempt(
                State::Next(StateName::DirectiveContainerBeforeSequenceOpen),
                State::Nok,
            );
            return State::Retry(space_or_tab_min_max(
                tokenizer,
                0,
                if tokenizer.parse_state.options.constructs.code_indented {
                    TAB_SIZE - 1
                } else {
                    usize::MAX
                },
            ));
        }

        if tokenizer.current == Some(b':') {
            return State::Retry(StateName::DirectiveContainerBeforeSequenceOpen);
        }
    }

    State::Nok
}

/// In opening fence, after prefix, at sequence.
///
/// ```markdown
/// > | :::note
///     ^
/// ```
pub fn before_sequence_open(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.current == Some(b':') {
        let tail = tokenizer.events.last();
        let mut prefix = 0;

        if let Some(event) = tail {
            if event.name == Name::SpaceOrTab {
                prefix = Slice::from_position(
                    tokenizer.parse_state.bytes,
                    &Position::from_exit_event(&tokenizer.events, tokenizer.events.len() - 1),
                )
                .len();
            }
        }

        tokenizer.tokenize_state.size_c = prefix;
        tokenizer.enter(Name::DirectiveContainer);
        tokenizer.enter(Name::DirectiveContainerFence);
        tokenizer.enter(Name::DirectiveContainerSequence);
        State::Retry(StateName::DirectiveContainerSequenceOpen)
    } else {
        State::Nok
    }
}

/// In opening sequence.
///
/// ```markdown
/// > | :::note
///     ^^^
/// ```
pub fn sequence_open(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.current == Some(b':') {
        tokenizer.tokenize_state.size += 1;
        tokenizer.consume();
        State::Next(StateName::DirectiveContainerSequenceOpen)
    } else if tokenizer.tokenize_state.size < SEQUENCE_SIZE_MIN {
        reset(tokenizer);
        State::Nok
    } else {
        tokenizer.exit(Name::DirectiveContainerSequence);
        State::Retry(StateName::DirectiveContainerNameStart)
    }
}

/// At name (원본 `factoryName` 의 `start`).
///
/// ```markdown
/// > | :::note
///        ^
/// ```
pub fn name_start(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        Some(byte) if is_name_byte(byte) => {
            tokenizer.enter(Name::DirectiveContainerName);
            tokenizer.consume();
            State::Next(StateName::DirectiveContainerName)
        }
        _ => {
            reset(tokenizer);
            State::Nok
        }
    }
}

/// In name (원본 `factoryName` 의 `name`).
///
/// ```markdown
/// > | :::note
///         ^^^
/// ```
pub fn name(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        Some(byte) if is_name_byte(byte) || byte == b'-' || byte == b'_' => {
            tokenizer.consume();
            State::Next(StateName::DirectiveContainerName)
        }
        _ => {
            tokenizer.exit(Name::DirectiveContainerName);
            // `-`/`_` 로 끝나는 이름은 허용하지 않는다
            if matches!(tokenizer.previous, Some(b'-' | b'_')) {
                reset(tokenizer);
                State::Nok
            } else {
                State::Retry(StateName::DirectiveContainerAfterName)
            }
        }
    }
}

/// After name, at optional label.
///
/// ```markdown
/// > | :::note[Title]
///            ^
/// ```
pub fn after_name(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.current == Some(b'[') {
        tokenizer.attempt(
            State::Next(StateName::DirectiveContainerAfterLabel),
            State::Next(StateName::DirectiveContainerAfterLabel),
        );
        State::Retry(StateName::DirectiveContainerLabelStart)
    } else {
        State::Retry(StateName::DirectiveContainerAfterLabel)
    }
}

/// At `[` of label (원본 `factoryLabel`, `disallowEol`).
///
/// ```markdown
/// > | :::note[Title]
///            ^
/// ```
pub fn label_start(tokenizer: &mut Tokenizer) -> State {
    tokenizer.tokenize_state.size_b = 0;
    tokenizer.enter(Name::DirectiveContainerLabel);
    tokenizer.enter(Name::DirectiveContainerLabelMarker);
    tokenizer.consume();
    tokenizer.exit(Name::DirectiveContainerLabelMarker);
    State::Next(StateName::DirectiveContainerLabelAfterStart)
}

/// After `[`, at label string or `]`.
///
/// ```markdown
/// > | :::note[Title]
///             ^
/// ```
pub fn label_after_start(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.current == Some(b']') {
        State::Retry(StateName::DirectiveContainerLabelAtClosing)
    } else {
        tokenizer.enter(Name::DirectiveContainerLabelString);
        tokenizer.enter_link(
            Name::Data,
            Link {
                previous: None,
                next: None,
                content: Content::Text,
            },
        );
        State::Retry(StateName::DirectiveContainerLabelInside)
    }
}

/// In label string.
///
/// ```markdown
/// > | :::note[Title]
///             ^^^^^
/// ```
pub fn label_inside(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None | Some(b'\n') => {
            tokenizer.tokenize_state.size_b = 0;
            State::Nok
        }
        Some(b'[') => {
            tokenizer.tokenize_state.size_b += 1;
            if tokenizer.tokenize_state.size_b > 32 {
                tokenizer.tokenize_state.size_b = 0;
                State::Nok
            } else {
                tokenizer.consume();
                State::Next(StateName::DirectiveContainerLabelInside)
            }
        }
        Some(b']') => {
            if tokenizer.tokenize_state.size_b == 0 {
                tokenizer.exit(Name::Data);
                tokenizer.exit(Name::DirectiveContainerLabelString);
                State::Retry(StateName::DirectiveContainerLabelAtClosing)
            } else {
                tokenizer.tokenize_state.size_b -= 1;
                tokenizer.consume();
                State::Next(StateName::DirectiveContainerLabelInside)
            }
        }
        Some(b'\\') => {
            tokenizer.consume();
            State::Next(StateName::DirectiveContainerLabelEscape)
        }
        Some(_) => {
            tokenizer.consume();
            State::Next(StateName::DirectiveContainerLabelInside)
        }
    }
}

/// After `\` in label string.
///
/// ```markdown
/// > | :::note[a\]b]
///               ^
/// ```
pub fn label_escape(tokenizer: &mut Tokenizer) -> State {
    if matches!(tokenizer.current, Some(b'[' | b'\\' | b']')) {
        tokenizer.consume();
        State::Next(StateName::DirectiveContainerLabelInside)
    } else {
        State::Retry(StateName::DirectiveContainerLabelInside)
    }
}

/// At `]` of label.
///
/// ```markdown
/// > | :::note[Title]
///                  ^
/// ```
pub fn label_at_closing(tokenizer: &mut Tokenizer) -> State {
    tokenizer.tokenize_state.size_b = 0;
    tokenizer.enter(Name::DirectiveContainerLabelMarker);
    tokenizer.consume();
    tokenizer.exit(Name::DirectiveContainerLabelMarker);
    tokenizer.exit(Name::DirectiveContainerLabel);
    State::Ok
}

/// After optional label, at optional attributes.
///
/// ```markdown
/// > | :::note{.warning}
///            ^
/// ```
pub fn after_label(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.current == Some(b'{') {
        tokenizer.attempt(
            State::Next(StateName::DirectiveContainerAfterAttributes),
            State::Next(StateName::DirectiveContainerAfterAttributes),
        );
        State::Retry(StateName::DirectiveContainerAttributesStart)
    } else {
        State::Retry(StateName::DirectiveContainerAfterAttributes)
    }
}

/// At `{` of attributes.
///
/// ```markdown
/// > | :::note{.warning}
///            ^
/// ```
pub fn attributes_start(tokenizer: &mut Tokenizer) -> State {
    tokenizer.enter(Name::DirectiveContainerAttributes);
    tokenizer.enter(Name::DirectiveContainerAttributesMarker);
    tokenizer.consume();
    tokenizer.exit(Name::DirectiveContainerAttributesMarker);
    State::Next(StateName::DirectiveContainerAttributesInside)
}

/// In attributes, before data or `}`.
///
/// ```markdown
/// > | :::note{.warning}
///             ^
/// ```
pub fn attributes_inside(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None | Some(b'\n' | b'{') => State::Nok,
        Some(b'}') => {
            tokenizer.enter(Name::DirectiveContainerAttributesMarker);
            tokenizer.consume();
            tokenizer.exit(Name::DirectiveContainerAttributesMarker);
            tokenizer.exit(Name::DirectiveContainerAttributes);
            State::Ok
        }
        Some(_) => {
            tokenizer.enter(Name::Data);
            State::Retry(StateName::DirectiveContainerAttributesData)
        }
    }
}

/// In attributes data.
///
/// ```markdown
/// > | :::note{.warning}
///             ^^^^^^^^
/// ```
pub fn attributes_data(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None | Some(b'\n' | b'{') => State::Nok,
        Some(b'}') => {
            tokenizer.exit(Name::Data);
            State::Retry(StateName::DirectiveContainerAttributesInside)
        }
        Some(_) => {
            tokenizer.consume();
            State::Next(StateName::DirectiveContainerAttributesData)
        }
    }
}

/// After optional attributes, at optional whitespace.
///
/// ```markdown
/// > | :::note
///            ^
/// ```
pub fn after_attributes(tokenizer: &mut Tokenizer) -> State {
    if matches!(tokenizer.current, Some(b'\t' | b' ')) {
        tokenizer.attempt(
            State::Next(StateName::DirectiveContainerOpenAfter),
            State::Nok,
        );
        State::Retry(space_or_tab(tokenizer))
    } else {
        State::Retry(StateName::DirectiveContainerOpenAfter)
    }
}

/// After opening fence, at eol/eof.
///
/// ```markdown
/// > | :::note
///            ^
///   | a
///   | :::
/// ```
pub fn open_after(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None => {
            tokenizer.exit(Name::DirectiveContainerFence);
            State::Retry(StateName::DirectiveContainerAfter)
        }
        Some(b'\n') => {
            tokenizer.exit(Name::DirectiveContainerFence);
            // Do not form containers.
            tokenizer.concrete = true;
            tokenizer.attempt(
                State::Next(StateName::DirectiveContainerContentStart),
                State::Next(StateName::DirectiveContainerAfter),
            );
            State::Retry(StateName::NonLazyContinuationStart)
        }
        _ => {
            reset(tokenizer);
            State::Nok
        }
    }
}

/// After the line ending of the opening fence, at content.
///
/// ```markdown
///   | :::note
/// > | a
///     ^
///   | :::
/// ```
pub fn content_start(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None => State::Retry(StateName::DirectiveContainerAfter),
        Some(b'\n') => {
            tokenizer.check(
                State::Next(StateName::DirectiveContainerEmptyContentNonLazyLineAfter),
                State::Next(StateName::DirectiveContainerAfter),
            );
            State::Retry(StateName::NonLazyContinuationStart)
        }
        Some(_) => {
            tokenizer.enter(Name::DirectiveContainerContent);
            State::Retry(StateName::DirectiveContainerLineStart)
        }
    }
}

/// At an empty first content line that is not lazy.
pub fn empty_content_non_lazy_line_after(tokenizer: &mut Tokenizer) -> State {
    tokenizer.enter(Name::DirectiveContainerContent);
    State::Retry(StateName::DirectiveContainerLineStart)
}

/// At start of a content line, before a closing fence or a chunk.
///
/// ```markdown
///   | :::note
/// > | a
///     ^
/// > | :::
///     ^
/// ```
pub fn line_start(tokenizer: &mut Tokenizer) -> State {
    tokenizer.attempt(
        State::Next(StateName::DirectiveContainerAfterContent),
        State::Next(StateName::DirectiveContainerLinePrefix),
    );
    State::Retry(StateName::DirectiveContainerClosingFenceStart)
}

/// At start of a content line that is not a closing fence, at optional prefix.
pub fn line_prefix(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.tokenize_state.size_c > 0 && matches!(tokenizer.current, Some(b'\t' | b' ')) {
        tokenizer.attempt(
            State::Next(StateName::DirectiveContainerChunkStart),
            State::Nok,
        );
        State::Retry(space_or_tab_min_max(
            tokenizer,
            0,
            tokenizer.tokenize_state.size_c,
        ))
    } else {
        State::Retry(StateName::DirectiveContainerChunkStart)
    }
}

/// Before a chunk, after optional prefix.
pub fn chunk_start(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None => State::Retry(StateName::DirectiveContainerAfterContent),
        Some(b'\n') => {
            tokenizer.check(
                State::Next(StateName::DirectiveContainerChunkNonLazyStart),
                State::Next(StateName::DirectiveContainerAfterContent),
            );
            State::Retry(StateName::NonLazyContinuationStart)
        }
        Some(_) => State::Retry(StateName::DirectiveContainerChunkNonLazyStart),
    }
}

/// At a chunk (원본 `chunkDocument`).
pub fn chunk_non_lazy_start(tokenizer: &mut Tokenizer) -> State {
    let current = tokenizer.events.len();
    let previous = tokenizer.tokenize_state.directive_container_chunk_index;
    if let Some(previous) = previous {
        tokenizer.events[previous].link.as_mut().unwrap().next = Some(current);
    }
    tokenizer.tokenize_state.directive_container_chunk_index = Some(current);
    tokenizer.enter_link(
        Name::DirectiveContainerChunk,
        Link {
            previous,
            next: None,
            content: Content::Document,
        },
    );
    State::Retry(StateName::DirectiveContainerContentContinue)
}

/// In a chunk.
pub fn content_continue(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None => {
            tokenizer.exit(Name::DirectiveContainerChunk);
            State::Retry(StateName::DirectiveContainerAfterContent)
        }
        Some(b'\n') => {
            tokenizer.check(
                State::Next(StateName::DirectiveContainerNonLazyLineAfter),
                State::Next(StateName::DirectiveContainerLineAfter),
            );
            State::Retry(StateName::NonLazyContinuationStart)
        }
        Some(_) => {
            tokenizer.consume();
            State::Next(StateName::DirectiveContainerContentContinue)
        }
    }
}

/// At eol in a chunk, the next line is not lazy: the eol belongs to the chunk.
pub fn non_lazy_line_after(tokenizer: &mut Tokenizer) -> State {
    tokenizer.consume();
    tokenizer.exit(Name::DirectiveContainerChunk);
    State::Next(StateName::DirectiveContainerLineStart)
}

/// At eol in a chunk, the next line is lazy: content ends here.
pub fn line_after(tokenizer: &mut Tokenizer) -> State {
    tokenizer.exit(Name::DirectiveContainerChunk);
    State::Retry(StateName::DirectiveContainerAfterContent)
}

/// After content (after a closing fence, a lazy line, or eof).
pub fn after_content(tokenizer: &mut Tokenizer) -> State {
    tokenizer.exit(Name::DirectiveContainerContent);
    State::Retry(StateName::DirectiveContainerAfter)
}

/// After directive container.
///
/// ```markdown
///   | :::note
///   | a
/// > | :::
///        ^
/// ```
pub fn after(tokenizer: &mut Tokenizer) -> State {
    tokenizer.exit(Name::DirectiveContainer);
    reset(tokenizer);
    // Feel free to interrupt.
    tokenizer.interrupt = false;
    // No longer concrete.
    tokenizer.concrete = false;
    State::Ok
}

/// At start of a closing fence line, at optional prefix (원본 `tokenizeClosingFence`).
///
/// ```markdown
///   | :::note
///   | a
/// > | :::
///     ^
/// ```
pub fn closing_fence_start(tokenizer: &mut Tokenizer) -> State {
    if matches!(tokenizer.current, Some(b'\t' | b' ')) {
        tokenizer.attempt(
            State::Next(StateName::DirectiveContainerClosingFenceBefore),
            State::Nok,
        );
        State::Retry(space_or_tab_min_max(
            tokenizer,
            0,
            if tokenizer.parse_state.options.constructs.code_indented {
                TAB_SIZE - 1
            } else {
                usize::MAX
            },
        ))
    } else {
        State::Retry(StateName::DirectiveContainerClosingFenceBefore)
    }
}

/// In closing fence, after optional prefix, at sequence.
pub fn closing_fence_before(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.current == Some(b':') {
        tokenizer.tokenize_state.size_b = 0;
        tokenizer.enter(Name::DirectiveContainerFence);
        tokenizer.enter(Name::DirectiveContainerSequence);
        State::Retry(StateName::DirectiveContainerClosingSequence)
    } else {
        State::Nok
    }
}

/// In closing sequence.
pub fn closing_sequence(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.current == Some(b':') {
        tokenizer.tokenize_state.size_b += 1;
        tokenizer.consume();
        State::Next(StateName::DirectiveContainerClosingSequence)
    } else if tokenizer.tokenize_state.size_b >= tokenizer.tokenize_state.size {
        tokenizer.tokenize_state.size_b = 0;
        tokenizer.exit(Name::DirectiveContainerSequence);
        if matches!(tokenizer.current, Some(b'\t' | b' ')) {
            tokenizer.attempt(
                State::Next(StateName::DirectiveContainerClosingSequenceAfter),
                State::Nok,
            );
            State::Retry(space_or_tab(tokenizer))
        } else {
            State::Retry(StateName::DirectiveContainerClosingSequenceAfter)
        }
    } else {
        tokenizer.tokenize_state.size_b = 0;
        State::Nok
    }
}

/// After closing sequence and optional whitespace, at eol/eof.
pub fn closing_sequence_after(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None | Some(b'\n') => {
            tokenizer.exit(Name::DirectiveContainerFence);
            State::Ok
        }
        _ => State::Nok,
    }
}

/// 구성요소 상태 초기화.
fn reset(tokenizer: &mut Tokenizer) {
    tokenizer.tokenize_state.size = 0;
    tokenizer.tokenize_state.size_b = 0;
    tokenizer.tokenize_state.size_c = 0;
    tokenizer.tokenize_state.directive_container_chunk_index = None;
}
