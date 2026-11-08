// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendar screen: the side panel on a card at the left (the mini month, the calendars with a box each, and the
//! details of the picked event), and the view on a card beside it, under a toolbar with the view's title, the steps
//! back, to today and on, and a pill per view. Without a calendar one card in the window's middle says there are none
//! yet and what will show here.
//!
//! The day and week views lay the hours out under a lane of blocks over whole days (`hours`), the month view the six
//! weeks of the month with the blocks across the days (`month`), the agenda the days that hold events (`agenda`).
//! Every event is a button named after its summary and its days and times; pressing it picks it, pressing it again
//! lets it go. Drawing only reads the [`Calendar`]; what was pressed comes back as [`Asked`] and is applied after
//! the frame. New event at the toolbar's end, and Edit and Delete under the picked event's details, open the event
//! editor (`editor`) or delete the event; the editor floats over the screen and changes its form as it is used.

mod agenda;
mod editor;
mod hours;
mod month;
mod side;

use egui::{Align, CentralPanel, Color32, Layout, Panel, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use super::paint::{line, line_height};
use super::{around, margin};
use crate::calendar::{Calendar, Day, EventRow, Picked, View};
use crate::component::button::{IconButton, TextButton};
use crate::component::pill::pill;
use crate::glass::Layers;
use crate::iconset::{IconId, Style};
use crate::keymap::Keymap;
use crate::theme::{Font, Theme};
use crate::tr;

/// The width of the side panel, in points.
const SIDE_WIDTH: f32 = 260.0;

/// What was pressed on the calendar screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Asked {
    /// A view's pill.
    View(View),
    /// The step on (`true`) or back.
    Step(bool),
    /// Today.
    Today,
    /// The mini month's step on (`true`) or back.
    Month(bool),
    /// A day of the mini month: the view goes there.
    Go(Day),
    /// A day's number in a view: the day view of that day.
    Open(Day),
    /// A calendar's box.
    Toggle(u64),
    /// An event pressed, or the details closed.
    Pick(Option<Picked>),
    /// New event: the editor on a new event.
    New,
    /// Edit: the editor on the picked event.
    Edit,
    /// Delete: the picked event goes.
    Delete,
    /// The editor's Save.
    Save,
    /// The editor's Cancel.
    Cancel,
}

/// The calendar card's content while there is no calendar: what will show here.
pub(super) fn empty(ui: &mut Ui, theme: &Theme) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let side = line_height(ui, theme.fonts.body) * 3.6;
    let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    theme.icons.paint(ui.painter(), rect, IconId::Calendar, Style::Duotone, p.accent);
    ui.add_space(s.m);
    ui.label(RichText::new(tr!("No calendars yet")).font(theme.fonts.title.id()).color(p.text));
    ui.add_space(s.xs);
    let about = tr!("The calendars of your accounts show here, by day, week, month and agenda, once an account has a CalDAV address.");
    ui.add(egui::Label::new(RichText::new(about).font(theme.fonts.body.id()).color(p.muted)).wrap());
}

/// The calendar screen with its calendars: the side panel and the view; what was pressed.
pub(super) fn screen(ui: &mut Ui, theme: &Theme, keymap: &Keymap, calendar: &Calendar, layers: &mut Layers) -> Vec<Asked> {
    let s = theme.spacing;
    let mut asked = Vec::new();
    Panel::left("calendar-side")
        .resizable(false)
        .exact_size(SIDE_WIDTH)
        .show_separator_line(false)
        .frame(around(s.gap, s.gap / 2.0, s.gap))
        .show(ui, |ui| layers.card(ui, margin(s.m), |ui| side::show(ui, theme, keymap, calendar, &mut asked)));
    CentralPanel::no_frame().frame(around(s.gap / 2.0, s.gap, s.gap)).show(ui, |ui| {
        layers.card(ui, margin(s.l), |ui| {
            toolbar(ui, theme, keymap, calendar, &mut asked);
            ui.add_space(s.m);
            match calendar.view() {
                View::Day => hours::show(ui, theme, calendar, 1, &mut asked),
                View::Week => hours::show(ui, theme, calendar, 7, &mut asked),
                View::Month => month::show(ui, theme, calendar, &mut asked),
                View::Agenda => agenda::show(ui, theme, calendar, &mut asked),
            }
        });
    });
    asked
}

