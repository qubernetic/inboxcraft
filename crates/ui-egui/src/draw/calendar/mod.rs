// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendar screen: the side panel on a card at the left (the mini month, the calendars with a box each, and the
//! details of the picked event), and the view on a card beside it, under a toolbar with the view's title, the steps
//! back, to today and on, and the views as a segmented control. Without a calendar one card in the window's middle says there are none
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

use egui::{
    Align, CentralPanel, Color32, CornerRadius, Layout, Panel, Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2,
};

use super::paint::{line, line_height};
use super::{around, margin};
use crate::calendar::{Calendar, Day, EventRow, Picked, View};
use crate::component::button::{IconButton, TextButton};
use crate::component::corner;
use crate::component::segmented::segmented;
use crate::glass::Layers;
use crate::iconset::{IconId, Style};
use crate::keymap::Keymap;
use crate::theme::{Face, Font, Theme, Weight};
use crate::tr;

/// The width of the side panel with the gaps around its card, in points: the card is 240 wide as the design draws it.
const SIDE_WIDTH: f32 = 255.0;

/// Between the toolbar and the view, in points.
const UNDER_TOOLBAR: f32 = 10.0;

/// The toolbar's height, in points: everything in it is centred on one line.
const TOOLBAR: f32 = 36.0;

/// The room over the toolbar, in points.
const OVER_TOOLBAR: i8 = 14;

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
        .show(ui, |ui| layers.card(ui, margin(s.l), |ui| side::show(ui, theme, keymap, calendar, &mut asked)));
    CentralPanel::no_frame().frame(around(s.gap / 2.0, s.gap, s.gap)).show(ui, |ui| {
        let room = egui::Margin { top: OVER_TOOLBAR, ..margin(s.l) };
        layers.card(ui, room, |ui| {
            toolbar(ui, theme, keymap, calendar, &mut asked);
            ui.add_space(UNDER_TOOLBAR);
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

/// The toolbar over the view: its title, the steps back, to today and on, and the views as a segmented control and
/// New event at the right.
fn toolbar(ui: &mut Ui, theme: &Theme, keymap: &Keymap, calendar: &Calendar, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let (back, on) = match calendar.view() {
        View::Day => (tr!("Previous day"), tr!("Next day")),
        View::Week => (tr!("Previous week"), tr!("Next week")),
        View::Month => (tr!("Previous month"), tr!("Next month")),
        View::Agenda => (tr!("Earlier days"), tr!("Later days")),
    };
    let font = theme.fonts.button;
    let views = View::ALL.map(|view| (view, view.text(), font));
    let title = Font { size: theme.fonts.title.size * 1.1, ..theme.fonts.title };
    let row = vec2(ui.available_width(), TOOLBAR);
    ui.allocate_ui_with_layout(row, Layout::left_to_right(Align::Center), |ui| {
        ui.set_min_height(TOOLBAR);
        ui.spacing_mut().item_spacing.x = 10.0;
        ui.label(RichText::new(calendar.title()).font(title.id()).color(p.text));
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
            if let Some(view) = segmented(ui, theme, &views, calendar.view()) {
                asked.push(Asked::View(view));
            }
        });
    });
}

/// The event editor over `screen`, while it is open; what was pressed in it.
pub(super) fn editor(ctx: &egui::Context, theme: &Theme, keymap: &Keymap, calendar: &mut Calendar, screen: Rect) -> Vec<Asked> {
    calendar.editor_mut().map(|editor| editor::show(ctx, theme, keymap, editor, screen)).unwrap_or_default()
}

/// An event's fill and its edge at full strength, for a calendar with the colour `server` gave it, if any, at `place`
/// in the list: the edge is the server's colour or one of the accounts' by place, the fill the event token of the place
/// or, for a server's colour, that colour laid as strongly as the token lays its own.
fn colours(theme: &Theme, server: Option<[u8; 3]>, place: usize) -> (Color32, Color32) {
    let p = &theme.palette;
    let token = p.events.get(place % p.events.len()).copied().unwrap_or(p.selection);
    let edge = p.accounts.get(place % p.accounts.len()).copied().unwrap_or(p.accent);
    match server {
        Some([r, g, b]) => {
            let edge = Color32::from_rgb(r, g, b);
            (edge.gamma_multiply(f32::from(token.a()) / 255.0), edge)
        }
        None => (token, edge),
    }
}

/// The colours of the events of calendar `id`.
fn calendar_colours(theme: &Theme, calendar: &Calendar, id: u64) -> (Color32, Color32) {
    colours(theme, calendar.calendar(id).and_then(|row| row.colour), calendar.index_of(id))
}

/// The colour of calendar `id` at full strength: its dot.
fn tint(theme: &Theme, calendar: &Calendar, id: u64) -> Color32 {
    calendar_colours(theme, calendar, id).1
}

/// `theme`'s caption in the SemiBold weight: an event's summary.
fn caption_strong(theme: &Theme) -> Font {
    Font { face: Face::Sans(Weight::SemiBold), ..theme.fonts.caption }
}

/// The caption in the Bold weight: today's number.
fn caption_bold(theme: &Theme) -> Font {
    Font { face: Face::Sans(Weight::Bold), ..theme.fonts.caption }
}

/// How a block is drawn where it shows, as the design draws each place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// A timed event in the hours: a wide edge, its text at the top.
    Timed,
    /// An event over whole days, in a lane.
    Band,
    /// A line in a month's cell, in plain weight.
    Chip,
    /// A row of the agenda.
    Row,
}

impl Shape {
    /// The width of the edge, the room from it to the text, and the corner, in points.
    fn metrics(self) -> (f32, f32, f32) {
        match self {
            Self::Timed => (4.0, 10.0, 4.0),
            Self::Band => (3.0, 8.0, 3.0),
            Self::Chip => (3.0, 5.0, 3.0),
            Self::Row => (3.0, 10.0, 4.0),
        }
    }
}

/// What an event's block says: its first line, and a second one while there is room.
struct Text<'a> {
    title: &'a str,
    detail: Option<&'a str>,
    shape: Shape,
}

