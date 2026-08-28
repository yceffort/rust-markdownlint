//! Data occurs in the [string][] and [text][] content types.
//!
//! It can include anything (except for line endings) and stops at certain
//! characters.
//!
//! [string]: crate::construct::string
//! [text]: crate::construct::text

use crate::event::{Event, Kind, Name};
use crate::state::{Name as StateName, State};
use crate::subtokenize::Subresult;
use crate::tokenizer::{DataSplitKind, Tokenizer};
use alloc::{vec, vec::Vec};

/// At beginning of data.
///
/// ```markdown
/// > | abc
///     ^
/// ```
pub fn start(tokenizer: &mut Tokenizer) -> State {
    // Make sure to eat the first `markers`.
    if let Some(byte) = tokenizer.current {
        if tokenizer.tokenize_state.is_marker(byte) {
            tokenizer.enter(Name::Data);
            tokenizer.consume();
            return State::Next(StateName::DataInside);
        }
    }

    State::Retry(StateName::DataAtBreak)
}

/// Before something.
///
/// ```markdown
/// > | abc
///     ^
/// ```
pub fn at_break(tokenizer: &mut Tokenizer) -> State {
    if let Some(byte) = tokenizer.current {
        if !tokenizer.tokenize_state.is_marker(byte) {
            if byte == b'\n' {
                tokenizer.enter(Name::LineEnding);
                tokenizer.consume();
                tokenizer.exit(Name::LineEnding);
                return State::Next(StateName::DataAtBreak);
            }
            tokenizer.enter(Name::Data);
            return State::Retry(StateName::DataInside);
        }
    }

    State::Ok
}

/// In data.
///
/// ```markdown
/// > | abc
///     ^^^
/// ```
pub fn inside(tokenizer: &mut Tokenizer) -> State {
    if let Some(byte) = tokenizer.current {
        if byte != b'\n' && !tokenizer.tokenize_state.is_marker(byte) {
            tokenizer.consume();
            return State::Next(StateName::DataInside);
        }
    }

    tokenizer.exit(Name::Data);
    State::Retry(StateName::DataAtBreak)
}

/// Merge adjacent data events.
pub fn resolve(tokenizer: &mut Tokenizer) -> Option<Subresult> {
    let mut index = 0;

    // Loop through events and merge adjacent data events.
    while index < tokenizer.events.len() {
        let event = &tokenizer.events[index];

        if event.kind == Kind::Enter && event.name == Name::Data {
            // Move to exit.
            index += 1;

            let mut exit_index = index;

            // Find the farthest `data` event exit event.
            while exit_index + 1 < tokenizer.events.len()
                && tokenizer.events[exit_index + 1].name == Name::Data
            {
                exit_index += 2;
            }

            if exit_index > index {
                tokenizer.map.add(index, exit_index - index, vec![]);
                // Change positional info.
                tokenizer.events[index].point = tokenizer.events[exit_index].point.clone();
                // Move to the end.
                index = exit_index;
            }
        }

        index += 1;
    }

    tokenizer.map.consume(&mut tokenizer.events);
    None
}

/// 로컬 확장: micromark 가 합치지 않는 경계에서 병합된 data 를 다시 나눈다.
///
/// micromark 는 data 병합(`resolveAllText`)을 label/attention 리졸버보다 먼저 돌리므로, 짝 없는
/// attention 시퀀스와 남은 label 시작은 최상위에서 인접 data 와 합쳐지지 않는다. 강조 매치 안과
/// 링크 라벨 안에서는 `insideSpan` 리졸버가 다시 합친다. 여기서는 [`resolve`] 가 전부 합친 뒤
/// `tokenize_state.data_splits` 에 기록된 경계 중 micromark 가 남겼을 것만 되살린다.
pub fn resolve_splits(tokenizer: &mut Tokenizer) -> Option<Subresult> {
    let mut splits = core::mem::take(&mut tokenizer.tokenize_state.data_splits);
    if splits.is_empty() {
        return None;
    }
    splits.sort_by_key(|split| split.start);

    let bytes = tokenizer.parse_state.bytes;
    let mut stack: Vec<Name> = vec![];
    let mut index = 0;

    while index < tokenizer.events.len() {
        let event = &tokenizer.events[index];
        if event.kind == Kind::Exit {
            stack.pop();
            index += 1;
            continue;
        }
        if event.name != Name::Data {
            stack.push(event.name.clone());
            index += 1;
            continue;
        }

        let enter = event.clone();
        let exit = tokenizer.events[index + 1].clone();
        debug_assert!(
            exit.kind == Kind::Exit && exit.name == Name::Data,
            "expected data exit"
        );
        let in_emphasis = stack
            .iter()
            .any(|name| matches!(name, Name::EmphasisText | Name::StrongText));
        let in_label = stack.iter().any(|name| *name == Name::LabelText);

        // 이 data 안에 들어 있는 경계 중 micromark 가 남겼을 것.
        let mut cuts: Vec<usize> = vec![];
        for split in splits
            .iter()
            .filter(|split| split.start >= enter.point.index && split.end <= exit.point.index)
        {
            let merged = match split.kind {
                DataSplitKind::Attention => in_emphasis || in_label,
                DataSplitKind::Label { merge_in_attention } => merge_in_attention && in_emphasis,
            };
            if merged {
                continue;
            }
            if split.start > enter.point.index {
                cuts.push(split.start);
            }
            if split.end < exit.point.index {
                cuts.push(split.end);
            }
        }
        cuts.dedup();

        if !cuts.is_empty() {
            let mut replacement = vec![];
            let mut previous = enter.point.clone();
            let mut cut_index = 0;
            while cut_index < cuts.len() {
                let point = previous.shift_to(bytes, cuts[cut_index]);
                replacement.push(Event {
                    kind: Kind::Enter,
                    name: Name::Data,
                    point: previous,
                    link: None,
                });
                replacement.push(Event {
                    kind: Kind::Exit,
                    name: Name::Data,
                    point: point.clone(),
                    link: None,
                });
                previous = point;
                cut_index += 1;
            }
            replacement.push(Event {
                kind: Kind::Enter,
                name: Name::Data,
                point: previous,
                link: None,
            });
            replacement.push(exit);
            // 원래 enter 의 link 는 첫 조각이 이어받는다.
            replacement[0].link = enter.link;
            tokenizer.map.add(index, 2, replacement);
        }

        index += 2;
    }

    tokenizer.map.consume(&mut tokenizer.events);
    None
}
