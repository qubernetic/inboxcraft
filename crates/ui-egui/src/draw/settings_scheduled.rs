// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Scheduled page, under Sending: everything that waits for its time, the next first, in one group
//! (`[ A ] WAITING · 7`), a row per entry (its time, its kind as a tag, the account's bar, its subject, whom it names
//! or how long before its event it rings, `cancel`). A cancel is confirmed in a dialog over the dimmed window, by
//! its row ([`crate::component::dialog`]): what is cancelled, when it was due, and what follows (the engine keeps no
//! undo for `schedule.cancel`). Nothing is cancelled but on the owner's press of the dialog's danger button; `Esc`, a
//! focus, a hover or a preview runs nothing.

use egui::{Align, Id, Layout, Rect, Role, Ui, WidgetInfo, pos2, vec2};

use super::{Asked, body, inset, rule, words};
use crate::component::button::{LinkSize, link_sized};
use crate::component::dialog::{self, Answer, Dialog, Tone};
use crate::component::label::section_label;
use crate::component::tag::{TagKind, tag};
use crate::keymap::Keymap;
use crate::mailbox::{AccountRow, Mailbox};
use crate::scheduled::{Hand, Kind, Scheduled, ScheduledRow};
use crate::settings::{Section, Settings};
use crate::theme::Theme;
use crate::tr;

/// An entry's row, in points at 100 %.
const ROW: f32 = 40.0;

/// The time's column, in points at 100 % (it grows with the type, as the kind's does).
const WHEN: f32 = 96.0;

/// The kind's column, in points at 100 %.
const KIND: f32 = 72.0;

/// The second text's column (whom it names, how long before), in points. Where the subject would keep less
/// than this beside it (1000 at 130 %), the second text stands under the subject.
const NOTE: f32 = 149.0;

/// The account's bar, in points.
const ACCOUNT_BAR: f32 = 3.0;

/// Between the row's parts, in points.
const GAP: f32 = 12.0;

/// Gives the keyboard to `response` when it is where the page asked it to go (`hand`), and takes the ask.
fn take(hand: &mut Option<Hand>, wanted: &Hand, response: &egui::Response) {
    if hand.as_ref() == Some(wanted) {
        *hand = None;
        response.request_focus();
    }
}

/// The page's head: the link back to Sending, the title, the two sentences under it. Whether the link was pressed.
fn head(ui: &mut Ui, theme: &Theme, keymap: &Keymap, hand: &mut Option<Hand>) -> bool {
    let p = &theme.palette;
    let mut left = false;
    inset(ui, |ui| {
        let width = ui.available_width();
        ui.allocate_ui_with_layout(vec2(width, super::where_high(theme)), Layout::left_to_right(Align::Center), |ui| {
            let back = link_sized(ui, theme, keymap, tr!("← Sending"), LinkSize::Small);
            take(hand, &Hand::Back, &back);
            left = back.clicked();
        });
        ui.add_space(super::HEAD_GAP);
        let tall = super::title_high(theme);
        ui.allocate_ui_with_layout(vec2(width, tall), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_height(tall);
            let title = tr!("Scheduled");
            let shown = crate::component::label(ui, theme.fonts.title, title, p.ink);
            shown.widget_info(|| WidgetInfo::labeled(Role::Heading, true, title));
        });
        ui.add_space(6.0);
        words(
            ui,
            theme.fonts.body,
            tr!(
                "Everything that waits for its time, the next first: a send, a snooze that comes back, a reminder when no reply came, a calendar alert."
            ),
            p.muted,
        );
        ui.add_space(super::HEAD_GAP);
        words(
            ui,
            theme.fonts.body,
            tr!(
                "Cancelling a send puts the message back in Drafts. Cancelling a snooze brings the message back to the inbox now. Cancelling a reminder or an alert removes only that entry: the message and the event stay as they are."
            ),
            p.muted,
        );
    });
    left
}

