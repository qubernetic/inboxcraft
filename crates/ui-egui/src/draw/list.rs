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
use egui::{
    Align, Color32, Frame, Id, Key, Layout, Margin, Popup, Rect, RichText, Role as WidgetRole, ScrollArea, Sense, Shape, Stroke,
    StrokeKind, Ui, UiBuilder, WidgetInfo, pos2, vec2,
};
use inboxcraft_model::Role;

use super::paint::line;
use inboxcraft_engine::ScreenerMode;

use crate::component::hover::Action;
use crate::component::mail_row::{Mark, MailRow, mail_row};
use crate::component::tag::TagKind;
use crate::component::pill::pill;
use crate::component::{avatar, badge, button, field, margin, popup, tooltip};
use crate::iconset::IconId;
use crate::keymap::{self, Keymap};
use crate::mailbox::{Act, Group, GroupKind, InboxView, Leaving, Mailbox, MessageRow, OutboxRow, Pile, Place, SendState, Waiting};
use crate::motion;
use crate::theme::Theme;
use crate::tr;

/// The room between a card's edges and its rows, so a selected or hovered row is a rounded band inside the card.
pub(super) const ROW_INSET: f32 = 6.0;

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

/// The widest the list is while a message is open beside it (its default width, with room to spare): the view switch
/// shows only over a wider list.
const SWITCH_ROOM: f32 = 520.0;

/// How far the pills reach out of the header's margin, to the left edge of the cards under them, and how much nearer
/// the next card they are than the header's margin keeps them.
const PILL_PULL: i8 = 4;

/// The view switch of the unified inbox, as the design file words and orders it: "Grouped" is the one list under
/// headings ([`InboxView::List`]), beside the simple list and the cards.
fn view_switch() -> [(InboxView, &'static str); 3] {
    [(InboxView::List, tr!("Grouped")), (InboxView::Simple, InboxView::Simple.text()), (InboxView::Cards, InboxView::Cards.text())]
}

/// The pills under the header: the piles in "Later"; elsewhere "Show done" and the park pile, after the view switch
/// over a wide unified inbox with groups to show. Not while searching.
fn pills(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, asked: &mut Header) {
    if !mailbox.query().is_empty() || matches!(mailbox.place(), Place::Senders | Place::Outbox) {
        return;
    }
    ui.add_space(theme.spacing.s3);
    let reach = Margin { left: -PILL_PULL, right: -PILL_PULL, top: 0, bottom: -PILL_PULL };
    Frame::new().outer_margin(reach).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(theme.spacing.s2, theme.spacing.s1);
            if let Place::Later(shown) = mailbox.place() {
                for pile in Pile::ALL {
                    let text = tr!("{pile} ({count})", pile = pile.text(), count = mailbox.pile_count(pile));
                    if pill(ui, theme, &text, pile == shown).clicked() {
                        asked.open = Some(Place::Later(pile));
                    }
                }
                return;
            }
            if mailbox.place() == Place::View(Role::Inbox) && mailbox.can_group() && ui.available_width() > SWITCH_ROOM {
                for (view, text) in view_switch() {
                    if pill(ui, theme, text, mailbox.inbox_view() == view).clicked() {
                        asked.view = Some(view);
                    }
                }
            }
            if pill(ui, theme, tr!("Show done"), mailbox.show_done()).clicked() {
                asked.show_done = Some(!mailbox.show_done());
            }
            let parked = mailbox.parked();
            if parked > 0 && pill(ui, theme, &tr!("Parked ({count})", count = parked), false).clicked() {
                asked.open = Some(Place::Later(Pile::Parked));
            }
        });
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
            if (response.has_focus() || response.lost_focus()) && ui.input(|input| input.key_pressed(Key::Escape)) {
                search.clear();
                changed = Some(String::new());
            }
        });
    });
    let mut asked = Header { searched: changed, ..Header::default() };
    pills(ui, theme, mailbox, &mut asked);
    asked
}

fn empty(ui: &mut Ui, theme: &Theme, title: &str, detail: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(theme.spacing.s6 * 3.0);
        ui.label(RichText::new(title).font(theme.fonts.heading.id()).color(theme.palette.ink));
        ui.add_space(theme.spacing.s1);
        ui.label(RichText::new(detail).font(theme.fonts.body.id()).color(theme.palette.muted));
    });
}

/// The two lines of a message row: the sender's with the date, and the subject's with the preview (both `caption`),
/// each as high as its tallest style's line.
fn row_lines(theme: &Theme) -> (f32, f32) {
    let f = &theme.fonts;
    let first = [f.body, f.strong, f.mono_s].map(|font| font.line).into_iter().fold(0.0, f32::max);
    (first, f.caption.line)
}

/// Where a row's first line starts below its top: the two lines and their gap centred in the fixed row height.
fn lines_top(theme: &Theme) -> f32 {
    let (first, second) = row_lines(theme);
    ((row_height(theme) - first - line_gap(theme) - second) / 2.0).max(0.0)
}

/// The side of a row's avatar: a little under its two lines.
fn face_side(theme: &Theme) -> f32 {
    let (first, second) = row_lines(theme);
    (first + second - theme.spacing.s1 / 2.0).max(1.0)
}

/// The gap between a row's two lines.
fn line_gap(theme: &Theme) -> f32 {
    theme.spacing.s1 / 2.0
}

