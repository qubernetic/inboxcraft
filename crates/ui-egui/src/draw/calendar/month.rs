// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The month view: the six weeks from the Monday on or before the month's first day, a cell per day with its number
//! (a button to the day view; today in the accent, the days of other months muted). Blocks over whole days run
//! across the cells of a week in lanes; under them the day's timed events, one line each with the start, as many as
//! fit, and a "+N more" that opens the day.

use egui::{Rect, Sense, Stroke, Ui, WidgetInfo, pos2, vec2};

use super::{Asked, Text, block, caption_strong};
use crate::calendar::layout::{self, Bar};
use crate::calendar::{Calendar, Day, EventRow, day};
use crate::draw::paint::{line, line_height};
use crate::theme::Theme;
use crate::tr;

/// The weeks shown.
const WEEKS: i64 = 6;

/// Where the cells are.
struct Grid {
    left: f32,
    top: f32,
    /// A cell's width and height.
    column: f32,
    row: f32,
    /// The height of a line of events.
    lane: f32,
    /// The height of the day's number at the top of a cell.
    number: f32,
    /// How many lines fit under a day's number.
    room: usize,
    /// The month shown.
    month: u8,
}

impl Grid {
    fn cell(&self, week: i64, index: usize) -> Rect {
        Rect::from_min_size(pos2(self.left + self.column * index as f32, self.top + self.row * week as f32), vec2(self.column, self.row))
    }
}

/// The month view of the calendar's focus.
pub(super) fn show(ui: &mut Ui, theme: &Theme, calendar: &Calendar, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let (first, _) = calendar.span();
    let events: Vec<&EventRow> = calendar.events().collect();
    let width = ui.available_width();
    let column = (width / 7.0).max(1.0);

    let head = line_height(ui, theme.fonts.caption) + s.s;
    let (rect, _) = ui.allocate_exact_size(vec2(width, head), Sense::hover());
    for index in 0..7_i64 {
        let name = day::weekday_short(first.plus(index).weekday());
        let galley = line(ui, &[(name, theme.fonts.caption, p.muted)], column);
        ui.painter().galley(pos2(rect.left() + column * index as f32 + s.xs, rect.top()), galley, p.muted);
    }

    let rect = ui.available_rect_before_wrap();
    ui.allocate_rect(rect, Sense::hover());
    let row = (rect.height() / WEEKS as f32).max(1.0);
    let lane = line_height(ui, caption_strong(theme)) + s.xs;
    let number = line_height(ui, theme.fonts.caption) + s.xs * 2.0;
    // A count of lines, never negative.
    let room = ((row - number) / lane).floor().max(0.0) as usize;
    let grid = Grid { left: rect.left(), top: rect.top(), column, row, lane, number, room, month: calendar.focus().month() };
    for week in 0..WEEKS {
        let top = grid.top + row * week as f32;
        ui.painter().hline(rect.left()..=rect.right(), top, Stroke::new(1.0, p.line));
        for index in 1..7 {
            let x = grid.left + column * index as f32;
            ui.painter().vline(x, top..=top + row, Stroke::new(1.0, p.line));
        }
        week_row(ui, theme, calendar, &events, &grid, week, asked);
    }
}

/// The days of week `week` of the grid: their numbers, the blocks across them and their timed events.
fn week_row(ui: &mut Ui, theme: &Theme, calendar: &Calendar, events: &[&EventRow], grid: &Grid, week: i64, asked: &mut Vec<Asked>) {
    let (first, _) = calendar.span();
    let start = first.plus(week * 7);
    let bars = layout::bars(events, start, 7);
    let total = layout::lanes(&bars);
    // The lanes shown: all, or one less than fit so that a line stays for "+N more".
    let lanes = if total <= grid.room { total } else { grid.room.saturating_sub(1) };
    for bar in bars.iter().filter(|bar| bar.lane < lanes) {
        let Some(event) = events.get(bar.event) else { continue };
        let (from, to) = (grid.cell(week, bar.from), grid.cell(week, bar.to));
        let y = from.top() + grid.number + grid.lane * bar.lane as f32;
        let area = Rect::from_min_max(pos2(from.left() + 2.0, y), pos2(to.right() - 2.0, y + grid.lane - 2.0));
        asked.extend(block(ui, theme, calendar, event, area, &Text { title: &event.summary, detail: None }, week));
    }
    for index in 0..7_usize {
        let day = start.plus(i64::try_from(index).unwrap_or(0));
        let cell = grid.cell(week, index);
        day_number(ui, theme, calendar, day, grid, cell, asked);
        day_events(ui, theme, calendar, events, grid, (week, index, day), &bars, lanes, asked);
    }
}