/// An event drawn over `rect`: a tint of its calendar's colour with an edge of it at full strength on the left and its
/// `text`; a button named after the event, selected while it is picked. `salt` tells apart the places one event shows
/// in. A press picks it, or lets it go when it was picked.
fn block(ui: &mut Ui, theme: &Theme, calendar: &Calendar, event: &EventRow, rect: Rect, text: &Text<'_>, salt: i64) -> Option<Asked> {
    let picked = calendar.picked().is_some_and(|picked| picked.key() == event.key());
    let id = ui.scope_id().with(("calendar-event", event.id, event.start, salt));
    let response = ui.interact(rect, id, Sense::click());
    response.widget_info(|| WidgetInfo::selected(egui::accesskit::Role::Button, true, picked, event.spoken()));
    if ui.is_rect_visible(rect) {
        paint_block(ui, theme, calendar_colours(theme, calendar, event.calendar), rect, &response, picked, text);
    }
    response.clicked().then(|| Asked::Pick(if picked { None } else { Some(event.key()) }))
}

fn paint_block(ui: &Ui, theme: &Theme, (fill, edge): (Color32, Color32), rect: Rect, response: &Response, picked: bool, text: &Text<'_>) {
    let p = &theme.palette;
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let (edge_width, pad, round) = text.shape.metrics();
    painter.rect_filled(rect, round, fill);
    let deeper = if picked {
        0.27
    } else if response.hovered() {
        0.12
    } else {
        0.0
    };
    if deeper > 0.0 {
        painter.rect_filled(rect, round, edge.gamma_multiply(deeper));
    }
    let round_left = CornerRadius { nw: corner(round).nw, sw: corner(round).sw, ne: 0, se: 0 };
    painter.rect_filled(Rect::from_min_size(rect.min, vec2(edge_width.min(rect.width()), rect.height())), round_left, edge);
    let left = rect.left() + edge_width + pad;
    let width = (rect.right() - left - 2.0).max(1.0);
    let font = if text.shape == Shape::Chip { theme.fonts.caption } else { caption_strong(theme) };
    let title = line(ui, &[(text.title, font, p.text)], width);
    let height = title.size().y;
    if let Some(detail) = text.detail {
        let top = rect.top() + 6.0;
        painter.galley(pos2(left, top), title, p.text);
        if rect.height() >= height * 2.0 + 8.0 {
            let detail = line(ui, &[(detail, theme.fonts.caption, p.muted)], width);
            painter.galley(pos2(left, top + height + 2.0), detail, p.muted);
        }
    } else {
        painter.galley(pos2(left, rect.center().y - height / 2.0), title, p.text);
    }
    if response.has_focus() {
        painter.rect_stroke(rect.shrink(1.0), round, Stroke::new(2.0, p.focus), StrokeKind::Inside);
    }
}

#[cfg(test)]
mod tests;