/// The height of a message row: the design's fixed mail row at the current text scale.
pub(super) fn row_height(theme: &Theme) -> f32 {
    theme.boxes().mail_row
}

/// The hover actions of `message`: Done (or "Not done" for a done one), mark read or unread, Delete.
fn actions(message: &MessageRow) -> [Action<'static>; 4] {
    #[cfg(test)]
    super::list_tests::ACTIONS_BUILT.with(|built| built.set(built.get().saturating_add(1)));
    // An open envelope opens an unread message; a closed one closes a read one again.
    let (read_icon, read) = if message.seen { (IconId::Mail, tr!("Mark as unread")) } else { (IconId::MarkRead, tr!("Mark as read")) };
    let done = if message.done { tr!("Not done") } else { tr!("Done") };
    [
        Action { icon: IconId::Done, name: done, action: Some(keymap::DONE), unavailable: None },
        Action { icon: read_icon, name: read, action: Some(keymap::READ), unavailable: None },
        Action { icon: IconId::Delete, name: tr!("Delete"), action: Some(keymap::DELETE), unavailable: None },
        // Its key shows dashed, as the sheet's unavailable Snooze; an unavailable action offers no key and runs nothing.
        Action { icon: IconId::Snooze, name: tr!("Snooze"), action: Some(keymap::SNOOZE), unavailable: Some(tr!("Open the message to snooze it")) },
    ]
}

/// What the hover action at `index` of [`actions`] does to `message`.
fn act(message: &MessageRow, index: usize) -> Option<Act> {
    match index {
        0 if message.done => Some(Act::Undone),
        0 => Some(Act::Done),
        1 if message.seen => Some(Act::Unread),
        1 => Some(Act::Read),
        2 => Some(Act::Delete),
        _ => None,
    }
}

/// The width of the group menu, in body text sizes.
const MENU_WIDTH: f32 = 18.0;

/// The group menu of a sender's mail: a choice for each group, the one it is in chosen, and "Let InboxCraft sort it";
/// the act chosen. A line under them says that all the sender's mail moves, the kept and the future.
pub(super) fn group_menu(ui: &mut Ui, theme: &Theme, current: Option<GroupKind>, address: &str) -> Option<Act> {
    let mut chosen = None;
    crate::component::label(ui, theme.fonts.label, tr!("Group"), theme.palette.muted);
    for kind in GroupKind::ALL {
        if popup::choice(ui, theme, kind.text(), current == Some(kind)).clicked() {
            chosen = Some(Act::Group(Some(kind)));
        }
    }
    if popup::choice(ui, theme, tr!("Let InboxCraft sort it"), false).clicked() {
        chosen = Some(Act::Group(None));
    }
    ui.add_space(theme.spacing.s1);
    let note = tr!("Moves all mail from {address}, kept and future.", address = address);
    ui.add(egui::Label::new(RichText::new(note).font(theme.fonts.caption.id()).color(theme.palette.muted)).wrap());
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
}

/// One message row across `ui`'s width, the mail row of the design; a click selects it.
fn row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, message: &MessageRow, account_marker: bool, set: (usize, usize)) -> Drawn {
    let selected = mailbox.selected() == Some(message.id);
    let keyed = selected && mailbox.keyed();
    let spoken = mailbox.spoken(message);
    let subject = if message.subject.is_empty() { tr!("(no subject)") } else { message.subject.as_str() };
    let line = format!("{subject}{}", after_subject(&message.preview));
    let time = mailbox.row_date(message);
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
    let marks: Vec<Mark> = [(message.pinned, Mark::Pin), (message.flagged, Mark::Flag), (message.done, Mark::Done)]
        .into_iter()
        .filter_map(|(shown, mark)| shown.then_some(mark))
        .collect();
    let account = account_marker.then(|| mailbox.account(message.account).map_or(0, |account| u64::try_from(account.index).unwrap_or(0)));
    let shown = MailRow {
        id: row_id(message.id),
        spoken: &spoken,
        sender: &message.sender,
        line: &line,
        time: &time,
        unread: !message.seen,
        current: selected,
        last: false,
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
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_position_in_set(set.0);
        node.set_size_of_set(set.1);
    });
    let mut acted = drawn.acted.and_then(|index| act(message, index));
    if message.group.is_some() {
        let width = theme.fonts.body.size * MENU_WIDTH;
        Popup::context_menu(response).frame(Frame::NONE).show(|ui| {
            popup::panel(ui, theme, width, |ui| {
                if let Some(chosen) = group_menu(ui, theme, message.group, &message.address) {
                    acted = Some(chosen);
                    ui.close();
                }
            });
        });
    }
    Drawn { clicked: response.clicked(), acted, focused: response.has_focus() }
}

/// The id of message `id`'s row: its focus and its context menu, wherever it is built.
pub(super) fn row_id(id: u64) -> Id {
    Id::unique("list-row").with(id)
}

