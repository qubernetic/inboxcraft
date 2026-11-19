// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the status line at the window's foot says: the facts at rest (how the syncing stands, the accounts, the
//! outbox, the next event) and at most one message at a time over them. The line is a strip of its own under the
//! body, so nothing it says covers a control of the panes.
//!
//! A message is something that just happened: an act that can be taken back ([`Message::Undo`], for the `undo` token's
//! 5 000 ms), a send that can still be taken back ([`Message::Countdown`]), a notice ([`Message::Notice`], 4 000 ms)
//! or a failure ([`Message::Error`], which stays until dismissed). The window holds the undo and the countdown itself
//! (they are its acts' and its sends' state) and hands them in each frame; the notices and the errors wait here, in a
//! queue of at most [`QUEUE`]. The one shown is the most urgent: a countdown, then an undo, then the oldest error,
//! then the oldest notice. A notice's time starts when it shows.
//!
//! Every text the line shows that comes from outside (an account's name, a server's failure, a subject) goes through
//! [`clean`] first: one line of at most [`MOST`] characters.
//!
//! Under 1280 points of window the line keeps three facts and its last three keys, under 980 two and two
//! ([`kept_under`]); the rule is written here and switched on with the widths.

use std::collections::VecDeque;

use crate::motion;

/// The most characters a text of the line keeps, the `…` included.
pub(crate) const MOST: usize = 300;

/// The most notices and errors waiting: past it the oldest notice goes, else the oldest error.
pub(crate) const QUEUE: usize = 8;

/// A message of the line.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Message {
    /// An act that can be taken back, until `until` (the window's clock, in seconds).
    Undo { text: String, until: f64 },
    /// A send that waits for its hold, until `until`; `text` names it.
    Countdown { text: String, until: f64 },
    /// A notice, until `until` once it shows.
    Notice { text: String, until: f64 },
    /// A failure: it stays until dismissed.
    Error { text: String },
}

impl Message {
    /// What it says.
    pub(crate) fn text(&self) -> &str {
        match self {
            Self::Undo { text, .. } | Self::Countdown { text, .. } | Self::Notice { text, .. } | Self::Error { text } => text,
        }
    }
}

/// A notice or an error waiting, with when it first showed.
#[derive(Debug, Clone, PartialEq)]
struct Waiting {
    message: Message,
    since: Option<f64>,
}

/// The line's messages: at most one shown, the notices and errors in a queue of at most [`QUEUE`].
#[derive(Debug, Default)]
pub(crate) struct Status {
    queue: VecDeque<Waiting>,
}

impl Status {
    /// Queues a notice (its `until` is counted from when it shows: give its length) or an error, its text cleaned. The
    /// queue is full at [`QUEUE`]: the oldest notice makes room, or with none the oldest error. An undo or a countdown
    /// is the window's, handed in to [`Status::shown`], and is not queued.
    pub(crate) fn push(&mut self, message: Message) {
        let message = match message {
            Message::Notice { text, until } => Message::Notice { text: clean(&text), until },
            Message::Error { text } => Message::Error { text: clean(&text) },
            Message::Undo { .. } | Message::Countdown { .. } => return,
        };
        if self.queue.len() >= QUEUE {
            let notice = self.queue.iter().position(|waiting| matches!(waiting.message, Message::Notice { .. }));
            self.queue.remove(notice.unwrap_or(0));
        }
        self.queue.push_back(Waiting { message, since: None });
    }

    /// The message the line shows at `now`, given the window's countdown and undo: the countdown, else the undo, else
    /// the oldest error, else the oldest notice, whose time starts now if it had not shown. A notice that ran out goes.
    pub(crate) fn shown(&mut self, now: f64, countdown: Option<Message>, undo: Option<Message>) -> Option<Message> {
        self.queue.retain(|waiting| match (&waiting.message, waiting.since) {
            (Message::Notice { until, .. }, Some(since)) => now - since < *until,
            _ => true,
        });
        if let Some(over) = countdown.or(undo) {
            return Some(over);
        }
        let at = self
            .queue
            .iter()
            .position(|waiting| matches!(waiting.message, Message::Error { .. }))
            .or_else(|| (!self.queue.is_empty()).then_some(0))?;
        let waiting = self.queue.get_mut(at)?;
        let since = *waiting.since.get_or_insert(now);
        Some(match &waiting.message {
            Message::Notice { text, until } => Message::Notice { text: text.clone(), until: since + until },
            other => other.clone(),
        })
    }

    /// Dismisses the error shown: the oldest. Its text, when there was one.
    pub(crate) fn dismiss(&mut self) -> Option<String> {
        let at = self.queue.iter().position(|waiting| matches!(waiting.message, Message::Error { .. }))?;
        self.queue.remove(at).map(|waiting| waiting.message.text().to_owned())
    }

    /// The newest notice waiting or shown: what it says.
    pub(crate) fn notice(&self) -> Option<&str> {
        self.queue.iter().rev().find(|waiting| matches!(waiting.message, Message::Notice { .. })).map(|waiting| waiting.message.text())
    }

    /// Drops every notice: something newer says what happened.
    pub(crate) fn forget_notices(&mut self) {
        self.queue.retain(|waiting| !matches!(waiting.message, Message::Notice { .. }));
    }

    /// How many notices and errors wait, the one shown included.
    #[cfg(test)]
    pub(crate) fn waiting(&self) -> usize {
        self.queue.len()
    }
}

/// How long an undo is offered, in seconds: the `undo` token.
pub(crate) const UNDO_SECONDS: f64 = motion::UNDO;

/// How long a notice shows, in seconds: the `notice` token.
pub(crate) const NOTICE_SECONDS: f64 = motion::NOTICE;

/// How many facts and keys the line keeps in a window `width` points wide: all of them from 1280, three and three under
/// it, two and two under 980.
#[cfg_attr(not(test), expect(dead_code, reason = "switched on with the widths"))]
pub(crate) fn kept_under(width: f32) -> Option<(usize, usize)> {
    if width < 980.0 {
        Some((2, 2))
    } else if width < 1280.0 {
        Some((3, 3))
    } else {
        None
    }
}

/// `text` from outside as one line of the status line: CR, LF, NUL and every other control character put as one space,
/// every bidi control (embeddings, overrides, isolates and the direction marks) left out, runs of spaces made one, the
/// ends trimmed, and at most [`MOST`] characters, the last a `…` when it was cut. It reads no further into `text` than
/// it keeps, so a text of any length costs no more than its first few hundred characters.
pub(crate) fn clean(text: &str) -> String {
    let mut out = String::new();
    let mut count = 0_usize;
    let mut space = false;
    let mut cut = false;
    for c in text.chars() {
        if bidi(c) {
            continue;
        }
        if c.is_control() || c.is_whitespace() {
            space = count > 0;
            continue;
        }
        // One more character (and the space before it) must leave room for the `…`.
        let needs = usize::from(space).saturating_add(1);
        if count.saturating_add(needs) > MOST.saturating_sub(1) {
            cut = true;
            break;
        }
        if space {
            out.push(' ');
            count = count.saturating_add(1);
            space = false;
        }
        out.push(c);
        count = count.saturating_add(1);
    }
    if cut {
        out.push('…');
    }
    out
}

/// Whether `c` is a bidi control: an embedding, override or isolate, or a direction mark.
fn bidi(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}')
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
