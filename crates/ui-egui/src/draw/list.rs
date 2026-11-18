// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The list side: the header with the view's name, whose mail it is and the search field; the rows on a card; the
//! empty states.
//!
//! A row is the design's mail row ([`crate::component::mail_row`]): the sender and the time, the subject and the
//! preview on line two; an unread row its square, a heavier sender and its line two in `ink`; the selected row the
//! `stage` fill and its bar; a row from a new sender, one back from a snooze or one that was not delivered its tag
//! after the sender. On top of the unified inbox, while senders wait, the screener card with Review, which opens "New senders".
//! The unified inbox shows its groups as the owner chose ([`InboxView`]): a card for each (badge, name, unread count,
//! "Done all", the first rows and "Show all (n)"), one list with a heading for each, or the simple list without them;
//! the rows no group holds come last. A row of a grouped account has a context menu that puts its sender's mail in
//! another group. `ui_shot` can show invented senders and groups instead ([`crate::mailbox::Showcase`]).
//!
//! A hovered row and a row selected by keyboard show the hover actions alike: Done, mark read or unread, Delete, on a
//! real account as on the sample. Done and Delete act at once, and the toast with Undo shows; the row's gap closes over
//! the `row_collapse` token (at once with motion off).
//!
//! Under the header, pills: over a wide unified inbox with groups the view switch (the one of the Triage settings
//! page), and in a folder or a view "Show done", which lists the done mail in its old place, dimmed, and
//! "Parked (n)" while mail of the place waits on the park pile; in "Later" a pill for each pile (snoozed, pins,
//! parked), the snoozed rows dated by when they come back. A pinned row shows the pin; a row back from a snooze says
//! "Returned" while it is unread. "Done all" on a group's card marks the whole group done, with one Undo.
//!
//! The outbox lists each send on a card: whom it goes to and its subject, where it stands ("Sending in 4 s",
//! "Scheduled · Sat 08:00", "Sending…", or "Not sent" with the server's reply under the subject), and its buttons: Undo
//! for a send in its hold and Edit for one scheduled, which take it back and open its draft, Edit and Send again for a
//! refused one. A sent message a delivery report says did not get through is marked "Not delivered" in its row.

use std::cell::Cell;
use std::collections::HashSet;

use egui::accesskit::Role as AccessRole;
use egui::{Align, Frame, Id, Layout, Margin, Rect, RichText, ScrollArea, Sense, Ui, UiBuilder, pos2, vec2};
use inboxcraft_model::Role;

use inboxcraft_engine::ScreenerMode;

use crate::component::draft_row::{DraftRow, draft_row};
use crate::component::group_head::{GroupHead, group_head};
use crate::component::hover::Action;
use crate::component::later_row::{LaterRow, Pile as LaterPile, later_row};
use crate::component::mail_row::{MailRow, Mark, mail_row};
use crate::component::outbox_row::{OutboxRow as OutboxRowView, SendButton, Standing, outbox_row as outbox_view};
use crate::component::pill::pill;
use crate::component::segmented::segmented_for;
use crate::component::tag::TagKind;
use crate::component::time_picker::{self, Ask, Picked, Preset, TimePicker};
use crate::component::{avatar, button, field, popup, tooltip};
use crate::iconset::IconId;
use crate::keymap::{self, Keymap};
use crate::mailbox::{Act, Group, GroupKind, InboxView, Leaving, Mailbox, MessageRow, OutboxRow, Pile, Place, SendState, Waiting};
use crate::motion;
use crate::theme::Theme;
use crate::tr;

/// The rows a group shows before "Show all".
pub(super) const GROUP_ROWS: usize = 3;

/// The search field's id, so a shortcut can focus it.
pub(super) fn search_id() -> egui::Id {
    egui::Id::unique("mail-search")
}

/// What the header asked for this frame.
#[derive(Debug, Default)]
pub(super) struct Header {
    /// The search text, when it changed.
    pub(super) searched: Option<String>,
    /// A place a pill opens: a pile of "Later", or the park pile.
    pub(super) open: Option<Place>,
    /// "Show done" turned on or off.
    pub(super) show_done: Option<bool>,
    /// A view of the unified inbox chosen on the view switch.
    pub(super) view: Option<InboxView>,
}

/// How far the pills reach out of the header's margin, to the left edge of the cards under them, and how much nearer
/// the next card they are than the header's margin keeps them.
const PILL_PULL: i8 = 4;

/// The view switch of the unified inbox, as the design file words and orders it: Grouped, Smart, List ([`InboxView::ALL`]'s
/// order; the arrows walk it so). "Grouped" is the one list under headings ([`InboxView::Grouped`]).
fn view_switch() -> [(InboxView, &'static str); 3] {
    [(InboxView::Grouped, tr!("Grouped")), (InboxView::Smart, InboxView::Smart.text()), (InboxView::List, InboxView::List.text())]
}

/// The pills under the header: the piles in "Later"; elsewhere "Show done" and the park pile, after the view switch
/// over a wide unified inbox with groups to show. Not while searching.
fn pills(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, asked: &mut Header) {
    if !mailbox.query().is_empty() {
        search_head(ui, theme, mailbox);
        return;
    }
    if matches!(mailbox.place(), Place::Senders | Place::Outbox) {
        return;
    }
    ui.add_space(theme.spacing.s3);
    let reach = Margin { left: -PILL_PULL, right: -PILL_PULL, top: 0, bottom: -PILL_PULL };
    Frame::new().outer_margin(reach).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(theme.spacing.s2, theme.spacing.s1);
            if let Place::Later(shown) = mailbox.place() {
                for pile in Pile::ALL {
                    // An empty pile's pill loses its number, never "(0)".
                    let count = mailbox.pile_count(pile);
                    let text = if count == 0 { pile.text().to_owned() } else { tr!("{pile} ({count})", pile = pile.text(), count = count) };
                    if pill(ui, theme, &text, pile == shown).clicked() {
                        asked.open = Some(Place::Later(pile));
                    }
                }
                return;
            }
            // The view switch of the design's list head, here on the line of the pills until the head is the
            // design's (stage I-2.2), the search field keeping its place.
            if mailbox.place() == Place::View(Role::Inbox) && mailbox.can_group() {
                let options = view_switch().map(|(view, text)| (view, text, theme.fonts.button));
                if let Some(view) = segmented_for(ui, theme, &options, mailbox.inbox_view(), Some(tr!("the inbox"))) {
                    asked.view = Some(view);
                }
            }
            if mailbox.place() == Place::View(Role::Drafts) {
                // The list of drafts is planned: its rows are drawn, coming soon.
                let _ = crate::component::soon::soon_tag(ui, theme);
                return;
            }
            if pill(ui, theme, tr!("Show done"), mailbox.show_done()).clicked() {
                asked.show_done = Some(!mailbox.show_done());
            }
            let parked = mailbox.parked();
            if parked > 0 && pill(ui, theme, &tr!("Parked ({count})", count = parked), false).clicked() {
                asked.open = Some(Place::Later(Pile::Parked));
            }
            // Several messages at once (the rows' check boxes and the bulk bar) are planned: the way in is drawn,
            // coming soon, its mark once. A part that does not fit the line is left out whole, as the design's widths
            // say: beside the reader the line keeps to one line and the planned Select gives way.
            let gap = theme.spacing.s2;
            let need =
                gap + crate::component::pill::width(ui, theme, tr!("Select")) + gap + crate::component::tag::width(ui, theme, tr!("coming soon"));
            if ui.available_size_before_wrap().x >= need {
                crate::component::soon::soon(ui, theme, |ui| {
                    let _ = pill(ui, theme, tr!("Select"), false);
                });
                let _ = crate::component::soon::soon_tag(ui, theme);
            }
        });
    });
}

/// The search's head under the list's head: what was searched and where, and the planned search in the text of
/// messages, coming soon.
fn search_head(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox) {
    ui.add_space(theme.spacing.s3);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.s3;
        let said = tr!("“{query}” in subjects, senders and recipients", query = mailbox.query());
        ui.add(egui::Label::new(theme.fonts.mono_s.rich(&said).color(theme.palette.muted)).truncate());
        crate::component::soon::soon(ui, theme, |ui| {
            let _ = button::link_sized(ui, theme, &Keymap::empty(), tr!("in message text"), button::LinkSize::Small);
        });
        let _ = crate::component::soon::soon_tag(ui, theme);
    });
}

/// The header above the list: the title, whose mail and how many, the search field, and the pills under them.
pub(super) fn header(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, search: &mut String) -> Header {
    let p = &theme.palette;
    let mut changed = None;
    ui.add_space(theme.spacing.s1);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(mailbox.title()).font(theme.fonts.title.id()).color(p.ink));
            let line = match mailbox.outbox_summary().filter(|_| mailbox.place() == Place::Outbox && mailbox.query().is_empty()) {
                Some(summary) => summary,
                None => tr!("{whose} · {shown}", whose = mailbox.subtitle(), shown = mailbox.list_count()),
            };
            ui.label(theme.fonts.mono_s.rich(&line).color(p.muted));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if !search.is_empty() && button::text(ui, theme, keymap, tr!("Clear"), None, false).clicked() {
                search.clear();
                changed = Some(String::new());
            }
            let field = field::search(ui, theme, search_id(), search, tr!("Search"), super::FOLDER_PANE * 0.75);
            let response = tooltip::attach(field, theme, keymap, tr!("Search mail"), Some(keymap::FIND));
            if response.changed() {
                changed = Some(search.clone());
            }
            // Esc empties the field, unless a float over it takes the key.
            if (response.has_focus() || response.lost_focus()) && popup::escape(ui.ctx()) {
                search.clear();
                changed = Some(String::new());
            }
        });
    });
    let mut asked = Header { searched: changed, ..Header::default() };
    pills(ui, theme, mailbox, &mut asked);
    asked
}

/// What `key`'s action is pressed with, and `with` after it ("H snoozes the open message."), or nothing when the
/// action has no key.
fn with_key(keymap: &Keymap, action: &str, with: impl FnOnce(&str) -> String) -> String {
    keymap.shortcut(action).map(|key| with(&crate::component::key::key_words(&key))).unwrap_or_default()
}

/// The empty state of a list with no row, as the design words it for its place: its label (`INBOX · EMPTY`), its
/// title and its help, centred in the pane.
fn empty_list(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox) {
    let place = mailbox.place_title();
    let upper = place.to_uppercase();
    let join = |first: &str, then: String| if then.is_empty() { first.to_owned() } else { format!("{first} {then}") };
    let (label, title, help) = if !mailbox.query().is_empty() {
        let help = tr!(
            "No subject, sender or recipient in {place} holds “{query}”. The text of messages is not searched yet.",
            place = place,
            query = mailbox.query()
        );
        (tr!("{place} · SEARCH", place = upper), tr!("No messages match.").to_owned(), help)
    } else {
        let palette = with_key(keymap, keymap::PALETTE, |key| tr!("or with {key}", key = key));
        let (title, help) = match mailbox.place() {
            Place::Later(Pile::Snoozed) => (
                tr!("Nothing is snoozed."),
                join(
                    tr!("Snoozed mail waits here until its time comes."),
                    with_key(keymap, keymap::SNOOZE, |key| tr!("{key} snoozes the open message.", key = key)),
                ),
            ),
            Place::Later(Pile::Pins) => (
                tr!("Nothing is pinned."),
                format!(
                    "{}{}.",
                    tr!("Pinned mail stays here, wherever it is filed. Pin the open message from the reader"),
                    if palette.is_empty() { String::new() } else { format!(", {palette}") }
                ),
            ),
            Place::Later(Pile::Parked) => (
                tr!("Nothing is parked."),
                format!(
                    "{}{}.",
                    tr!("Parked mail waits here, out of the inbox, without a time. Park the open message from the reader"),
                    if palette.is_empty() { String::new() } else { format!(", {palette}") }
                ),
            ),
            Place::Senders => {
                (tr!("Nobody is waiting."), tr!("The first mail of an address you have not let in waits here until you answer.").to_owned())
            }
            Place::Outbox => (
                tr!("Nothing waits to be sent."),
                tr!("A message stays here while it is held, scheduled or on its way. Sent mail is in Sent.").to_owned(),
            ),
            Place::View(Role::Inbox) => (
                tr!("Nothing left to do.\nNew mail lands here."),
                tr!("Done mail is out of the way, not gone: “Show done” lists it again in its old place.").to_owned(),
            ),
            Place::View(Role::Sent) => (
                tr!("Nothing sent yet."),
                join(
                    tr!("What you send is kept here, with what came of it."),
                    with_key(keymap, keymap::COMPOSE, |key| tr!("{key} writes a message.", key = key)),
                ),
            ),
            Place::View(Role::Drafts) => (
                tr!("No drafts."),
                join(
                    tr!("What you start and do not send is kept here."),
                    with_key(keymap, keymap::COMPOSE, |key| tr!("{key} writes a message.", key = key)),
                ),
            ),
            _ => (
                tr!("This folder holds no messages."),
                tr!("Mail moved or filed here shows in this list. “Show done” lists what was finished here.").to_owned(),
            ),
        };
        let label = match mailbox.place() {
            Place::Later(pile) => tr!("{place} · {pile}", place = upper, pile = pile.text().to_uppercase()),
            _ => tr!("{place} · EMPTY", place = upper),
        };
        (label, title.to_owned(), help)
    };
    crate::component::empty::empty_state(ui, theme, &label, &title, &help);
}

