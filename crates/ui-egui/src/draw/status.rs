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

use egui::{Align, Area, Id, Layout, Order, Rect, Ui, UiBuilder, pos2, vec2};
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

/// The key hints the line may show with a message open, and away from the mail, in order: the action and its word.
pub(crate) const HINTS: [(&str, &str); 5] =
    [(keymap::NEXT, "next"), (keymap::PREVIOUS, "previous"), (keymap::DONE, "done"), (keymap::SNOOZE, "snooze"), (keymap::PALETTE, "commands")];

/// The key hints beside a list with nothing open (the frames `shell/default`, `drafts-list/filled`).
const LIST_HINTS: [(&str, &str); 4] = [(keymap::NEXT, "next"), (keymap::PREVIOUS, "previous"), (keymap::OPEN, "open"), (keymap::PALETTE, "commands")];

/// The key hints beside the outbox, whose rows do not open (the frame `outbox/mixed`).
const OUTBOX_HINTS: [(&str, &str); 3] = [(keymap::NEXT, "next"), (keymap::PREVIOUS, "previous"), (keymap::PALETTE, "commands")];

/// The key hints beside a list that holds nothing (the frame `drafts-list/empty`).
const EMPTY_HINTS: [(&str, &str); 2] = [(keymap::COMPOSE, "write"), (keymap::PALETTE, "commands")];

/// The key hints beside the docked composer (the frames `size-1100/composer`, `size-800/composer`).
const COMPOSER_HINTS: [(&str, &str); 2] = [(keymap::SEND, "send"), (keymap::PALETTE, "commands")];

/// The key hints on Home (the frames `home/*`).
const HOME_HINTS: [(&str, &str); 4] =
    [(keymap::MAIL, "mail"), (keymap::CALENDAR, "calendar"), (keymap::COMPOSE, "write"), (keymap::PALETTE, "commands")];

/// The key hints on Home before any account (the frame `home/first-run`).
const FIRST_RUN_HINTS: [(&str, &str); 1] = [(keymap::PALETTE, "commands")];

/// The key hints on the calendar (the frames `calendar-*`).
const CALENDAR_HINTS: [(&str, &str); 3] = [(keymap::MAIL, "mail"), (keymap::COMPOSE, "write"), (keymap::PALETTE, "commands")];

/// The key hints on the settings, where `Esc` leaves them shown (the frames `settings-*` draw a `back` there too).
const SETTINGS_HINTS: [(&str, &str); 2] = [(keymap::MAIL, "mail"), (keymap::PALETTE, "commands")];

/// The key hints on a settings page alone in one pane, whose `Esc` goes back to the sections (`settings-800/*`).
const SETTINGS_PAGE_HINTS: [(&str, &str); 3] = [(keymap::MAIL, "mail"), (SECTIONS, "back"), (keymap::PALETTE, "commands")];

/// The key hints in one pane with the message in the list's place, whose `Esc` gives the list back
/// (`size-800/inbox-reader`).
const ONE_PANE_HINTS: [(&str, &str); 2] = [(keymap::BACK, "list"), (keymap::PALETTE, "commands")];

/// The key hints under the command line's results, which read their keys themselves (`command-line/*`): shown, and a
/// press on one does nothing (the window's gate under a modal surface drops it).
const RESULTS_HINTS: [(&str, &str); 4] = [(RESULTS_NEXT, "next"), (RESULTS_PREVIOUS, "previous"), (RESULTS_RUN, "run"), (RESULTS_CLOSE, "close")];

/// The key hints while an error shows and `Esc` reaches nothing else (`status-line/error`).
const ERROR_HINTS: [(&str, &str); 2] = [(DISMISS, "dismiss"), (keymap::PALETTE, "commands")];

/// The key hints while an error shows beside the docked composer and `Esc` reaches nothing else (the list over it):
/// both sets kept, the error's `dismiss` first. A narrow line keeps the last two, so there the composer's `send`, whose
/// key still works, stays beside commands, and `Esc` still dismisses.
const ERROR_COMPOSER_HINTS: [(&str, &str); 3] = [(DISMISS, "dismiss"), (keymap::SEND, "send"), (keymap::PALETTE, "commands")];

