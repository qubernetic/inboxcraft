// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The list side: the header with the view's name, whose mail it is and the search field; the rows on a card; the
//! empty states.
//!
//! A row shows the sender's avatar, the sender and the date, the subject and the preview; an unread row its dot and
//! a heavier sender and subject; the selected row the selection gradient. The parts whose data does not exist yet come
//! from the mailbox's [`Showcase`], which only `ui_shot` fills, so the app draws none of them: the screener card on
//! top of the unified inbox and the grouped inbox's cards (badge, name, unread count, "Done all", "Show all (n)").
//!
//! On the sample mailbox ([`Mailbox::can_act`]) a hovered row and a row selected by keyboard show the hover actions
//! alike, the row's text fading out before them: Done, mark read or unread, Delete. Done and Delete let the row slide
//! out first; the window does the act once it is out and shows the toast with Undo. Anywhere else no action is drawn.

use std::cell::Cell;
use std::sync::Arc;

use egui::{
    Align, Color32, Frame, Galley, Id, Key, Layout, Rect, Response, RichText, Role, ScrollArea, Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder,
    WidgetInfo, pos2, vec2,
};

use super::paint::{focus_ring, line, line_height, selection};
use crate::component::hover::{self, Action};
use crate::component::{avatar, badge, button, field, margin, tooltip};
use crate::glass;
use crate::gradient::{self, Gradient, Stop};
use crate::iconset::{IconId, Style};
use crate::keymap::{self, Keymap};
use crate::mailbox::{Act, Group, Mailbox, MessageRow, Place, Showcase};
use crate::motion;
use crate::theme::Theme;
use crate::tr;

/// The room between a card's edges and its rows, so a selected or hovered row is a rounded band inside the card.
pub(super) const ROW_INSET: f32 = 6.0;

/// The room the vertical scroll bar takes at the right of a scroll area, kept free so it never lies over a card or a
/// row's date.
fn scroll_room(ui: &Ui) -> f32 {
    let scroll = &ui.spacing().scroll;
    scroll.bar_width + scroll.bar_inner_margin + scroll.bar_outer_margin
}

/// The rows a group shows before "Show all".
pub(super) const GROUP_ROWS: usize = 3;

/// The search field's id, so a shortcut can focus it.
pub(super) fn search_id() -> egui::Id {
    egui::Id::unique("mail-search")
}

/// The header above the list. Returns the search text when it changed this frame.
pub(super) fn header(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, search: &mut String) -> Option<String> {
    let p = &theme.palette;
    let mut changed = None;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(mailbox.title()).font(theme.fonts.title.id()).color(p.text));
            let count = mailbox.messages().len();
            let shown = if count == 1 { tr!("1 message").to_owned() } else { tr!("{count} messages", count = count) };
            ui.label(
                RichText::new(tr!("{whose} · {shown}", whose = mailbox.subtitle(), shown = shown)).font(theme.fonts.caption.id()).color(p.muted),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if !search.is_empty() && button::text(ui, theme, keymap, tr!("Clear"), None, false).clicked() {
                search.clear();
                changed = Some(String::new());
            }
            let field = field::search(ui, theme, search_id(), search, tr!("Search"), theme.spacing.folder_pane * 0.75);
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
    changed
}

fn empty(ui: &mut Ui, theme: &Theme, title: &str, detail: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(theme.spacing.xl * 3.0);
        ui.label(RichText::new(title).font(theme.fonts.heading.id()).color(theme.palette.text));
        ui.add_space(theme.spacing.xs);
        ui.label(RichText::new(detail).font(theme.fonts.body.id()).color(theme.palette.muted));
    });
}

/// The two lines of a message row: the sender's and the subject's, each as high as its tallest font.
fn row_lines(ui: &Ui, theme: &Theme) -> (f32, f32) {
    let f = &theme.fonts;
    let first = [f.sender, f.sender_unread, f.caption].map(|font| line_height(ui, font)).into_iter().fold(0.0, f32::max);
    let second = [f.subject, f.subject_unread, f.preview].map(|font| line_height(ui, font)).into_iter().fold(0.0, f32::max);
    (first, second)
}

