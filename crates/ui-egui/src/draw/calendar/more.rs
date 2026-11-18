// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The events of one day in a float, as "+N more" opens it: the day's name and count on the float's title row, then a
//! row per event (its time or `all day`, its account's bar, its title, its calendar at the right; the picked one on the
//! `stage` with a `cyan_edge` bar), at most [`MOST_ROWS`] of them, and Open the day. A press on a row picks the event
//! and closes the float; Esc or a press outside closes it.

use egui::{Context, Id, Rect, Sense, Ui, WidgetInfo, pos2, vec2};

use super::grid::{pixels, snap};
use super::{Asked, bar_colour};
use crate::calendar::{Calendar, Day, EventRow, day};
use crate::component::popup::{self, Float};
use crate::draw::paint::{run_at, run_right};
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

/// The float's width, in points, as the design draws it.
const WIDTH: f32 = 320.0;

/// A row's height, in points.
const ROW: f32 = 32.0;

/// The most rows the float lists; the rest are in the day view, which Open the day shows.
pub(super) const MOST_ROWS: usize = 50;

/// The float's id.
fn float_id() -> Id {
    Id::unique("calendar-more-on-day")
}

/// Where "+N more" of `day` was drawn last: the float hangs from it.
pub(super) fn anchor_id(day: Day) -> Id {
    Id::unique(("calendar-more-anchor", day))
}

/// The id of the row of `event` in the float.
pub(super) fn row_id(event: &EventRow) -> Id {
    Id::unique(("calendar-more-row", event.id, event.start))
}

/// The float of `day`'s events.
pub(super) fn show(ctx: &Context, theme: &Theme, keymap: &Keymap, calendar: &Calendar, day: Day, asked: &mut Vec<Asked>) {
    let anchor =
        ctx.data(|data| data.get_temp::<Rect>(anchor_id(day))).unwrap_or_else(|| Rect::from_center_size(ctx.content_rect().center(), vec2(1.0, 1.0)));
    let float = Float { id: float_id(), anchor, width: WIDTH, returns_to: Id::NULL };
    let events: Vec<&EventRow> = calendar.events().filter(|event| event.covers(day)).collect();
    let shown = popup::float(ctx, theme, &float, |ui| {
        let count = if events.len() == 1 { tr!("1 event").to_owned() } else { tr!("{count} events", count = events.len()) };
        popup::title(ui, theme, &day::dated(day), Some(&count));
        let mut out = Vec::new();
        popup::scrolled(ui, theme, float_id().with("rows"), ROW * 12.0, |ui| {
            for event in events.iter().take(MOST_ROWS) {
                out.extend(row(ui, theme, calendar, event, day));
            }
        });
        if let Some(more) = events.len().checked_sub(MOST_ROWS).filter(|more| *more > 0) {
            popup::empty(ui, theme, &tr!("and {count} more in the day", count = more));
        }
        popup::hairline(ui, theme);
        if popup::item(ui, theme, keymap, IconId::Calendar, tr!("Open the day"), None).clicked() {
            out.push(Asked::Open(day));
            out.push(Asked::More(None));
        }
        out
    });
    match shown {
        Some(out) => asked.extend(out),
        None => asked.push(Asked::More(None)),
    }
}

/// One event of the day: a button that picks it and closes the float.
fn row(ui: &mut Ui, theme: &Theme, calendar: &Calendar, event: &EventRow, day: Day) -> Vec<Asked> {
    let p = &theme.palette;
    let ppp = ui.pixels_per_point();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::hover());
    let response = ui.interact(rect, row_id(event), Sense::click());
    let picked = calendar.picked().is_some_and(|picked| picked.key() == event.key());
    response.widget_info(|| WidgetInfo::selected(egui::accesskit::Role::Button, true, picked, event.spoken()));
    if picked || response.hovered() || response.has_focus() {
        ui.painter().rect_filled(rect, 0.0, p.stage);
    }
    if picked {
        ui.painter().rect_filled(Rect::from_min_size(rect.min, vec2(pixels(2.0, ppp), rect.height())), 0.0, p.cyan_edge);
    }
    let middle = rect.center().y;
    let time = if event.all_day || event.start.day != day { tr!("all day").to_owned() } else { event.start.clock() };
    let time = crate::component::cut(ui, &time, theme.fonts.mono_s, p.muted, 52.0);
    run_at(ui.painter(), pos2(rect.left() + 12.0, snap(middle - theme.fonts.mono_s.line / 2.0, ppp)), time, p.muted);
    let bar = Rect::from_min_size(pos2(rect.left() + 72.0, snap(middle - 6.0, ppp)), vec2(pixels(3.0, ppp), 12.0));
    ui.painter().rect_filled(bar, 0.0, bar_colour(theme, calendar, event.calendar));
    let name = calendar.calendar(event.calendar).map(|row| row.name.clone()).unwrap_or_default();
    let right = rect.right() - 12.0;
    let place = crate::component::cut(ui, &name, theme.fonts.mono_s, p.muted, 80.0);
    let place_width = if name.is_empty() { 0.0 } else { place.size().x.ceil() + 8.0 };
    let title = crate::component::cut(ui, &event.summary, theme.fonts.body, p.ink, (right - (rect.left() + 83.0) - place_width).max(1.0));
    run_at(ui.painter(), pos2(rect.left() + 83.0, snap(middle - theme.fonts.body.line / 2.0, ppp)), title, p.ink);
    if !name.is_empty() {
        run_right(ui.painter(), right, snap(middle - theme.fonts.mono_s.line / 2.0, ppp), place, p.muted);
    }
    if response.clicked() {
        return vec![Asked::Pick(Some(event.key())), Asked::More(None)];
    }
    Vec::new()
}