/// The row of `message`, which has left the list, in the line `ui` gives it: drawn whole from the line's top and cut
/// to the line, as its gap closes; it takes no pointer, focus or act.
fn leaving_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, message: &MessageRow, account_marker: bool) {
    let line = ui.max_rect();
    let whole = Rect::from_min_size(line.min, vec2(line.width(), row_height(theme)));
    let subject = if message.subject.is_empty() { tr!("(no subject)") } else { message.subject.as_str() };
    let words = format!("{subject}{}", after_subject(&message.preview));
    let time = mailbox.row_date(message);
    let account = account_marker.then(|| mailbox.account(message.account).map_or(0, |account| u64::try_from(account.index).unwrap_or(0)));
    let shown = MailRow {
        id: Id::unique("list-leaving").with(message.id),
        spoken: "",
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
    let mut face = ui.new_child(UiBuilder::new().max_rect(whole));
    face.shrink_clip_rect(line);
    let _ = mail_row(&mut face, theme, keymap, &shown, false, &[], Some(crate::component::Shown::Rest));
}

/// What follows a row's subject: a dash and the preview, as the design's line two, or nothing without one.
pub(super) fn after_subject(preview: &str) -> String {
    if preview.trim().is_empty() { String::new() } else { format!(" — {preview}") }
}

/// A square card across `ui`'s width with `add` drawn inside; `edge` rings it with the accent's edge.
fn sheet<R>(ui: &mut Ui, theme: &Theme, edge: bool, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = &theme.palette;
    let slot = ui.painter().add(Shape::Noop);
    let inner = Frame::new().inner_margin(margin(ROW_INSET, ROW_INSET)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
    let rect = inner.response.rect;
    let colour = if edge { p.cyan_edge } else { p.line };
    let shapes = vec![Shape::rect_filled(rect, 0.0, p.surface), Shape::rect_stroke(rect, 0.0, Stroke::new(1.0, colour), StrokeKind::Inside)];
    ui.painter().set(slot, Shape::Vec(shapes));
    inner.inner
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

/// The state of a send as its row shows it: the icon and the colour of its words.
fn standing_look(theme: &Theme, state: &SendState) -> (IconId, Color32) {
    match state {
        SendState::Waiting(_) => (IconId::Snooze, theme.palette.muted),
        SendState::Sending => (IconId::Send, theme.palette.muted),
        SendState::Refused { .. } => (IconId::Failed, theme.palette.minus),
    }
}

/// One send of the outbox across `ui`'s width: the first recipient's avatar, whom it goes to and the subject (with the
/// server's reply for a refused one), where it stands, and its buttons; the button pressed.
fn outbox_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, row: &OutboxRow, set: (usize, usize)) -> Option<Outbox> {
    let s = &theme.spacing;
    let p = &theme.palette;
    let f = &theme.fonts;
    let mut asked = None;
    let (first, second) = row_lines(theme);
    let height = row_height(theme);
    let refused = match &row.state {
        SendState::Refused { reply, .. } => Some(reply.as_str()),
        _ => None,
    };
    let tall = height + if refused.is_some() { f.caption.line + s.s1 } else { 0.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), tall), Sense::hover());
    let spoken = mailbox.spoken_outbox(row);
    response.widget_info(|| WidgetInfo::labeled(WidgetRole::ListItem, true, &spoken));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_position_in_set(set.0);
        node.set_size_of_set(set.1);
    });
    let side = face_side(theme);
    // A send has no unread dot, so its avatar stands further out than a message's.
    let indent = s.s3 + 2.0;
    let face = Rect::from_min_size(pos2(rect.left() + indent, rect.top() + (height - side) / 2.0), vec2(side, side));
    let whom = mailbox.recipients(row);
    avatar::paint(ui.painter(), theme, face, row.to.first().map_or(whom.as_str(), String::as_str), &row.address);
    let left = rect.left() + indent + side + s.s3;
    let subject = if row.subject.is_empty() { tr!("(no subject)") } else { row.subject.as_str() };
    let subject_label = row.subject.clone();
    // The buttons and where it stands, from the right; the text takes what is left.
    let mut right = rect.right() - s.s3;
    let mut controls = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_max(pos2(left, rect.top()), pos2(right, rect.top() + height)))
            .layout(Layout::right_to_left(Align::Center)),
    );
    controls.spacing_mut().item_spacing.x = s.s1;
    let mut press = |ui: &mut Ui, name: &str, spoken: String, primary: bool, pressed: Outbox| {
        let button = if primary { button::TextButton::new(name).primary() } else { button::TextButton::new(name).secondary() };
        let response = button.show(ui, theme, keymap);
        ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(spoken.as_str()));
        if response.clicked() {
            asked = Some(pressed);
        }
    };
    match (&row.state, mailbox.seconds_left(row.draft)) {
        (SendState::Waiting(_), Some(_)) => {
            press(&mut controls, tr!("Undo"), tr!("Undo the send of “{subject}”", subject = subject_label), false, Outbox::Unsend);
        }
        (SendState::Waiting(_), None) => {
            press(&mut controls, tr!("Edit"), tr!("Take “{subject}” back and edit it", subject = subject_label), false, Outbox::Unsend);
        }
        (SendState::Refused { .. }, _) => {
            press(&mut controls, tr!("Send again"), tr!("Send “{subject}” again", subject = subject_label), true, Outbox::Retry);
            // An answer to an invitation changes only from the invitation.
            if !row.invite_answer {
                press(&mut controls, tr!("Edit"), tr!("Edit “{subject}”", subject = subject_label), false, Outbox::Edit);
            }
        }
        (SendState::Sending, _) => {}
    }
    let (icon, ink) = standing_look(theme, &row.state);
    let standing = line(&controls, &[(&mailbox.standing(row), f.mono_s, ink)], f32::INFINITY);
    let mark = f.mono_s.line;
    let (chip, _) = controls.allocate_exact_size(vec2(standing.size().x + mark + s.s1 + s.s2 * 2.0, mark + s.s1 * 2.0), Sense::hover());
    controls.painter().rect_filled(chip, 0.0, p.panel);
    let at = Rect::from_min_size(pos2(chip.left() + s.s2, chip.center().y - mark / 2.0), vec2(mark, mark));
    crate::iconset::paint(controls.painter(), at, icon, ink);
    controls.painter().galley(pos2(at.right() + s.s1, chip.center().y - standing.size().y / 2.0), standing, ink);
    right = right.min(controls.min_rect().left() - s.s3);
    let top = rect.top() + lines_top(theme);
    let to = tr!("To {whom}", whom = whom);
    let to = line(ui, &[(&to, f.body, p.ink)], (right - left).max(1.0));
    ui.painter().galley(pos2(left, top + (first - to.size().y) / 2.0), to, p.ink);
    let second_top = top + first + line_gap(theme);
    let subject = line(ui, &[(subject, f.caption, p.ink)], (right - left).max(1.0));
    ui.painter().galley(pos2(left, second_top + (second - subject.size().y) / 2.0), subject, p.ink);
    if let Some(reply) = refused {
        let reply = line(ui, &[(reply, f.caption, p.minus)], (rect.right() - s.s3 - left).max(1.0));
        ui.painter().galley(pos2(left, second_top + second + s.s1), reply, p.minus);
    }
    asked
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
    /// "Done all" pressed this frame: the group's messages.
    done_all: Cell<Option<Vec<u64>>>,
    /// Review pressed on the screener card.
    review: Cell<bool>,
    /// A button of a send in the outbox pressed, and its draft.
    outbox: Cell<Option<(u64, Outbox)>>,
}