/// The timed events of `day` (in column `index` of `week`) under the `lanes` of blocks shown, and "+N more" for
/// what does not fit, blocks of the day left out included.
#[expect(clippy::too_many_arguments, reason = "the day's place in the grid and the blocks already laid out")]
fn day_events(
    ui: &mut Ui,
    theme: &Theme,
    calendar: &Calendar,
    events: &[&EventRow],
    grid: &Grid,
    (week, index, day): (i64, usize, Day),
    bars: &[Bar],
    lanes: usize,
    asked: &mut Vec<Asked>,
) {
    let p = &theme.palette;
    let cell = grid.cell(week, index);
    let hidden = bars.iter().filter(|bar| bar.lane >= lanes && bar.from <= index && index <= bar.to).count();
    let timed: Vec<&EventRow> = events.iter().copied().filter(|event| !event.is_block() && event.covers(day)).collect();
    let free = grid.room.saturating_sub(lanes);
    let all = hidden.saturating_add(timed.len());
    let lines = if all > free { free.saturating_sub(1).min(timed.len()) } else { timed.len() };
    let mut y = cell.top() + grid.number + grid.lane * lanes as f32;
    let salt = week.saturating_mul(7).saturating_add(i64::try_from(index).unwrap_or(0));
    for event in timed.iter().take(lines) {
        let area = Rect::from_min_size(pos2(cell.left() + 2.0, y), vec2(cell.width() - 4.0, grid.lane - 2.0));
        let title = format!("{} {}", event.start.clock(), event.summary);
        asked.extend(block(ui, theme, calendar, event, area, &Text { title: &title, detail: None }, salt));
        y += grid.lane;
    }
    let left_out = all.saturating_sub(lines);
    if left_out == 0 || free == 0 {
        return;
    }
    let area = Rect::from_min_size(pos2(cell.left() + 2.0, y), vec2(cell.width() - 4.0, grid.lane - 2.0));
    let response = ui.interact(area, ui.scope_id().with(("calendar-more", week, index)), Sense::click());
    let spoken = tr!("{count} more on {day}", count = left_out, day = day::long(day));
    response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Button, true, &spoken));
    let galley = line(ui, &[(&tr!("+{count} more", count = left_out), theme.fonts.caption, p.muted)], area.width());
    ui.painter().galley(pos2(area.left() + theme.spacing.xs, area.top()), galley, p.muted);
    if response.clicked() {
        asked.push(Asked::Open(day));
    }
}

/// The number of `day` at the top of its `cell`, a button that opens the day; today in the accent, a day of another
/// month than the grid's muted.
fn day_number(ui: &mut Ui, theme: &Theme, calendar: &Calendar, day: Day, grid: &Grid, cell: Rect, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let today = day == calendar.today();
    let colour = if today {
        p.on_accent
    } else if day.month() == grid.month {
        p.text
    } else {
        p.muted
    };
    let galley = line(ui, &[(&day.date().to_string(), caption_strong(theme), colour)], cell.width());
    let side = grid.number - s.xs;
    let spot = Rect::from_min_size(pos2(cell.left() + s.xs, cell.top() + s.xs / 2.0), vec2(side.max(galley.size().x + s.s), side));
    let response = ui.interact(spot, ui.scope_id().with(("calendar-number", day)), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Button, true, day::long(day)));
    if today {
        ui.painter().rect_filled(spot, side / 2.0, p.accent);
    } else if response.hovered() {
        ui.painter().rect_filled(spot, side / 2.0, p.hover);
    }
    ui.painter().galley(spot.center() - galley.size() / 2.0, galley, colour);
    if response.clicked() {
        asked.push(Asked::Open(day));
    }
}
