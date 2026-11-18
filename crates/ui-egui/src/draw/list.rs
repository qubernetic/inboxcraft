// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The list side: the header with the view's name, whose mail it is and the search field; the rows on a card; the
//! empty states.
//!
//! A row shows the sender's avatar, the sender and the date, the subject and the preview; an unread row its dot and
//! a heavier sender and subject; the selected row the selection; a row from a new sender says so after the
//! sender. On top of the unified inbox, while senders wait, the screener card with Review, which opens "New senders".
//! The unified inbox shows its groups as the owner chose ([`InboxView`]): a card for each (badge, name, unread count,
//! "Done all", the first rows and "Show all (n)"), one list with a heading for each, or the simple list without them;
//! the rows no group holds come last. A row of a grouped account has a context menu that puts its sender's mail in
//! another group. `ui_shot` can show invented senders and groups instead ([`crate::mailbox::Showcase`]).
//!
//! A hovered row and a row selected by keyboard show the hover actions alike, the row's text fading out before them:
//! Done, mark read or unread, Delete, on a real account as on the sample. Done and Delete let the row slide out first;
//! the window does the act once it is out and shows the toast with Undo.
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

use egui::{
    Align, Color32, Frame, Id, Key, Layout, Margin, Popup, Rect, Response, RichText, Role as WidgetRole, ScrollArea, Sense, Shape, Stroke,
    StrokeKind, Ui, UiBuilder, WidgetInfo, pos2, vec2,
};
use inboxcraft_model::Role;

use super::paint::{Ring, focus_ring, line, selection};
use inboxcraft_engine::ScreenerMode;

use crate::component::hover::{self, Action};
use crate::component::pill::pill;
use crate::component::{avatar, badge, button, field, margin, popup, tooltip};
use crate::iconset::IconId;
use crate::keymap::{self, Keymap};
use crate::mailbox::{Act, Group, GroupKind, InboxView, Mailbox, MessageRow, OutboxRow, Pile, Place, SendState};
use crate::motion;
use crate::theme::Theme;
use crate::tr;

/// The room between a card's edges and its rows, so a selected or hovered row is a rounded band inside the card.
pub(super) const ROW_INSET: f32 = 6.0;

/// A vertical scroll area for the list's cards. Its bar takes its own room beside them only while the content
/// overflows, so cards run the pane's whole width whenever they all fit and never lie under the bar.
fn scrolled<R>(ui: &mut Ui, theme: &Theme, add: impl FnOnce(&mut Ui) -> R) -> R {
    crate::component::scroll::show(ui, theme, ScrollArea::vertical().auto_shrink([false, false]), add).inner
}

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

/// How far right of a row's left edge the avatar starts: past the room for the unread dot.
fn face_indent(theme: &Theme) -> f32 {
    theme.spacing.s4 + theme.spacing.s2
}

/// How far right of a row's left edge its text starts: past the room for the unread dot and the avatar.
fn text_indent(theme: &Theme) -> f32 {
    face_indent(theme) + face_side(theme) + theme.spacing.s3
}

/// The gap between a row's two lines.
fn line_gap(theme: &Theme) -> f32 {
    theme.spacing.s1 / 2.0
}

/// The height of a message row: the design's fixed mail row at the current text scale.
pub(super) fn row_height(theme: &Theme) -> f32 {
    theme.boxes().mail_row
}

/// How far the account's stripe stands from a row's left edge, and how much further in it pushes the row's content.
const STRIPE_INSET: f32 = 6.0;
const STRIPE_ROOM: f32 = 15.0;

/// The width of the account's stripe.
const STRIPE_WIDTH: f32 = 3.0;

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
    /// It is the row sliding out, and it is all the way out.
    out: bool,
}