impl Rows<'_> {
    /// Draws `message`'s row across `ui`, the `rank`th (from 0) of the list's `size` rows.
    fn mail(&self, ui: &mut Ui, message: &MessageRow, rank: usize, size: usize) {
        let drawn = row(ui, self.theme, self.keymap, self.mailbox, message, self.account_marker, (rank, size));
        if drawn.focused {
            ui.data_mut(|data| data.insert_temp(focused_row_id(), message.id));
        }
        if drawn.clicked {
            self.clicked.set(Some(message.id));
        }
        if let Some(act) = drawn.acted {
            self.acted.set(Some((message.id, act)));
        }
    }
}

/// The screener card: the waiting senders' avatars, how many wait, and Review; whether Review was pressed.
fn screener(ui: &mut Ui, theme: &Theme, keymap: &Keymap, waiting: &[Waiting]) -> bool {
    let s = &theme.spacing;
    let p = &theme.palette;
    let mut review = false;
    sheet(ui, theme, true, |ui| {
        Frame::new().inner_margin(margin(s.s2, s.s1)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let side = theme.fonts.strong.line + s.s1 + theme.fonts.caption.line;
                let shown = waiting.iter().take(3);
                let count = u16::try_from(shown.len()).unwrap_or(3);
                let overlap = side * 0.3;
                let width = f32::from(count) * side - f32::from(count.saturating_sub(1)) * overlap;
                let (faces, _) = ui.allocate_exact_size(vec2(width.max(1.0), side), Sense::hover());
                for (index, waiting) in shown.enumerate() {
                    let x = faces.left() + (side - overlap) * index as f32;
                    let face = Rect::from_min_size(pos2(x, faces.top()), vec2(side, side));
                    ui.painter().circle_filled(face.center(), side / 2.0 + 1.5, p.surface);
                    avatar::paint(ui.painter(), theme, face, &waiting.name, &waiting.address);
                }
                ui.add_space(s.s2);
                // Review first, at the right end; the two lines take what is left, cut rather than running under it.
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    review = button::text(ui, theme, keymap, tr!("Review"), Some("screener.review"), true).clicked();
                    ui.add_space(s.s2);
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let count = waiting.len();
                        let title = if count == 1 { tr!("1 new sender").to_owned() } else { tr!("{count} new senders", count = count) };
                        ui.add(egui::Label::new(RichText::new(title).font(theme.fonts.strong.id()).color(p.ink)).truncate());
                        let note = RichText::new(tr!("Held back until you let them in")).font(theme.fonts.caption.id()).color(p.muted);
                        ui.add(egui::Label::new(note).truncate());
                    });
                });
            });
        });
    });
    review
}

