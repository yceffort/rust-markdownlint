//! Heading (atx) occurs in the [flow][] content type.
//!
//! ## Grammar
//!
//! Heading (atx) forms with the following BNF
//! (<small>see [construct][crate::construct] for character groups</small>):
//!
//! ```bnf
//! heading_atx ::= 1*6'#' [ 1*space_or_tab line [ 1*space_or_tab 1*'#' ] ] *space_or_tab
//! ```
//!
//! As this construct occurs in flow, like all flow constructs, it must be
//! followed by an eol (line ending) or eof (end of file).
//!
//! `CommonMark` introduced the requirement on whitespace existing after the
//! opening sequence and before text.
//! In older markdown versions, this was not required, and headings would form
//! without it.
//!
//! In markdown, it is also possible to create headings with a
//! [heading (setext)][heading_setext] construct.
//! The benefit of setext headings is that their text can include line endings,
//! and by extensions also hard breaks (e.g., with
//! [hard break (escape)][hard_break_escape]).
//! However, their limit is that they cannot form `<h3>` through `<h6>`
//! headings.
//!
//! > 🏛 **Background**: the word *setext* originates from a small markup
//! > language by Ian Feldman from 1991.
//! > See [*§ Setext* on Wikipedia][wiki_setext] for more info.
//! > The word *atx* originates from a tiny markup language by Aaron Swartz
//! > from 2002.
//! > See [*§ atx, the true structured text format* on `aaronsw.com`][atx] for
//! > more info.
//!
//! ## HTML
//!
//! Headings in markdown relate to the `<h1>` through `<h6>` elements in HTML.
//! See [*§ 4.3.6 The `h1`, `h2`, `h3`, `h4`, `h5`, and `h6` elements* in the
//! HTML spec][html] for more info.
//!
//! ## Recommendation
//!
//! Always use heading (atx), never heading (setext).
//!
//! ## Tokens
//!
//! * [`HeadingAtx`][Name::HeadingAtx]
//! * [`HeadingAtxSequence`][Name::HeadingAtxSequence]
//! * [`HeadingAtxText`][Name::HeadingAtxText]
//! * [`SpaceOrTab`][Name::SpaceOrTab]
//!
//! ## References
//!
//! * [`heading-atx.js` in `micromark`](https://github.com/micromark/micromark/blob/main/packages/micromark-core-commonmark/dev/lib/heading-atx.js)
//! * [*§ 4.2 ATX headings* in `CommonMark`](https://spec.commonmark.org/0.31/#atx-headings)
//!
//! [flow]: crate::construct::flow
//! [heading_setext]: crate::construct::heading_setext
//! [hard_break_escape]: crate::construct::hard_break_escape
//! [html]: https://html.spec.whatwg.org/multipage/sections.html#the-h1,-h2,-h3,-h4,-h5,-and-h6-elements
//! [wiki_setext]: https://en.wikipedia.org/wiki/Setext
//! [atx]: http://www.aaronsw.com/2002/atx/

use crate::construct::partial_space_or_tab::{space_or_tab, space_or_tab_min_max};
use crate::event::{Content, Event, Kind, Link, Name};
use crate::resolve::Name as ResolveName;
use crate::state::{Name as StateName, State};
use crate::subtokenize::Subresult;
use crate::tokenizer::Tokenizer;
use crate::util::constant::{HEADING_ATX_OPENING_FENCE_SIZE_MAX, TAB_SIZE};
use alloc::vec;

/// Start of a heading (atx).
///
/// ```markdown
/// > | ## aa
///     ^
/// ```
pub fn start(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.parse_state.options.constructs.heading_atx {
        tokenizer.enter(Name::HeadingAtx);
        if matches!(tokenizer.current, Some(b'\t' | b' ')) {
            tokenizer.attempt(State::Next(StateName::HeadingAtxBefore), State::Nok);
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
            State::Retry(StateName::HeadingAtxBefore)
        }
    } else {
        State::Nok
    }
}

/// After optional whitespace, at `#`.
///
/// ```markdown
/// > | ## aa
///     ^
/// ```
pub fn before(tokenizer: &mut Tokenizer) -> State {
    if Some(b'#') == tokenizer.current {
        tokenizer.enter(Name::HeadingAtxSequence);
        State::Retry(StateName::HeadingAtxSequenceOpen)
    } else {
        State::Nok
    }
}

/// In opening sequence.
///
/// ```markdown
/// > | ## aa
///     ^
/// ```
pub fn sequence_open(tokenizer: &mut Tokenizer) -> State {
    if tokenizer.current == Some(b'#')
        && tokenizer.tokenize_state.size < HEADING_ATX_OPENING_FENCE_SIZE_MAX
    {
        tokenizer.tokenize_state.size += 1;
        tokenizer.consume();
        State::Next(StateName::HeadingAtxSequenceOpen)
    }
    // Always at least one `#`.
    else if matches!(tokenizer.current, None | Some(b'\t' | b'\n' | b' ')) {
        tokenizer.tokenize_state.size = 0;
        tokenizer.exit(Name::HeadingAtxSequence);
        State::Retry(StateName::HeadingAtxAtBreak)
    } else {
        tokenizer.tokenize_state.size = 0;
        State::Nok
    }
}