/// The Scheduled page.
pub(super) fn page(ui: &mut Ui, theme: &Theme, keymap: &Keymap, settings: &mut Settings, mailbox: &Mailbox) -> Option<Asked> {
    let mut asked = None;
    let scheduled = settings.scheduled_mut();
    let mut hand = scheduled.take_hand();
    if head(ui, theme, keymap, &mut hand) {
        asked = Some(Asked::Section(Section::Sending));
    }
    if let Some(note) = scheduled.note() {
        ui.add_space(12.0);
        inset(ui, |ui| words(ui, theme.fonts.body, note, theme.palette.warn));
    }
    let skipped = scheduled.skipped();
    if skipped > 0 {
        let note = if skipped == 1 {
            tr!("1 waiting entry could not be read and is not listed.").to_owned()
        } else {
            tr!("{n} waiting entries could not be read and are not listed.", n = skipped)
        };
        ui.add_space(12.0);
        inset(ui, |ui| words(ui, theme.fonts.body, &note, theme.palette.warn));
    }
    let near = list(ui, theme, keymap, scheduled, mailbox, &mut hand, &mut asked);
    // The confirmation, over the dimmed window: its answer is the only ask while it shows.
    if let Some(row) = scheduled.confirming().and_then(|id| scheduled.rows().iter().find(|row| row.id == id)) {
        let when = crate::scheduled::when(row.due, mailbox.now(), mailbox.offset());
        if let Some(answer) = confirmation(ui, theme, keymap, row, &when, near) {
            asked = Some(answer);
        }
    }
    // A hand to a row not drawn this frame (it went, or is out of sight) goes to the link back instead of nowhere.
    if hand.is_some() {
        scheduled.hand_to(Hand::Back);
    }
    asked
}

/// The entries, in one group; where the entry asked about stands, while it is in sight.
fn list(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    scheduled: &Scheduled,
    mailbox: &Mailbox,
    hand: &mut Option<Hand>,
    asked: &mut Option<Asked>,
) -> Option<Rect> {
    let accounts = mailbox.accounts();
    let mut near = None;
    let (now, offset) = (mailbox.now(), mailbox.offset());
    body(ui, theme, "scheduled", |ui| {
        if scheduled.rows().is_empty() {
            ui.add_space(super::GROUP_TOP);
            crate::component::empty::empty_state(
                ui,
                theme,
                tr!("Scheduled · nothing"),
                tr!("Nothing waits for its time."),
                tr!("Send later, snooze, a reminder and an event's alert each put a line here until their time comes."),
            );
            return;
        }
        let total = scheduled.rows().len();
        ui.add_space(super::GROUP_TOP);
        inset(ui, |ui| section_label(ui, theme, "[ A ]", &tr!("Waiting · {n}", n = total), false));
        ui.add_space(super::GROUP_UNDER);
        rule(ui, theme);
        let fit = Fit::of(ui, theme, ui.available_width());
        for (at, row) in scheduled.rows().iter().enumerate() {
            let ends = at == 0 || at.saturating_add(1) == total;
            let account = row.account.and_then(|id| accounts.iter().find(|account| account.id == id));
            // The time is formatted only for a row that is drawn: a row out of sight only takes its room.
            let when = || crate::scheduled::when(row.due, now, offset);
            let (top, left, right) = (ui.cursor().top(), ui.max_rect().left(), ui.max_rect().right());
            let pressed = entry_row(ui, theme, keymap, row, when, account, (ends, fit), hand);
            if pressed.is_some() {
                *asked = pressed;
            }
            let drawn = Rect::from_min_max(pos2(left, top), pos2(right, ui.cursor().top()));
            if scheduled.confirming() == Some(row.id) && ui.clip_rect().intersects(drawn) {
                near = Some(drawn);
            }
        }
    });
    near
}

/// How every entry's row of the page is laid out, measured once a frame, not per row.
#[derive(Debug, Clone, Copy)]
struct Fit {
    /// The room cancel takes: the link's words and its 16, in points.
    link: f32,
    /// Whether the second text stands under the subject: beside it the subject would keep less than the second text.
    two_lines: bool,
    /// The row's height, in points.
    high: f32,
}

impl Fit {
    /// The fit of a row `width` wide.
    fn of(ui: &Ui, theme: &Theme, width: f32) -> Self {
        let f = &theme.fonts;
        let link = crate::component::galley(ui, tr!("cancel"), f.mono_s, theme.palette.ink).size().x.ceil() + 16.0;
        let fixed = 16.0 + Theme::size(theme, WHEN) + GAP + Theme::size(theme, KIND) + GAP + ACCOUNT_BAR + GAP;
        let two_lines = width - fixed - link - NOTE - 2.0 * GAP < NOTE;
        let high = if two_lines { ROW.max(f.body.line + f.caption.line) } else { ROW.max(f.body.line + 18.0) }.round();
        Self { link, two_lines, high }
    }
}