/// The height of a message row: the design's fixed mail row at the current text scale.
pub(super) fn row_height(theme: &Theme) -> f32 {
    theme.boxes().mail_row
}

/// Where each of a row's hover actions ([`actions`]) stands: an action's id is `actions_id(id).with(place)`, and the
/// place is what [`act`] reads (Snooze's, what its time picker hangs from).
const DONE_ACTION: usize = 0;
pub(crate) const SNOOZE_ACTION: usize = 1;
const READ_ACTION: usize = 2;
const DELETE_ACTION: usize = 3;

/// The hover actions of `message`, in their places, the design file's order: Done (or "Not done" for a done one),
/// Snooze, mark read or unread, Delete.
fn actions(message: &MessageRow) -> [Action<'static>; 4] {
    #[cfg(test)]
    super::list_tests::ACTIONS_BUILT.with(|built| built.set(built.get().saturating_add(1)));
    // An open envelope opens an unread message; a closed one closes a read one again.
    let (read_icon, read) = if message.seen { (IconId::Mail, tr!("Mark as unread")) } else { (IconId::MarkRead, tr!("Mark as read")) };
    let done = if message.done { tr!("Not done") } else { tr!("Done") };
    [
        Action { icon: IconId::Done, name: done, action: Some(keymap::DONE), unavailable: None },
        // It opens the time picker at the row ([`snooze_picker`]); the time chosen snoozes the message.
        Action { icon: IconId::Snooze, name: tr!("Snooze"), action: Some(keymap::SNOOZE), unavailable: None },
        Action { icon: read_icon, name: read, action: Some(keymap::READ), unavailable: None },
        Action { icon: IconId::Delete, name: tr!("Delete"), action: Some(keymap::DELETE), unavailable: None },
    ]
}

/// The id of the time picker float opened from the Snooze of the message `id`'s row.
pub(super) fn snooze_popup(id: u64) -> Id {
    Id::unique("list-snooze-picker").with(id)
}

/// The id the row's time picker keeps its state under between frames (one picker is open at a time).
fn snooze_state() -> Id {
    Id::unique("list-snooze-picker-state")
}

/// Where the list keeps the open time picker ([`SnoozeOpen`]) and where the row of its message was drawn
/// ([`SnoozeRow`]).
fn snooze_open_id() -> Id {
    Id::unique("list-snooze-open")
}

fn snooze_row_id() -> Id {
    Id::unique("list-snooze-row")
}

/// Where the list keeps the row the keyboard is on, as a pass drew it in sight: `(its response, the pass)`.
fn keyed_row_id() -> Id {
    Id::unique("list-keyed-row")
}

/// The time picker open from a row's Snooze: it belongs to a message, not to the row drawn for it.
#[derive(Debug, Clone, Copy)]
struct SnoozeOpen {
    /// The message it snoozes.
    message: u64,
    /// Where it hangs from: the rect of the row's Snooze when it opened, relative to the row's top left, so it follows
    /// the row while the group hides (the pointer gone into the float) or the row moves (mail coming in above).
    anchor: Rect,
    /// The last pass that showed it: a pass between without it (the list not drawn) closes it.
    shown: u64,
}

/// The row of the open picker's message as a pass drew it, in sight.
#[derive(Debug, Clone, Copy)]
struct SnoozeRow {
    message: u64,
    rect: Rect,
    /// The row's response, which the keyboard goes back to.
    row: Id,
    pass: u64,
}

/// Opens the time picker of the message `message`'s row from its Snooze (the hover action's `id`) on the row `rect`.
fn open_snooze(ctx: &egui::Context, message: u64, action: Id, rect: Rect) {
    let anchor = ctx.read_response(action).map_or(rect, |response| response.rect).translate(-rect.min.to_vec2());
    let shown = ctx.cumulative_pass_nr();
    ctx.data_mut(|data| {
        data.insert_temp(snooze_open_id(), SnoozeOpen { message, anchor, shown });
        data.get_temp_mut_or_default::<TimePicker>(snooze_state()).open();
    });
}

/// The row of `message` was drawn in sight at `rect` (`row` its response): the open picker of that message, if any,
/// stands under it this frame.
fn note_snooze_row(ctx: &egui::Context, message: u64, rect: Rect, row: Id) {
    let open = ctx.data(|data| data.get_temp::<SnoozeOpen>(snooze_open_id())).is_some_and(|open| open.message == message);
    if open {
        let pass = ctx.cumulative_pass_nr();
        ctx.data_mut(|data| data.insert_temp(snooze_row_id(), SnoozeRow { message, rect, row, pass }));
    }
}

/// Closes the row's time picker, whichever message it is open on.
fn close_snooze(ctx: &egui::Context) {
    ctx.data_mut(|data| {
        data.remove::<SnoozeOpen>(snooze_open_id());
        data.remove::<SnoozeRow>(snooze_row_id());
    });
}

/// The time picker open from a row's Snooze, after the list drew its rows: under its message's row if this pass drew
/// that row in sight, and the pass before showed it; else it closes for good (scrolled away, another list or screen,
/// the message gone), whichever list drew or did not draw the row, and the keyboard it held goes nowhere. The time chosen snoozes the message. A choice or Cancel
/// closes it and gives the row the keyboard; so do `Esc` and a press outside it, through the float.
fn snooze_picker(ctx: &egui::Context, theme: &Theme, mailbox: &Mailbox) -> Option<(u64, Act)> {
    let open = ctx.data(|data| data.get_temp::<SnoozeOpen>(snooze_open_id()))?;
    let pass = ctx.cumulative_pass_nr();
    let drawn = ctx.data(|data| data.get_temp::<SnoozeRow>(snooze_row_id())).filter(|row| row.message == open.message && row.pass == pass);
    let Some(drawn) = drawn.filter(|_| open.shown.saturating_add(1) >= pass) else {
        close_snooze(ctx);
        // The keyboard it held goes back to the list, as Esc gives it back: to the keyed row in sight, else nowhere.
        let popup = snooze_popup(open.message);
        let held = ctx
            .memory(|memory| memory.focused())
            .and_then(|focused| ctx.read_response(focused))
            .is_some_and(|response| response.layer_id.id == popup);
        if held {
            let keyed = ctx.data(|data| data.get_temp::<(Id, u64)>(keyed_row_id())).filter(|(_, at)| *at == pass).map(|(row, _)| row);
            ctx.memory_mut(|memory| match keyed {
                Some(row) => memory.request_focus(row),
                None => {
                    if let Some(focused) = memory.focused() {
                        memory.surrender_focus(focused);
                    }
                }
            });
        }
        return None;
    };
    ctx.data_mut(|data| data.insert_temp(snooze_open_id(), SnoozeOpen { shown: pass, ..open }));
    let float = popup::Float {
        id: snooze_popup(open.message),
        anchor: open.anchor.translate(drawn.rect.min.to_vec2()),
        width: time_picker::WIDTH,
        returns_to: drawn.row,
    };
    let shown = popup::float(ctx, theme, &float, |ui| {
        let presets: Vec<Preset<'_>> = mailbox.presets().iter().map(|(name, at)| Preset { name, at: *at }).collect();
        let ask = Ask { title: tr!("Snooze until"), confirm: tr!("Snooze"), alert: None, presets: &presets, extra: None, note: None };
        let mut state = ui.data_mut(|data| data.get_temp::<TimePicker>(snooze_state())).unwrap_or_default();
        let picked = time_picker::show(ui, theme, &mut state, &ask, mailbox.now(), mailbox.offset());
        ui.data_mut(|data| data.insert_temp(snooze_state(), state));
        picked
    });
    match shown {
        Some(Picked::At { at, .. }) => {
            close_snooze(ctx);
            ctx.memory_mut(|memory| memory.request_focus(drawn.row));
            Some((open.message, Act::Snooze(at)))
        }
        Some(Picked::Cancelled | Picked::Extra) => {
            close_snooze(ctx);
            ctx.memory_mut(|memory| memory.request_focus(drawn.row));
            None
        }
        Some(Picked::Open) => None,
        // Esc or a press outside: the float already gave the row the keyboard.
        None => {
            close_snooze(ctx);
            None
        }
    }
}

/// What the hover action at `index` of [`actions`] does to `message` at once (Snooze asks for a time first).
fn act(message: &MessageRow, index: usize) -> Option<Act> {
    match index {
        DONE_ACTION if message.done => Some(Act::Undone),
        DONE_ACTION => Some(Act::Done),
        READ_ACTION if message.seen => Some(Act::Unread),
        READ_ACTION => Some(Act::Read),
        DELETE_ACTION => Some(Act::Delete),
        _ => None,
    }
}

/// The id of the group menu of the message `id`'s row.
pub(super) fn group_menu_id(id: u64) -> Id {
    Id::unique("list-group-menu").with(id)
}

/// The width of the group menu, its edges included, as the design file has it.
const MENU_WIDTH: f32 = 256.0;

/// The group menu of a sender's mail: a choice for each group, the one it is in chosen, and "Let InboxCraft sort it";
/// the act chosen. A line under them says that all the sender's mail moves, the kept and the future.
pub(super) fn group_menu(ui: &mut Ui, theme: &Theme, current: Option<GroupKind>, address: &str, focus_first: bool) -> Option<Act> {
    let mut chosen = None;
    // The choices stand straight under the head, no line between.
    popup::head(ui, theme, tr!("Group"), None);
    for (index, kind) in GroupKind::SHOWN.into_iter().enumerate() {
        let choice = popup::choice(ui, theme, kind.text(), current == Some(kind));
        if focus_first && index == 0 {
            choice.request_focus();
        }
        if choice.clicked() {
            chosen = Some(Act::Group(Some(kind)));
        }
    }
    if popup::choice(ui, theme, tr!("Let InboxCraft sort it"), false).clicked() {
        chosen = Some(Act::Group(None));
    }
    let note = tr!("Moves all mail from {address}, kept and future.", address = address);
    // 12 in, as a float's words stand, on caption lines of 18: 8 above, 10 below.
    let (top, side, bottom) = (8.0, 12.0, 10.0);
    let width = ui.available_width();
    let mut job = egui::text::LayoutJob::single_section(note.clone(), theme.fonts.caption.format(theme.palette.muted));
    job.wrap.max_width = (width - side * 2.0).max(1.0);
    let text = ui.painter().layout_job(job);
    let (rect, response) = ui.allocate_exact_size(vec2(width, top + text.size().y + bottom), egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(AccessRole::Label, true, &note));
    if ui.is_rect_visible(rect) {
        super::paint::rows_at(ui.painter(), rect.min + vec2(side, top), width - side * 2.0, text, theme.palette.muted);
    }
    chosen
}

/// What a message row did this frame.
#[derive(Debug, Default)]
struct Drawn {
    /// It was clicked.
    clicked: bool,
    /// A hover action was pressed.
    acted: Option<Act>,
    /// It has the keyboard's focus.
    focused: bool,
    /// It gained the focus this frame.
    gained: bool,
    /// Its Snooze opened the time picker: the presets are to be read again.
    presets: bool,
}

/// Where a row stands in its list: its place among the list's rows and how many there are (a screen reader says "12 of
/// 1000"), and whether it is the last line of its card or list, which draws no line under it.
#[derive(Debug, Clone, Copy)]
struct Slot {
    rank: usize,
    size: usize,
    last: bool,
}

impl Slot {
    /// Tells AccessKit the place of the widget `id` in its list.
    fn tell(self, ui: &Ui, id: Id) {
        ui.ctx().accesskit_node_builder(id, |node| {
            node.set_position_in_set(self.rank);
            node.set_size_of_set(self.size);
        });
    }
}