/// One line of a list. A list is its lines one under another, each as high as [`line_height`] says for its kind, so
/// where every line stands is known without building one, and a frame builds only the lines in sight.
#[derive(Debug, Clone, Copy)]
pub(super) enum Line<'a> {
    /// A group's head: its badge, name and unread count, and on a card "Done all".
    Head(&'a Group, HeadState),
    /// A message's row.
    Mail(&'a MessageRow),
    /// The row of a message that has just left the list, and how far its gap has closed (0 to 1): its height closes
    /// over the `row_collapse` token.
    Leaving(&'a MessageRow, f32),
    /// "Show all (n)" or "Show fewer" under a group's first rows on its card.
    More(&'a Group, HeadState),
    /// The screener card, while senders wait.
    Card(&'a [Waiting]),
    /// A send of the outbox.
    Send(&'a OutboxRow),
}

/// How a group's head and its "Show all" stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HeadState {
    /// Its unread messages.
    pub unread: u64,
    /// Its messages in the list.
    pub count: usize,
    /// It shows all its rows: on a card, after "Show all"; in the grouped list, always.
    pub open: bool,
    /// It heads a card of the cards view (a larger name, and "Done all"), not a part of the grouped list.
    pub card: bool,
}

/// `points` rounded to a whole even number of points, as [`Theme::size`] rounds: whole pixels at 1×, 1.5× and 2×.
fn even(points: f32) -> f32 {
    if points.is_finite() { (points.max(0.0) / 2.0).ceil() * 2.0 } else { 0.0 }
}

/// The height of `line`, fixed for its kind at the text scale: a group's head and "Show all" the group head's size,
/// a row the mail row's, a send the mail row's (a refused one a line more), the screener card its two lines and its
/// margins.
pub(super) fn line_height(theme: &Theme, line: &Line<'_>) -> f32 {
    let boxes = theme.boxes();
    let (f, s) = (&theme.fonts, &theme.spacing);
    match line {
        Line::Head(..) | Line::More(..) => boxes.group_head,
        Line::Mail(_) => boxes.mail_row,
        Line::Leaving(_, gone) => even(boxes.mail_row * (1.0 - gone.clamp(0.0, 1.0))),
        Line::Send(row) if matches!(row.state, SendState::Refused { .. }) => boxes.mail_row + even(f.caption.line + s.s1),
        Line::Send(_) => boxes.mail_row,
        Line::Card(_) => {
            let faces = f.strong.line + s.s1 + f.caption.line;
            even(faces.max(boxes.button) + s.s1 * 2.0 + ROW_INSET * 2.0)
        }
    }
}

/// A list laid out without building it: its lines, the top and bottom of each down the content (from 0), the cards
/// they stand on, and the whole height. A line's bottom is the next line's top, the same number, so two lines never
/// overlap or leave a hairline between them, and the last line's bottom (the last card's, on cards) is the height.
pub(super) struct Listing<'a> {
    /// The lines, in order.
    pub lines: Vec<Line<'a>>,
    tops: Vec<f32>,
    bottoms: Vec<f32>,
    /// How many [`Line::Mail`] and [`Line::Send`] lines stand before each line: its place in the list for AccessKit.
    ranks: Vec<usize>,
    /// The cards the lines stand on, top and bottom; none for a list on one card that scrolls inside it.
    pub sheets: Vec<(f32, f32)>,
    /// How far in from the content's sides a line on a card stands.
    inset: f32,
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
    /// The top of the card open now.
    sheet: Option<f64>,
}

impl<'a, 't> Stack<'a, 't> {
    fn new(theme: &'t Theme, inset: f32) -> Self {
        let listing =
            Listing { lines: Vec::new(), tops: Vec::new(), bottoms: Vec::new(), ranks: Vec::new(), sheets: Vec::new(), inset, height: 0.0, rows: 0 };
        Self { theme, listing, y: 0.0, sheet: None }
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

    /// The gap between two cards, after anything before.
    fn gap(&mut self) {
        if self.y > 0.0 {
            self.y += f64::from(self.theme.spacing.s2);
        }
    }

    /// Starts a card: the gap after the one above, and the card's inset.
    fn open(&mut self) {
        self.gap();
        self.sheet = Some(self.y);
        self.y += f64::from(ROW_INSET);
    }

    /// Ends the card open now with its inset.
    fn close(&mut self) {
        if let Some(top) = self.sheet.take() {
            self.y += f64::from(ROW_INSET);
            self.listing.sheets.push((top as f32, self.y as f32));
        }
    }

    fn finish(mut self) -> Listing<'a> {
        self.close();
        self.listing.height = self.y as f32;
        self.listing
    }
}

/// The rows of the list in its order as lines: each message's, and the row that has just left the list (`leaving`,
/// how far its gap has closed) at its old place.
fn listed<'m>(mailbox: &'m Mailbox, leaving: Option<(&'m Leaving, f32)>) -> impl Iterator<Item = (Line<'m>, Option<&'m str>)> {
    let ghost = move |at: usize| {
        leaving.filter(|(left, _)| left.at == at).map(|(left, gone)| (Line::Leaving(&left.row, gone), Some(left.group.as_deref().unwrap_or(""))))
    };
    let count = mailbox.messages().len();
    mailbox
        .messages()
        .iter()
        .enumerate()
        .flat_map(move |(at, message)| ghost(at).into_iter().chain(std::iter::once((Line::Mail(message), None))))
        .chain(leaving.filter(|(left, _)| left.at >= count).map(|(left, gone)| (Line::Leaving(&left.row, gone), Some(left.group.as_deref().unwrap_or("")))))
}

/// The rows of each of `groups` and those no group holds, in the list's order, the row that has just left among them
/// (`leaving`): one pass over the list.
fn sorted<'m>(mailbox: &'m Mailbox, groups: &[Group], leaving: Option<(&'m Leaving, f32)>) -> (Vec<Vec<Line<'m>>>, Vec<Line<'m>>) {
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
            }
        }
        if !held {
            rest.push(line);
        }
    }
    (members, rest)
}

