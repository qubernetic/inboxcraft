// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The text of a field as its edit sees it: a [`TextBuffer`] around the field's own `String` that never lets it pass
//! its limit, keeps a journal of the frame's changes, and when it refuses an insert rolls the whole frame back in the
//! same buffer.

use std::any::TypeId;
use std::ops::Range;

use egui::TextBuffer;
use egui::text::CharIndex;

use super::Cap;

/// One change made to the text in a frame, in bytes, as it was made: what undoes it.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    /// `len` bytes went in at `at`.
    Inserted { at: usize, len: usize },
    /// `text` came out at `at`.
    Removed { at: usize, text: String },
}

/// The most steps a frame's journal keeps; past them, or past [`JOURNAL_BYTES`] removed bytes beyond the length of
/// the frame's starting text, it keeps one copy of that text instead and no more steps. So a frame of 10 000 events
/// keeps at most 4 096 steps, the bytes they removed (at most the larger of the text's length and 64 KiB, and one
/// step more), and then at most one copy of the text.
pub(super) const JOURNAL_STEPS: usize = 4096;

/// The removed bytes a frame's journal keeps beyond the length of the frame's starting text (see [`JOURNAL_STEPS`]).
pub(super) const JOURNAL_BYTES: usize = 64 << 10;

/// Overwrites every byte `text` holds, its spare room too, with zeros and leaves it empty.
pub(super) fn wipe(text: &mut String) {
    let mut bytes = std::mem::take(text).into_bytes();
    let room = bytes.capacity();
    bytes.clear();
    bytes.resize(room, 0);
    std::hint::black_box(&bytes);
}

/// What came of a frame of [`Capped`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Outcome {
    /// An insert was refused, and the frame was rolled back: the text is as the frame found it.
    pub refused: bool,
    /// The frame changed the text (and was not rolled back).
    pub changed: bool,
}

/// The type [`Capped::type_id`] names: no one can take a `Capped` for another buffer.
struct CappedMarker;

/// A field's `String` as its `TextEdit` edits it. Every mutating method of egui 0.37's [`TextBuffer`] goes through
/// it: `insert_text` and `delete_char_range`, and `clear`, `replace_with` and `take`, which it overrides; the trait's
/// other mutators (`insert_text_at`, `decrease_indentation`, `delete_selected`, `delete_selected_ccursor_range`,
/// `delete_previous_char`, `delete_next_char`, `delete_previous_word`, `delete_next_word`,
/// `delete_surrounding_chars`, `delete_paragraph_before_cursor`, `delete_paragraph_after_cursor`) are its defaults
/// over those two.
///
/// The text never holds more than `limit` (in the cap's unit): an insert that would take it past is not made, and
/// the whole frame is rolled back from the journal at once, in the same buffer, so that the rest of the frame (and
/// egui's undo history) sees the text the frame began with; after that the frame changes nothing more. Nothing is
/// ever cut. A password's buffer never moves while it is big enough; when it is not, the text moves to a bigger one
/// and the old one is zeroed. What the journal took out of a password is zeroed when it goes.
pub(super) struct Capped<'a> {
    text: &'a mut String,
    cap: Cap,
    limit: usize,
    /// The text's size now, in the cap's unit.
    size: usize,
    mutable: bool,
    secret: bool,
    journal: Vec<Step>,
    /// The bytes the journal's removed texts hold.
    removed: usize,
    /// The length in bytes of the text the frame began with.
    start_len: usize,
    /// The text the frame began with, once the journal grew past its bounds.
    base: Option<String>,
    refused: bool,
}

impl<'a> Capped<'a> {
    /// `text`, holding at most `limit` in the unit of `cap`; it takes no change when not `mutable`; `secret` for a
    /// password.
    pub(super) fn new(text: &'a mut String, cap: Cap, limit: usize, mutable: bool, secret: bool) -> Self {
        let size = cap.measure(text);
        let start_len = text.len();
        Self { text, cap, limit, size, mutable, secret, journal: Vec::new(), removed: 0, start_len, base: None, refused: false }
    }

    /// What came of the frame; the journal goes (zeroed for a password).
    pub(super) fn finish(&mut self) -> Outcome {
        let changed = !self.refused && (!self.journal.is_empty() || self.base.is_some());
        self.forget();
        Outcome { refused: self.refused, changed }
    }

    /// The byte where the character `index` starts, or the text's end.
    fn byte_at(&self, index: usize) -> usize {
        if index == 0 {
            return 0;
        }
        self.text.char_indices().nth(index).map_or(self.text.len(), |(at, _)| at)
    }

    fn insert(&mut self, insert: &str, index: usize) -> usize {
        if !self.mutable || self.refused || insert.is_empty() {
            return 0;
        }
        let added = self.cap.measure(insert);
        if self.size.checked_add(added).is_none_or(|after| after > self.limit) {
            self.refuse();
            return 0;
        }
        let at = self.byte_at(index);
        if self.secret {
            self.make_room(insert.len());
        }
        if !self.text.is_char_boundary(at) {
            return 0;
        }
        self.text.insert_str(at, insert);
        self.size = self.size.saturating_add(added);
        self.record(Step::Inserted { at, len: insert.len() });
        insert.chars().count()
    }