/// One message row across `ui`'s width; a click selects it. The row leaving the list slides to the right and fades
/// over the row slide token, a frame's worth at a time; with reduced motion it is out at once.
fn row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, message: &MessageRow, account_marker: bool) -> Drawn {
    let height = row_height(theme);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let selected = mailbox.selected() == Some(message.id);
    response.widget_info(|| WidgetInfo::selected(WidgetRole::ListBoxOption, true, selected, mailbox.spoken(message)));
    let leaving = mailbox.leaving().is_some_and(|(id, _)| id == message.id);
    if !ui.is_rect_visible(rect) {
        return Drawn { clicked: response.clicked(), acted: None, out: leaving };
    }
    let gone = if leaving { motion::appear(ui.ctx(), Id::unique("row-leaving").with(message.id), theme.motion.row_slide) } else { 0.0 };
    let mut face = ui.new_child(UiBuilder::new().max_rect(rect));
    if leaving {
        face.shrink_clip_rect(rect);
        face.multiply_opacity(1.0 - gone);
    }
    if message.done {
        // "Show done": a done message in its old place, dimmed.
        face.multiply_opacity(DONE_OPACITY);
    }
    let shifted = rect.translate(vec2(rect.width() * gone, 0.0));
    let mut acted = paint_row(&mut face, theme, keymap, mailbox, message, account_marker, shifted, &response, leaving);
    if response.has_focus() {
        focus_ring(ui.painter(), theme, response.rect, Ring::Inside);
    }
    if message.group.is_some() && !leaving {
        let width = theme.fonts.body.size * MENU_WIDTH;
        Popup::context_menu(&response).frame(Frame::NONE).show(|ui| {
            popup::panel(ui, theme, width, |ui| {
                if let Some(chosen) = group_menu(ui, theme, message.group, &message.address) {
                    acted = Some(chosen);
                    ui.close();
                }
            });
        });
    }
    Drawn { clicked: response.clicked(), acted, out: leaving && gone >= 1.0 }
}

/// How strongly a done row shows under "Show done".
const DONE_OPACITY: f32 = 0.55;

