//! 로컬 패치: markdownlint 의 undefined reference 감지.
//!
//! micromark JS 는 `labelEnd` 토크나이저의 nok 경로를 가로채 실패한 레퍼런스를
//! 기록한다. 같은 지점인 `construct::label_end::nok` 에서 스레드 로컬로 기록한다.

use crate::event::{Event, Kind, Name};
use crate::tokenizer::Tokenizer;
use alloc::vec::Vec;
use core::cell::RefCell;

/// 실패한 label reference 하나. 위치는 (line, byte index).
#[derive(Debug, Clone)]
pub struct UndefinedRef {
    /// label start(`[`/`![`) enter 위치.
    pub start: (usize, usize),
    /// nok 시점 마지막 이벤트 끝 위치.
    pub end: (usize, usize),
    /// label start 이후의 data/lineEnding 스팬. `true` 면 lineEnding.
    pub data: Vec<(bool, (usize, usize), (usize, usize))>,
}

std::thread_local! {
    static REFS: RefCell<Vec<UndefinedRef>> = const { RefCell::new(Vec::new()) };
}

/// 지금까지 기록된 undefined reference 를 꺼내고 비운다.
pub fn take() -> Vec<UndefinedRef> {
    REFS.with(|r| r.borrow_mut().split_off(0))
}

pub(crate) fn clear() {
    REFS.with(|r| r.borrow_mut().clear());
}

pub(crate) fn record(tokenizer: &Tokenizer, open: usize) {
    let events: &[Event] = &tokenizer.events;
    let Some(last) = events.last() else { return };
    let start = &events[open].point;
    let mut data = Vec::new();
    let mut pending: Option<(Name, bool, usize, usize)> = None;
    for ev in &events[open..] {
        let le = ev.name == Name::LineEnding;
        // micromark 라면 data 였을 autolink literal (markdown-rs 는 `[` 뒤에서도 허용) 포함
        let datalike = matches!(
            ev.name,
            Name::Data
                | Name::GfmAutolinkLiteralProtocol
                | Name::GfmAutolinkLiteralWww
                | Name::GfmAutolinkLiteralEmail
                | Name::GfmFootnoteCallMarker
        );
        if !le && !datalike {
            continue;
        }
        match ev.kind {
            Kind::Enter => {
                if pending.is_none() {
                    pending = Some((ev.name.clone(), le, ev.point.line, ev.point.index));
                }
            }
            Kind::Exit => {
                if pending.as_ref().is_some_and(|(n, ..)| *n == ev.name) {
                    let (_, ple, l, i) = pending.take().unwrap();
                    data.push((ple, (l, i), (ev.point.line, ev.point.index)));
                }
            }
        }
    }
    REFS.with(|r| {
        r.borrow_mut().push(UndefinedRef {
            start: (start.line, start.index),
            end: (last.point.line, last.point.index),
            data,
        });
    });
}