/// An entry's row, laid out as `fit` says; drawn only near the view unless it `ends` the list.
#[allow(clippy::too_many_arguments, reason = "the row's parts and the page's hand, each its own")]
fn entry_row(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    row: &ScheduledRow,
    when: impl FnOnce() -> String,
    account: Option<&AccountRow>,
    (ends, fit): (bool, Fit),
    hand: &mut Option<Hand>,
) -> Option<Asked> {
    let width = ui.available_width();
    let (when_w, kind_w, note_w) = (Theme::size(theme, WHEN), Theme::size(theme, KIND), NOTE);
    let f = &theme.fonts;
    let Fit { link, two_lines, high } = fit;
    let top = ui.cursor().top();
    let rect = Rect::from_min_size(pos2(ui.max_rect().left(), top), vec2(width, high));
    let clip = ui.clip_rect();
    if !ends && !clip.expand2(vec2(0.0, clip.height())).intersects(rect) {
        ui.allocate_space(vec2(width, high + 1.0));
        return None;
    }
    let p = &theme.palette;
    let mut asked = None;
    let when = when();
    let when = when.as_str();
    // The row's own scope, so rows above it laid out or passed over as the page scrolls never change its link's id.
    let own =
        egui::UiBuilder::new().scope_id(Id::unique("settings-scheduled").with(row.id)).max_rect(rect).layout(Layout::left_to_right(Align::Center));
    ui.scope_builder(own, |ui| {
        ui.set_min_height(high);
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(16.0);
        let shown = crate::component::cut(ui, when, theme.fonts.mono, p.ink, when_w);
        let (place, _) = ui.allocate_exact_size(vec2(when_w, high), egui::Sense::hover());
        ui.painter().galley(pos2(place.left(), place.center().y - shown.size().y / 2.0), shown, p.ink);
        ui.add_space(GAP);
        ui.allocate_ui_with_layout(vec2(kind_w, high), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_width(kind_w);
            tag(ui, theme, row.kind.word(), TagKind::Default);
        });
        ui.add_space(GAP);
        let _ = crate::component::account_bar(ui, theme, account.map_or(0, |row| u64::try_from(row.index).unwrap_or(0)));
        ui.add_space(GAP);
        let word = tr!("cancel");
        let title = row.title();
        if two_lines {
            // The subject over the second text, the two lines filling the row, both as wide as the row leaves.
            let room = (ui.available_width() - link - GAP).max(1.0);
            let (place, _) = ui.allocate_exact_size(vec2(room, high), egui::Sense::hover());
            let subject = crate::component::cut(ui, &title, f.body, p.ink, room);
            let top = place.top() + ((high - f.body.line - f.caption.line) / 2.0).round();
            ui.painter().galley(pos2(place.left(), top + (f.body.line - subject.size().y) / 2.0), subject, p.ink);
            let note = crate::component::cut(ui, &row.note(), f.caption, p.muted, room);
            ui.painter().galley(pos2(place.left(), top + f.body.line + (f.caption.line - note.size().y) / 2.0), note, p.muted);
        } else {
            let room = (ui.available_width() - link - note_w - 2.0 * GAP).max(1.0);
            let subject = crate::component::cut(ui, &title, f.body, p.ink, room);
            let (place, _) = ui.allocate_exact_size(vec2(room, high), egui::Sense::hover());
            ui.painter().galley(pos2(place.left(), place.center().y - subject.size().y / 2.0), subject, p.ink);
            ui.add_space(GAP);
            let note = crate::component::cut(ui, &row.note(), f.caption, p.muted, note_w);
            let (place, _) = ui.allocate_exact_size(vec2(note_w, high), egui::Sense::hover());
            ui.painter().galley(pos2(place.left(), place.center().y - note.size().y / 2.0), note, p.muted);
        }
        ui.add_space(GAP);
        let cancel = link_sized(ui, theme, keymap, word, LinkSize::Small);
        take(hand, &Hand::Cancel(row.id), &cancel);
        let spoken = tr!("Cancel the {kind} {when}: {subject}", kind = row.kind.word(), when = when, subject = title);
        ui.ctx().accesskit_node_builder(cancel.id, |node| node.set_label(spoken.as_str()));
        if crate::component::button::owner_pressed(ui, &cancel) {
            asked = Some(Asked::AskCancelScheduled(row.id));
        }
    });
    rule(ui, theme);
    asked
}