/// The id of the error's `dismiss` hint, by `Esc` ([`fixed_key`]): a press on it dismisses the error shown.
pub(crate) const DISMISS: &str = "status.dismiss";

/// The id of a settings page's `back` hint in one pane, by `Esc`: a press on it shows the list of sections.
pub(crate) const SECTIONS: &str = "status.settings_back";

/// The ids of the command line's results' hints, by their own keys; they name no action.
const RESULTS_NEXT: &str = "status.results.next";
const RESULTS_PREVIOUS: &str = "status.results.previous";
const RESULTS_RUN: &str = "status.results.run";
const RESULTS_CLOSE: &str = "status.results.close";

/// The key of a hint the window reads itself, not through the keymap: `Esc`, the results' arrows and `Enter`. The list's
/// `Esc` is [`keymap::BACK`]'s, which has no key of its own.
pub(crate) fn fixed_key(id: &str) -> Option<egui::KeyboardShortcut> {
    let key = match id {
        DISMISS | SECTIONS | RESULTS_CLOSE | keymap::BACK => egui::Key::Escape,
        RESULTS_NEXT => egui::Key::ArrowDown,
        RESULTS_PREVIOUS => egui::Key::ArrowUp,
        RESULTS_RUN => egui::Key::Enter,
        _ => return None,
    };
    Some(egui::KeyboardShortcut::new(egui::Modifiers::NONE, key))
}

/// The id of the line's one key hint under a confirmation dialog: its `Esc`, the safe answer. It names no action: the
/// window's gate under a modal surface drops whatever the line asks while the dialog shows.
pub(crate) const DIALOG_ESC: &str = "dialog.esc";

/// What the window shows, for the line's key hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Keys {
    /// A message open, or a place other than the mail's lists.
    Message,
    /// A list with nothing open.
    List,
    /// A list that holds nothing.
    Empty,
    /// The outbox with sends in it: nothing opens from it.
    Outbox,
    /// The docked composer.
    Composer,
    /// Home.
    Home,
    /// Home before any account.
    FirstRun,
    /// The calendar.
    Calendar,
    /// The settings.
    Settings,
    /// A settings page alone in one pane.
    SettingsPage,
    /// One pane, the message in the list's place.
    OnePane,
    /// The command line's results.
    Results,
    /// An error shown, `Esc` free to dismiss it.
    Error,
    /// An error shown beside the docked composer, `Esc` free to dismiss it.
    ErrorComposer,
}

/// The key hints the line may show for `keys`, in order: the action and its word.
pub(crate) fn hints(keys: Keys) -> &'static [(&'static str, &'static str)] {
    match keys {
        Keys::Message => &HINTS,
        Keys::List => &LIST_HINTS,
        Keys::Empty => &EMPTY_HINTS,
        Keys::Outbox => &OUTBOX_HINTS,
        Keys::Composer => &COMPOSER_HINTS,
        Keys::Home => &HOME_HINTS,
        Keys::FirstRun => &FIRST_RUN_HINTS,
        Keys::Calendar => &CALENDAR_HINTS,
        Keys::Settings => &SETTINGS_HINTS,
        Keys::SettingsPage => &SETTINGS_PAGE_HINTS,
        Keys::OnePane => &ONE_PANE_HINTS,
        Keys::Results => &RESULTS_HINTS,
        Keys::Error => &ERROR_HINTS,
        Keys::ErrorComposer => &ERROR_COMPOSER_HINTS,
    }
}

/// Whether the hint `id` has something to act on: the command line's results' arrows need a row to step to, and their
/// `Enter` a row to run or a search to make (`listed`). Every other hint is checked as its key is.
pub(crate) fn has_work(id: &str, listed: bool) -> bool {
    !matches!(id, RESULTS_NEXT | RESULTS_PREVIOUS | RESULTS_RUN) || listed
}

/// An event this near, in minutes, is the accent of the line.
const SOON_MINUTES: i64 = 15;

/// How the syncing stands, as the line's first fact says it when nothing fails.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Sync {
    /// Synced, the last end of a round on the clock, if there was one.
    Synced(Option<String>),
    /// Syncing: the share done (0 to 1) and the note.
    Syncing(f32, String),
    /// No account signed in: since when, on the clock, as the window first read it so.
    Offline(Option<String>),
}