/// Paints a message row into `rect`; the hover action pressed. A leaving row shows no actions.
#[expect(clippy::too_many_arguments, reason = "the parts of one row")]
fn paint_row(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    mailbox: &Mailbox,
    message: &MessageRow,
    account_marker: bool,
    rect: Rect,
    response: &Response,
    leaving: bool,
) -> Option<Act> {
    let s = &theme.spacing;
    let p = &theme.palette;
    let f = &theme.fonts;
    let height = rect.height();
    let selected = mailbox.selected() == Some(message.id);
    if selected {
        selection(ui.painter(), theme, rect, 0.0);
    } else if pointer_on(ui, rect) {
        ui.painter().rect_filled(rect, 0.0, p.stage);
    }
    let (first_height, second_height) = row_lines(theme);
    let first = rect.top() + lines_top(theme);
    let second = first + first_height + line_gap(theme);
    // With the account's stripe the dot, the avatar and the text stand further in.
    let inset = if account_marker {
        let index = mailbox.account(message.account).map_or(0, |account| account.index);
        let marker = Rect::from_min_size(pos2(rect.left() + STRIPE_INSET, rect.top() + s.s2), vec2(STRIPE_WIDTH, (height - 2.0 * s.s2).max(1.0)));
        ui.painter().rect_filled(marker, 1.0, theme.account_colour(u64::try_from(index).unwrap_or(0)));
        STRIPE_ROOM
    } else {
        0.0
    };
    if !message.seen {
        ui.painter().circle_filled(pos2(rect.left() + inset + s.s2, rect.center().y), 3.0, p.cyan_edge);
    }
    let side = face_side(theme);
    let face = Rect::from_center_size(pos2(rect.left() + inset + face_indent(theme) + side / 2.0, rect.center().y), vec2(side, side));
    avatar::paint(ui.painter(), theme, face, &message.sender, &message.address);
    let left = rect.left() + inset + text_indent(theme);
    let right = rect.right() - s.s3;
    // Unread shows on the first line (the sender in `strong`) and in the dot; the second line is `caption` either way.
    let sender_font = if message.seen { f.body } else { f.strong };
    // The hover actions, as the mail row's sheet places them: on the first line, their right edge 16 in from the row's.
    // The row keeps its middle: the group has the line right of the middle (4 clear of it), and what does not fit there
    // gives way from the end, so a press on the row's middle opens it at every width of the list. A hovered row gives
    // its time way to the group (`.row:hover .time` is hidden); the keyed row keeps its time, as the sheet's focus
    // state, and its group stands left of it. A flag or pin stays, 8 left of the group (or of the time), 8 apart, and
    // the sender is cut 4 short of them.
    let hovered = !leaving && pointer_on(ui, response.rect);
    let keyed = !leaving && selected && mailbox.keyed();
    let reach = hovered || keyed;
    // Built only for the row whose group shows: the other rows hand the group nothing.
    let built = reach.then(|| actions(message));
    let actions: &[Action<'static>] = built.as_ref().map_or(&[], |built| built.as_slice());
    let flag_size = theme.mark_side();
    let mut cursor = right;
    if !hovered {
        // Both lines sit on the middle of their line, so a smaller date lines up with the sender.
        let date = line(ui, &[(&mailbox.row_date(message), f.mono_s, p.muted)], f32::INFINITY);
        let date_left = right - date.size().x;
        ui.painter().galley(pos2(date_left, first + (first_height - date.size().y) / 2.0), date, p.muted);
        cursor = date_left - s.s2;
    }
    // Laid out only for the row whose group shows: the other rows of a long list lay out no chip.
    let actions_line = reach.then(|| {
        let line_right = if hovered { rect.right() - s.s4 } else { cursor };
        Rect::from_min_max(pos2(rect.center().x + s.s1, first), pos2(line_right.max(rect.center().x + s.s1), first + first_height))
    });
    if let Some(at) = actions_line {
        let group = hover::place(ui, theme, keymap, at, actions);
        if group.width() > 0.0 {
            cursor = group.left() - s.s2;
        }
    }
    // The flag nearest the time or the group, the pin left of it, on the sender's line.
    for (shown, icon, colour) in [(message.flagged, IconId::Flag, p.ink), (message.pinned, IconId::Pin, p.cyan_text)] {
        if shown {
            let at = Rect::from_min_size(pos2(cursor - flag_size, first + (first_height - flag_size) / 2.0), vec2(flag_size, flag_size));
            crate::iconset::paint(ui.painter(), at, icon, colour);
            cursor -= flag_size + s.s2;
        }
    }
    let end = cursor;
    let mark = if message.bounced.is_some() {
        Some((tr!("Not delivered"), p.minus))
    } else if message.new_sender {
        Some((tr!("New sender"), p.cyan_text))
    } else if mailbox.returned(message) {
        Some((tr!("Returned"), p.cyan_text))
    } else {
        None
    };
    let sender = match mark {
        Some((mark, ink)) => {
            let mark = format!("  {mark}");
            line(ui, &[(&message.sender, sender_font, p.ink), (&mark, f.caption, ink)], end - s.s1 - left)
        }
        None => line(ui, &[(&message.sender, sender_font, p.ink)], end - s.s1 - left),
    };
    ui.painter().galley(pos2(left, first + (first_height - sender.size().y) / 2.0), sender, p.ink);
    let subject = if message.subject.is_empty() { tr!("(no subject)") } else { message.subject.as_str() };
    let preview = after_subject(&message.preview);
    let text = line(ui, &[(subject, f.caption, p.ink), (&preview, f.caption, p.muted)], right - left);
    ui.painter().galley(pos2(left, second + (second_height - text.size().y) / 2.0), text, p.ink);
    // Last, so the solid float covers whatever of the row lies under it.
    let shown = hover::actions(ui, theme, keymap, Id::unique("row-actions").with(message.id), actions_line.unwrap_or(rect), reach, actions);
    shown.clicked.and_then(|index| act(message, index))
}

/// Whether the pointer is on `rect`, a row of the list: rows touch, so a row owns its top edge and not its bottom one,
/// and the pointer on their shared edge is the lower row's alone.
fn pointer_on(ui: &Ui, rect: Rect) -> bool {
    ui.rect_contains_pointer(rect) && ui.ctx().pointer_latest_pos().is_some_and(|at| at.y < rect.bottom())
}

/// What follows a row's subject: a dot and the preview, or nothing without one.
pub(super) fn after_subject(preview: &str) -> String {
    if preview.trim().is_empty() { String::new() } else { format!("  · {preview}") }
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
    /// The row sliding out is all the way out, or was not drawn: its act can be done.
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
fn outbox_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, row: &OutboxRow) -> Option<Outbox> {
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

/// The outbox on one card: each send in it, newest last; what a button asked for.
fn outbox(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox) -> Option<(u64, Outbox)> {
    let mut asked = None;
    scrolled(ui, theme, |ui| {
        sheet(ui, theme, false, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            for row in mailbox.outbox() {
                if let Some(pressed) = outbox_row(ui, theme, keymap, mailbox, row) {
                    asked = Some((row.draft, pressed));
                }
            }
        });
    });
    asked
}

/// What the rows of a card share.
struct Rows<'a> {
    theme: &'a Theme,
    keymap: &'a Keymap,
    mailbox: &'a Mailbox,
    account_marker: bool,
    /// A hover action pressed this frame.
    acted: Cell<Option<(u64, Act)>>,
    /// Whether the row sliding out was drawn, and whether it is out.
    leaving: Cell<(bool, bool)>,
    /// "Done all" pressed this frame: the group's messages.
    done_all: Cell<Option<Vec<u64>>>,
    /// How many rows this frame laid out, seen or not: what the frame bench reads ([`rows_built`]).
    built: Cell<usize>,
}

