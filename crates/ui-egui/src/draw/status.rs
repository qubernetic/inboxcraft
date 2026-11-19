// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The status line drawn in its region at the window's foot ([`super::shell::Regions::status`]): a strip of its own,
//! so nothing it says covers the panes. At rest it says the facts ([`Facts`]): the worst state first (a failure with
//! the count of the others, offline, the sync's progress line, or when the last sync ended), the accounts, the outbox
//! and the next of today's events, `·` between them; at its right the key hints of what works now. A message
//! ([`Message`]) takes the line's left, at most 560 wide, with the state's fact after it.
//!
//! The key hints are drawn first, from the right end: the facts and the message have what is left, cut and clipped to
//! it, so no fact pushes a key out of the window.

use egui::{Align, Layout, Rect, Ui, UiBuilder, pos2, vec2};
use serde_json::{Value, json};

use crate::component::key_hint::key_hint;
use crate::component::progress::{ProgressState, progress};
use crate::component::status_item::{self, Kind, StatusItem};
use crate::component::status_message::{self, Left, StatusMessage};
use crate::component::{button, soon};
use crate::keymap::{self, Keymap};
use crate::mailbox::{Mailbox, SendState};
use crate::status::{Message, QUEUE, clean};
use crate::theme::Theme;
use crate::tr;

/// The widest a message is drawn, in points.
pub(crate) const MESSAGE: f32 = 560.0;

/// The room at the line's right end, in points.
const RIGHT: f32 = 12.0;

/// The room before the first fact, in points.
const LEFT: f32 = 6.0;

/// The gap between two parts of the line, in points.
const GAP: f32 = 4.0;

/// The room the keys' block keeps at its sides, in points.
const KEYS_PAD: f32 = 6.0;

/// The gap between two key hints, in points.
const KEYS_GAP: f32 = 16.0;

/// The key hints the line may show, in order: the action and its word.
pub(crate) const HINTS: [(&str, &str); 5] =
    [(keymap::NEXT, "next"), (keymap::PREVIOUS, "previous"), (keymap::DONE, "done"), (keymap::SNOOZE, "snooze"), (keymap::COMMAND_LINE, "commands")];

/// An event this near, in minutes, is the accent of the line.
const SOON_MINUTES: i64 = 15;

/// How the syncing stands, as the line's first fact says it when nothing fails.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Sync {
    /// Synced, the last end of a round on the clock, if there was one.
    Synced(Option<String>),
    /// Syncing: the share done (0 to 1) and the note.
    Syncing(f32, String),
    /// No account signed in.
    Offline,
}

/// The facts the line says at rest, read from the engine's `safe` commands ([`read`]).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Facts {
    /// How the syncing stands.
    pub sync: Sync,
    /// The accounts.
    pub accounts: usize,
    /// The accounts' failures, `name: what failed`, cleaned, the first [`QUEUE`].
    pub failures: Vec<String>,
    /// How many accounts fail.
    pub failing: usize,
    /// The messages in the outbox.
    pub outbox: usize,
    /// When the first held send goes, on the clock, when one waits for a later time.
    pub outbox_goes: Option<String>,
    /// The next of today's events: its start, its title, the minutes until it starts.
    pub next: Option<(String, String, i64)>,
}

impl Default for Facts {
    fn default() -> Self {
        Self { sync: Sync::Synced(None), accounts: 0, failures: Vec::new(), failing: 0, outbox: 0, outbox_goes: None, next: None }
    }
}

/// The facts as the mailbox has them now, through `sync.status`, `account.list` and the next event; a refused read
/// leaves its fact out. Runs commands: call it when the mailbox changed or a second has passed, not every frame.
pub(crate) fn read(mailbox: &mut Mailbox) -> Facts {
    let mut facts = Facts::default();
    let status = mailbox.run("sync.status", &json!({})).unwrap_or(Value::Null);
    let state = status.get("state").and_then(Value::as_str).unwrap_or("synced");
    let number = |key: &str| status.get(key).and_then(Value::as_u64).unwrap_or(0);
    let (done, of) = (number("done"), number("of"));
    let last = status.get("last_synced").and_then(Value::as_str).and_then(crate::mailbox::parse_date).map(|at| mailbox.short_date(at));
    facts.sync = match state {
        "offline" => Sync::Offline,
        "syncing" => {
            #[expect(clippy::cast_precision_loss, reason = "a share of the work: its last digits are not shown")]
            let share = if of == 0 { 0.0 } else { (done.min(of) as f64 / of as f64) as f32 };
            let note = if of == 0 { String::new() } else { tr!("{percent} %", percent = (share * 100.0).round()) };
            Sync::Syncing(share, note)
        }
        _ => Sync::Synced(last),
    };
    if let Ok(answer) = mailbox.run("account.list", &json!({})) {
        let accounts = answer.get("accounts").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
        facts.accounts = accounts.len();
        for account in accounts {
            let Some(failure) = account.get("failure").filter(|failure| !failure.is_null()) else { continue };
            facts.failing = facts.failing.saturating_add(1);
            if facts.failures.len() < QUEUE {
                let name = account.get("label").and_then(Value::as_str).unwrap_or_default();
                let what = failure.get("message").and_then(Value::as_str).unwrap_or_default();
                facts.failures.push(clean(&tr!("{name}: {what}", name = name, what = what)));
            }
        }
    }
    facts.outbox = mailbox.outbox().len();
    let now = mailbox.now();
    let later = mailbox
        .outbox()
        .iter()
        .filter_map(|row| match row.state {
            SendState::Waiting(Some(at)) if at.duration_since(now).is_some_and(|left| left.as_secs() > 60) => Some(at),
            _ => None,
        })
        .min();
    facts.outbox_goes = later.map(|at| mailbox.short_date(at));
    facts.next = crate::home::next_event(mailbox).map(|(time, title, minutes)| (time, clean(&title), minutes));
    facts
}