/// One message row across `ui`'s width, the mail row of the design; a click selects it.
fn row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, message: &MessageRow, account_marker: bool, slot: Slot) -> Drawn {
    let selected = mailbox.selected() == Some(message.id);
    let keyed = selected && mailbox.keyed();
    let text = row_text(ui.ctx(), mailbox, message);
    let (spoken, line, time) = (&text.spoken, &text.line, &text.time);
    let back = text.back;
    let tag = if message.bounced.is_some() {
        Some((tr!("Not delivered"), TagKind::Minus))
    } else if message.new_sender {
        Some((tr!("New sender"), TagKind::Accent))
    } else if mailbox.returned(message) {
        Some((tr!("Returned"), TagKind::Warn))
    } else {
        None
    };
    // The pin, the flag nearest the time (or the actions), the done row's tick.
    let marks: Vec<Mark> = [(message.pinned, Mark::Pin), (message.flagged, Mark::Flag), (message.done, Mark::Done), (back, Mark::Snoozed)]
        .into_iter()
        .filter_map(|(shown, mark)| shown.then_some(mark))
        .collect();
    let account = account_marker.then(|| mailbox.account(message.account).map_or(0, |account| u64::try_from(account.index).unwrap_or(0)));
    let shown = MailRow {
        id: row_id(message.id),
        spoken,
        sender: &message.sender,
        line,
        time,
        unread: !message.seen,
        current: selected,
        last: slot.last,
        account,
        tag,
        marks: &marks,
        done: message.done,
        checked: false,
    };
    // Built only for the row whose group can show: the other rows hand the group nothing.
    let reach = keyed || ui.rect_contains_pointer(ui.available_rect_before_wrap());
    let built = reach.then(|| actions(message));
    let list: &[Action<'static>] = built.as_ref().map_or(&[], |built| built.as_slice());
    let drawn = mail_row(ui, theme, keymap, &shown, keyed, list, None);
    let response = &drawn.row.response;
    // egui does not scroll to what gains the focus: a row Tab reaches (the next one is always built, two below the
    // viewport) comes into sight, so Tab walks the whole list.
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    // Its place among all the list's rows, built or not: a screen reader says "12 of 1000".
    slot.tell(ui, response.id);
    keep_arrows(ui, response);
    let rect = response.rect;
    // In sight: the time picker open on this message, if any, stands under this row ([`snooze_picker`]); the keyed
    // row is where the keyboard goes when a picker closes unasked.
    if ui.is_rect_visible(rect) {
        note_snooze_row(ui.ctx(), message.id, rect, response.id);
        if keyed {
            let pass = ui.ctx().cumulative_pass_nr();
            ui.ctx().data_mut(|data| data.insert_temp(keyed_row_id(), (response.id, pass)));
        }
    }
    let mut acted = drawn.acted.and_then(|index| act(message, index));
    let presets = drawn.acted == Some(SNOOZE_ACTION);
    if presets {
        open_snooze(ui.ctx(), message.id, actions_id(message.id).with(SNOOZE_ACTION), rect);
        note_snooze_row(ui.ctx(), message.id, rect, response.id);
    }
    let mut closed = false;
    if message.group.is_some() {
        let (chosen, closed_here) = menu(ui, theme, response, message);
        acted = acted.or(chosen);
        closed = closed_here;
    }
    // The focus its menu gives back as it closes is not a walk to the row: it selects nothing.
    let gained = response.gained_focus() && !closed;
    Drawn { clicked: response.clicked(), acted, focused: response.has_focus(), gained, presets }
}

/// What a mail row says, worked out once for a list of rows and kept while it stays ([`row_text`]).
#[derive(Debug)]
struct RowText {
    /// Its name for a screen reader.
    spoken: String,
    /// Line two: the subject and the preview.
    line: String,
    /// The time at the right, or when a snoozed message comes back.
    time: String,
    /// It is snoozed and says when it comes back.
    back: bool,
}

/// The rows' words kept between frames: for which list of rows (its serial, place, search, the minute and the
/// wall clock's offset, since a date is said from now, where the owner is), and each message's with the pass it was
/// last used in.
#[derive(Debug, Default)]
struct RowTexts {
    key: (u64, Option<Place>, String, i64, i16),
    texts: std::collections::HashMap<u64, (u64, std::sync::Arc<RowText>)>,
}

/// The most rows whose words are kept; past it, those not used in the last frames go.
const MOST_KEPT_TEXTS: usize = 512;

/// The words of `message`'s row: worked out the first time it is built in a list of rows, then kept until the rows
/// are listed again, the place or the search change, or the minute turns; at most [`MOST_KEPT_TEXTS`] kept.
fn row_text(ctx: &egui::Context, mailbox: &Mailbox, message: &MessageRow) -> std::sync::Arc<RowText> {
    type Store = std::sync::Arc<std::sync::Mutex<RowTexts>>;
    let store: Store = ctx.data_mut(|data| data.get_temp_mut_or_default::<Store>(Id::unique("list-row-texts")).clone());
    let mut kept = store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let minute = mailbox.now().unix_seconds().div_euclid(60);
    let key = (mailbox.rows_serial(), Some(mailbox.place()), mailbox.query().to_owned(), minute, mailbox.offset().minutes());
    if kept.key != key {
        kept.key = key;
        kept.texts.clear();
    }
    let pass = ctx.cumulative_pass_nr();
    if let Some((used, text)) = kept.texts.get_mut(&message.id) {
        *used = pass;
        return text.clone();
    }
    if kept.texts.len() >= MOST_KEPT_TEXTS {
        kept.texts.retain(|_, (used, _)| pass.saturating_sub(*used) <= 1);
    }
    #[cfg(test)]
    super::list_tests::TEXTS_BUILT.with(|built| built.set(built.get().saturating_add(1)));
    let subject = if message.subject.is_empty() { tr!("(no subject)") } else { message.subject.as_str() };
    let line = format!("{subject}{}", after_subject(&message.preview));
    // A snoozed message (in a search, or Later searched) is the sheet's snoozed row: the clock and when it comes back.
    let back = mailbox.back_at(message);
    let time = back.as_ref().map_or_else(|| mailbox.row_date(message), |when| tr!("back {when}", when = when));
    let text = std::sync::Arc::new(RowText { spoken: mailbox.spoken(message), line, time, back: back.is_some() });
    if kept.texts.len() < MOST_KEPT_TEXTS {
        kept.texts.insert(message.id, (pass, text.clone()));
    }
    text
}

/// One message of a pile of Later across `ui`'s width, the design's later row: who, the pile's mark, the subject and
/// when it comes back; a snoozed one offers "bring back now" while hovered or keyed.
#[expect(clippy::too_many_arguments, reason = "the parts of one row")]
fn later(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    mailbox: &Mailbox,
    message: &MessageRow,
    pile: Pile,
    account_marker: bool,
    slot: Slot,
) -> Drawn {
    let selected = mailbox.selected() == Some(message.id);
    let keyed = selected && mailbox.keyed();
    let spoken = mailbox.spoken(message);
    let subject = if message.subject.is_empty() { tr!("(no subject)") } else { message.subject.as_str() };
    let date = mailbox.row_date(message);
    let (stands, when) = match pile {
        _ if mailbox.returned(message) => (LaterPile::Returned, date),
        Pile::Snoozed => (LaterPile::Snoozed, tr!("back {when}", when = date)),
        Pile::Pins => (LaterPile::Pinned, date),
        Pile::Parked => (LaterPile::Parked, date),
    };
    let account = account_marker.then(|| mailbox.account(message.account).map_or(0, |account| u64::try_from(account.index).unwrap_or(0)));
    let shown = LaterRow {
        id: row_id(message.id),
        spoken: &spoken,
        who: &message.sender,
        subject,
        when: &when,
        unread: !message.seen,
        current: selected,
        last: slot.last,
        account,
        pile: stands,
        link: (stands == LaterPile::Snoozed).then_some(tr!("bring back now")),
    };
    let drawn = later_row(ui, theme, keymap, &shown, keyed, None);
    let response = &drawn.row.response;
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    slot.tell(ui, response.id);
    keep_arrows(ui, response);
    Drawn {
        clicked: response.clicked(),
        acted: drawn.link.then_some(Act::Unsnooze),
        focused: response.has_focus(),
        gained: response.gained_focus(),
        presets: false,
    }
}

/// Keeps the arrow keys from moving egui's focus off the row `response` while it has the focus: they are the list's
/// keys ([`keymap::DOWN`], [`keymap::UP`]), which move the keyed row and the focus with it.
fn keep_arrows(ui: &Ui, response: &egui::Response) {
    if response.has_focus() {
        let filter = egui::EventFilter { vertical_arrows: true, ..egui::EventFilter::default() };
        ui.memory_mut(|memory| memory.set_focus_lock_filter(response.id, filter));
    }
}

/// Where the message whose group menu the keyboard asked for is kept until its row is drawn.
fn menu_asked_id() -> Id {
    Id::unique("list-menu-asked")
}

/// Asks for the group menu of message `id`'s row (`Shift F10`, the Menu key): the list brings the row into sight
/// and the row opens it when drawn.
pub(super) fn ask_menu(ctx: &egui::Context, id: u64) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|data| {
        data.insert_temp(menu_asked_id(), (id, pass));
        data.insert_temp(bring_id(), id);
    });
}

/// Where the row the list must bring into sight in its next pass is kept (a menu asked for a row out of sight).
fn bring_id() -> Id {
    Id::unique("list-bring-into-sight")
}

/// Brings message `id`'s row into sight in the list's next pass, as a newly keyed row is.
pub(super) fn bring(ctx: &egui::Context, id: u64) {
    ctx.data_mut(|data| data.insert_temp(bring_id(), id));
}

/// Where the row the keyboard lands on is kept, with the pass that asked for it ([`land`]).
fn land_id() -> Id {
    Id::unique("list-land")
}

/// The keyboard lands on message `id`'s row when the list is next drawn with nothing else focused (`Esc` from the
/// reader, the list back in front, the window crossing 980): on the row if it is in sight, else on the nearest row in
/// sight. Asked while a widget about to go (the reader's) still has the focus, it waits for egui to drop it, two
/// passes at most; a widget that keeps the focus keeps it.
pub(super) fn land(ctx: &egui::Context, id: u64) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|data| data.insert_temp(land_id(), (id, pass)));
}

/// The row the keyboard lands on for `wanted` in a viewport from `top` to `bottom`: `wanted` when its row is wholly in
/// sight, else the nearest message row wholly in sight (the first one when `wanted` is above or not listed, the last
/// one when it is below).
fn landing(listing: &Listing<'_>, wanted: u64, top: f32, bottom: f32) -> Option<u64> {
    let (first, end) = listing.in_sight(top, bottom);
    let whole = |index: &usize| match listing.lines.get(*index) {
        Some(Line::Mail(message)) => listing.span(*index).filter(|(above, below)| *above >= top && *below <= bottom).map(|_| message.id),
        _ => None,
    };
    let at = listing.find(wanted);
    if let Some(id) = at.as_ref().and_then(whole) {
        return Some(id);
    }
    let below = at.and_then(|at| listing.span(at)).is_some_and(|(above, _)| above >= bottom);
    if below { (first..end).rev().find_map(|index| whole(&index)) } else { (first..end).find_map(|index| whole(&index)) }
}

/// Where the open group menu is kept: the message whose row showed it, and the pass that did.
fn menu_shown_id() -> Id {
    Id::unique("list-menu-shown")
}

/// Settles the rows' menus after a pass: an ask ([`ask_menu`]) no row took in a whole pass is dropped (its row was not
/// drawn: its message gone), so it never opens later, away from the moment it was asked for; a menu whose row this
/// pass did not draw closes, as every float not shown.
pub(super) fn settle_menus(ctx: &egui::Context) {
    let pass = ctx.cumulative_pass_nr();
    if let Some((id, _)) = ctx.data(|data| data.get_temp::<(u64, u64)>(menu_shown_id())).filter(|(_, at)| *at < pass) {
        popup::set_open(ctx, group_menu_id(id), false);
        ctx.data_mut(|data| data.remove::<(u64, u64)>(menu_shown_id()));
    }
    // A landing no list took (the list not drawn, or a place without rows: New senders' queue) is dropped, so it
    // never lands later, in another place.
    if ctx.data(|data| data.get_temp::<(u64, u64)>(land_id())).is_some_and(|(_, at)| pass > at.saturating_add(2)) {
        ctx.data_mut(|data| data.remove::<(u64, u64)>(land_id()));
    }
    if ctx.data(|data| data.get_temp::<(u64, u64)>(menu_asked_id())).is_some_and(|(_, at)| at < pass) {
        ctx.data_mut(|data| data.remove::<(u64, u64)>(menu_asked_id()));
    }
}

/// The group menu of `message`'s row `response`: by a right click at the pointer, or asked for by the keyboard
/// ([`ask_menu`]) under the row, its first choice focused. A menu the keyboard opened gives its row the keyboard back
/// however it closes (Esc, a choice, a press outside); one the pointer opened, on Esc. A menu closing never changes
/// the message the reader shows. The act chosen, and whether the menu closed in this call (and so gave the row the
/// focus it gains now, if any).
fn menu(ui: &Ui, theme: &Theme, response: &egui::Response, message: &MessageRow) -> (Option<Act>, bool) {
    let ctx = ui.ctx().clone();
    let popup_id = group_menu_id(message.id);
    let first_id = popup_id.with("focus-first");
    if ctx.data(|data| data.get_temp::<(u64, u64)>(menu_asked_id())).is_some_and(|(id, _)| id == message.id) {
        ctx.data_mut(|data| {
            data.remove::<(u64, u64)>(menu_asked_id());
            data.insert_temp(first_id, true);
        });
        popup::open_under(&ctx, popup_id);
    }
    let first = ctx.data_mut(|data| data.remove_temp::<bool>(first_id)).unwrap_or(false);
    let width = MENU_WIDTH;
    let was_open = popup::is_open(&ctx, popup_id);
    let acted = popup::context_menu(&ctx, theme, response, popup_id, width, |ui| {
        let chosen = group_menu(ui, theme, message.group, &message.address, first);
        (chosen, chosen.is_some())
    })
    .flatten();
    let open = popup::is_open(&ctx, popup_id);
    if open {
        let pass = ctx.cumulative_pass_nr();
        ctx.data_mut(|data| data.insert_temp(menu_shown_id(), (message.id, pass)));
    }
    (acted, was_open && !open)
}

/// The id of the hover action group of message `id`'s row: its n-th action is `actions_id(id).with(n)` (Snooze's
/// [`SNOOZE_ACTION`]), what a picker opened from an action anchors to.
pub(crate) fn actions_id(id: u64) -> Id {
    crate::component::mail_row::actions_id(row_id(id))
}