/// How far right of a row's left edge its text starts: past the room for the unread dot and the avatar.
fn text_indent(ui: &Ui, theme: &Theme) -> f32 {
    let s = &theme.spacing;
    let (first, second) = row_lines(ui, theme);
    s.m + s.xs + first + s.xs + second + s.m
}

/// The height of a message row: its two lines and air, from the font's metrics at the current text scale.
pub(super) fn row_height(ui: &Ui, theme: &Theme) -> f32 {
    let (first, second) = row_lines(ui, theme);
    theme.spacing.s * 2.0 + first + theme.spacing.xs + second
}

/// Paints `galley` at `at`, faded out before `until` when the hover actions show.
fn put(ui: &Ui, theme: &Theme, at: egui::Pos2, galley: Arc<Galley>, colour: Color32, until: Option<f32>) {
    match until {
        Some(until) => glass::faded(ui.painter(), at, &galley, colour, until, theme.spacing.xl),
        None => {
            ui.painter().galley(at, galley, colour);
        }
    }
}

/// The hover actions of `message`: Done, mark read or unread, Delete.
fn actions(message: &MessageRow) -> [Action<'static>; 4] {
    // An open envelope opens an unread message; a closed one closes a read one again.
    let (read_icon, read) = if message.seen { (IconId::Mail, tr!("Mark as unread")) } else { (IconId::MarkRead, tr!("Mark as read")) };
    [
        Action { icon: IconId::Done, name: tr!("Done"), action: Some(keymap::DONE), unavailable: None },
        Action { icon: read_icon, name: read, action: Some(keymap::READ), unavailable: None },
        Action { icon: IconId::Delete, name: tr!("Delete"), action: Some(keymap::DELETE), unavailable: None },
        Action { icon: IconId::Snooze, name: tr!("Snooze"), action: None, unavailable: Some(tr!("Not available yet")) },
    ]
}