/// After something, before something else.
///
/// ```markdown
/// > | ## aa
///       ^
/// ```
pub fn at_break(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        None | Some(b'\n') => {
            tokenizer.exit(Name::HeadingAtx);
            tokenizer.register_resolver(ResolveName::HeadingAtx);
            // Feel free to interrupt.
            tokenizer.interrupt = false;
            State::Ok
        }
        Some(b'\t' | b' ') => {
            tokenizer.attempt(State::Next(StateName::HeadingAtxAtBreak), State::Nok);
            State::Retry(space_or_tab(tokenizer))
        }
        Some(b'#') => {
            tokenizer.enter(Name::HeadingAtxSequence);
            State::Retry(StateName::HeadingAtxSequenceFurther)
        }
        Some(_) => {
            tokenizer.enter_link(
                Name::Data,
                Link {
                    previous: None,
                    next: None,
                    content: Content::Text,
                },
            );
            State::Retry(StateName::HeadingAtxData)
        }
    }
}

/// In further sequence (after whitespace).
///
/// Could be normal “visible” hashes in the heading or a final sequence.
///
/// ```markdown
/// > | ## aa ##
///           ^
/// ```
pub fn sequence_further(tokenizer: &mut Tokenizer) -> State {
    if let Some(b'#') = tokenizer.current {
        tokenizer.consume();
        State::Next(StateName::HeadingAtxSequenceFurther)
    } else {
        tokenizer.exit(Name::HeadingAtxSequence);
        State::Retry(StateName::HeadingAtxAtBreak)
    }
}

/// In text.
///
/// ```markdown
/// > | ## aa
///        ^
/// ```
pub fn data(tokenizer: &mut Tokenizer) -> State {
    match tokenizer.current {
        // Note: `#` for closing sequence must be preceded by whitespace, otherwise it’s just text.
        None | Some(b'\t' | b'\n' | b' ') => {
            tokenizer.exit(Name::Data);
            State::Retry(StateName::HeadingAtxAtBreak)
        }
        _ => {
            tokenizer.consume();
            State::Next(StateName::HeadingAtxData)
        }
    }
}

/// Resolve heading (atx).
///
/// micromark `resolveHeadingAtx` 와 동일: 여는 시퀀스(와 공백) 뒤부터, 닫는 시퀀스(앞 공백 포함)
/// 전까지를 하나의 `HeadingAtxText` 로 묶는다. 본문 안의 `#` 시퀀스(`## # a`)도 본문에 포함된다.
pub fn resolve(tokenizer: &mut Tokenizer) -> Option<Subresult> {
    let mut index = 0;

    while index < tokenizer.events.len() {
        let event = &tokenizer.events[index];

        if event.kind == Kind::Enter && event.name == Name::HeadingAtx {
            let start = index;
            let mut end = start + 1;
            while tokenizer.events[end].name != Name::HeadingAtx {
                end += 1;
            }
            resolve_one(tokenizer, start, end);
            index = end;
        }

        index += 1;
    }

    tokenizer.map.consume(&mut tokenizer.events);
    None
}

/// `start`..=`end` 는 한 heading 의 Enter/Exit `HeadingAtx` 인덱스.
fn resolve_one(tokenizer: &mut Tokenizer, start: usize, end: usize) {
    let name = |offset: isize| -> Option<&Name> {
        let i = start as isize + offset;
        if i < 0 || i as usize > end {
            None
        } else {
            Some(&tokenizer.events[i as usize].name)
        }
    };
    let mut content_end = (end - start) as isize - 1;
    // micromark 는 들여쓰기를 heading 밖의 linePrefix 로 두지만 여기서는 안쪽 `SpaceOrTab` 이다.
    let mut content_start: isize = if name(1) == Some(&Name::SpaceOrTab) {
        5
    } else {
        3
    };

    // Prefix whitespace, part of the opening.
    if name(content_start) == Some(&Name::SpaceOrTab) {
        content_start += 2;
    }

    // Suffix whitespace, part of the closing.
    if content_end - 2 > content_start && name(content_end) == Some(&Name::SpaceOrTab) {
        content_end -= 2;
    }

    if name(content_end) == Some(&Name::HeadingAtxSequence)
        && (content_start == content_end - 1
            || (content_end - 4 > content_start
                && name(content_end - 2) == Some(&Name::SpaceOrTab)))
    {
        content_end -= if content_start + 1 == content_end {
            2
        } else {
            4
        };
    }

    if content_end > content_start {
        let content_start = start + content_start as usize;
        let content_end = start + content_end as usize;
        let start_point = tokenizer.events[content_start].point.clone();
        let end_point = tokenizer.events[content_end].point.clone();
        tokenizer.map.add(
            content_start,
            content_end - content_start + 1,
            vec![
                Event {
                    kind: Kind::Enter,
                    name: Name::HeadingAtxText,
                    point: start_point.clone(),
                    link: None,
                },
                Event {
                    kind: Kind::Enter,
                    name: Name::Data,
                    point: start_point,
                    link: Some(Link {
                        previous: None,
                        next: None,
                        content: Content::Text,
                    }),
                },
                Event {
                    kind: Kind::Exit,
                    name: Name::Data,
                    point: end_point.clone(),
                    link: None,
                },
                Event {
                    kind: Kind::Exit,
                    name: Name::HeadingAtxText,
                    point: end_point,
                    link: None,
                },
            ],
        );
    }
}