/// The id of message `id`'s row: its focus and its context menu, wherever it is built.
pub(super) fn row_id(id: u64) -> Id {
    Id::unique("list-row").with(id)
}

/// The row of `message`, which has left the list, in the line `ui` gives it: drawn whole from the line's top and cut
/// to the line as its gap closes, faded `gone.faded` of the way; it takes no pointer, focus or act (a disabled row,
/// named for a screen reader). In Later it is the later row it was.
fn leaving_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, message: &MessageRow, account_marker: bool, gone: Gone) {
    let line = ui.max_rect();
    let whole = Rect::from_min_size(line.min, vec2(line.width(), row_height(theme)));
    let spoken = mailbox.spoken(message);
    let subject = if message.subject.is_empty() { tr!("(no subject)") } else { message.subject.as_str() };
    let account = account_marker.then(|| mailbox.account(message.account).map_or(0, |account| u64::try_from(account.index).unwrap_or(0)));
    let id = Id::unique("list-leaving").with(message.id);
    let mut face = ui.new_child(UiBuilder::new().max_rect(whole));
    face.shrink_clip_rect(line);
    // Disabled is drawn at full strength here: the fade is the motion's alone.
    face.visuals_mut().disabled_alpha = 1.0;
    face.disable();
    face.multiply_opacity(1.0 - gone.faded.clamp(0.0, 1.0));
    let rest = Some(crate::component::Shown::Rest);
    if let (Place::Later(pile), true) = (mailbox.place(), mailbox.query().is_empty()) {
        let date = mailbox.row_date(message);
        let (stands, when) = match pile {
            _ if mailbox.returned(message) => (LaterPile::Returned, date),
            Pile::Snoozed => (LaterPile::Snoozed, tr!("back {when}", when = date)),
            Pile::Pins => (LaterPile::Pinned, date),
            Pile::Parked => (LaterPile::Parked, date),
        };
        let shown = LaterRow {
            id,
            spoken: &spoken,
            who: &message.sender,
            subject,
            when: &when,
            unread: !message.seen,
            current: false,
            last: false,
            account,
            pile: stands,
            link: None,
        };
        let _ = later_row(&mut face, theme, keymap, &shown, false, rest);
        return;
    }
    let words = format!("{subject}{}", after_subject(&message.preview));
    let time = mailbox.row_date(message);
    let shown = MailRow {
        id,
        spoken: &spoken,
        sender: &message.sender,
        line: &words,
        time: &time,
        unread: !message.seen,
        current: false,
        last: false,
        account,
        tag: None,
        marks: &[],
        done: false,
        checked: false,
    };
    let _ = mail_row(&mut face, theme, keymap, &shown, false, &[], rest);
}

/// What follows a row's subject: a dash and the preview, as the design's line two, or nothing without one.
pub(super) fn after_subject(preview: &str) -> String {
    if preview.trim().is_empty() { String::new() } else { tr!(" — {preview}", preview = preview) }
}

/// What the list asked for this frame.
#[derive(Debug, Default)]
pub(super) struct Asked {
    /// The row clicked.
    pub(super) clicked: Option<u64>,
    /// A hover action pressed, and on which message.
    pub(super) acted: Option<(u64, Act)>,
    /// The gap of the row that left the list has closed, or no list was drawn: it is let go.
    pub(super) out: bool,
    /// Review on the screener card: "New senders" opens.
    pub(super) review: bool,
    /// "Done all" on a group's card: the group's messages.
    pub(super) done_all: Option<Vec<u64>>,
    /// A button of a send in the outbox, and its draft.
    pub(super) outbox: Option<(u64, Outbox)>,
    /// A pill of the screener's mode under "New senders".
    pub(super) screener: Option<ScreenerMode>,
    /// The row that gained the keyboard's focus this frame (`Tab` onto it).
    pub(super) gained: Option<u64>,
    /// A row's Snooze opened the time picker: the presets still ahead are to be read.
    pub(super) presets: bool,
}

/// What a button of a send in the outbox asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Outbox {
    /// Undo or Edit on a send that waits: it is taken back, and its draft opens.
    Unsend,
    /// Edit on a refused send: its draft opens.
    Edit,
    /// Send again, a refused send.
    Retry,
}

/// One send of the outbox across `ui`'s width, the design's outbox row: whom it goes to and the subject (with the
/// server's reply for a refused one), where it stands, and its buttons; the button pressed.
fn outbox_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, row: &OutboxRow, account_marker: bool, slot: Slot) -> Option<Outbox> {
    let subject = if row.subject.is_empty() { tr!("(no subject)") } else { row.subject.as_str() };
    let left = mailbox.seconds_left(row.draft);
    let standing = match (&row.state, left) {
        (SendState::Waiting(_), Some(left)) => {
            let hold = f32::from(mailbox.hold_seconds().max(1));
            Standing::Countdown { share: (left as f32 / hold).clamp(0.0, 1.0) }
        }
        (SendState::Waiting(_), None) => Standing::Scheduled,
        (SendState::Sending, _) => Standing::Sending,
        (SendState::Refused { .. }, _) => Standing::Refused,
    };
    let reply = match &row.state {
        SendState::Refused { reply, .. } => Some(reply.as_str()),
        _ => None,
    };
    let label = &row.subject;
    let undo = tr!("Undo the send of “{subject}”", subject = label);
    let take_back = tr!("Take “{subject}” back and edit it", subject = label);
    let again = tr!("Send “{subject}” again", subject = label);
    let edit = tr!("Edit “{subject}”", subject = label);
    let (buttons, asks): (Vec<SendButton<'_>>, Vec<Outbox>) = match standing {
        Standing::Countdown { .. } => (vec![SendButton { name: tr!("Undo"), spoken: &undo }], vec![Outbox::Unsend]),
        Standing::Scheduled => (vec![SendButton { name: tr!("Edit"), spoken: &take_back }], vec![Outbox::Unsend]),
        Standing::Sending => (Vec::new(), Vec::new()),
        // An answer to an invitation changes only from the invitation.
        Standing::Refused if row.invite_answer => (vec![SendButton { name: tr!("Send again"), spoken: &again }], vec![Outbox::Retry]),
        Standing::Refused => (
            vec![SendButton { name: tr!("Send again"), spoken: &again }, SendButton { name: tr!("Edit"), spoken: &edit }],
            vec![Outbox::Retry, Outbox::Edit],
        ),
    };
    let to = tr!("To {whom}", whom = mailbox.recipients(row));
    let words = mailbox.standing_shown(row);
    let spoken = mailbox.spoken_outbox(row);
    let account = account_marker.then(|| mailbox.account(row.account).map_or(0, |account| u64::try_from(account.index).unwrap_or(0)));
    let shown = OutboxRowView {
        id: Id::unique("list-send-row").with(row.draft),
        spoken: &spoken,
        to: &to,
        account,
        subject,
        words: &words,
        standing,
        reply,
        buttons: &buttons,
        last: slot.last,
    };
    let pressed = outbox_view(ui, theme, keymap, &shown, None);
    // Its place among the outbox's sends: a screen reader says "2 of 3".
    slot.tell(ui, shown.id);
    pressed.and_then(|index| asks.get(index).copied())
}

/// What the lines of a list share, and what they asked for this frame.
struct Rows<'a> {
    theme: &'a Theme,
    keymap: &'a Keymap,
    mailbox: &'a Mailbox,
    account_marker: bool,
    /// The row clicked this frame.
    clicked: Cell<Option<u64>>,
    /// A hover action pressed this frame.
    acted: Cell<Option<(u64, Act)>>,
    /// A row's Snooze opened the time picker this frame.
    presets: Cell<bool>,
    /// "Done all" pressed this frame: the group's messages.
    done_all: Cell<Option<Vec<u64>>>,
    /// Review pressed on the screener card.
    review: Cell<bool>,
    /// A button of a send in the outbox pressed, and its draft.
    outbox: Cell<Option<(u64, Outbox)>>,
    /// The row that gained the focus this frame.
    gained: Cell<Option<u64>>,
}

impl Rows<'_> {
    /// Draws `message`'s row across `ui`, the `rank`th (from 0) of the list's `size` rows.
    fn mail(&self, ui: &mut Ui, message: &MessageRow, slot: Slot) {
        let drawn = match self.mailbox.place() {
            Place::Later(pile) if self.mailbox.query().is_empty() => {
                later(ui, self.theme, self.keymap, self.mailbox, message, pile, self.account_marker, slot)
            }
            _ => row(ui, self.theme, self.keymap, self.mailbox, message, self.account_marker, slot),
        };
        if drawn.focused {
            ui.data_mut(|data| data.insert_temp(focused_row_id(), message.id));
        }
        if drawn.gained {
            self.gained.set(Some(message.id));
        }
        if drawn.clicked {
            self.clicked.set(Some(message.id));
        }
        if let Some(act) = drawn.acted {
            self.acted.set(Some((message.id, act)));
        }
        if drawn.presets {
            self.presets.set(true);
        }
    }
}

/// The screener card's height at 100 % text, in points: the design's 64, its line under it inside.
const SCREENER_HEIGHT: f32 = 64.0;

/// The screener card across `ui`'s line, as the design draws it on top of the inbox: the people mark, how many new
/// senders wait and that they are held back, the first three's avatars and Review, which opens New senders; the line
/// under it. Whether Review was pressed.
fn screener(ui: &mut Ui, theme: &Theme, keymap: &Keymap, waiting: &[Waiting]) -> bool {
    let p = &theme.palette;
    let f = &theme.fonts;
    let rect = ui.max_rect();
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 0.0, p.panel);
    super::paint::hairline(&painter, pos2(rect.left(), rect.bottom() - 1.0), pos2(rect.right(), rect.bottom() - 1.0), p.line);
    const PAD: f32 = 16.0;
    const GAP: f32 = 12.0;
    let icon = theme.boxes().icon;
    let mark = Rect::from_min_size(pos2(rect.left() + PAD, rect.top() + ((rect.height() - 1.0 - icon) / 2.0).round()), vec2(icon, icon));
    crate::iconset::paint(&painter, mark, IconId::People, p.cyan_text);
    // Review at the right end, the avatars before it, the two lines in what is left.
    let face = Theme::size(theme, 24.0);
    let shown: Vec<&Waiting> = waiting.iter().take(3).collect();
    let row = Rect::from_min_max(pos2(rect.left() + PAD, rect.top()), pos2(rect.right() - PAD, rect.bottom() - 1.0));
    let (review, right) = ui
        .scope_builder(UiBuilder::new().max_rect(row).id_salt("screener-parts").layout(Layout::right_to_left(Align::Center)), |ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            let review = button::TextButton::new(tr!("Review")).secondary().action("screener.review").show(ui, theme, keymap);
            let mut right = review.rect.left();
            if !shown.is_empty() {
                let count = shown.len() as f32;
                let (faces, _) = ui.allocate_exact_size(vec2(count * face + (count - 1.0) * 4.0, face), Sense::hover());
                for (index, waiting) in shown.iter().enumerate() {
                    let at = Rect::from_min_size(pos2(faces.left() + (face + 4.0) * index as f32, faces.top()), vec2(face, face));
                    avatar::paint(&painter, theme, at, &waiting.name, &waiting.address);
                }
                right = faces.left();
            }
            (review.clicked(), right)
        })
        .inner;
    let left = mark.right() + GAP;
    let room = (right - GAP - left).max(0.0);
    let count = waiting.len();
    let title = if count == 1 { tr!("1 new sender").to_owned() } else { tr!("{count} new senders", count = count) };
    let lines = f.strong.line + 2.0 + f.caption.line;
    let top = rect.top() + ((rect.height() - 1.0 - lines) / 2.0).round();
    let words = Rect::from_min_size(pos2(left, top), vec2(room, lines));
    let note = tr!("Held back until you let them in");
    // A screen reader reads the two lines as they are written.
    for (at, said) in [(0.0, title.as_str()), (f.strong.line + 2.0, note)] {
        let place =
            Rect::from_min_size(pos2(words.left(), words.top() + at), vec2(room.max(1.0), if at == 0.0 { f.strong.line } else { f.caption.line }));
        let id = ui.scope_id().with(("screener-words", at == 0.0));
        ui.interact(place, id, Sense::hover()).widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, said));
    }
    let title = crate::component::fitted(ui, &title, f.strong, p.ink, room);
    super::paint::run_at(&painter, pos2(left, top), title, p.ink);
    let note = crate::component::fitted(ui, note, f.caption, p.muted, room);
    super::paint::run_at(&painter, pos2(left, top + f.strong.line + 2.0), note, p.muted);
    review
}