impl Rows<'_> {
    /// Draws `messages` one under another; the one clicked.
    fn show<'m>(&self, ui: &mut Ui, messages: impl IntoIterator<Item = &'m MessageRow>) -> Option<u64> {
        let mut clicked = None;
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        for message in messages {
            self.built.set(self.built.get().saturating_add(1));
            let drawn = row(ui, self.theme, self.keymap, self.mailbox, message, self.account_marker);
            if drawn.clicked {
                clicked = Some(message.id);
            }
            if let Some(act) = drawn.acted {
                self.acted.set(Some((message.id, act)));
            }
            if self.mailbox.leaving().is_some_and(|(id, _)| id == message.id) {
                self.leaving.set((true, drawn.out));
            }
        }
        clicked
    }
}

/// The screener card: the waiting senders' avatars, how many wait, and Review; whether Review was pressed.
fn screener(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox) -> bool {
    let s = &theme.spacing;
    let p = &theme.palette;
    let waiting = mailbox.waiting();
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

/// A group's card: its badge, name and unread count, "Done all", its first rows and "Show all (n)". The one clicked.
fn group_card(ui: &mut Ui, rows: &Rows<'_>, group: &Group, messages: &[&MessageRow]) -> Option<u64> {
    let theme = rows.theme;
    let s = &theme.spacing;
    let p = &theme.palette;
    let id = Id::unique("list-group").with(&group.name);
    let all = ui.data(|data| data.get_temp::<bool>(id)).unwrap_or(false);
    let mut clicked = None;
    sheet(ui, theme, false, |ui| {
        Frame::new().inner_margin(margin(s.s2, s.s1)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let side = theme.fonts.heading.line * 1.4;
                badge::group(ui, theme, group.icon, &group.name, side);
                ui.add_space(s.s1);
                let unread = messages.iter().filter(|message| !message.seen).count();
                let unread = u64::try_from(unread).unwrap_or(u64::MAX);
                let name = line(ui, &[(&group.name, theme.fonts.heading, p.ink)], f32::INFINITY);
                let count = if unread > 0 { badge::count_width(ui, theme, unread) + s.s2 } else { 0.0 };
                let (rect, response) = ui.allocate_exact_size(vec2(name.size().x + count, side), Sense::hover());
                let spoken = if unread > 0 { tr!("{name}, {count} unread", name = group.name, count = unread) } else { group.name.clone() };
                response.widget_info(|| WidgetInfo::labeled(WidgetRole::Label, true, &spoken));
                ui.painter().galley(pos2(rect.left(), rect.center().y - name.size().y / 2.0), name, p.ink);
                if unread > 0 {
                    badge::count(ui, theme, unread, rect.right(), rect.center().y);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if button::text(ui, theme, rows.keymap, tr!("Done all"), None, false).clicked() {
                        rows.done_all.set(Some(messages.iter().map(|message| message.id).collect()));
                    }
                });
            });
        });
        let shown = if all { messages.len() } else { messages.len().min(GROUP_ROWS) };
        clicked = rows.show(ui, messages.iter().take(shown).copied());
        if messages.len() > GROUP_ROWS {
            // A link under the rows' text, as far in as the text.
            ui.horizontal(|ui| {
                ui.add_space((text_indent(theme) - s.s4 - button::LINK_PAD).max(0.0));
                let label = if all { tr!("Show fewer").to_owned() } else { tr!("Show all ({count})", count = messages.len()) };
                if button::link(ui, theme, rows.keymap, &label).clicked() {
                    ui.data_mut(|data| data.insert_temp(id, !all));
                }
            });
        }
    });
    clicked
}

