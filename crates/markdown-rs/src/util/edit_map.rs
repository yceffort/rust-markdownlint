//! Deal with several changes in events, batching them together.
//!
//! Preferably, changes should be kept to a minimum.
//! Sometimes, it’s needed to change the list of events, because parsing can be
//! messy, and it helps to expose a cleaner interface of events to the compiler
//! and other users.
//! It can also help to merge many adjacent similar events.
//! And, in other cases, it’s needed to parse subcontent: pass some events
//! through another tokenizer and inject the result.

use crate::event::Event;
use alloc::{collections::BTreeMap, vec::Vec};

/// Shift `previous` and `next` links according to `jumps`.
///
/// This fixes links in case there are events removed or added between them.
fn shift_links(events: &mut [Event], jumps: &[(usize, usize, usize)]) {
    let mut jump_index = 0;
    let mut index = 0;
    let mut add = 0;
    let mut rm = 0;

    while index < events.len() {
        let rm_curr = rm;

        while jump_index < jumps.len() && jumps[jump_index].0 <= index {
            add = jumps[jump_index].2;
            rm = jumps[jump_index].1;
            jump_index += 1;
        }

        // Ignore items that will be removed.
        if rm > rm_curr {
            index += rm - rm_curr;
        } else {
            if let Some(link) = &events[index].link {
                if let Some(next) = link.next {
                    events[next].link.as_mut().unwrap().previous = Some(index + add - rm);

                    while jump_index < jumps.len() && jumps[jump_index].0 <= next {
                        add = jumps[jump_index].2;
                        rm = jumps[jump_index].1;
                        jump_index += 1;
                    }

                    events[index].link.as_mut().unwrap().next = Some(next + add - rm);
                    index = next;
                    continue;
                }
            }

            index += 1;
        }
    }
}

/// Tracks a bunch of edits.
///
/// 로컬 패치: 원본은 `Vec<(at, remove, add)>` 를 추가마다 선형 탐색하고(O(K²)) `consume` 에서
/// `split_off`/`append` 로 이벤트 전체를 두 번 복사했다. 인덱스 순서를 유지하는 맵과
/// 한 번에 새 벡터를 만드는 `consume` 으로 바꿨다. 결과는 같다.
#[derive(Debug)]
pub struct EditMap {
    /// Record of changes: index → (remove, add).
    map: BTreeMap<usize, (usize, Vec<Event>)>,
}

impl EditMap {
    /// Create a new edit map.
    pub fn new() -> EditMap {
        EditMap {
            map: BTreeMap::new(),
        }
    }
    /// Create an edit: a remove and/or add at a certain place.
    pub fn add(&mut self, index: usize, remove: usize, add: Vec<Event>) {
        add_impl(self, index, remove, add, false);
    }
    /// Create an edit: but insert `add` before existing additions.
    pub fn add_before(&mut self, index: usize, remove: usize, add: Vec<Event>) {
        add_impl(self, index, remove, add, true);
    }
    /// Done, change the events.
    pub fn consume(&mut self, events: &mut Vec<Event>) {
        if self.map.is_empty() {
            return;
        }

        // Calculate jumps: where items in the current list move to.
        let mut jumps = Vec::with_capacity(self.map.len());
        let mut add_acc = 0;
        let mut remove_acc = 0;
        for (at, (remove, add)) in &self.map {
            remove_acc += remove;
            add_acc += add.len();
            jumps.push((*at, remove_acc, add_acc));
        }

        shift_links(events, &jumps);

        // Rebuild in one pass: keep, skip removed, insert added.
        let old = core::mem::take(events);
        let len_before = old.len();
        let mut new = Vec::with_capacity(len_before + add_acc - remove_acc);
        let mut iter = old.into_iter();
        let mut pos = 0;
        for (at, (remove, add)) in core::mem::take(&mut self.map) {
            debug_assert!(at >= pos, "expected edits not to overlap");
            new.extend(iter.by_ref().take(at - pos));
            for _ in 0..remove {
                iter.next();
            }
            pos = at + remove;
            new.extend(add);
        }
        new.extend(iter);
        *events = new;
    }
}

/// Create an edit.
fn add_impl(edit_map: &mut EditMap, at: usize, remove: usize, mut add: Vec<Event>, before: bool) {
    if remove == 0 && add.is_empty() {
        return;
    }

    match edit_map.map.get_mut(&at) {
        Some(entry) => {
            entry.0 += remove;
            if before {
                add.append(&mut entry.1);
                entry.1 = add;
            } else {
                entry.1.append(&mut add);
            }
        }
        None => {
            edit_map.map.insert(at, (remove, add));
        }
    }
}