/// One line of a list. A list is its lines one under another, each as high as [`line_height`] says for its kind, so
/// where every line stands is known without building one, and a frame builds only the lines in sight.
#[derive(Debug, Clone, Copy)]
pub(super) enum Line<'a> {
    /// A group's head: its index, name and unread count; in Smart how many of its rows show and "show all".
    Head(&'a Group, HeadState),
    /// The head of a group whose last row has just left the list: it leaves with the row, `Gone` of the way.
    HeadLeaving(&'a Group, HeadState, Gone),
    /// A message's row.
    Mail(&'a MessageRow),
    /// A draft's row in Drafts: drawn coming soon until the list of drafts is wired (stage I-2.9).
    Draft(&'a MessageRow),
    /// The row of a message that has just left the list, and how far it has gone.
    Leaving(&'a MessageRow, Gone),
    /// The screener card, while senders wait.
    Card(&'a [Waiting]),
    /// A send of the outbox.
    Send(&'a OutboxRow),
}

/// How far the row that left the list has gone, each from 0 to 1: how much of its height has closed, and how far it
/// has faded. Done, Delete, Archive, Move and Park close the row over `row_collapse` on the design's curve, fading on
/// CSS's `ease` as it closes (the design's M5 at the reduced level, the presentation's `.row.gone`); Snooze fades it over `fast` in its
/// place, and then it is gone (M6 at the reduced level).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Gone {
    /// The share of its height closed.
    pub closed: f32,
    /// The share of its fade.
    pub faded: f32,
}

/// How a group's head stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HeadState {
    /// Its unread messages.
    pub unread: u64,
    /// Its messages in the list.
    pub count: usize,
    /// It shows all its rows: in Smart after "show all", in Grouped always.
    pub open: bool,
    /// It folds to its first [`GROUP_ROWS`] rows (Smart) and has more: its head says how many show and offers "show
    /// all" or "show fewer".
    pub folds: bool,
    /// Its place among the groups shown, from 0: its index letter.
    pub index: usize,
}

/// `points` rounded to a whole even number of points, as [`Theme::size`] rounds: whole pixels at 1×, 1.5× and 2×.
fn even(points: f32) -> f32 {
    if points.is_finite() { (points.max(0.0) / 2.0).ceil() * 2.0 } else { 0.0 }
}

/// The height of `line`, fixed for its kind at the text scale: a group's head the group head's size, a row the mail
/// row's, a send the mail row's (a refused one a line more), the screener card its own; a head or a row leaving the
/// part of its height still open.
pub(super) fn line_height(theme: &Theme, line: &Line<'_>) -> f32 {
    let boxes = theme.boxes();
    let open = |gone: &Gone| 1.0 - gone.closed.clamp(0.0, 1.0);
    match line {
        Line::Head(..) => boxes.group_head,
        Line::HeadLeaving(_, _, gone) => even(boxes.group_head * open(gone)),
        Line::Mail(_) => boxes.mail_row,
        Line::Draft(_) => Theme::size(theme, crate::component::draft_row::HEIGHT),
        Line::Leaving(_, gone) => even(boxes.mail_row * open(gone)),
        Line::Send(row) if matches!(row.state, SendState::Refused { .. }) => Theme::size(theme, crate::component::outbox_row::REFUSED_HEIGHT),
        Line::Send(_) => Theme::size(theme, crate::component::outbox_row::HEIGHT),
        Line::Card(_) => Theme::size(theme, SCREENER_HEIGHT),
    }
}

/// A list laid out without building it: its lines, the top and bottom of each down the content (from 0), and the
/// whole height. A line's bottom is the next line's top, the same number, so two lines never overlap or leave a
/// hairline between them, and the last line's bottom is the height. The rows are lines across the pane, not cards.
pub(super) struct Listing<'a> {
    /// The lines, in order.
    pub lines: Vec<Line<'a>>,
    tops: Vec<f32>,
    bottoms: Vec<f32>,
    /// How many [`Line::Mail`] and [`Line::Send`] lines stand before each line: its place in the list for AccessKit.
    ranks: Vec<usize>,
    /// The content's height.
    pub height: f32,
    /// How many rows (messages or sends) it lists.
    pub rows: usize,
}

impl Listing<'_> {
    /// The lines that meet the content between `top` and `bottom` (a viewport): the first and one past the last;
    /// empty where none does.
    pub(super) fn in_sight(&self, top: f32, bottom: f32) -> (usize, usize) {
        let first = self.bottoms.partition_point(|at| *at <= top);
        let end = self.tops.partition_point(|at| *at < bottom);
        (first.min(end), end)
    }

    /// The top and bottom of line `index`.
    pub(super) fn span(&self, index: usize) -> Option<(f32, f32)> {
        Some((*self.tops.get(index)?, *self.bottoms.get(index)?))
    }

    /// The line of message `id`.
    fn find(&self, id: u64) -> Option<usize> {
        self.lines.iter().position(|line| matches!(line, Line::Mail(message) if message.id == id))
    }
}

/// Lays out lines one under another, summing their heights in `f64`, so 10 000 rows lose no pixel.
struct Stack<'a, 't> {
    theme: &'t Theme,
    listing: Listing<'a>,
    y: f64,
}

impl<'a, 't> Stack<'a, 't> {
    fn new(theme: &'t Theme) -> Self {
        let listing = Listing { lines: Vec::new(), tops: Vec::new(), bottoms: Vec::new(), ranks: Vec::new(), height: 0.0, rows: 0 };
        Self { theme, listing, y: 0.0 }
    }

    /// Puts `line` under the last.
    fn push(&mut self, line: Line<'a>) {
        let top = self.y;
        self.y += f64::from(line_height(self.theme, &line));
        let listing = &mut self.listing;
        listing.tops.push(top as f32);
        listing.bottoms.push(self.y as f32);
        listing.ranks.push(listing.rows);
        if matches!(line, Line::Mail(_) | Line::Send(_)) {
            listing.rows = listing.rows.saturating_add(1);
        }
        listing.lines.push(line);
    }

    fn finish(mut self) -> Listing<'a> {
        self.listing.height = self.y as f32;
        self.listing
    }
}

/// The rows of the list in its order as lines: each message's, and the row that has just left the list (`leaving`,
/// how far its gap has closed) at its old place.
fn listed<'m>(mailbox: &'m Mailbox, leaving: Option<(&'m Leaving, Gone)>) -> impl Iterator<Item = (Line<'m>, Option<&'m str>)> {
    let ghost = move |at: usize| {
        leaving.filter(|(left, _)| left.at == at).map(|(left, gone)| (Line::Leaving(&left.row, gone), Some(left.group.as_deref().unwrap_or(""))))
    };
    let count = mailbox.messages().len();
    mailbox
        .messages()
        .iter()
        .enumerate()
        .flat_map(move |(at, message)| ghost(at).into_iter().chain(std::iter::once((Line::Mail(message), None))))
        .chain(
            leaving
                .filter(|(left, _)| left.at >= count)
                .map(|(left, gone)| (Line::Leaving(&left.row, gone), Some(left.group.as_deref().unwrap_or("")))),
        )
}

/// The rows of each of `groups` and those no group holds, in the list's order, the row that has just left among them
/// (`leaving`): one pass over the list.
fn sorted<'m>(mailbox: &'m Mailbox, groups: &[Group], leaving: Option<(&'m Leaving, Gone)>) -> (Vec<Vec<Line<'m>>>, Vec<Line<'m>>) {
    let sets: Vec<HashSet<u64>> = groups.iter().map(|group| group.messages.iter().copied().collect()).collect();
    let mut members = vec![Vec::new(); groups.len()];
    let mut rest = Vec::new();
    for (line, left_from) in listed(mailbox, leaving) {
        let mut held = false;
        for ((set, members), group) in sets.iter().zip(members.iter_mut()).zip(groups) {
            let holds = match (line, left_from) {
                (_, Some(name)) => name == group.name,
                (Line::Mail(message), None) => set.contains(&message.id),
                _ => false,
            };
            if holds {
                members.push(line);
                held = true;
                break;
            }
        }
        if !held {
            rest.push(line);
        }
    }
    (members, rest)
}

/// The head of a group of `lines`: its messages' rows counted, the row that has left not; `smart` folds it to its
/// first rows unless `open`.
fn head_state(lines: &[Line<'_>], smart: bool, open: bool, index: usize) -> HeadState {
    let messages = || lines.iter().filter_map(|line| if let Line::Mail(message) = line { Some(*message) } else { None });
    let unread = u64::try_from(messages().filter(|message| !message.seen).count()).unwrap_or(u64::MAX);
    let count = messages().count();
    let folds = smart && count > GROUP_ROWS;
    HeadState { unread, count, open: open || !folds, folds, index }
}

/// The whole list as lines, laid out: in the unified inbox the screener card while senders `wait`; in the List
/// `view` every row by time; in Grouped each of `groups` under its head with all its rows, then the rows no group holds
/// under `other`'s; in Smart the same with each group's first [`GROUP_ROWS`] rows unless `open` says it shows all.
/// The row that has just left the list stands at its old place while it goes, `gone` of the way (`None`: gone, it is
/// left out); a group's head whose only row it was leaves with it.
#[expect(clippy::too_many_arguments, reason = "what the lines of a list are made of")]
pub(super) fn lines<'a>(
    theme: &Theme,
    mailbox: &'a Mailbox,
    view: InboxView,
    groups: &'a [Group],
    other: &'a Group,
    wait: bool,
    open: &dyn Fn(&Group) -> bool,
    gone: Option<Gone>,
) -> Listing<'a> {
    let leaving = mailbox.left_row().zip(gone);
    let mut stack = Stack::new(theme);
    if wait {
        stack.push(Line::Card(mailbox.waiting()));
    }
    // Drafts: the design's draft rows, coming soon.
    let drafts = mailbox.place() == Place::View(Role::Drafts) && mailbox.query().is_empty();
    if view == InboxView::List || groups.is_empty() {
        for (line, _) in listed(mailbox, leaving) {
            stack.push(match line {
                Line::Mail(message) if drafts => Line::Draft(message),
                line => line,
            });
        }
        return stack.finish();
    }
    let (members, rest) = sorted(mailbox, groups, leaving);
    let rows = |line: &Line<'_>| matches!(line, Line::Mail(_) | Line::Leaving(..));
    let rest = Some((other, &rest)).filter(|(_, rest)| rest.iter().any(rows));
    let parts = groups.iter().zip(&members).filter(|(_, lines)| lines.iter().any(rows)).chain(rest);
    let smart = view == InboxView::Smart;
    for (index, (group, lines)) in parts.enumerate() {
        let state = head_state(lines, smart, open(group), index);
        // A group whose rows have all left: its head goes as its last row goes.
        let ghost = lines.iter().find_map(|line| if let Line::Leaving(_, gone) = line { Some(*gone) } else { None });
        match ghost.filter(|_| state.count == 0) {
            Some(gone) => stack.push(Line::HeadLeaving(group, state, gone)),
            None => stack.push(Line::Head(group, state)),
        }
        let shown = if state.open { state.count } else { GROUP_ROWS };
        let mut shown_rows = 0;
        // A row leaving from under a folded group's first rows is not drawn: only those the group shows.
        for line in lines {
            if shown_rows == shown && state.count > 0 {
                break;
            }
            shown_rows += usize::from(matches!(line, Line::Mail(_)));
            stack.push(*line);
        }
    }
    stack.finish()
}

/// The sends of the outbox as lines.
fn sends<'a>(theme: &Theme, mailbox: &'a Mailbox) -> Listing<'a> {
    let mut stack = Stack::new(theme);
    for row in mailbox.outbox() {
        stack.push(Line::Send(row));
    }
    stack.finish()
}

/// What a group's head was pressed for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct HeadPressed {
    /// "done all".
    done_all: bool,
    /// "show all" or "show fewer".
    fold: bool,
}

/// A group's head across `ui`, the design's group head: its index, name and unread count; in Smart, while it has more
/// rows than it shows, how many show and "show all" or "show fewer"; "done all" while it is hovered or keyed.
fn head(ui: &mut Ui, theme: &Theme, keymap: &Keymap, group: &Group, state: HeadState) -> HeadPressed {
    let letter = char::from(b'A'.saturating_add(u8::try_from(state.index % 26).unwrap_or(0)));
    let index = format!("[ {letter} ]");
    let shown = state.folds.then(|| {
        let showing = if state.open { state.count } else { GROUP_ROWS.min(state.count) };
        tr!("{shown} of {count} ·", shown = showing, count = state.count)
    });
    let link = state.folds.then(|| if state.open { tr!("show fewer") } else { tr!("show all") });
    let head = GroupHead {
        id: Id::unique("list-group-head").with(&group.name),
        index: &index,
        name: &group.name,
        unread: state.unread,
        shown: shown.as_deref(),
        link,
        // A head over no message left (its last row leaving) offers nothing to mark done.
        hover_link: (state.count > 0).then_some(tr!("done all")),
    };
    let pressed = group_head(ui, theme, keymap, &head, None);
    HeadPressed { done_all: pressed.hover_link, fold: pressed.link }
}

/// The head of a group whose last row is leaving, in the line `ui` gives it: drawn whole from the line's top, cut to
/// the line as it closes and faded as the row is; it takes no pointer or focus.
fn leaving_head(ui: &mut Ui, theme: &Theme, keymap: &Keymap, group: &Group, state: HeadState, gone: Gone) {
    let line = ui.max_rect();
    let whole = Rect::from_min_size(line.min, vec2(line.width(), theme.boxes().group_head));
    let mut face = ui.new_child(UiBuilder::new().max_rect(whole).id_salt("leaving-head"));
    face.shrink_clip_rect(line);
    face.visuals_mut().disabled_alpha = 1.0;
    face.disable();
    face.multiply_opacity(1.0 - gone.faded.clamp(0.0, 1.0));
    let letter = char::from(b'A'.saturating_add(u8::try_from(state.index % 26).unwrap_or(0)));
    let index = format!("[ {letter} ]");
    let head = GroupHead {
        id: Id::unique("list-leaving-head").with(&group.name),
        index: &index,
        name: &group.name,
        unread: 0,
        shown: None,
        link: None,
        hover_link: None,
    };
    let _ = group_head(&mut face, theme, keymap, &head, Some(crate::component::Shown::Rest));
}