/// The head of a group of `lines`: its messages' rows counted, the row that has left not.
fn head_state(lines: &[Line<'_>], open: bool, card: bool) -> HeadState {
    let messages = || lines.iter().filter_map(|line| if let Line::Mail(message) = line { Some(*message) } else { None });
    let unread = u64::try_from(messages().filter(|message| !message.seen).count()).unwrap_or(u64::MAX);
    HeadState { unread, count: messages().count(), open, card }
}

/// The whole list as lines, laid out: in the cards `view` the screener card while senders `wait`, a card for each of
/// `groups` (its head, its first rows or all of them as `open` says, and "Show all") and one for the rows no group
/// holds; in the grouped list the screener card and one card with each group's rows under its head and the rest
/// under `other`'s; in the simple list every row, on the one card around the scroll area. The row that has just left
/// the list stands at its old place while its gap closes, `gone` of the way (from 0 to 1; closed, it is left out).
#[expect(clippy::too_many_arguments, reason = "what the lines of a list are made of")]
pub(super) fn lines<'a>(
    theme: &Theme,
    mailbox: &'a Mailbox,
    view: InboxView,
    groups: &'a [Group],
    other: &'a Group,
    wait: bool,
    open: &dyn Fn(&Group) -> bool,
    gone: f32,
) -> Listing<'a> {
    let leaving = mailbox.left_row().filter(|_| gone < 1.0).map(|left| (left, gone.max(0.0)));
    if view == InboxView::Simple {
        let mut stack = Stack::new(theme, 0.0);
        for (line, _) in listed(mailbox, leaving) {
            stack.push(line);
        }
        return stack.finish();
    }
    let mut stack = Stack::new(theme, ROW_INSET);
    if wait {
        stack.push(Line::Card(mailbox.waiting()));
    }
    let (members, rest) = sorted(mailbox, groups, leaving);
    let parts = groups.iter().zip(&members).filter(|(_, lines)| lines.iter().any(|line| matches!(line, Line::Mail(_))));
    if view == InboxView::Cards {
        for (group, lines) in parts {
            let state = head_state(lines, open(group), true);
            stack.open();
            stack.push(Line::Head(group, state));
            let shown = if state.open { state.count } else { state.count.min(GROUP_ROWS) };
            let mut rows = 0;
            for line in lines {
                let row = matches!(line, Line::Mail(_));
                if row && rows == shown {
                    break;
                }
                rows += usize::from(row);
                stack.push(*line);
            }
            if state.count > GROUP_ROWS {
                stack.push(Line::More(group, state));
            }
            stack.close();
        }
        if rest.iter().any(|line| matches!(line, Line::Mail(_))) {
            stack.open();
            for line in rest {
                stack.push(line);
            }
            stack.close();
        }
        return stack.finish();
    }
    let rest = Some((other, &rest)).filter(|(_, rest)| rest.iter().any(|line| matches!(line, Line::Mail(_))));
    let mut started = false;
    for (group, lines) in parts.chain(rest) {
        if !started {
            stack.open();
            started = true;
        }
        stack.push(Line::Head(group, head_state(lines, true, false)));
        for line in lines {
            stack.push(*line);
        }
    }
    stack.finish()
}

/// The sends of the outbox as lines, on the one card around the scroll area.
fn sends<'a>(theme: &Theme, mailbox: &'a Mailbox) -> Listing<'a> {
    let mut stack = Stack::new(theme, 0.0);
    for row in mailbox.outbox() {
        stack.push(Line::Send(row));
    }
    stack.finish()
}

/// A group's head across `ui`: its badge, name and unread count, and on a card "Done all" at the right; whether "Done
/// all" was pressed.
fn head(ui: &mut Ui, theme: &Theme, keymap: &Keymap, group: &Group, state: HeadState) -> bool {
    let s = &theme.spacing;
    let p = &theme.palette;
    let font = if state.card { theme.fonts.heading } else { theme.fonts.strong };
    let side = if state.card { theme.fonts.heading.line * 1.4 } else { theme.fonts.strong.line * 1.2 };
    badge::group(ui, theme, group.icon, &group.name, side);
    ui.add_space(s.s1);
    let name = line(ui, &[(&group.name, font, p.ink)], f32::INFINITY);
    let count = if state.unread > 0 { badge::count_width(ui, theme, state.unread) + s.s2 } else { 0.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(name.size().x + count, side), Sense::hover());
    let spoken = if state.unread > 0 { tr!("{name}, {count} unread", name = group.name, count = state.unread) } else { group.name.clone() };
    response.widget_info(|| WidgetInfo::labeled(WidgetRole::Label, true, &spoken));
    ui.painter().galley(pos2(rect.left(), rect.center().y - name.size().y / 2.0), name, p.ink);
    if state.unread > 0 {
        badge::count(ui, theme, state.unread, rect.right(), rect.center().y);
    }
    let mut done_all = false;
    if state.card {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let pressed = button::text(ui, theme, keymap, tr!("Done all"), None, false);
            if pressed.gained_focus() {
                pressed.scroll_to_me(None);
            }
            done_all = pressed.clicked();
        });
    }
    done_all
}

