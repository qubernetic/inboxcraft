// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The list side: the header with the view's name, whose mail it is and the search field; the rows; the empty
//! states.

use egui::{Align, Key, Layout, Rect, Response, RichText, Role, ScrollArea, Sense, Stroke, TextEdit, Ui, WidgetInfo, pos2, vec2};

use super::paint::{flag, focus_ring, line};
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
            ui.label(RichText::new(mailbox.title()).size(theme.type_scale.title).color(p.text));
            let count = mailbox.messages().len();
            let shown = if count == 1 { tr!("1 message").to_owned() } else { tr!("{count} messages", count = count) };
            ui.label(RichText::new(tr!("{whose} · {shown}", whose = mailbox.subtitle(), shown = shown)).size(theme.type_scale.small).color(p.muted));
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
        ui.label(RichText::new(title).size(theme.type_scale.heading).color(theme.palette.text));
        ui.add_space(theme.spacing.xs);
        ui.label(RichText::new(detail).size(theme.type_scale.body).color(theme.palette.muted));
    });
}

/// One message row. Returns its response; a click selects it.
fn row(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, message: &MessageRow, account_marker: bool) -> Response {
    let s = &theme.spacing;
    let p = &theme.palette;
    let t = &theme.type_scale;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), s.row_height), Sense::click());
    let selected = mailbox.selected() == Some(message.id);
    response.widget_info(|| WidgetInfo::selected(Role::ListBoxOption, true, selected, mailbox.spoken(message)));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter();
    let fill = if selected {
        p.selection
    } else if response.hovered() {
        p.background
    } else {
        p.surface
    };
    painter.rect_filled(rect, 0.0, fill);
    painter.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, p.panel));
    if account_marker {
        let index = mailbox.account(message.account).map_or(0, |account| account.index);
        let marker = Rect::from_min_size(pos2(rect.left(), rect.top() + s.s), vec2(3.0, s.row_height - 2.0 * s.s));
        painter.rect_filled(marker, 1.0, theme.account_colour(u64::try_from(index).unwrap_or(0)));
    }
    let left = rect.left() + s.l + s.s;
    let right = rect.right() - s.l;
    let first = rect.top() + s.s;
    let second = first + t.body + s.xs * 1.5;
    if !message.seen {
        painter.circle_filled(pos2(rect.left() + s.m, first + t.body / 2.0 + 1.0), 3.5, p.unread);
    }
    let date = line(ui, &[(&mailbox.when(message), p.muted)], t.small, f32::INFINITY);
    let date_left = right - date.size().x;
    painter.galley(pos2(date_left, first + (t.body - t.small) / 2.0), date, p.muted);
    let sender_colour = if message.seen { p.text } else { p.accent };
    let sender = line(ui, &[(&message.sender, sender_colour)], t.body, date_left - s.m - left);
    painter.galley(pos2(left, first), sender, sender_colour);
    let flag_room = if message.flagged { t.body } else { 0.0 };
    let subject = if message.subject.is_empty() { tr!("(no subject)") } else { message.subject.as_str() };
    let preview = format!("   {}", message.preview);
    let text = line(ui, &[(subject, p.text), (&preview, p.muted)], t.body, right - flag_room - left);
    painter.galley(pos2(left, second), text, p.text);
    if message.flagged {
        flag(ui, p.flag, pos2(right - t.body * 0.7, second + 2.0), t.body - 3.0);
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
    ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, theme.spacing.row_height, count, |ui, range| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        for message in mailbox.messages().get(range).unwrap_or_default() {
            if row(ui, theme, mailbox, message, account_marker).clicked() {
                clicked = Some(message.id);
            }
        }
    });
    (clicked, false)
}