    fn delete(&mut self, from: usize, to: usize) {
        if !self.mutable || self.refused || from >= to {
            return;
        }
        let (start, end) = (self.byte_at(from), self.byte_at(to));
        self.delete_bytes(start, end);
    }

    fn delete_bytes(&mut self, start: usize, end: usize) {
        if !self.mutable || self.refused || start >= end {
            return;
        }
        let Some(gone) = self.text.get(start..end) else { return };
        let size = self.cap.measure(gone);
        let gone = if self.base.is_some() { String::new() } else { gone.to_owned() };
        self.text.replace_range(start..end, "");
        self.size = self.size.saturating_sub(size);
        self.record(Step::Removed { at: start, text: gone });
    }

    /// A password's buffer grows only here: to a new one, the old one zeroed, never by `String`'s own moving.
    fn make_room(&mut self, more: usize) {
        let needed = self.text.len().saturating_add(more);
        if needed <= self.text.capacity() {
            return;
        }
        let room = needed.max(self.text.capacity().saturating_mul(2)).max(self.cap.most().saturating_mul(4));
        let mut fresh = String::with_capacity(room);
        fresh.push_str(self.text);
        let mut old = std::mem::replace(self.text, fresh);
        wipe(&mut old);
    }

    /// Keeps `step` in the journal, or, past its bounds, a copy of the frame's starting text instead.
    fn record(&mut self, step: Step) {
        if self.base.is_some() {
            self.drop_step(step);
            return;
        }
        if let Step::Removed { text, .. } = &step {
            self.removed = self.removed.saturating_add(text.len());
        }
        self.journal.push(step);
        if self.journal.len() >= JOURNAL_STEPS || self.removed > self.start_len.max(JOURNAL_BYTES) {
            let mut base = String::with_capacity(self.text.capacity());
            base.push_str(self.text);
            let steps = std::mem::take(&mut self.journal);
            self.undo(&mut base, steps);
            self.removed = 0;
            self.base = Some(base);
        }
    }

    /// Undoes `steps` on `text`, the last first.
    fn undo(&self, text: &mut String, steps: Vec<Step>) {
        for step in steps.into_iter().rev() {
            match step {
                Step::Inserted { at, len } => {
                    let end = at.saturating_add(len);
                    if text.get(at..end).is_some() {
                        text.replace_range(at..end, "");
                    }
                }
                Step::Removed { at, text: mut gone } => {
                    if text.is_char_boundary(at) {
                        text.insert_str(at, &gone);
                    }
                    if self.secret {
                        wipe(&mut gone);
                    }
                }
            }
        }
    }

    fn drop_step(&self, step: Step) {
        if let Step::Removed { text: mut gone, .. } = step
            && self.secret
        {
            wipe(&mut gone);
        }
    }

    /// Refuses the frame: the text goes back to what the frame began with, in the same buffer.
    fn refuse(&mut self) {
        self.refused = true;
        if let Some(mut base) = self.base.take() {
            self.text.clear();
            self.text.push_str(&base);
            if self.secret {
                wipe(&mut base);
            }
        } else {
            let steps = std::mem::take(&mut self.journal);
            let mut text = std::mem::take(self.text);
            self.undo(&mut text, steps);
            *self.text = text;
        }
        self.removed = 0;
        self.size = self.cap.measure(self.text);
    }

    /// Lets the journal go, a password's removed texts zeroed.
    fn forget(&mut self) {
        for step in std::mem::take(&mut self.journal) {
            self.drop_step(step);
        }
        if let Some(mut base) = self.base.take()
            && self.secret
        {
            wipe(&mut base);
        }
        self.removed = 0;
    }
}

impl Drop for Capped<'_> {
    fn drop(&mut self) {
        self.forget();
    }
}

impl TextBuffer for Capped<'_> {
    fn is_mutable(&self) -> bool {
        self.mutable
    }

    fn as_str(&self) -> &str {
        self.text.as_str()
    }

    fn insert_text(&mut self, text: &str, char_index: CharIndex) -> usize {
        self.insert(text, char_index.0)
    }

    fn delete_char_range(&mut self, char_range: Range<CharIndex>) {
        self.delete(char_range.start.0, char_range.end.0);
    }

    fn clear(&mut self) {
        let end = self.text.len();
        self.delete_bytes(0, end);
    }

    fn replace_with(&mut self, text: &str) {
        self.clear();
        self.insert(text, 0);
    }

    fn take(&mut self) -> String {
        let taken = self.text.clone();
        self.clear();
        taken
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<CappedMarker>()
    }
}

#[cfg(test)]
#[path = "field_capped_tests.rs"]
mod tests;
