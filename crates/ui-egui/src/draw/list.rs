// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The list side: the header with the view's name, whose mail it is and the search field; the rows; the empty
//! states.

use egui::{Align, Key, Layout, Rect, Response, RichText, Role, ScrollArea, Sense, Stroke, TextEdit, Ui, WidgetInfo, pos2, vec2};

use super::paint::{flag, focus_ring, line, line_height, selection};
use crate::mailbox::{Mailbox, MessageRow};
use crate::theme::Theme;
use crate::tr;

/// The search field's id, so a shortcut can focus it.
pub(super) fn search_id() -> egui::Id {
    egui::Id::unique("mail-search")
}

/// The header above the list. Returns the search text when it changed this frame.
pub(super) fn header(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, search: &mut String) -> Option<String> {
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
            if !search.is_empty() && ui.button(tr!("Clear")).clicked() {
                search.clear();
                changed = Some(String::new());
            }
            let field = TextEdit::singleline(search).id(search_id()).hint_text(tr!("Search mail")).desired_width(theme.spacing.folder_pane);
            let response = ui.add(field);
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

/// The height of a message row: its two lines and air, from the font's metrics at the current text scale.
pub(super) fn row_height(ui: &Ui, theme: &Theme) -> f32 {
    let (first, second) = row_lines(ui, theme);
    theme.spacing.s * 2.0 + first + theme.spacing.xs + second
}

/// One message row. Returns its response; a click selects it.
fn row(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, message: &MessageRow, account_marker: bool) -> Response {
    let s = &theme.spacing;
    let p = &theme.palette;
    let f = &theme.fonts;
    let height = row_height(ui, theme);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let selected = mailbox.selected() == Some(message.id);
    response.widget_info(|| WidgetInfo::selected(Role::ListBoxOption, true, selected, mailbox.spoken(message)));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter();
    if selected {
        selection(painter, theme, rect, 0.0);
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, p.hover);
    }
    painter.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, p.line));
    if account_marker {
        let index = mailbox.account(message.account).map_or(0, |account| account.index);
        let marker = Rect::from_min_size(pos2(rect.left(), rect.top() + s.s), vec2(3.0, height - 2.0 * s.s));
        painter.rect_filled(marker, 1.0, theme.account_colour(u64::try_from(index).unwrap_or(0)));
    }
    let (first_height, second_height) = row_lines(ui, theme);
    let left = rect.left() + s.l + s.s;
    let right = rect.right() - s.l;
    let first = rect.top() + s.s;
    let second = first + first_height + s.xs;
    let (sender_font, subject_font) = if message.seen { (f.sender, f.subject) } else { (f.sender_unread, f.subject_unread) };
    if !message.seen {
        painter.circle_filled(pos2(rect.left() + s.m, first + first_height / 2.0), 3.5, p.unread);
    }
    // Both lines sit on the middle of their line, so a smaller date lines up with the sender.
    let date = line(ui, &[(&mailbox.when(message), f.caption, p.muted)], f32::INFINITY);
    let date_left = right - date.size().x;
    painter.galley(pos2(date_left, first + (first_height - date.size().y) / 2.0), date, p.muted);
    let sender = line(ui, &[(&message.sender, sender_font, p.text)], date_left - s.m - left);
    painter.galley(pos2(left, first + (first_height - sender.size().y) / 2.0), sender, p.text);
    let flag_size = (second_height * 0.75).max(1.0);
    let flag_room = if message.flagged { flag_size + s.s } else { 0.0 };
    let subject = if message.subject.is_empty() { tr!("(no subject)") } else { message.subject.as_str() };
    let preview = format!("   {}", message.preview);
    let text = line(ui, &[(subject, subject_font, p.text), (&preview, f.preview, p.muted)], right - flag_room - left);
    painter.galley(pos2(left, second + (second_height - text.size().y) / 2.0), text, p.text);
    if message.flagged {
        flag(ui, p.flag, pos2(right - flag_size * 0.8, second + (second_height - flag_size) / 2.0), flag_size);
    }
    focus_ring(ui, theme, &response);
    response
}

/// The rows, or what to say when there are none. Returns the message clicked this frame, and whether Add account
/// was (the empty window offers it when `can_add`).
pub(super) fn rows(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, can_add: bool) -> (Option<u64>, bool) {
    if mailbox.accounts().is_empty() {
        if !can_add {
            empty(ui, theme, tr!("No accounts yet"), tr!("Start InboxCraft with --sample to look around an invented mailbox."));
            return (None, false);
        }
        empty(ui, theme, tr!("No accounts yet"), tr!("Add an account to read its mail here. Nothing leaves this computer before you do."));
        let mut add = false;
        ui.vertical_centered(|ui| {
            ui.add_space(theme.spacing.l);
            add = ui.button(tr!("Add account")).clicked();
        });
        return (None, add);
    }
    if mailbox.messages().is_empty() {
        if mailbox.query().is_empty() {
            empty(ui, theme, tr!("Nothing here"), tr!("This folder holds no messages."));
        } else {
            empty(ui, theme, tr!("No messages match"), &tr!("No subject, sender or recipient holds “{query}”.", query = mailbox.query()));
        }
        return (None, false);
    }
    let account_marker = mailbox.narrowed().is_none() && mailbox.accounts().len() > 1;
    let mut clicked = None;
    let count = mailbox.messages().len();
    let height = row_height(ui, theme);
    ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, height, count, |ui, range| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        for message in mailbox.messages().get(range).unwrap_or_default() {
            if row(ui, theme, mailbox, message, account_marker).clicked() {
                clicked = Some(message.id);
            }
        }
    });
    (clicked, false)
}