/// What the hover action at `index` of [`actions`] does to `message`.
fn act(message: &MessageRow, index: usize) -> Option<Act> {
    match index {
        0 => Some(Act::Done),
        1 if message.seen => Some(Act::Unread),
        1 => Some(Act::Read),
        2 => Some(Act::Delete),
        _ => None,
    }
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
    let height = row_height(ui, theme);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let selected = mailbox.selected() == Some(message.id);
    response.widget_info(|| WidgetInfo::selected(Role::ListBoxOption, true, selected, mailbox.spoken(message)));
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
    let shifted = rect.translate(vec2(rect.width() * gone, 0.0));
    let acted = paint_row(&mut face, theme, keymap, mailbox, message, account_marker, shifted, &response, leaving);
    focus_ring(ui, theme, &response);
    Drawn { clicked: response.clicked(), acted, out: leaving && gone >= 1.0 }
}

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
        selection(ui.painter(), theme, rect, s.corner);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, s.corner, p.hover);
    }
    let reach = mailbox.can_act() && !leaving && (ui.rect_contains_pointer(response.rect) || (selected && mailbox.keyed()));
    let shown = hover::actions(ui, theme, keymap, Id::unique("row-actions").with(message.id), rect, reach, &actions(message));
    let acted = shown.clicked.and_then(|index| act(message, index));
    let until = (shown.openness > 0.0).then_some(shown.left - s.xs);
    let (first_height, second_height) = row_lines(ui, theme);
    let first = rect.top() + s.s;
    let second = first + first_height + s.xs;
    if account_marker {
        let index = mailbox.account(message.account).map_or(0, |account| account.index);
        let marker = Rect::from_min_size(pos2(rect.left(), rect.top() + s.s), vec2(3.0, height - 2.0 * s.s));
        ui.painter().rect_filled(marker, 1.0, theme.account_colour(u64::try_from(index).unwrap_or(0)));
    }
    if !message.seen {
        ui.painter().circle_filled(pos2(rect.left() + s.s, rect.center().y), 3.5, p.unread);
    }
    let side = first_height + s.xs + second_height;
    let face = Rect::from_min_size(pos2(rect.left() + s.m + s.xs, first), vec2(side, side));
    avatar::paint(ui.painter(), theme, face, &message.sender, &message.address);
    let left = rect.left() + text_indent(ui, theme);
    let right = rect.right() - s.m;
    let (sender_font, subject_font) = if message.seen { (f.sender, f.subject) } else { (f.sender_unread, f.subject_unread) };
    // Both lines sit on the middle of their line, so a smaller date lines up with the sender.
    let date = line(ui, &[(&mailbox.when(message), f.caption, p.muted)], f32::INFINITY);
    let date_left = right - date.size().x;
    put(ui, theme, pos2(date_left, first + (first_height - date.size().y) / 2.0), date, p.muted, until);
    let sender = line(ui, &[(&message.sender, sender_font, p.text)], date_left - s.m - left);
    put(ui, theme, pos2(left, first + (first_height - sender.size().y) / 2.0), sender, p.text, until);
    let flag_size = second_height.max(1.0);
    let flag_room = if message.flagged { flag_size + s.s } else { 0.0 };
    let subject = if message.subject.is_empty() { tr!("(no subject)") } else { message.subject.as_str() };
    let preview = format!("   {}", message.preview);
    match until {
        None => {
            let text = line(ui, &[(subject, subject_font, p.text), (&preview, f.preview, p.muted)], right - flag_room - left);
            ui.painter().galley(pos2(left, second + (second_height - text.size().y) / 2.0), text, p.text);
            if message.flagged {
                let at = Rect::from_min_size(pos2(right - flag_size, second + (second_height - flag_size) / 2.0), vec2(flag_size, flag_size));
                theme.icons.paint(ui.painter(), at, IconId::Flag, Style::Fill, p.flag);
            }
        }
        // The subject and the preview fade each in their own colour.
        Some(until) => {
            let subject = line(ui, &[(subject, subject_font, p.text)], right - left);
            let after = left + subject.size().x;
            put(ui, theme, pos2(left, second + (second_height - subject.size().y) / 2.0), subject, p.text, Some(until));
            if after < until {
                let preview = line(ui, &[(&preview, f.preview, p.muted)], right - after);
                put(ui, theme, pos2(after, second + (second_height - preview.size().y) / 2.0), preview, p.muted, Some(until));
            }
        }
    }
    acted
}