/// The confirmation of a cancel, in its dialog: what is cancelled, when it was due, what follows; "Keep it" and the
/// danger button. What the owner answered, if anything.
fn confirmation(ui: &Ui, theme: &Theme, keymap: &Keymap, row: &ScheduledRow, when: &str, near: Option<Rect>) -> Option<Asked> {
    let title = row.title();
    let (question, follows, press) = match row.kind {
        Kind::Send if !row.who.is_empty() => (
            tr!("Cancel the send to {who}?", who = row.who),
            tr!("The message goes back to Drafts, where you can change it or send it."),
            tr!("Cancel the send"),
        ),
        Kind::Send => (
            tr!("Cancel the send of “{subject}”?", subject = title),
            tr!("The message goes back to Drafts, where you can change it or send it."),
            tr!("Cancel the send"),
        ),
        Kind::Snooze => (
            tr!("Cancel the snooze of “{subject}”?", subject = title),
            tr!("The message comes back to its inbox now, as if its time had come."),
            tr!("Cancel the snooze"),
        ),
        Kind::Reminder => (
            tr!("Cancel the reminder on “{subject}”?", subject = title),
            tr!("The message itself stays as it is; no reminder comes if no reply arrives."),
            tr!("Cancel the reminder"),
        ),
        Kind::Alert => (
            tr!("Cancel the alert for “{subject}”?", subject = title),
            tr!("The event stays in the calendar; this alert does not ring."),
            tr!("Cancel the alert"),
        ),
    };
    // A send names whom it goes to in the question: the line names the message.
    let due = if row.kind == Kind::Send && !row.who.is_empty() {
        tr!("“{subject}” will not go at its time, {when}.", subject = title, when = when)
    } else {
        tr!("Due {when}.", when = when)
    };
    let lines = [(due.as_str(), Tone::Ink), (follows, Tone::Muted)];
    let dialog = Dialog { id: Id::unique("settings-scheduled-confirm"), question: &question, lines: &lines, keep: tr!("Keep it"), act: press, near };
    match dialog::show(ui, theme, keymap, &dialog)? {
        Answer::Keep => Some(Asked::KeepScheduled),
        Answer::Act => Some(Asked::CancelScheduled(row.id)),
    }
}

/// What the Scheduled page asked for, done: a cancel through `schedule.cancel`, on the owner's press only.
impl super::super::InboxApp {
    /// Shows the Scheduled page with the entries `schedule.list` answers now.
    pub fn open_scheduled(&mut self) {
        self.settings.open_page(Some(crate::settings::Under::Scheduled.id()));
        self.load_scheduled();
    }

    /// Reads the entries again; a refusal leaves none and says why on the page.
    fn load_scheduled(&mut self) {
        match self.mailbox.run("schedule.list", &serde_json::json!({})) {
            Ok(answer) => self.settings.scheduled_mut().read(&answer),
            Err(error) => {
                self.settings.scheduled_mut().read(&serde_json::Value::Null);
                self.settings.scheduled_mut().set_note(Some(error.to_string()));
            }
        }
    }

    /// Does what the Scheduled page asked for.
    pub(in crate::draw) fn apply_scheduled(&mut self, asked: Asked) {
        match asked {
            Asked::Scheduled => self.open_scheduled(),
            // The dialog takes the keyboard on its "Keep it" as it opens.
            Asked::AskCancelScheduled(id) => self.settings.scheduled_mut().ask_cancel(id),
            Asked::KeepScheduled => {
                if let Some(id) = self.settings.scheduled().confirming() {
                    self.settings.scheduled_mut().hand_to(Hand::Cancel(id));
                }
                self.settings.scheduled_mut().keep();
            }
            Asked::CancelScheduled(id) => {
                let at = self.settings.scheduled().rows().iter().position(|row| row.id == id);
                // A snooze's cancel brings its message back now (`mail.unsnooze`, which also takes the entry off the
                // schedule), never leaving it in Snoozed with nothing to bring it back.
                let snoozed =
                    at.and_then(|at| self.settings.scheduled().rows().get(at)).filter(|row| row.kind == Kind::Snooze).and_then(|row| row.message);
                let cancelled = match snoozed {
                    Some(message) => self.mailbox.run("mail.unsnooze", &serde_json::json!({ "id": message })),
                    None => self.mailbox.run("schedule.cancel", &serde_json::json!({ "id": id })),
                };
                self.settings.scheduled_mut().keep();
                self.load_scheduled();
                if let Err(error) = cancelled {
                    self.settings.scheduled_mut().set_note(Some(error.to_string()));
                }
                // The keyboard goes to the row that took the cancelled one's place, or the one before, or back.
                let rows = self.settings.scheduled().rows();
                let next = at.and_then(|at| rows.get(at).or_else(|| at.checked_sub(1).and_then(|before| rows.get(before)))).map(|row| row.id);
                self.settings.scheduled_mut().hand_to(next.map_or(Hand::Back, Hand::Cancel));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "settings_scheduled_tests.rs"]
mod tests;
