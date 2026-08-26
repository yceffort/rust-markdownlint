//! Turn bytes of markdown into events.

use crate::event::{Content, Event, Point};
use crate::message;
use crate::state::{Name as StateName, State};
use crate::subtokenize::subtokenize;
use crate::tokenizer::Tokenizer;
use crate::util::location::Location;
use crate::ParseOptions;
use alloc::{string::String, vec, vec::Vec};

/// Info needed, in all content types, when parsing markdown.
///
/// Importantly, this contains a set of known definitions.
/// It also references the input value as bytes (`u8`).
#[derive(Debug)]
pub struct ParseState<'a> {
    /// Configuration.
    pub location: Option<Location>,
    /// Configuration.
    pub options: &'a ParseOptions,
    /// List of chars.
    pub bytes: &'a [u8],
    /// Set of defined definition identifiers.
    pub definitions: Vec<String>,
    /// Set of defined GFM footnote definition identifiers.
    pub gfm_footnote_definitions: Vec<String>,
}

/// Turn a string of markdown into events.
///
/// Passes the bytes back so the compiler can access the source.
pub fn parse<'a>(
    value: &'a str,
    options: &'a ParseOptions,
) -> Result<(Vec<Event>, ParseState<'a>), message::Message> {
    let bytes = value.as_bytes();

    // 로컬 패치: 이전 parse 의 undefined reference 잔여물 제거.
    crate::undefined_refs::clear();

    let mut parse_state = ParseState {
        options,
        bytes,
        location: if options.mdx_esm_parse.is_some() || options.mdx_expression_parse.is_some() {
            Some(Location::new(bytes))
        } else {
            None
        },
        definitions: vec![],
        gfm_footnote_definitions: vec![],
    };

    let start = Point {
        line: 1,
        column: 1,
        index: 0,
        vs: 0,
    };
    let mut tokenizer = Tokenizer::new(start, &parse_state);

    let state = tokenizer.push(
        (0, 0),
        (parse_state.bytes.len(), 0),
        State::Next(StateName::DocumentStart),
    );
    let mut result = tokenizer.flush(state, true)?;
    let mut events = tokenizer.events;

    // 로컬 확장: 중첩 document(directive 본문)를 먼저 전부 풀어 그 안의 정의를
    // `parse_state` 에 넣는다. micromark 는 하위 tokenizer 가 같은 `parser.defined` 를
    // 즉시 갱신하지만 여기서는 pass 가 끝나야 합쳐지므로, 텍스트를 풀기 전에 끝낸다.
    loop {
        let fn_defs = &mut parse_state.gfm_footnote_definitions;
        let defs = &mut parse_state.definitions;
        fn_defs.append(&mut result.gfm_footnote_definitions);
        defs.append(&mut result.definitions);

        if result.done {
            break;
        }

        result = subtokenize(&mut events, &parse_state, Some(&Content::Document))?;
    }
    result.done = false;

    loop {
        let fn_defs = &mut parse_state.gfm_footnote_definitions;
        let defs = &mut parse_state.definitions;
        fn_defs.append(&mut result.gfm_footnote_definitions);
        defs.append(&mut result.definitions);

        if result.done {
            return Ok((events, parse_state));
        }

        result = subtokenize(&mut events, &parse_state, None)?;
    }
}