/// A rounded card across `ui`'s width with `add` drawn inside; `edge` rings it with the preset's gradient.
fn sheet<R>(ui: &mut Ui, theme: &Theme, edge: bool, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = &theme.palette;
    let s = &theme.spacing;
    let slot = ui.painter().add(Shape::Noop);
    let inner = Frame::new().inner_margin(margin(ROW_INSET, ROW_INSET)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
    let rect = inner.response.rect;
    let shapes = if edge {
        let [first, middle, last] = p.gradient;
        let ring = Gradient::Linear {
            from: pos2(0.0, 0.0),
            to: pos2(1.0, 1.0),
            stops: vec![Stop::new(0.0, first), Stop::new(0.5, middle), Stop::new(1.0, last)],
        };
        let width = EDGE;
        vec![gradient::filled(ui.painter(), rect, s.card, &ring), Shape::rect_filled(rect.shrink(width), s.card - width, p.surface)]
    } else {
        vec![Shape::rect_filled(rect, s.card, p.surface), Shape::rect_stroke(rect, s.card, Stroke::new(1.0, p.line), StrokeKind::Inside)]
    };
    ui.painter().set(slot, Shape::Vec(shapes));
    inner.inner
}

/// The width of the screener card's gradient edge.
const EDGE: f32 = 1.5;

/// What the list asked for this frame.
#[derive(Debug, Default)]
pub(super) struct Asked {
    /// The row clicked.
    pub(super) clicked: Option<u64>,
    /// Add account, in the empty window.
    pub(super) add: bool,
    /// A hover action pressed, and on which message.
    pub(super) acted: Option<(u64, Act)>,
    /// The row sliding out is all the way out, or was not drawn: its act can be done.
    pub(super) out: bool,
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
}

impl Rows<'_> {
    /// Draws `messages` one under another; the one clicked.
    fn show<'m>(&self, ui: &mut Ui, messages: impl IntoIterator<Item = &'m MessageRow>) -> Option<u64> {
        let mut clicked = None;
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        for message in messages {
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

/// The screener card: the waiting senders' avatars, how many wait, and Review.
fn screener(ui: &mut Ui, theme: &Theme, keymap: &Keymap, showcase: &Showcase) {
    let s = &theme.spacing;
    let p = &theme.palette;
    sheet(ui, theme, true, |ui| {
        Frame::new().inner_margin(margin(s.s, s.xs)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let side = line_height(ui, theme.fonts.strong) + s.xs + line_height(ui, theme.fonts.caption);
                let shown = showcase.screener.iter().take(3);
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
                ui.add_space(s.s);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let waiting = showcase.screener.len();
                    let title = if waiting == 1 { tr!("1 new sender").to_owned() } else { tr!("{count} new senders", count = waiting) };
                    ui.label(RichText::new(title).font(theme.fonts.strong.id()).color(p.text));
                    ui.label(RichText::new(tr!("Held back until you let them in")).font(theme.fonts.caption.id()).color(p.muted));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    button::text(ui, theme, keymap, tr!("Review"), Some("screener.review"), true);
                });
            });
        });
    });
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
        Frame::new().inner_margin(margin(s.s, s.xs)).show(ui, |ui| {
            ui.horizontal(|ui| {
                let side = line_height(ui, theme.fonts.heading) * 1.4;
                badge::group(ui, theme, group.icon, &group.name, side);
                ui.add_space(s.xs);
                let unread = messages.iter().filter(|message| !message.seen).count();
                let unread = u64::try_from(unread).unwrap_or(u64::MAX);
                let name = line(ui, &[(&group.name, theme.fonts.heading, p.text)], f32::INFINITY);
                let count = if unread > 0 { badge::count_width(ui, theme, unread) + s.s } else { 0.0 };
                let (rect, response) = ui.allocate_exact_size(vec2(name.size().x + count, side), Sense::hover());
                let spoken = if unread > 0 { tr!("{name}, {count} unread", name = group.name, count = unread) } else { group.name.clone() };
                response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &spoken));
                ui.painter().galley(pos2(rect.left(), rect.center().y - name.size().y / 2.0), name, p.text);
                if unread > 0 {
                    badge::count(ui, theme, unread, rect.right(), rect.center().y);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    button::text(ui, theme, rows.keymap, tr!("Done all"), Some("mail.done.all"), false);
                });
            });
        });
        let shown = if all { messages.len() } else { messages.len().min(GROUP_ROWS) };
        clicked = rows.show(ui, messages.iter().take(shown).copied());
        if messages.len() > GROUP_ROWS {
            // A link under the rows' text, as far in as the text.
            ui.horizontal(|ui| {
                ui.add_space(text_indent(ui, theme) - button::LINK_PAD);
                let label = if all { tr!("Show fewer").to_owned() } else { tr!("Show all ({count})", count = messages.len()) };
                if button::link(ui, theme, rows.keymap, &label).clicked() {
                    ui.data_mut(|data| data.insert_temp(id, !all));
                }
            });
        }
    });
    clicked
}