/// "Show all (n)" or "Show fewer" across `ui`, as far in as the rows' text; whether it was pressed.
fn more(ui: &mut Ui, theme: &Theme, keymap: &Keymap, state: HeadState) -> bool {
    ui.add_space((crate::component::mail_row::TEXT_LEFT - theme.spacing.s4 - button::LINK_PAD).max(0.0));
    let label = if state.open { tr!("Show fewer").to_owned() } else { tr!("Show all ({count})", count = state.count) };
    let link = button::link(ui, theme, keymap, &label);
    if link.gained_focus() {
        link.scroll_to_me(None);
    }
    link.clicked()
}

/// What a line's ui is told apart by, so a row keeps its id (its focus, its context menu) wherever it is built.
fn salt(line: &Line<'_>) -> Id {
    match line {
        Line::Head(group, _) => Id::unique("list-head").with(&group.name),
        Line::Mail(message) => Id::unique("list-mail").with(message.id),
        Line::Leaving(message, _) => Id::unique("list-leaving-line").with(message.id),
        Line::More(group, _) => Id::unique("list-more").with(&group.name),
        Line::Card(_) => Id::unique("list-screener"),
        Line::Send(row) => Id::unique("list-send").with(row.draft),
    }
}

/// Draws `line` across `ui`, the line's own room.
fn draw_line(ui: &mut Ui, rows: &Rows<'_>, line: &Line<'_>, rank: usize, size: usize) {
    let (theme, keymap, mailbox) = (rows.theme, rows.keymap, rows.mailbox);
    match line {
        Line::Head(group, state) => {
            if head(ui, theme, keymap, group, *state) {
                let all = mailbox.messages().iter().filter(|message| group.messages.contains(&message.id)).map(|message| message.id);
                rows.done_all.set(Some(all.collect()));
            }
        }
        Line::Mail(message) => rows.mail(ui, message, rank, size),
        Line::Leaving(message, _) => leaving_row(ui, theme, keymap, mailbox, message, rows.account_marker),
        Line::More(group, state) => {
            if more(ui, theme, keymap, *state) {
                ui.data_mut(|data| data.insert_temp(group_open_id(&group.name), !state.open));
            }
        }
        Line::Card(waiting) => {
            if screener(ui, theme, keymap, waiting) {
                rows.review.set(true);
            }
        }
        Line::Send(row) => {
            if let Some(pressed) = outbox_row(ui, theme, keymap, mailbox, row, (rank, size)) {
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
    let moved = keyed.filter(|keyed| kept.as_ref().is_none_or(|kept| kept.keyed != Some(*keyed)));
    let mut scroll_to = None;
    if let Some((top, bottom)) = moved.and_then(|id| listing.find(id)).and_then(|index| listing.span(index)) {
        match &kept {
            // The least scroll that shows the row whole, set before the lines are built, so the row is built (and
            // can keep a focus given to it) in this frame.
            Some(kept) if kept.height > 0 => {
                let (offset, view) = (forced.unwrap_or_else(|| points(kept.offset)), points(kept.height));
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
    let mut area = ScrollArea::vertical().id_salt(id_salt).auto_shrink([false, false]);
    // egui springs an offset past last frame's end back to it (rubber-banding), and the offset that keeps a message
    // in place after the lines above it grew can lie past that end: the frame that sets an offset does not
    // rubber-band.
    let kinetic = ui.spacing().scroll.kinetic;
    if let Some(offset) = forced {
        area = area.vertical_scroll_offset(offset);
        ui.spacing_mut().scroll.kinetic.rubber_band = false;
    }
    let output = crate::component::scroll::show_viewport(ui, theme, area, |ui, viewport| {
        ui.set_min_height(listing.height);
        let room = ui.max_rect();
        let at = |top: f32, bottom: f32, inset: f32| {
            Rect::from_min_max(pos2(room.left() + inset, room.top() + top), pos2(room.right() - inset, room.top() + bottom))
        };
        let p = &theme.palette;
        for &(top, bottom) in &listing.sheets {
            if bottom >= viewport.top() && top <= viewport.bottom() {
                let rect = at(top, bottom, 0.0);
                ui.painter().rect_filled(rect, 0.0, p.surface);
                ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, p.line), StrokeKind::Inside);
            }
        }
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
            let inset = match line {
                Line::Card(_) => 0.0,
                Line::Head(..) => listing.inset + theme.spacing.s2,
                _ => listing.inset,
            };
            let layout = match line {
                Line::Head(..) | Line::More(..) => Layout::left_to_right(Align::Center),
                _ => Layout::top_down(Align::Min),
            };
            let rect = at(top, bottom, inset);
            let rank = listing.ranks.get(index).copied().unwrap_or(0);
            listed.scope_builder(UiBuilder::new().id_salt(salt(line)).max_rect(rect).layout(layout), |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, if matches!(line, Line::Card(_)) { theme.spacing.s2 } else { 0.0 });
                draw_line(ui, rows, line, rank, listing.rows);
            });
        }
        if let Some((top, bottom)) = scroll_to {
            ui.scroll_to_rect(at(top, bottom, 0.0), None);
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

/// The lines of `listing` on one card that fits them, scrolling inside it (a list without cards of its own).
fn on_card(ui: &mut Ui, rows: &Rows<'_>, listing: &Listing<'_>, id_salt: &str) -> Built {
    let theme = rows.theme;
    let p = &theme.palette;
    let room = ui.available_rect_before_wrap();
    let content = listing.height + ROW_INSET * 2.0;
    let card = Rect::from_min_size(room.min, vec2(room.width(), content.min(room.height()).max(1.0)));
    super::paint::pane(ui.painter(), theme, card);
    ui.painter().rect_stroke(card, 0.0, Stroke::new(1.0, p.line), StrokeKind::Inside);
    let inside = card.shrink(ROW_INSET);
    // The bar takes its room beside the rows only when they do not all fit, so a short list runs the card's width.
    ui.scope_builder(UiBuilder::new().max_rect(inside), |ui| {
        ui.shrink_clip_rect(inside);
        show_lines(ui, rows, listing, id_salt)
    })
    .inner
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

/// "New senders": the list, then under it the screener's mode as pills, the choice the Triage settings page offers.
fn senders(ui: &mut Ui, rows: &Rows<'_>, listing: &Listing<'_>) -> (Built, Option<ScreenerMode>) {
    let theme = rows.theme;
    let s = &theme.spacing;
    let room = ui.available_rect_before_wrap();
    let lines = if room.width() < 360.0 { 2.0 } else { 1.0 };
    let bar = (theme.fonts.button.line + s.s2) * lines + s.s2 * 2.0;
    let list = Rect::from_min_max(room.min, pos2(room.right(), (room.bottom() - bar).max(room.top())));
    let built = ui.scope_builder(UiBuilder::new().max_rect(list), |ui| on_card(ui, rows, listing, "mail-list")).inner;
    ui.add_space(s.s2);
    let mut mode = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(s.s1, s.s1);
        crate::component::label(ui, theme.fonts.label, tr!("Screener"), theme.palette.muted);
        for choice in ScreenerMode::ALL {
            if pill(ui, theme, super::settings::screener_name(choice), rows.mailbox.screener_mode() == choice).clicked() {
                mode = Some(choice);
            }
        }
    });
    (built, mode)
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

/// The rows, or what to say when there are none. Returns the message clicked this frame.
pub(super) fn rows(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox) -> Asked {
    let outbox = mailbox.place() == Place::Outbox && mailbox.query().is_empty();
    if mailbox.messages().is_empty() && !outbox {
        if let (true, Place::Later(pile)) = (mailbox.query().is_empty(), mailbox.place()) {
            let detail = match pile {
                Pile::Snoozed => tr!("Snoozed mail waits here until its time comes."),
                Pile::Pins => tr!("Pinned mail stays here, wherever it is filed."),
                Pile::Parked => tr!("Parked mail waits here, out of the inbox, without a time."),
            };
            empty(ui, theme, tr!("Nothing here"), detail);
        } else if mailbox.query().is_empty() {
            empty(ui, theme, tr!("Nothing here"), tr!("This folder holds no messages."));
        } else {
            empty(ui, theme, tr!("No messages match"), &tr!("No subject, sender or recipient holds “{query}”.", query = mailbox.query()));
        }
        return Asked { out: true, ..Asked::default() };
    }
    let inbox = mailbox.place() == Place::View(Role::Inbox) && mailbox.query().is_empty();
    let wait = inbox && !mailbox.waiting().is_empty();
    let groups = mailbox.groups();
    let view = if groups.is_empty() { InboxView::Simple } else { mailbox.inbox_view() };
    let grouping = view != InboxView::Simple;
    let rows = Rows {
        theme,
        keymap,
        mailbox,
        // Whose each row is shows in the simple list of more than one account; the grouped cards leave it out.
        account_marker: !grouping && mailbox.narrowed().is_none() && mailbox.accounts().len() > 1,
        clicked: Cell::new(None),
        acted: Cell::new(None),
        done_all: Cell::new(None),
        review: Cell::new(false),
        outbox: Cell::new(None),
    };
    let other = Group { icon: IconId::Inbox, name: tr!("Other").to_owned(), messages: Vec::new() };
    let open = |group: &Group| ui.data(|data| data.get_temp::<bool>(group_open_id(&group.name))).unwrap_or(false);
    // The gap of the row that left closes over `row_collapse`; with motion off it is closed at once.
    let gone = mailbox.left_row().map_or(1.0, |left| motion::appear(ui.ctx(), Id::unique("row-leaving").with(left.row.id), theme.motion.row_collapse));
    let listing = if outbox { sends(theme, mailbox) } else { lines(theme, mailbox, view, &groups, &other, wait, &open, gone) };
    let mut screener_mode = None;
    let built = if outbox {
        on_card(ui, &rows, &listing, "outbox-list")
    } else if grouping {
        show_lines(ui, &rows, &listing, "mail-list")
    } else if mailbox.place() == Place::Senders && mailbox.query().is_empty() {
        let (built, mode) = senders(ui, &rows, &listing);
        screener_mode = mode;
        built
    } else {
        if wait {
            rows.review.set(screener(ui, theme, keymap, mailbox.waiting()));
            ui.add_space(theme.spacing.s2);
        }
        on_card(ui, &rows, &listing, "mail-list")
    };
    ui.ctx().data_mut(|data| {
        data.insert_temp(rows_built_id(), built.count);
        data.insert_temp(built_id(), built);
    });
    Asked {
        clicked: rows.clicked.get(),
        acted: rows.acted.get(),
        out: mailbox.left_row().is_some() && gone >= 1.0,
        review: rows.review.get(),
        done_all: rows.done_all.take(),
        outbox: rows.outbox.get(),
        screener: screener_mode,
    }
}