/// What a line's ui is told apart by, so a row keeps its id (its focus, its context menu) wherever it is built.
fn salt(line: &Line<'_>) -> Id {
    match line {
        Line::Head(group, _) => Id::unique("list-head").with(&group.name),
        Line::HeadLeaving(group, ..) => Id::unique("list-head-leaving").with(&group.name),
        Line::Mail(message) => Id::unique("list-mail").with(message.id),
        Line::Draft(message) => Id::unique("list-draft").with(message.id),
        Line::Leaving(message, _) => Id::unique("list-leaving-line").with(message.id),
        Line::Card(_) => Id::unique("list-screener"),
        Line::Send(row) => Id::unique("list-send").with(row.draft),
    }
}

/// Draws `line` across `ui`, the line's own room.
fn draw_line(ui: &mut Ui, rows: &Rows<'_>, line: &Line<'_>, slot: Slot) {
    let (theme, keymap, mailbox) = (rows.theme, rows.keymap, rows.mailbox);
    match line {
        Line::Head(group, state) => {
            let pressed = head(ui, theme, keymap, group, *state);
            if pressed.done_all {
                let all = mailbox.messages().iter().filter(|message| group.messages.contains(&message.id)).map(|message| message.id);
                rows.done_all.set(Some(all.collect()));
            }
            if pressed.fold {
                ui.data_mut(|data| data.insert_temp(group_open_id(&group.name), !state.open));
            }
        }
        Line::HeadLeaving(group, state, gone) => leaving_head(ui, theme, keymap, group, *state, *gone),
        Line::Mail(message) => rows.mail(ui, message, slot),
        Line::Draft(message) => {
            let subject = (!message.subject.is_empty()).then_some(message.subject.as_str());
            let spoken = mailbox.spoken(message);
            let when = tr!("edited {when}", when = mailbox.row_date(message));
            let to = if message.sender.is_empty() { tr!("no recipient yet").to_owned() } else { tr!("to {whom}", whom = message.sender) };
            let row = DraftRow { id: Id::unique("list-draft-row").with(message.id), spoken: &spoken, subject, to: &to, when: &when, last: false };
            crate::component::soon::soon(ui, theme, |ui| {
                let _ = draft_row(ui, theme, keymap, &row, false, None);
            });
        }
        Line::Leaving(message, gone) => leaving_row(ui, theme, keymap, mailbox, message, rows.account_marker, *gone),
        Line::Card(waiting) => {
            if screener(ui, theme, keymap, waiting) {
                rows.review.set(true);
            }
        }
        Line::Send(row) => {
            if let Some(pressed) = outbox_row(ui, theme, keymap, mailbox, row, rows.account_marker, slot) {
                rows.outbox.set(Some((row.draft, pressed)));
            }
        }
    }
}

/// Where the list kept, last frame, the message it holds in place and the keyed row it last brought into sight.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Kept {
    /// What the list showed: a message stays in place only in the same list.
    place: Place,
    view: InboxView,
    query: String,
    /// The first message in sight, and how far below the viewport's top its row's top stood (above it: negative).
    anchor: Option<(u64, i64)>,
    /// The viewport's top then, in hundredths of a point.
    offset: i64,
    /// The viewport's height then, in hundredths of a point.
    height: i64,
    /// The keyed row's message last frame: a row is brought into sight when the keyed message changes (a key or a
    /// press moved it) or the list is first shown, never because lines above it came or went.
    keyed: Option<u64>,
}

/// Lands the keyboard asked for by [`land`] on its row before the lines are built, so the row sees the focus it
/// gained (and the window keys it) in this pass; `forced` is the offset this pass sets, if any.
fn land_now(ui: &Ui, listing: &Listing<'_>, kept: Option<&Kept>, forced: Option<f32>) {
    let ctx = ui.ctx();
    let Some((wanted, asked)) = ctx.data(|data| data.get_temp::<(u64, u64)>(land_id())) else { return };
    let free = ctx.memory(|memory| memory.focused().is_none());
    if !free && ctx.cumulative_pass_nr() <= asked.saturating_add(2) {
        // The widget about to go still has the focus: egui drops it at the end of the pass.
        ctx.request_repaint();
        return;
    }
    ctx.data_mut(|data| data.remove::<(u64, u64)>(land_id()));
    if !free {
        return;
    }
    let target = match kept {
        Some(kept) if kept.height > 0 => {
            let room = ui.available_rect_before_wrap().height();
            let view = if room.is_finite() && room > 0.0 { room } else { points(kept.height) };
            let offset = forced.unwrap_or_else(|| points(kept.offset));
            landing(listing, wanted, offset, offset + view)
        }
        // The list first drawn: its row comes into sight as a keyed one does.
        _ => listing.find(wanted).map(|_| wanted),
    };
    if let Some(target) = target {
        ctx.memory_mut(|memory| memory.request_focus(row_id(target)));
    }
}

/// Hundredths of a point, which [`Kept`] compares exactly.
fn hundredths(points: f32) -> i64 {
    if points.is_finite() { (f64::from(points) * 100.0).round() as i64 } else { 0 }
}

/// Points from [`hundredths`].
fn points(hundredths: i64) -> f32 {
    (hundredths as f64 / 100.0) as f32
}

/// Draws `listing` in a scroll area of `ui` (told apart by `id_salt`), building only the lines that meet the viewport
/// and two above and below it; what they asked for goes into `rows`. When the lines above the first message in sight
/// change (a group opens or folds, a message comes in), that message stays where it was, unless the list stood at its
/// top; a row moved to by the keyboard is scrolled into sight whether it was built or not.
fn show_lines(ui: &mut Ui, rows: &Rows<'_>, listing: &Listing<'_>, id_salt: &str) -> Built {
    let (theme, mailbox) = (rows.theme, rows.mailbox);
    let kept_id = ui.make_persistent_id(id_salt).with("kept");
    let kept = ui.data(|data| data.get_temp::<Kept>(kept_id));
    let same = |kept: &Kept| kept.place == mailbox.place() && kept.view == mailbox.inbox_view() && kept.query == mailbox.query();
    let kept = kept.filter(same);
    // The offset this frame sets, if any: the first message in sight kept in place when the lines above it changed,
    // then the keyed row brought into sight when the keyed message changed.
    let mut forced = None;
    if let Some(kept) = &kept
        && kept.offset > 50
        && let Some((id, below)) = kept.anchor
        && let Some((top, _)) = listing.find(id).and_then(|index| listing.span(index))
    {
        let wanted = hundredths(top).saturating_sub(below);
        if wanted != kept.offset {
            forced = Some(points(wanted).max(0.0));
        }
    }
    let keyed = mailbox.selected().filter(|_| mailbox.keyed());
    // A row a menu was asked for comes into sight as a newly keyed one does.
    let bring = ui.data_mut(|data| data.remove_temp::<u64>(bring_id()));
    let moved = keyed.filter(|keyed| kept.as_ref().is_none_or(|kept| kept.keyed != Some(*keyed))).or(bring);
    let mut scroll_to = None;
    if let Some((top, bottom)) = moved.and_then(|id| listing.find(id)).and_then(|index| listing.span(index)) {
        match &kept {
            // The least scroll that shows the row whole, set before the lines are built, so the row is built (and
            // can keep a focus given to it) in this frame.
            Some(kept) if kept.height > 0 => {
                // The viewport this frame: the area fills the room left, which a resized window changes.
                let room = ui.available_rect_before_wrap().height();
                let view = if room.is_finite() && room > 0.0 { room } else { points(kept.height) };
                let offset = forced.unwrap_or_else(|| points(kept.offset));
                let wanted = if top < offset {
                    top
                } else if bottom > offset + view {
                    bottom - view
                } else {
                    offset
                };
                if forced.is_some() || hundredths(wanted) != kept.offset {
                    forced = Some(wanted.max(0.0));
                }
            }
            // The list first shown: egui scrolls to it once it knows the viewport.
            _ => scroll_to = Some((top, bottom)),
        }
    }
    land_now(ui, listing, kept.as_ref(), forced);
    let mut area = ScrollArea::vertical().id_salt(id_salt).auto_shrink([false, false]);
    // egui springs an offset past last frame's end back to it (rubber-banding), and the offset that keeps a message
    // in place after the lines above it grew can lie past that end: the frame that sets an offset does not
    // rubber-band.
    let kinetic = ui.spacing().scroll.kinetic;
    if let Some(offset) = forced {
        area = area.vertical_scroll_offset(offset);
        ui.spacing_mut().scroll.kinetic.rubber_band = false;
    }
    let output = crate::component::scroll::show_viewport_over(ui, theme, area, |ui, viewport| {
        ui.set_min_height(listing.height);
        let room = ui.max_rect();
        let at = |top: f32, bottom: f32| Rect::from_min_max(pos2(room.left(), room.top() + top), pos2(room.right(), room.top() + bottom));
        let (first, end) = listing.in_sight(viewport.top(), viewport.bottom());
        let from = first.saturating_sub(2);
        let to = end.saturating_add(2).min(listing.lines.len());
        // The rows are a list box of all the list's rows, of which only those built are in AccessKit's tree.
        let sends = matches!(listing.lines.first(), Some(Line::Send(_)));
        let (role, name) = if sends { (AccessRole::List, tr!("Sends")) } else { (AccessRole::ListBox, tr!("Messages")) };
        let mut listed = ui.new_child(UiBuilder::new().id_salt("lines").max_rect(room).accessibility_role(role));
        let set = listed.unique_id();
        listed.ctx().accesskit_node_builder(set, |node| {
            node.set_label(name);
            node.set_size_of_set(listing.rows);
        });
        for index in from..to {
            let (Some(line), Some((top, bottom))) = (listing.lines.get(index), listing.span(index)) else { break };
            let layout = match line {
                Line::Head(..) | Line::HeadLeaving(..) => Layout::left_to_right(Align::Center),
                _ => Layout::top_down(Align::Min),
            };
            let rect = at(top, bottom);
            let rank = listing.ranks.get(index).copied().unwrap_or(0);
            // Every row keeps the line under it, the last too: the list's rows are lines across the pane, and the pane
            // goes on under the last one.
            let slot = Slot { rank, size: listing.rows, last: false };
            listed.scope_builder(UiBuilder::new().id_salt(salt(line)).max_rect(rect).layout(layout), |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                draw_line(ui, rows, line, slot);
            });
        }
        if let Some((top, bottom)) = scroll_to {
            ui.scroll_to_rect(at(top, bottom), None);
        }
        let whole = (first..end)
            .filter(|index| {
                matches!(listing.lines.get(*index), Some(Line::Mail(_)))
                    && listing.span(*index).is_some_and(|(top, bottom)| top >= viewport.top() && bottom <= viewport.bottom())
            })
            .count();
        ui.data_mut(|data| data.insert_temp(page_id(), whole));
        let anchor = (first..end).find_map(|index| match listing.lines.get(index) {
            Some(Line::Mail(message)) => Some((message.id, hundredths(listing.span(index)?.0 - viewport.top()))),
            _ => None,
        });
        (Built { first: from, count: to.saturating_sub(from) }, anchor, hundredths(viewport.top()), hundredths(viewport.height()))
    });
    ui.spacing_mut().scroll.kinetic = kinetic;
    let (built, anchor, offset, height) = output.inner;
    let kept = Kept { place: mailbox.place(), view: mailbox.inbox_view(), query: mailbox.query().to_owned(), anchor, offset, height, keyed };
    ui.data_mut(|data| {
        data.insert_temp(Id::unique("list-view-rect"), output.inner_rect);
        data.insert_temp(kept_id, kept);
        if let Some((id, _)) = anchor {
            data.insert_temp(Id::unique("list-first-in-sight"), id);
        }
    });
    built
}

/// The empty window's card: the inbox icon, "No accounts yet" and what to do. Where the window can add accounts
/// (`can_add`), Add account, which opens the settings on Accounts; whether it was pressed.
pub(super) fn welcome(ui: &mut Ui, theme: &Theme, keymap: &Keymap, can_add: bool) -> bool {
    let p = &theme.palette;
    let s = &theme.spacing;
    let side = theme.fonts.body.line * 3.6;
    let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    crate::iconset::paint(ui.painter(), rect, IconId::Inbox, p.cyan_text);
    ui.add_space(s.s3);
    ui.label(RichText::new(tr!("No accounts yet")).font(theme.fonts.title.id()).color(p.ink));
    ui.add_space(s.s1);
    let about = if can_add {
        tr!("Add an account to read its mail here. Nothing leaves this computer before you do.")
    } else {
        tr!("Start InboxCraft with --sample to look around an invented mailbox.")
    };
    ui.add(egui::Label::new(RichText::new(about).font(theme.fonts.body.id()).color(p.muted)).wrap());
    if !can_add {
        return false;
    }
    ui.add_space(s.s4);
    let add = button::TextButton::new(tr!("Add account")).icon(IconId::Add).primary().show(ui, theme, keymap).clicked();
    ui.add_space(s.s1);
    ui.label(RichText::new(tr!("Opens Settings · Accounts")).font(theme.fonts.caption.id()).color(p.muted));
    add
}