/// The rows of `group`, in the list's order.
fn members<'m>(mailbox: &'m Mailbox, group: &Group) -> Vec<&'m MessageRow> {
    mailbox.messages().iter().filter(|message| group.messages.contains(&message.id)).collect()
}

/// The rows no group holds, in the list's order.
fn rest<'m>(mailbox: &'m Mailbox, groups: &[Group]) -> Vec<&'m MessageRow> {
    mailbox.messages().iter().filter(|message| !groups.iter().any(|group| group.messages.contains(&message.id))).collect()
}

/// The unified inbox as cards: the screener card while senders `wait`, a card for each group, and the rows no group
/// holds on a card of their own. The row clicked, and whether Review was pressed.
fn grouped(ui: &mut Ui, rows: &Rows<'_>, groups: &[Group], wait: bool) -> (Option<u64>, bool) {
    let s = &rows.theme.spacing;
    let mailbox = rows.mailbox;
    let (mut clicked, mut review) = (None, false);
    scrolled(ui, rows.theme, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, s.s2);
        if wait {
            review = screener(ui, rows.theme, rows.keymap, mailbox);
        }
        for group in groups {
            let messages = members(mailbox, group);
            if !messages.is_empty() {
                clicked = clicked.or(group_card(ui, rows, group, &messages));
            }
        }
        let rest = rest(mailbox, groups);
        if !rest.is_empty() {
            clicked = clicked.or(sheet(ui, rows.theme, false, |ui| rows.show(ui, rest)));
        }
    });
    (clicked, review)
}

/// A group's heading in the grouped list: its badge, name and unread count.
fn heading(ui: &mut Ui, theme: &Theme, icon: IconId, name: &str, messages: &[&MessageRow]) {
    let s = &theme.spacing;
    let p = &theme.palette;
    Frame::new().inner_margin(margin(s.s2, s.s1)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let side = theme.fonts.strong.line * 1.2;
            badge::group(ui, theme, icon, name, side);
            ui.add_space(s.s1);
            let unread = u64::try_from(messages.iter().filter(|message| !message.seen).count()).unwrap_or(u64::MAX);
            let galley = line(ui, &[(name, theme.fonts.strong, p.ink)], f32::INFINITY);
            let count = if unread > 0 { badge::count_width(ui, theme, unread) + s.s2 } else { 0.0 };
            let (rect, response) = ui.allocate_exact_size(vec2(galley.size().x + count, side), Sense::hover());
            let spoken = if unread > 0 { tr!("{name}, {count} unread", name = name, count = unread) } else { name.to_owned() };
            response.widget_info(|| WidgetInfo::labeled(WidgetRole::Label, true, &spoken));
            ui.painter().galley(pos2(rect.left(), rect.center().y - galley.size().y / 2.0), galley, p.ink);
            if unread > 0 {
                badge::count(ui, theme, unread, rect.right(), rect.center().y);
            }
        });
    });
}

/// The unified inbox as one grouped list: the screener card while senders `wait`, then one card with each group's
/// rows under its heading, and the rows no group holds under "Other". The row clicked, and whether Review was pressed.
fn listed(ui: &mut Ui, rows: &Rows<'_>, groups: &[Group], wait: bool) -> (Option<u64>, bool) {
    let s = &rows.theme.spacing;
    let mailbox = rows.mailbox;
    let (mut clicked, mut review) = (None, false);
    scrolled(ui, rows.theme, |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, s.s2);
        if wait {
            review = screener(ui, rows.theme, rows.keymap, mailbox);
        }
        clicked = sheet(ui, rows.theme, false, |ui| {
            let mut clicked = None;
            for group in groups {
                let messages = members(mailbox, group);
                if !messages.is_empty() {
                    heading(ui, rows.theme, group.icon, &group.name, &messages);
                    clicked = clicked.or(rows.show(ui, messages));
                }
            }
            let rest = rest(mailbox, groups);
            if !rest.is_empty() {
                heading(ui, rows.theme, IconId::Inbox, tr!("Other"), &rest);
                clicked = clicked.or(rows.show(ui, rest));
            }
            clicked
        });
    });
    (clicked, review)
}