/// The facts the line says at rest, read from the engine's `safe` commands ([`read`]).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Facts {
    /// How the syncing stands.
    pub sync: Sync,
    /// The accounts.
    pub accounts: usize,
    /// When the window first read the accounts offline, while they still are: what "offline since" says.
    pub offline_since: Option<inboxcraft_time::Timestamp>,
    /// The accounts' failures, `name: what failed`, cleaned, at most [`QUEUE`], the worst first: what needs the owner
    /// (a refused sign-in, a certificate) before what mends itself (no connection), each in the accounts' order.
    pub failures: Vec<String>,
    /// How many problems there are: the failing accounts and the sends the server refused.
    pub problems: usize,
    /// The messages in the outbox.
    pub outbox: usize,
    /// When the first held send goes, on the clock, when one waits for a later time.
    pub outbox_goes: Option<String>,
    /// The next of today's events: its start, its title, the minutes until it starts.
    pub next: Option<(String, String, i64)>,
}

impl Default for Facts {
    fn default() -> Self {
        Self {
            sync: Sync::Synced(None),
            accounts: 0,
            offline_since: None,
            failures: Vec::new(),
            problems: 0,
            outbox: 0,
            outbox_goes: None,
            next: None,
        }
    }
}

/// The time from `now` to the end of its minute: when a fact that counts minutes changes.
pub(crate) fn to_next_minute(now: inboxcraft_time::Timestamp) -> std::time::Duration {
    let into = now.unix_seconds().rem_euclid(60).unsigned_abs();
    std::time::Duration::from_secs(60_u64.saturating_sub(into)).saturating_sub(std::time::Duration::from_nanos(u64::from(now.subsec_nanos())))
}