/// One sender waiting in New senders: their newest held message (whose row the queue's card shows) and how many more
/// they sent.
struct Asker<'a> {
    newest: &'a MessageRow,
    more: usize,
}

/// The senders New senders holds, one for each address in the order their newest mail came (the list's order), at
/// most [`QUEUE_MOST`].
/// Worked out once for a list of rows (its serial, place and search) and kept: which rows are the senders' newest and
/// how many more each sent.
fn askers<'a>(ctx: &egui::Context, mailbox: &'a Mailbox) -> Vec<Asker<'a>> {
    type Kept = ((u64, Option<Place>, String), std::sync::Arc<[(usize, usize)]>);
    let key = (mailbox.rows_serial(), Some(mailbox.place()), mailbox.query().to_owned());
    let id = Id::unique("queue-askers");
    let kept = ctx.data(|data| data.get_temp::<Kept>(id)).filter(|(kept, _)| *kept == key).map(|(_, found)| found);
    let found = kept.unwrap_or_else(|| {
        let mut at: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let mut found: Vec<(usize, usize)> = Vec::new();
        for (index, message) in mailbox.messages().iter().enumerate() {
            let address = message.address.to_lowercase();
            if let Some(asker) = at.get(&address).and_then(|place| found.get_mut(*place)) {
                asker.1 = asker.1.saturating_add(1);
            } else if found.len() < QUEUE_MOST {
                at.insert(address, found.len());
                found.push((index, 0));
            }
        }
        let found: std::sync::Arc<[(usize, usize)]> = found.into();
        ctx.data_mut(|data| data.insert_temp::<Kept>(id, (key, found.clone())));
        found
    });
    found.iter().filter_map(|(index, more)| mailbox.messages().get(*index).map(|newest| Asker { newest, more: *more })).collect()
}

/// The most senders the queue shows a card for at once; the rest come as those are answered.
const QUEUE_MOST: usize = 50;

/// The queue's room around it and between its parts, in points, and the widest a card gets.
const QUEUE_PAD: (f32, f32) = (56.0, 36.0);
const QUEUE_GAP: f32 = 20.0;
const CARD_GAP: f32 = 12.0;
const CARD_WIDE: f32 = 900.0;

/// "New senders", the design's queue: its label, "Who gets in?", what it is for, the screener's mode, then a card for
/// each sender who waits (their avatar, name, account and address, their newest message and how many more, its first
/// line) with Let in and Keep out; the first card is the current one, its edge in the accent, Let in primary and the
/// line that says why it waits under its text. What was pressed: a card's answer, a mode.
fn senders(ui: &mut Ui, rows: &Rows<'_>) -> (Built, Option<ScreenerMode>) {
    let (theme, keymap, mailbox) = (rows.theme, rows.keymap, rows.mailbox);
    let (p, f) = (&theme.palette, &theme.fonts);
    let askers = askers(ui.ctx(), mailbox);
    let mut mode = None;
    let room = ui.available_rect_before_wrap();
    ui.painter().rect_filled(room, 0.0, p.bg);
    let area = ScrollArea::vertical().id_salt("new-senders").auto_shrink([false, false]);
    crate::component::scroll::show(ui, theme, area, |ui| {
        let pad = if ui.available_width() > CARD_WIDE { QUEUE_PAD.0 } else { theme.spacing.s4 };
        Frame::new().inner_margin(Margin::symmetric(i8::try_from(pad as i32).unwrap_or(16), 36)).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, QUEUE_GAP);
            let count = askers.len();
            let word = if count == 1 {
                tr!("NEW SENDERS · 1 FIRST-TIME SENDER").to_owned()
            } else {
                tr!("NEW SENDERS · {count} FIRST-TIME SENDERS", count = count)
            };
            crate::component::label::section_label(ui, theme, "[ 01 ]", &word, false);
            ui.label(RichText::new(tr!("Who gets in?")).font(f.display.id()).color(p.ink));
            let about =
                tr!("Nobody new reaches the inbox before you say so. If you keep a sender out, their mail goes to Blocked, never to the inbox.");
            ui.add(egui::Label::new(RichText::new(about).font(f.read.id()).color(p.muted)).wrap());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.spacing.s3;
                crate::component::label(ui, f.label, tr!("Screener"), p.muted);
                let modes = ScreenerMode::ALL.map(|choice| (choice, super::settings::screener_name(choice), f.button));
                if let Some(choice) = segmented_for(ui, theme, &modes, mailbox.screener_mode(), Some(tr!("the screener"))) {
                    mode = Some(choice);
                }
            });
            ui.spacing_mut().item_spacing.y = CARD_GAP;
            // The cards just answered, while they leave (M11): each at its place in the queue, as far as it has gone.
            let leaving: Vec<(Answered, Gone, f32)> = answered(ui.ctx(), &askers)
                .into_iter()
                .map(|left| {
                    let (gone, tint) = answer_gone(ui.ctx(), theme, &left);
                    (left, gone, tint)
                })
                .collect();
            let still: Vec<Answered> = leaving.iter().filter(|(_, gone, _)| gone.closed < 1.0).map(|(left, ..)| left.clone()).collect();
            ui.data_mut(|data| data.insert_temp(answered_id(), still));
            let holding = leaving.iter().any(|(left, gone, _)| left.at == 0 && gone.closed <= 0.0);
            let mut answers = Vec::new();
            for index in 0..=askers.len() {
                for (left, gone, tint) in leaving.iter().filter(|(left, ..)| left.at.min(askers.len()) == index) {
                    leaving_card(ui, theme, keymap, mailbox, left, *gone, *tint);
                }
                let Some(asker) = askers.get(index) else { break };
                // The next card becomes the current one when the answered one has faded.
                let current = index == 0 && !holding;
                let (act, high) = sender_card(ui, theme, keymap, mailbox, asker, current, None);
                if let Some(act) = act {
                    rows.acted.set(Some((asker.newest.id, act)));
                    if !motion::reduced(ui.ctx()) {
                        answers.push(Answered { row: asker.newest.clone(), more: asker.more, at: index, keep_out: act == Act::Block, high, current });
                    }
                }
            }
            if !answers.is_empty() {
                ui.data_mut(|data| data.get_temp_mut_or_default::<Vec<Answered>>(answered_id()).extend(answers));
            }
            if let Some(id) = ui.data_mut(|data| data.remove_temp::<u64>(Id::unique("new-sender-opened"))) {
                rows.clicked.set(Some(id));
            }
        });
    });
    (Built { first: 0, count: askers.len() }, mode)
}

/// The card of a sender just answered, while it leaves the queue (M11 at the level the app plays): its edge in the
/// answer's colour, then it fades, then its place closes and the next card steps up.
#[derive(Debug, Clone)]
struct Answered {
    /// The sender's newest message, as the card showed it.
    row: MessageRow,
    more: usize,
    /// Its place in the queue.
    at: usize,
    /// Keep out (the edge in `minus`); Let in otherwise (`plus`).
    keep_out: bool,
    /// The card's height, in points.
    high: f32,
    /// It was the current card (its edge `cyan_edge`, from which the answer's colour eases in).
    current: bool,
}

/// Where the [`Answered`] cards still leaving are kept.
fn answered_id() -> Id {
    Id::unique("queue-answered")
}

/// Forgets the cards still leaving: the queue is not shown, so they never play their motion again later.
fn forget_answered(ctx: &egui::Context) {
    ctx.data_mut(|data| data.remove::<Vec<Answered>>(answered_id()));
}

/// How long a card answered shows its answer's colour before it fades, in seconds (the presentation's 160 ms).
const ANSWER_HOLD: f32 = 0.16;

/// The cards answered and still leaving, but those whose sender still waits (the answer failed): none is drawn twice.
fn answered(ctx: &egui::Context, askers: &[Asker<'_>]) -> Vec<Answered> {
    let left = ctx.data(|data| data.get_temp::<Vec<Answered>>(answered_id())).unwrap_or_default();
    left.into_iter()
        .filter(|left| {
            let address = left.row.address.to_lowercase();
            !askers.iter().any(|asker| asker.newest.address.to_lowercase() == address)
        })
        .collect()
}

/// How far the answered card has gone in this frame, as the presentation plays M11 at the reduced level: its edge
/// eases to the answer's colour over `fast` (CSS's `ease`) and holds it until [`ANSWER_HOLD`], it fades over `fast` at
/// an even pace, then its place closes over `fast` on the design's curve. Repaints are asked for until it has gone.
/// The share of the edge's colour comes with it.
fn answer_gone(ctx: &egui::Context, theme: &Theme, left: &Answered) -> (Gone, f32) {
    let (hold, fade, close) = (ANSWER_HOLD, theme.motion.fast, theme.motion.fast);
    let whole = hold + fade + close;
    let id = Id::unique("queue-leaving").with(left.row.address.to_lowercase());
    let at = motion::elapsed(ctx, id, whole) * whole;
    let share = |from: f32, span: f32| if span > 0.0 { ((at - from) / span).clamp(0.0, 1.0) } else { 1.0 };
    let tint = motion::css_ease(share(0.0, theme.motion.fast));
    (Gone { faded: share(hold, fade), closed: motion::ease(share(hold + fade, close)) }, tint)
}

/// Draws the card `left` answered as far as it has `gone`: whole in its answer's colour, fading, then only the room it
/// leaves, closing. What is pressed on it does nothing: it was answered. `tint`: how far its edge has eased from its
/// own colour to the answer's.
fn leaving_card(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, left: &Answered, gone: Gone, tint: f32) {
    if gone.closed > 0.0 {
        // The card's place and the gap under it close together; the next card steps up into them.
        let room = (left.high + CARD_GAP) * (1.0 - gone.closed);
        if room > 0.0 && room.is_finite() {
            ui.add_space(room);
        }
        return;
    }
    let p = &theme.palette;
    let answer = if left.keep_out { p.minus } else { p.plus };
    let from = if left.current { p.cyan_edge } else { p.line };
    let edge = if tint >= 1.0 { answer } else { from.lerp_to_gamma(answer, tint.clamp(0.0, 1.0)) };
    let asker = Asker { newest: &left.row, more: left.more };
    ui.scope(|ui| {
        ui.multiply_opacity(1.0 - gone.faded);
        let _ = sender_card(ui, theme, keymap, mailbox, &asker, left.current, Some(edge));
    });
}

/// [`sender_card`] for `newest`, the current one, in whatever room `ui` gives: its height (the tests' hostile rooms).
#[cfg(test)]
pub(super) fn sender_card_of(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, newest: &MessageRow) -> f32 {
    sender_card(ui, theme, keymap, mailbox, &Asker { newest, more: 0 }, true, None).1
}

/// The card of the sender `asker` across `ui` (at most [`CARD_WIDE`]): `current` for the first of the queue, its edge
/// `edge` when given (an answered card's). The answer pressed, Let in or Keep out, and the card's height.
fn sender_card(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    mailbox: &Mailbox,
    asker: &Asker<'_>,
    current: bool,
    edge: Option<egui::Color32>,
) -> (Option<Act>, f32) {
    let live = edge.is_none();
    let (p, f, s) = (&theme.palette, &theme.fonts, &theme.spacing);
    let message = asker.newest;
    let wide = ui.available_width().min(CARD_WIDE).floor();
    let mut answer = None;
    let mut opened = false;
    let open = &mut opened;
    let id = Id::unique("new-sender-card").with(message.address.to_lowercase());
    let inner = ui.push_id(id, |ui| {
        ui.set_max_width(wide);
        let slot = ui.painter().add(egui::Shape::Noop);
        let shown = Frame::new().inner_margin(Margin { left: 20, right: 20, top: 16, bottom: 16 }).show(ui, |ui| {
            ui.set_width((wide - 40.0).max(0.0));
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                let side = crate::component::avatar::AvatarSize::L.side(theme);
                let (face, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
                avatar::paint(ui.painter(), theme, face, &message.sender, &message.address);
                // The answers at the right; under the text when the card is too narrow for both.
                let narrow = ui.available_width() < 520.0;
                let mut text = |ui: &mut Ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            ui.add(egui::Label::new(RichText::new(&message.sender).font(f.strong.id()).color(p.ink)).truncate());
                            if mailbox.accounts().len() > 1 {
                                let index = mailbox.account(message.account).map_or(0, |account| u64::try_from(account.index).unwrap_or(0));
                                let _ = crate::component::account_bar(ui, theme, index);
                            }
                            ui.add(egui::Label::new(theme.fonts.mono_s.rich(&message.address).color(p.muted)).truncate());
                        });
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 10.0;
                            let subject = if message.subject.is_empty() { tr!("(no subject)") } else { message.subject.as_str() };
                            // The subject opens the message as a preview in the reader, which asks the same and more; on a
                            // card answered and leaving it is plain text.
                            let sense = if live { Sense::click() } else { Sense::hover() };
                            let opened = ui.add(egui::Label::new(RichText::new(subject).font(f.body.id()).color(p.ink)).truncate().sense(sense));
                            if live {
                                let spoken = mailbox.spoken(message);
                                opened.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, true, &spoken));
                            }
                            if opened.clicked() {
                                *open = true;
                            }
                            if asker.more > 0 {
                                ui.label(theme.fonts.mono_s.rich(&tr!("and {count} more", count = asker.more)).color(p.muted));
                            }
                            ui.label(theme.fonts.mono_s.rich(&mailbox.row_date(message)).color(p.muted));
                        });
                        if !message.preview.trim().is_empty() {
                            ui.add(egui::Label::new(RichText::new(&message.preview).font(f.caption.id()).color(p.muted)).truncate());
                        }
                        if current {
                            why_line(ui, theme, &tr!("first mail from this address  ·  held back since {when}", when = mailbox.row_date(message)));
                        }
                    });
                };
                // Let in, then Keep out, read from the left; laid from the right, Keep out comes first.
                let mut answers = |ui: &mut Ui, from_right: bool| {
                    ui.spacing_mut().item_spacing.x = s.s2;
                    let tone = if current { crate::component::button::Tone::Primary } else { crate::component::button::Tone::Secondary };
                    for keep_out in if from_right { [true, false] } else { [false, true] } {
                        let (name, act) =
                            if keep_out { (tr!("Keep out"), Act::Block) } else { (tr!("Let in"), Act::Accept { group: None, open: false }) };
                        let shown = if keep_out { button::TextButton::new(name).secondary() } else { button::TextButton::new(name).tone(tone) };
                        if shown.show(ui, theme, keymap).clicked() {
                            answer = Some(act);
                        }
                    }
                };
                if narrow {
                    ui.vertical(|ui| {
                        text(ui);
                        ui.add_space(s.s2);
                        ui.horizontal(|ui| answers(ui, false));
                    });
                } else {
                    let answers_wide = 180.0;
                    ui.allocate_ui(vec2((ui.available_width() - answers_wide - 16.0).max(1.0), 0.0), |ui| text(ui));
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| answers(ui, true));
                }
            });
        });
        let rect = shown.response.rect;
        let edge = edge.unwrap_or(if current { p.cyan_edge } else { p.line });
        let card = vec![
            egui::Shape::rect_filled(rect, 0.0, p.surface),
            egui::Shape::rect_stroke(rect, 0.0, egui::Stroke::new(1.0, edge), egui::StrokeKind::Inside),
        ];
        ui.painter().set(slot, egui::Shape::Vec(card));
        rect.height()
    });
    if opened && live {
        ui.data_mut(|data| data.insert_temp(Id::unique("new-sender-opened"), message.id));
    }
    (answer, inner.inner)
}