/// The rows of the list on one card that fits them, scrolling inside it.
fn plain(ui: &mut Ui, rows: &Rows<'_>) -> Option<u64> {
    let theme = rows.theme;
    let p = &theme.palette;
    let count = rows.mailbox.messages().len();
    let height = row_height(theme);
    let room = ui.available_rect_before_wrap();
    let content = height * count as f32 + ROW_INSET * 2.0;
    let card = Rect::from_min_size(room.min, vec2(room.width(), content.min(room.height()).max(1.0)));
    super::paint::pane(ui.painter(), theme, card);
    ui.painter().rect_stroke(card, 0.0, Stroke::new(1.0, p.line), StrokeKind::Inside);
    let inside = card.shrink(ROW_INSET);
    // The bar takes its room beside the rows only when they do not all fit, so a short list runs the card's width.
    let mut clicked = None;
    ui.scope_builder(UiBuilder::new().max_rect(inside), |ui| {
        ui.shrink_clip_rect(inside);
        crate::component::scroll::show_rows(ui, theme, ScrollArea::vertical().auto_shrink([false, false]), height, count, |ui, range| {
            clicked = rows.show(ui, rows.mailbox.messages().get(range).unwrap_or_default());
        });
    });
    clicked
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
fn senders(ui: &mut Ui, rows: &Rows<'_>) -> (Option<u64>, Option<ScreenerMode>) {
    let theme = rows.theme;
    let s = &theme.spacing;
    let room = ui.available_rect_before_wrap();
    let lines = if room.width() < 360.0 { 2.0 } else { 1.0 };
    let bar = (theme.fonts.button.line + s.s2) * lines + s.s2 * 2.0;
    let list = Rect::from_min_max(room.min, pos2(room.right(), (room.bottom() - bar).max(room.top())));
    let clicked = ui.scope_builder(UiBuilder::new().max_rect(list), |ui| plain(ui, rows)).inner;
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
    (clicked, mode)
}

/// Where the rows a frame built are kept for [`rows_built`].
fn rows_built_id() -> Id {
    Id::unique("list-rows-built")
}

/// How many message rows the list laid out in the last frame that drew rows (each row allocated, whether it was on
/// screen or not), taken out so the next read sees only a later frame's: `None` when no frame drew rows since. For
/// the frame bench (`ui_bench`).
#[must_use]
pub fn rows_built(ctx: &egui::Context) -> Option<usize> {
    ctx.data_mut(|data| data.remove_temp::<usize>(rows_built_id()))
}

/// The rows, or what to say when there are none. Returns the message clicked this frame.
pub(super) fn rows(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox) -> Asked {
    if mailbox.place() == Place::Outbox && mailbox.query().is_empty() {
        return Asked { out: true, outbox: outbox(ui, theme, keymap, mailbox), ..Asked::default() };
    }
    if mailbox.messages().is_empty() {
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
        acted: Cell::new(None),
        leaving: Cell::new((false, false)),
        done_all: Cell::new(None),
        built: Cell::new(0),
    };
    let mut screener_mode = None;
    let (clicked, review) = match view {
        InboxView::Cards => grouped(ui, &rows, &groups, wait),
        InboxView::List => listed(ui, &rows, &groups, wait),
        InboxView::Simple if mailbox.place() == Place::Senders && mailbox.query().is_empty() => {
            let (clicked, mode) = senders(ui, &rows);
            screener_mode = mode;
            (clicked, false)
        }
        InboxView::Simple => {
            let review = wait && screener(ui, theme, keymap, mailbox);
            if wait {
                ui.add_space(theme.spacing.s2);
            }
            (plain(ui, &rows), review)
        }
    };
    let (drawn, out) = rows.leaving.get();
    let built = rows.built.get();
    ui.ctx().data_mut(|data| data.insert_temp(rows_built_id(), built));
    Asked { clicked, acted: rows.acted.get(), out: !drawn || out, review, done_all: rows.done_all.take(), outbox: None, screener: screener_mode }
}