/// One fact of the line: its text, its kind, and what a press of it asks for.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Part {
    /// A fact.
    Item(String, Kind),
    /// The sync's progress line: the share done and its note.
    Progress(f32, String),
    /// A small link: its words and whether it is coming soon.
    Link(&'static str, bool),
}

/// The line's facts in order, from `facts`; with a message shown only the state's first fact, the other parts left
/// out.
pub(crate) fn parts(facts: &Facts, message: bool) -> Vec<Part> {
    let mut parts = Vec::new();
    let sync = match &facts.sync {
        Sync::Synced(Some(at)) => Part::Item(tr!("synced {at}", at = at), Kind::Ok),
        Sync::Synced(None) => Part::Item(tr!("synced").to_owned(), Kind::Ok),
        Sync::Syncing(share, note) => Part::Progress(*share, note.clone()),
        Sync::Offline => Part::Item(tr!("offline").to_owned(), Kind::Offline),
    };
    if let Some(first) = facts.failures.first() {
        // The worst state wins the first fact, with a count of the rest.
        parts.push(Part::Item(first.clone(), Kind::Error));
        if message {
            return parts;
        }
        let more = facts.failing.saturating_sub(1);
        if more > 0 {
            parts.push(Part::Item(tr!("and {n} more problems", n = more), Kind::Plain));
            parts.push(Part::Link(tr!("open log"), true));
            return parts;
        }
        parts.push(Part::Link(tr!("open account"), false));
    }
    if message {
        parts.push(sync);
        return parts;
    }
    let offline = facts.sync == Sync::Offline;
    parts.push(sync);
    if offline {
        parts.push(Part::Item(tr!("showing what is kept on this computer").to_owned(), Kind::Plain));
        return parts;
    }
    if facts.accounts > 1 {
        parts.push(Part::Item(tr!("{n} accounts", n = facts.accounts), Kind::Plain));
    }
    if facts.outbox > 0 {
        match &facts.outbox_goes {
            Some(at) => parts.push(Part::Item(tr!("outbox {n} · one goes at {at}", n = facts.outbox, at = at), Kind::Waiting)),
            None => parts.push(Part::Item(tr!("outbox {n}", n = facts.outbox), Kind::Plain)),
        }
    }
    if let Some((time, title, minutes)) = &facts.next {
        if (0..=SOON_MINUTES).contains(minutes) {
            parts.push(Part::Item(tr!("next: {title} {time} · in {m} min", title = title, time = time, m = minutes), Kind::Accent));
        } else {
            parts.push(Part::Item(tr!("next: {title} {time}", title = title, time = time), Kind::Plain));
        }
    }
    parts
}

/// What was pressed in the line this frame.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Pressed {
    /// The message's Undo, by its link or its key.
    pub undo: bool,
    /// The error's dismiss.
    pub dismiss: bool,
    /// "open account" of a failing account.
    pub open_account: bool,
    /// A key hint pressed: its action.
    pub hint: Option<&'static str>,
}

/// What the line shows this frame.
pub(crate) struct Shown<'a> {
    /// The facts.
    pub facts: &'a Facts,
    /// The message, if one shows.
    pub message: Option<&'a Message>,
    /// The window's clock, in seconds.
    pub now: f64,
    /// The whole of the undo's or countdown's time, in seconds: what its bar is a share of.
    pub whole: f64,
    /// The key hints that work now: the action, its key in words, its word.
    pub hints: &'a [(&'static str, String, &'static str)],
    /// Whether Undo's key works now (nothing quiets it).
    pub undo_key: bool,
    /// Whether Undo rests: drawn unavailable, its link and its key do nothing.
    pub rests: bool,
}