/// The line that says why a message waits, as the design's why-line: `WHY HERE` in the accent and `text`, on the panel
/// with its edge.
fn why_line(ui: &mut Ui, theme: &Theme, text: &str) {
    let (p, f) = (&theme.palette, &theme.fonts);
    let slot = ui.painter().add(egui::Shape::Noop);
    let shown = Frame::new().inner_margin(Margin { left: 12, right: 12, top: 8, bottom: 8 }).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            crate::component::label(ui, f.label, tr!("WHY HERE"), p.cyan_text);
            ui.add(egui::Label::new(f.mono_s.rich(text).color(p.muted)).truncate());
        });
    });
    let rect = shown.response.rect;
    ui.painter().set(
        slot,
        egui::Shape::Vec(vec![
            egui::Shape::rect_filled(rect, 0.0, p.panel),
            egui::Shape::rect_stroke(rect, 0.0, egui::Stroke::new(1.0, p.panel_line), egui::StrokeKind::Inside),
        ]),
    );
}

/// What the list laid out in a frame: from which line, how many.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Built {
    /// The first line built.
    pub first: usize,
    /// How many lines were built from it.
    pub count: usize,
}

/// How many rows Page Down and Page Up move: the rows wholly in sight in the last frame, less one, so the row at
/// the edge stays in sight; at least one.
pub(super) fn page(ctx: &egui::Context) -> usize {
    ctx.data(|data| data.get_temp::<usize>(page_id())).unwrap_or(1).saturating_sub(1).max(1)
}

/// The messages of the list's rows in the order they stand in the last frame that drew the list: the order its keys
/// move in.
pub(super) fn order(ctx: &egui::Context) -> std::sync::Arc<[u64]> {
    ctx.data(|data| data.get_temp::<std::sync::Arc<[u64]>>(order_id())).unwrap_or_else(|| std::sync::Arc::from(Vec::new()))
}

/// Where [`order`] is kept.
fn order_id() -> Id {
    Id::unique("list-order")
}

/// Where [`page`] is kept.
fn page_id() -> Id {
    Id::unique("list-page")
}

/// The message whose row has the keyboard's focus: the row that had it when the list was last drawn, if it still
/// has it.
pub(super) fn focused_row(ctx: &egui::Context) -> Option<u64> {
    let id = ctx.data(|data| data.get_temp::<u64>(focused_row_id()))?;
    ctx.memory(|memory| memory.has_focus(row_id(id))).then_some(id)
}

/// Where [`focused_row`] is kept.
fn focused_row_id() -> Id {
    Id::unique("list-focused-row")
}

/// The list's viewport on the screen in the last frame that drew one.
#[cfg(test)]
pub(super) fn view_rect(ctx: &egui::Context) -> Option<Rect> {
    ctx.data(|data| data.get_temp::<Rect>(Id::unique("list-view-rect")))
}

/// What the list built in the last frame that drew one.
#[cfg(test)]
pub(super) fn built(ctx: &egui::Context) -> Option<Built> {
    ctx.data(|data| data.get_temp::<Built>(built_id()))
}

/// Where [`built`] is kept.
fn built_id() -> Id {
    Id::unique("list-built")
}

/// The message whose row was the first in sight in the last frame that drew the list with one in sight.
#[cfg(test)]
pub(super) fn first_in_sight(ctx: &egui::Context) -> Option<u64> {
    ctx.data(|data| data.get_temp::<u64>(Id::unique("list-first-in-sight")))
}

/// Where whether `group`'s card shows all its rows is kept.
pub(super) fn group_open_id(group: &str) -> Id {
    Id::unique("list-group").with(group)
}

/// Where the rows a frame built are kept for [`rows_built`].
fn rows_built_id() -> Id {
    Id::unique("list-rows-built")
}

/// How many lines (rows, heads, the screener card) the list laid out in the last frame that drew rows, taken out so
/// the next read sees only a later frame's: `None` when no frame drew rows since. For the frame bench (`ui_bench`).
#[must_use]
pub fn rows_built(ctx: &egui::Context) -> Option<usize> {
    ctx.data_mut(|data| data.remove_temp::<usize>(rows_built_id()))
}

/// How far `left` has gone in this frame: Snooze fades over `fast` (the design's M6 at the reduced level); the other
/// acts close the row over `row_collapse` on the design's curve, fading as it closes (M5). Repaints are asked for until
/// it has gone; with motion off it is gone at once.
fn leaving(ctx: &egui::Context, theme: &Theme, left: &Leaving) -> Gone {
    let id = Id::unique("row-leaving").with(left.row.id);
    if matches!(left.act, Act::Snooze(_)) {
        let faded = motion::elapsed(ctx, id, theme.motion.fast);
        Gone { closed: if faded >= 1.0 { 1.0 } else { 0.0 }, faded }
    } else {
        let share = motion::elapsed(ctx, id, theme.motion.row_collapse);
        // The presentation's `.row`: height on the design's curve, opacity on CSS's default `ease`.
        Gone { closed: motion::ease(share), faded: motion::css_ease(share) }
    }
}

/// "Other": the messages of the list no group of `groups` holds, kept for the list of rows, its place, search and
/// `view`, so a frame copies nothing.
fn other_of(ctx: &egui::Context, mailbox: &Mailbox, view: InboxView, groups: &[Group]) -> std::sync::Arc<Group> {
    type Kept = ((u64, Option<Place>, String, InboxView, usize), std::sync::Arc<Group>);
    let key = (mailbox.rows_serial(), Some(mailbox.place()), mailbox.query().to_owned(), view, groups.len());
    let id = Id::unique("list-rest");
    if let Some((_, other)) = ctx.data(|data| data.get_temp::<Kept>(id)).filter(|(kept, _)| *kept == key) {
        return other;
    }
    let held: HashSet<u64> = groups.iter().flat_map(|group| group.messages.iter().copied()).collect();
    let messages = mailbox.messages().iter().map(|message| message.id).filter(|id| !held.contains(id)).collect();
    let other = std::sync::Arc::new(Group { icon: IconId::Inbox, name: tr!("Other").to_owned(), messages });
    ctx.data_mut(|data| data.insert_temp::<Kept>(id, (key, other.clone())));
    other
}

/// The rows, or what to say when there are none. Returns the message clicked this frame.
pub(super) fn rows(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox) -> Asked {
    let mut asked = drawn_rows(ui, theme, keymap, mailbox);
    // After every row of every list path: the picker stands under its message's row only if one was drawn.
    if let Some(snoozed) = snooze_picker(ui.ctx(), theme, mailbox) {
        asked.acted = asked.acted.or(Some(snoozed));
    }
    asked
}

/// [`rows`] before the row's time picker.
fn drawn_rows(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox) -> Asked {
    // The line between the list's head and its rows, in the panel's line colour; the rows start under it.
    let room = ui.max_rect();
    super::paint::hairline(ui.painter(), room.left_top(), room.right_top(), theme.palette.panel_line);
    ui.add_space(1.0);
    #[cfg(test)]
    ui.data_mut(|data| data.remove::<Rect>(Id::unique("list-view-rect")));
    if mailbox.place() != Place::Senders {
        forget_answered(ui.ctx());
    }
    let outbox = mailbox.place() == Place::Outbox && mailbox.query().is_empty();
    let none = if outbox { mailbox.outbox().is_empty() } else { mailbox.messages().is_empty() };
    // A list with nothing in it, the inbox with senders waiting aside: its empty state.
    let waits = mailbox.place() == Place::View(Role::Inbox) && mailbox.query().is_empty() && !mailbox.waiting().is_empty();
    // The last sender answered in New senders: their card leaves before the queue's empty state shows.
    let answering =
        mailbox.place() == Place::Senders && ui.data(|data| data.get_temp::<Vec<Answered>>(answered_id())).is_some_and(|left| !left.is_empty());
    if none && !waits && mailbox.left_row().is_none() && !answering {
        ui.data_mut(|data| data.insert_temp::<std::sync::Arc<[u64]>>(order_id(), std::sync::Arc::from(Vec::new())));
        empty_list(ui, theme, keymap, mailbox);
        return Asked { out: true, ..Asked::default() };
    }
    let inbox = mailbox.place() == Place::View(Role::Inbox) && mailbox.query().is_empty();
    let wait = inbox && !mailbox.waiting().is_empty();
    let groups = mailbox.groups();
    let view = if groups.is_empty() { InboxView::List } else { mailbox.inbox_view() };
    let rows = Rows {
        theme,
        keymap,
        mailbox,
        // Whose each row is shows in every view of more than one account, as the design file's rows show it.
        account_marker: mailbox.narrowed().is_none() && mailbox.accounts().len() > 1,
        clicked: Cell::new(None),
        acted: Cell::new(None),
        presets: Cell::new(false),
        done_all: Cell::new(None),
        review: Cell::new(false),
        outbox: Cell::new(None),
        gained: Cell::new(None),
    };
    // "Other" stands over the messages no group holds: its "done all" marks those, built or not. Worked out once for a
    // list of rows and its view.
    let other = other_of(ui.ctx(), mailbox, view, &groups);
    let open = |group: &Group| ui.data(|data| data.get_temp::<bool>(group_open_id(&group.name))).unwrap_or(false);
    // How far the row that left has gone (with motion off, at once); once gone it is let go.
    let gone = mailbox.left_row().map(|left| leaving(ui.ctx(), theme, left));
    let went = gone.is_none_or(|gone| gone.faded >= 1.0);
    let listing = if outbox { sends(theme, mailbox) } else { lines(theme, mailbox, view, &groups, &other, wait, &open, gone.filter(|_| !went)) };
    // The drawn order, kept as it was while it is the same: a frame at rest allocates nothing for it.
    let drawn = || listing.lines.iter().filter_map(|line| if let Line::Mail(message) = line { Some(message.id) } else { None });
    let same = ui.data(|data| data.get_temp::<std::sync::Arc<[u64]>>(order_id())).is_some_and(|kept| kept.iter().copied().eq(drawn()));
    if !same {
        let order: std::sync::Arc<[u64]> = drawn().collect();
        ui.data_mut(|data| data.insert_temp(order_id(), order));
    }
    let mut screener_mode = None;
    let built = if outbox {
        show_lines(ui, &rows, &listing, "outbox-list")
    } else if mailbox.place() == Place::Senders && mailbox.query().is_empty() {
        let (built, mode) = senders(ui, &rows);
        screener_mode = mode;
        built
    } else {
        show_lines(ui, &rows, &listing, "mail-list")
    };
    ui.ctx().data_mut(|data| {
        data.insert_temp(rows_built_id(), built.count);
        data.insert_temp(built_id(), built);
    });
    Asked {
        clicked: rows.clicked.get(),
        acted: rows.acted.get(),
        out: mailbox.left_row().is_some() && went,
        review: rows.review.get(),
        done_all: rows.done_all.take(),
        outbox: rows.outbox.get(),
        screener: screener_mode,
        gained: rows.gained.get(),
        presets: rows.presets.get(),
    }
}
