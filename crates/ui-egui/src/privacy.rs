// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the Privacy page of the settings holds: the senders whose remote pictures load without asking, as
//! `remote.list` answered them, read once each time the page is shown.
//!
//! The list is the owner's own choice, but the addresses came from mail, which is outside data: a row keeps the address
//! as listed (to ask again by) and shows it plain and capped where the row is built.

use serde_json::Value;

use crate::component::{capped, plain};

/// The most characters of an address a row shows: an address holds no more (RFC 5321).
const ADDRESS: usize = 254;

/// The most senders the page keeps.
pub const MOST: usize = 5_000;

/// A sender as a row of the page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenderRow {
    /// The address as `remote.list` gave it, to ask again by.
    pub address: String,
    /// The address as the row shows it: plain, on one line, capped.
    pub shown: String,
}

impl SenderRow {
    /// The row of one sender of `remote.list`'s answer; `None` for one that is no text or blank.
    #[must_use]
    pub fn read(value: &Value) -> Option<Self> {
        let address = value.as_str().filter(|address| !address.trim().is_empty())?;
        let (kept, _) = capped(address, ADDRESS);
        Some(Self { address: address.to_owned(), shown: plain(kept).split_whitespace().collect::<Vec<_>>().join(" ") })
    }
}

/// Where the keyboard goes when the page is drawn next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hand {
    /// The "ask again" of the row of this address.
    AskAgain(String),
}

/// The Privacy page's state.
#[derive(Debug, Default)]
pub struct Privacy {
    /// The senders, once read since the page was shown.
    rows: Option<Vec<SenderRow>>,
    /// Where the keyboard goes when the page is drawn next.
    hand: Option<Hand>,
    /// Why the last read or "ask again" was not done.
    note: Option<String>,
}

impl Privacy {
    /// The senders, once read since the page was shown.
    #[must_use]
    pub fn rows(&self) -> Option<&[SenderRow]> {
        self.rows.as_deref()
    }

    /// Takes `remote.list`'s answer: at most [`MOST`] rows.
    pub fn read(&mut self, answer: &Value) {
        let listed = answer.get("senders").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
        self.rows = Some(listed.iter().filter_map(SenderRow::read).take(MOST).collect());
    }

    /// Why the last read or "ask again" was not done.
    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// Says why the last read or "ask again" was not done; `None` when it was.
    pub fn set_note(&mut self, note: Option<String>) {
        self.note = note;
    }

    /// Asks that the keyboard go to `hand` when the page is drawn next.
    pub fn hand_to(&mut self, hand: Hand) {
        self.hand = Some(hand);
    }

    /// Where the keyboard was asked to go, taken.
    pub fn take_hand(&mut self) -> Option<Hand> {
        self.hand.take()
    }

    /// Another place or page is shown: the list is read again when the page shows next.
    pub fn leave(&mut self) {
        self.rows = None;
        self.hand = None;
        self.note = None;
    }
}