/// Draws the line in `rect`; what was pressed in it.
pub(crate) fn line(ui: &mut Ui, theme: &Theme, keymap: &Keymap, rect: Rect, shown: &Shown<'_>) -> Pressed {
    let mut pressed = Pressed::default();
    let p = &theme.palette;
    ui.painter().rect_filled(rect, 0.0, p.panel);
    // The keys first, from the right end: what is left is the facts'.
    let mut keys_left = rect.right() - RIGHT;
    if !shown.hints.is_empty() {
        let keys = Rect::from_min_max(pos2(rect.left(), rect.top()), pos2(rect.right() - RIGHT - KEYS_PAD, rect.bottom()));
        let mut child = ui.new_child(UiBuilder::new().id_salt("status-keys").max_rect(keys).layout(Layout::right_to_left(Align::Center)));
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        child.spacing_mut().item_spacing.x = KEYS_GAP;
        for (action, key, word) in shown.hints.iter().rev() {
            if key_hint(&mut child, theme, key, tr_word(word), None).1 {
                pressed.hint = Some(action);
            }
        }
        keys_left = child.min_rect().left() - KEYS_PAD;
    }
    let room = Rect::from_min_max(rect.min, pos2((keys_left - GAP).max(rect.left()), rect.bottom()));
    let mut left = ui.new_child(UiBuilder::new().id_salt("status-facts").max_rect(room).layout(Layout::left_to_right(Align::Center)));
    left.set_clip_rect(room.intersect(ui.clip_rect()));
    left.spacing_mut().item_spacing.x = GAP;
    if let Some(message) = shown.message {
        let wide = MESSAGE.min(room.width()).max(1.0);
        let at = Rect::from_min_size(room.min, vec2(wide, rect.height()));
        let mut child = left.new_child(UiBuilder::new().id_salt("status-message").max_rect(at).layout(Layout::left_to_right(Align::Center)));
        let pressed_in = message_in(&mut child, theme, keymap, message, shown);
        pressed.undo = pressed_in == Some(status_message::Press::Undo);
        pressed.dismiss = pressed_in == Some(status_message::Press::Dismiss);
        left.advance_cursor_after_rect(at);
    } else {
        left.add_space(LEFT);
    }
    let parts = parts(shown.facts, shown.message.is_some());
    for (at, part) in parts.iter().enumerate() {
        if at > 0 {
            left.label(crate::component::galley(&left, "·", theme.fonts.mono_s, p.dim));
        }
        match part {
            Part::Item(text, kind) => {
                status_item::status_item(&mut left, theme, StatusItem::new(text, *kind));
            }
            Part::Progress(share, note) => {
                progress(&mut left, theme, keymap, tr!("syncing"), ProgressState::Running(*share), note);
            }
            Part::Link(words, coming) => {
                let link = |ui: &mut Ui| button::link_sized(ui, theme, keymap, words, button::LinkSize::Small);
                let response = if *coming { soon::soon(&mut left, theme, link) } else { link(&mut left) };
                pressed.open_account |= !coming && response.clicked();
            }
        }
    }
    pressed
}

/// The message in `ui`, its Undo resting while `shown.rests`; what was pressed in it, Undo by its key too while that
/// works.
fn message_in(ui: &mut Ui, theme: &Theme, keymap: &Keymap, message: &Message, shown: &Shown<'_>) -> Option<status_message::Press> {
    // Its key is not offered while something else has it (a field, the composer).
    let key = keymap.chip(ui.ctx(), keymap::UNDO).filter(|_| shown.undo_key);
    let left = |until: f64| {
        let rest = (until - shown.now).max(0.0);
        let whole = if shown.whole > 0.0 { shown.whole } else { rest.max(1.0) };
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "seconds of a hold of a few seconds, at least 0")]
        let seconds = rest.ceil().min(f64::from(u32::MAX)) as u32;
        #[expect(clippy::cast_possible_truncation, reason = "a share of 0 to 1")]
        let share = (rest.ceil() / whole.ceil().max(1.0)).clamp(0.0, 1.0) as f32;
        Some(Left { seconds, share })
    };
    let (kind, undoes) = match message {
        Message::Undo { until, .. } => (status_message::Kind::Undo { key: key.as_deref(), left: left(*until) }, true),
        Message::Countdown { until, .. } => (status_message::Kind::Countdown { key: key.as_deref(), left: left(*until) }, true),
        Message::Notice { .. } => (status_message::Kind::Notice, false),
        Message::Error { .. } => (status_message::Kind::Error, false),
    };
    let drawn = StatusMessage { kind, text: message.text() };
    let press = ui.add_enabled_ui(!(undoes && shown.rests), |ui| status_message::status_message(ui, theme, keymap, &drawn)).inner;
    let press = press.filter(|_| !(undoes && shown.rests));
    if undoes && !shown.rests && shown.undo_key && !soon::is_soon(ui) && keymap.pressed(ui.ctx(), keymap::UNDO) {
        return Some(status_message::Press::Undo);
    }
    press
}

/// A key hint's word, translated.
fn tr_word(word: &str) -> &'static str {
    match word {
        "next" => tr!("next"),
        "previous" => tr!("previous"),
        "done" => tr!("done"),
        "snooze" => tr!("snooze"),
        _ => tr!("commands"),
    }
}