/// The toolbar over the view: its title, the steps back, to today and on, and the views' pills at the right.
fn toolbar(ui: &mut Ui, theme: &Theme, keymap: &Keymap, calendar: &Calendar, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let (back, on) = match calendar.view() {
        View::Day => (tr!("Previous day"), tr!("Next day")),
        View::Week => (tr!("Previous week"), tr!("Next week")),
        View::Month => (tr!("Previous month"), tr!("Next month")),
        View::Agenda => (tr!("Earlier days"), tr!("Later days")),
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(calendar.title()).font(theme.fonts.title.id()).color(p.text));
        ui.add_space(theme.spacing.m);
        if IconButton::new(IconId::Previous, back).show(ui, theme, keymap).clicked() {
            asked.push(Asked::Step(false));
        }
        if TextButton::new(tr!("Today")).show(ui, theme, keymap).clicked() {
            asked.push(Asked::Today);
        }
        if IconButton::new(IconId::Next, on).show(ui, theme, keymap).clicked() {
            asked.push(Asked::Step(true));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if TextButton::new(tr!("New event")).primary().icon(IconId::Add).show(ui, theme, keymap).clicked() {
                asked.push(Asked::New);
            }
            ui.add_space(theme.spacing.s);
            for view in View::ALL.into_iter().rev() {
                if pill(ui, theme, view.text(), calendar.view() == view).clicked() {
                    asked.push(Asked::View(view));
                }
            }
        });
    });
}

/// The event editor over `screen`, while it is open; what was pressed in it.
pub(super) fn editor(ctx: &egui::Context, theme: &Theme, keymap: &Keymap, calendar: &mut Calendar, screen: Rect) -> Vec<Asked> {
    calendar.editor_mut().map(|editor| editor::show(ctx, theme, keymap, editor, screen)).unwrap_or_default()
}

/// The colour of calendar `id`: the server's, or one of the accounts' colours by its place in the list.
fn tint(theme: &Theme, calendar: &Calendar, id: u64) -> Color32 {
    match calendar.calendar(id).and_then(|row| row.colour) {
        Some([r, g, b]) => Color32::from_rgb(r, g, b),
        None => {
            let colours = &theme.palette.accounts;
            colours.get(calendar.index_of(id) % colours.len()).copied().unwrap_or(theme.palette.accent)
        }
    }
}

/// The caption size in the strong weight: an event's summary.
fn caption_strong(theme: &Theme) -> Font {
    Font { size: theme.fonts.caption.size, ..theme.fonts.strong }
}

/// What an event's block says: its first line, and a second one while there is room.
struct Text<'a> {
    title: &'a str,
    detail: Option<&'a str>,
}

/// An event drawn over `rect`: a tint of its calendar's colour with a bar of it on the left and its `text`; a button
/// named after the event, selected while it is picked. `salt` tells apart the places one event shows in. A press
/// picks it, or lets it go when it was picked.
fn block(ui: &mut Ui, theme: &Theme, calendar: &Calendar, event: &EventRow, rect: Rect, text: &Text<'_>, salt: i64) -> Option<Asked> {
    let picked = calendar.picked().is_some_and(|picked| picked.key() == event.key());
    let id = ui.scope_id().with(("calendar-event", event.id, event.start, salt));
    let response = ui.interact(rect, id, Sense::click());
    response.widget_info(|| WidgetInfo::selected(egui::accesskit::Role::Button, true, picked, event.spoken()));
    if ui.is_rect_visible(rect) {
        paint_block(ui, theme, tint(theme, calendar, event.calendar), rect, &response, picked, text);
    }
    response.clicked().then(|| Asked::Pick(if picked { None } else { Some(event.key()) }))
}

fn paint_block(ui: &Ui, theme: &Theme, colour: Color32, rect: Rect, response: &Response, picked: bool, text: &Text<'_>) {
    let p = &theme.palette;
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let strength = if picked {
        0.45
    } else if response.hovered() {
        0.3
    } else {
        0.18
    };
    let corner = 4.0;
    painter.rect_filled(rect, corner, colour.gamma_multiply(strength));
    painter.rect_filled(Rect::from_min_size(rect.min, vec2(3.0_f32.min(rect.width()), rect.height())), corner, colour);
    let pad = 6.0;
    let width = (rect.width() - pad - 2.0).max(1.0);
    let title = line(ui, &[(text.title, caption_strong(theme), p.text)], width);
    let height = title.size().y;
    painter.galley(pos2(rect.left() + pad, rect.top() + 2.0), title, p.text);
    if let Some(detail) = text.detail
        && rect.height() >= height * 2.0 + 4.0
    {
        let detail = line(ui, &[(detail, theme.fonts.caption, p.muted)], width);
        painter.galley(pos2(rect.left() + pad, rect.top() + 2.0 + height), detail, p.muted);
    }
    if response.has_focus() {
        painter.rect_stroke(rect.shrink(1.0), corner, Stroke::new(2.0, p.focus), StrokeKind::Inside);
    }
}

#[cfg(test)]
mod tests;