/// The unified inbox with the showcase's screener and groups, the rows no group holds on a card of their own.
fn grouped(ui: &mut Ui, rows: &Rows<'_>, showcase: &Showcase) -> Option<u64> {
    let s = &rows.theme.spacing;
    let mailbox = rows.mailbox;
    let mut clicked = None;
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width((ui.available_width() - scroll_room(ui)).max(1.0));
        ui.spacing_mut().item_spacing = vec2(0.0, s.s);
        if !showcase.screener.is_empty() {
            screener(ui, rows.theme, rows.keymap, showcase);
        }
        for group in &showcase.groups {
            let messages: Vec<&MessageRow> = mailbox.messages().iter().filter(|message| group.messages.contains(&message.id)).collect();
            if !messages.is_empty() {
                clicked = clicked.or(group_card(ui, rows, group, &messages));
            }
        }
        let rest: Vec<&MessageRow> =
            mailbox.messages().iter().filter(|message| !showcase.groups.iter().any(|group| group.messages.contains(&message.id))).collect();
        if !rest.is_empty() {
            clicked = clicked.or(sheet(ui, rows.theme, false, |ui| rows.show(ui, rest)));
        }
    });
    clicked
}

/// The rows of the list on one card that fits them, scrolling inside it.
fn plain(ui: &mut Ui, rows: &Rows<'_>) -> Option<u64> {
    let theme = rows.theme;
    let s = &theme.spacing;
    let p = &theme.palette;
    let count = rows.mailbox.messages().len();
    let height = row_height(ui, theme);
    let room = ui.available_rect_before_wrap();
    let content = height * count as f32 + ROW_INSET * 2.0;
    let card = Rect::from_min_size(room.min, vec2(room.width(), content.min(room.height()).max(1.0)));
    ui.painter().rect_filled(card, s.card, p.surface);
    ui.painter().rect_stroke(card, s.card, Stroke::new(1.0, p.line), StrokeKind::Inside);
    let inside = card.shrink(ROW_INSET);
    let mut clicked = None;
    ui.scope_builder(UiBuilder::new().max_rect(inside), |ui| {
        ui.shrink_clip_rect(inside);
        ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, height, count, |ui, range| {
            ui.set_max_width((ui.available_width() - scroll_room(ui)).max(1.0));
            clicked = rows.show(ui, rows.mailbox.messages().get(range).unwrap_or_default());
        });
    });
    clicked
}

/// The rows, or what to say when there are none. Returns the message clicked this frame, and whether Add account
/// was (the empty window offers it when `can_add`).
pub(super) fn rows(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, can_add: bool) -> Asked {
    if mailbox.accounts().is_empty() {
        if !can_add {
            empty(ui, theme, tr!("No accounts yet"), tr!("Start InboxCraft with --sample to look around an invented mailbox."));
            return Asked { out: true, ..Asked::default() };
        }
        empty(ui, theme, tr!("No accounts yet"), tr!("Add an account to read its mail here. Nothing leaves this computer before you do."));
        let mut add = false;
        ui.vertical_centered(|ui| {
            ui.add_space(theme.spacing.l);
            add = button::text(ui, theme, keymap, tr!("Add account"), None, true).clicked();
        });
        return Asked { add, out: true, ..Asked::default() };
    }
    if mailbox.messages().is_empty() {
        if mailbox.query().is_empty() {
            empty(ui, theme, tr!("Nothing here"), tr!("This folder holds no messages."));
        } else {
            empty(ui, theme, tr!("No messages match"), &tr!("No subject, sender or recipient holds “{query}”.", query = mailbox.query()));
        }
        return Asked { out: true, ..Asked::default() };
    }
    let showcase = mailbox.showcase();
    let inbox = mailbox.place() == Place::View(inboxcraft_model::Role::Inbox) && mailbox.query().is_empty();
    let grouping = inbox && (!showcase.screener.is_empty() || !showcase.groups.is_empty());
    let rows = Rows {
        theme,
        keymap,
        mailbox,
        // Whose each row is shows in the simple list of more than one account; the grouped cards leave it out.
        account_marker: !grouping && mailbox.narrowed().is_none() && mailbox.accounts().len() > 1,
        acted: Cell::new(None),
        leaving: Cell::new((false, false)),
    };
    let clicked = if grouping { grouped(ui, &rows, showcase) } else { plain(ui, &rows) };
    let (drawn, out) = rows.leaving.get();
    Asked { clicked, add: false, acted: rows.acted.get(), out: !drawn || out }
}