/// The facts as the mailbox has them now, through `sync.status`, `account.list` and the next event; a refused read
/// leaves its fact out. Offline, since `since` (the last read's [`Facts::offline_since`]) when it was offline then, else
/// since now. Runs commands: call it when the mailbox changed or a second has passed, not every frame.
pub(crate) fn read(mailbox: &mut Mailbox, since: Option<inboxcraft_time::Timestamp>) -> Facts {
    let mut facts = Facts::default();
    let status = mailbox.run("sync.status", &json!({})).unwrap_or(Value::Null);
    let state = status.get("state").and_then(Value::as_str).unwrap_or("synced");
    let number = |key: &str| status.get(key).and_then(Value::as_u64).unwrap_or(0);
    let (done, of) = (number("done"), number("of"));
    let last = status.get("last_synced").and_then(Value::as_str).and_then(crate::mailbox::parse_date).map(|at| mailbox.short_date(at));
    facts.sync = match state {
        "offline" => {
            // A clock set back since the first read never makes it a time still to come.
            let now = mailbox.now();
            let since = since.map_or(now, |since| since.min(now));
            facts.offline_since = Some(since);
            Sync::Offline(Some(mailbox.short_date(since)))
        }
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
        // What mends itself waits behind what needs the owner; each list is capped as it is built.
        let mut mending = Vec::new();
        for account in accounts {
            let Some(failure) = account.get("failure").filter(|failure| !failure.is_null()) else { continue };
            facts.problems = facts.problems.saturating_add(1);
            let (what, mends) = reason(failure);
            let into = if mends { &mut mending } else { &mut facts.failures };
            if into.len() < QUEUE {
                let name = account.get("label").and_then(Value::as_str).unwrap_or_default();
                into.push(clean(&tr!("{name}: {what}", name = name, what = what)));
            }
        }
        facts.failures.extend(mending);
        facts.failures.truncate(QUEUE);
    }
    let refused = mailbox.outbox().iter().filter(|row| matches!(row.state, SendState::Refused { .. })).count();
    facts.problems = facts.problems.saturating_add(refused);
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

/// A failing account's reason in the window's few words, by the `kind` of `failure` as `account.list` gives it
/// ("sign-in refused"); the engine's sentence where the kind is one the window does not know. And whether it mends
/// itself, as the engine has it ([`inboxcraft_model::ErrorKind::needs_the_owner`]), so it waits behind what needs the
/// owner; a kind the window does not know waits too.
fn reason(failure: &Value) -> (String, bool) {
    use crate::calendar::sync::Why;
    let kind = failure.get("kind").and_then(Value::as_str).and_then(error_kind);
    let mends = !kind.is_some_and(inboxcraft_model::ErrorKind::needs_the_owner);
    match kind.map(Why::of) {
        Some(why) => (why.words().to_owned(), mends),
        None => (failure.get("message").and_then(Value::as_str).unwrap_or_default().to_owned(), mends),
    }
}

/// The error kind named in snake case (`login_refused`), as the engine names it.
fn error_kind(name: &str) -> Option<inboxcraft_model::ErrorKind> {
    inboxcraft_model::ErrorKind::ALL.into_iter().find(|kind| inboxcraft_engine::error_kind_name(*kind) == name)
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
        Sync::Offline(Some(at)) => Part::Item(tr!("offline since {at}", at = at), Kind::Offline),
        Sync::Offline(None) => Part::Item(tr!("offline").to_owned(), Kind::Offline),
    };
    if let Some(first) = facts.failures.first() {
        // The worst state wins the first fact, with a count of the rest.
        parts.push(Part::Item(first.clone(), Kind::Error));
        if message {
            return parts;
        }
        let more = facts.problems.saturating_sub(1);
        if more > 0 {
            let words = if more == 1 { tr!("and 1 more problem").to_owned() } else { tr!("and {n} more problems", n = more) };
            parts.push(Part::Item(words, Kind::Plain));
            parts.push(Part::Link(tr!("open log"), true));
            return parts;
        }
        parts.push(Part::Link(tr!("open account"), false));
    }
    if message {
        parts.push(sync);
        return parts;
    }
    let offline = matches!(facts.sync, Sync::Offline(_));
    parts.push(sync);
    if offline {
        parts.push(Part::Item(tr!("showing what is kept on this computer").to_owned(), Kind::Plain));
        // No command yet signs the accounts in before their wait ends: the way to try again is drawn coming soon.
        parts.push(Part::Link(tr!("try again"), true));
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
    /// Whether the message lies over a modal surface's dimmed window, so a press reaches its Undo.
    pub lifted: bool,
    /// Whether Undo's key was pressed and taken before the frame drew anything that reads it (the window takes it then, before the command line's field).
    pub undo_taken: bool,
    /// Whether `Esc` was pressed in this frame and reaches nothing else: it dismisses the error shown.
    pub escape: bool,
}

/// Draws the line in `rect`; what was pressed in it.
pub(crate) fn line(ui: &mut Ui, theme: &Theme, keymap: &Keymap, rect: Rect, shown: &Shown<'_>) -> Pressed {
    let mut pressed = Pressed::default();
    let p = &theme.palette;
    ui.painter().rect_filled(rect, 0.0, p.panel);
    // In a narrow window the line keeps its first facts and its last keys ([`crate::status::kept_under`]).
    let kept = crate::status::kept_under(ui.ctx().content_rect().width());
    let hints = match kept {
        Some((_, keys)) => shown.hints.get(shown.hints.len().saturating_sub(keys)..).unwrap_or(shown.hints),
        None => shown.hints,
    };
    // The keys first, from the right end: what is left is the facts'.
    let mut keys_left = rect.right() - RIGHT;
    if !hints.is_empty() {
        let keys = Rect::from_min_max(pos2(rect.left(), rect.top()), pos2(rect.right() - RIGHT - KEYS_PAD, rect.bottom()));
        let mut child = ui.new_child(UiBuilder::new().id_salt("status-keys").max_rect(keys).layout(Layout::right_to_left(Align::Center)));
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        child.spacing_mut().item_spacing.x = KEYS_GAP;
        for (action, key, word) in hints.iter().rev() {
            if key_hint(&mut child, theme, key, tr_word(word), None).1 {
                if *action == DISMISS {
                    pressed.dismiss = true;
                } else {
                    pressed.hint = Some(action);
                }
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
        let builder = UiBuilder::new().id_salt("status-message").max_rect(at).layout(Layout::left_to_right(Align::Center));
        let pressed_in = if shown.lifted {
            let area = Area::new(Id::unique("status-message-lifted")).order(Order::Middle).fixed_pos(at.min).constrain(false).fade_in(false).show(
                ui.ctx(),
                |ui| {
                    let mut child = ui.new_child(builder);
                    child.set_clip_rect(at.intersect(rect));
                    message_in(&mut child, theme, keymap, message, shown)
                },
            );
            ui.ctx().move_to_top(area.response.layer_id);
            area.inner
        } else {
            message_in(&mut left.new_child(builder), theme, keymap, message, shown)
        };
        pressed.undo = pressed_in == Some(status_message::Press::Undo);
        pressed.dismiss |= pressed_in == Some(status_message::Press::Dismiss) || (shown.escape && matches!(message, Message::Error { .. }));
        left.advance_cursor_after_rect(at);
    } else {
        left.add_space(LEFT);
    }
    let mut parts = parts(shown.facts, shown.message.is_some());
    if let Some((facts, _)) = kept {
        parts.truncate(facts);
    }
    for (at, part) in parts.iter().enumerate() {
        // A link follows its fact without a `·`, as the frames draw it.
        if at > 0 && !matches!(part, Part::Link(..)) {
            left.label(crate::component::galley(&left, "·", theme.fonts.mono_s, p.dim));
        }
        match part {
            Part::Item(text, kind) => {
                status_item::status_item(&mut left, theme, StatusItem::new(text, *kind));
            }
            // At motion off a sync is one print, no bar and no share, its ring still (M9).
            Part::Progress(..) if crate::motion::level(left.ctx()) == crate::motion::Level::Off => {
                status_item::status_item(&mut left, theme, StatusItem::new(tr!("syncing…"), Kind::Waiting));
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
    // At motion off the undo line keeps its seconds without the bar (M5); a send's countdown keeps its bar.
    let still = crate::motion::level(ui.ctx()) == crate::motion::Level::Off;
    let left = |until: f64, bar: bool| {
        let rest = (until - shown.now).max(0.0);
        let whole = if shown.whole > 0.0 { shown.whole } else { rest.max(1.0) };
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "seconds of a hold of a few seconds, at least 0")]
        let seconds = rest.ceil().min(f64::from(u32::MAX)) as u32;
        #[expect(clippy::cast_possible_truncation, reason = "a share of 0 to 1")]
        let share = (rest.ceil() / whole.ceil().max(1.0)).clamp(0.0, 1.0) as f32;
        Some(Left { seconds, share, bar })
    };
    let (kind, undoes) = match message {
        Message::Undo { until, .. } => (status_message::Kind::Undo { key: key.as_deref(), left: left(*until, !still) }, true),
        Message::Countdown { until, .. } => (status_message::Kind::Countdown { key: key.as_deref(), left: left(*until, true) }, true),
        Message::Notice { .. } => (status_message::Kind::Notice, false),
        Message::Done { .. } => (status_message::Kind::Done, false),
        Message::Error { .. } => (status_message::Kind::Error, false),
    };
    let drawn = StatusMessage { kind, text: message.text() };
    let press = ui.add_enabled_ui(!(undoes && shown.rests), |ui| status_message::status_message(ui, theme, keymap, &drawn)).inner;
    let press = press.filter(|_| !(undoes && shown.rests));
    if undoes && !shown.rests && shown.undo_key && !soon::is_soon(ui) && (shown.undo_taken || keymap.pressed(ui.ctx(), keymap::UNDO)) {
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
        "open" => tr!("open"),
        "write" => tr!("write"),
        "send" => tr!("send"),
        "cancel" => tr!("cancel"),
        "keep it" => tr!("keep it"),
        "mail" => tr!("mail"),
        "calendar" => tr!("calendar"),
        "back" => tr!("back"),
        "list" => tr!("list"),
        "run" => tr!("run"),
        "close" => tr!("close"),
        "dismiss" => tr!("dismiss"),
        _ => tr!("commands"),
    }
}

#[cfg(test)]
#[path = "sync_motion_tests.rs"]
mod sync_motion_tests;

#[cfg(test)]
#[path = "status_read_tests.rs"]
mod read_tests;

#[cfg(test)]
#[path = "status_keys_tests.rs"]
mod keys_tests;
