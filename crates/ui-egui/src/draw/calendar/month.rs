// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The month view: the weekdays' names, then the six weeks from the Monday on or before the month's first day on a grid
//! of `panel_line` hairlines over the whole view. A cell holds its day's number (a button to the day view; the first of
//! a month with the month's name, the days of other months `muted`, today in a `cyan_edge` box with `today` beside it,
//! the day the arrows picked on `cyan`), the bands over whole days across the week's cells in lanes that share one
//! line, and under them the day's timed events as chips, as many as fit, then a "+N more" link that opens the day's
//! events in a float. Every count is bounded by the cell's room: a day of 500 events draws a few chips and one link.

use egui::{Rect, Sense, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use super::grid::{Lines, lane_pitch, line as line_width, snap};
use super::{Asked, Kind, Sizes, Words, block};
use crate::calendar::layout::{self, Bar};
use crate::calendar::{Calendar, Day, EventRow, day};
use crate::component::button::{LinkSize, link_sized};
use crate::draw::paint::run_at;
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

/// The weeks shown.
const WEEKS: usize = 6;

/// The most lanes of bands a week shows, whatever its room.
const MOST_LANES: usize = 3;

/// From a cell's top to its first band, in points.
const BANDS_TOP: f32 = 24.0;

/// From a cell's top to its first chip under no band, in points.
const CHIPS_TOP: f32 = 26.0;

/// A chip's height and the step from one chip to the next, in points.
const CHIP: (f32, f32) = (20.0, 22.0);

/// The room left and right of a chip in its cell, in points.
const CHIP_SIDE: f32 = 6.0;

/// The id of the number of `day` in the month: the keyboard's focus while the arrows pick days.
pub(super) fn number_id(day: Day) -> egui::Id {
    egui::Id::unique(("calendar-number", day))
}

/// The month view of the calendar's focus.
pub(super) fn show(ui: &mut Ui, theme: &Theme, keymap: &Keymap, calendar: &Calendar, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let ppp = ui.pixels_per_point();
    let line = line_width(ppp);
    let rect = ui.max_rect();
    let (first, _) = calendar.span();
    let events: Vec<&EventRow> = calendar.events().collect();
    let cols = Lines::cut(rect.left(), rect.right(), 7, ppp);

    let head = snap(rect.top() + super::fixed(theme, 28.0), ppp);
    for index in 0..7_usize {
        let name = day::weekday_short(first.plus(i64::try_from(index).unwrap_or(0)).weekday());
        let font = theme.fonts.label;
        let text = crate::component::galley(ui, name, font, p.muted);
        let y = snap((rect.top() + head) / 2.0 - font.line / 2.0, ppp);
        run_at(ui.painter(), pos2(cols.line(index) + 8.0, y), text, p.muted);
    }
    ui.painter().rect_filled(Rect::from_min_max(pos2(rect.left(), head), pos2(rect.right(), head + line)), 0.0, p.line);
    let weeks = Lines::cut(head + line, rect.bottom(), WEEKS, ppp);
    for index in 1..WEEKS {
        let y = weeks.line(index);
        ui.painter().rect_filled(Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + line)), 0.0, p.panel_line);
    }
    for index in 1..7 {
        let x = cols.line(index);
        ui.painter().rect_filled(Rect::from_min_max(pos2(x, weeks.line(0)), pos2(x + line, rect.bottom())), 0.0, p.panel_line);
    }
    let grid = Grid { cols, weeks, month: calendar.focus().month() };
    for week in 0..WEEKS {
        week_row(ui, theme, keymap, calendar, &events, &grid, week, asked);
    }
}

/// Where the cells are.
struct Grid {
    cols: Lines,
    weeks: Lines,
    /// The month shown.
    month: u8,
}

impl Grid {
    /// The cell of day `index` of week `week`, from its lines (the first line of the grid is its edge).
    fn cell(&self, week: usize, index: usize) -> Rect {
        Rect::from_min_max(pos2(self.cols.line(index), self.weeks.line(week)), pos2(self.cols.line(index + 1), self.weeks.line(week + 1)))
    }
}

/// The days of week `week` of the grid: their numbers, the bands across them and their chips.
#[expect(clippy::too_many_arguments, reason = "the week's place in the grid and the events laid out")]
fn week_row(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    calendar: &Calendar,
    events: &[&EventRow],
    grid: &Grid,
    week: usize,
    asked: &mut Vec<Asked>,
) {
    let ppp = ui.pixels_per_point();
    let line = grid.cols.width;
    let (first, _) = calendar.span();
    let start = first.plus(i64::try_from(week.saturating_mul(7)).unwrap_or(0));
    let bars = layout::bars(events, start, 7);
    let pitch = lane_pitch(Sizes::of(theme).lane, ppp);
    let top = grid.weeks.line(week);
    let row = grid.weeks.line(week + 1) - top;
    // The lanes that fit over one chip and the link, at most a few.
    let room = ((row - BANDS_TOP - CHIP.1 - 20.0) / pitch).floor();
    let fit = if room.is_finite() && room > 0.0 { (room as usize).min(MOST_LANES) } else { 0 };
    let lanes = layout::lanes(&bars).min(fit);
    for bar in bars.iter().filter(|bar| bar.lane < lanes) {
        let Some(event) = events.get(bar.event) else { continue };
        let (mut left, right) = grid.cols.cell(bar.from, bar.to.saturating_add(1));
        if bar.from == 0 && !bar.starts {
            left -= line;
        }
        let y = snap(top + BANDS_TOP, ppp) + pitch * bar.lane as f32;
        let area = Rect::from_min_max(pos2(left, y), pos2(right, y + pitch + line));
        let words = Words { title: &event.summary, detail: "", time: "" };
        let salt = 1000_i64.saturating_add(i64::try_from(week).unwrap_or(0));
        asked.extend(block(ui, theme, calendar, event, area, Kind::Band, &words, salt));
    }
    for index in 0..7_usize {
        let day = start.plus(i64::try_from(index).unwrap_or(0));
        let cell = grid.cell(week, index);
        day_number(ui, theme, calendar, day, grid, cell, asked);
        day_events(ui, theme, keymap, calendar, events, (cell, index, day), (&bars, lanes), asked);
    }
}

/// The timed events of `day` (column `index`, in `cell`) as chips under its lanes of bands, as many as fit, and
/// "+N more" for the rest, bands left out included.
#[expect(clippy::too_many_arguments, reason = "the day's place in the grid and the bands already laid out")]
fn day_events(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    calendar: &Calendar,
    events: &[&EventRow],
    (cell, index, day): (Rect, usize, Day),
    (bars, lanes): (&[Bar], usize),
    asked: &mut Vec<Asked>,
) {
    let ppp = ui.pixels_per_point();
    let pitch = lane_pitch(Sizes::of(theme).lane, ppp);
    let over = |bar: &&Bar| bar.from <= index && index <= bar.to;
    let hidden = bars.iter().filter(over).filter(|bar| bar.lane >= lanes).count();
    let mine = bars.iter().filter(over).filter(|bar| bar.lane < lanes).map(|bar| bar.lane + 1).max().unwrap_or(0);
    let timed: Vec<&EventRow> = events.iter().copied().filter(|event| !event.is_block() && event.covers(day)).collect();
    let chips_top = snap(cell.top() + CHIPS_TOP + pitch * mine as f32, ppp);
    let bottom = cell.bottom();
    let all = timed.len().saturating_add(hidden);
    // As many chips as fit; with a link under them when some are left out.
    let fits = |count: usize, link: bool| chips_top + CHIP.1 * count as f32 - 2.0 + if link { 4.0 + 16.0 } else { 0.0 } <= bottom;
    let shown = if hidden == 0 && fits(timed.len(), false) {
        timed.len()
    } else {
        let mut count = timed.len().min(64);
        while count > 0 && !fits(count, true) {
            count -= 1;
        }
        count
    };
    for (row, event) in timed.iter().take(shown).enumerate() {
        let y = chips_top + CHIP.1 * row as f32;
        let area = Rect::from_min_size(pos2(cell.left() + CHIP_SIDE, y), vec2((cell.width() - CHIP_SIDE * 2.0).max(1.0), CHIP.0));
        let time = event.start.clock();
        let words = Words { title: &event.summary, detail: "", time: if event.start.day == day { &time } else { "" } };
        let salt = 2000_i64.saturating_add(day.since(Day::EPOCH));
        asked.extend(block(ui, theme, calendar, event, area, Kind::Chip, &words, salt));
    }
    let left_out = all.saturating_sub(shown);
    if left_out == 0 || !fits(shown, true) {
        return;
    }
    let y = chips_top + CHIP.1 * shown as f32 + 2.0;
    let place = Rect::from_min_size(pos2(cell.left() + 9.0, y), vec2((cell.width() - 18.0).max(1.0), 16.0));
    let words = tr!("+{count} more", count = left_out);
    let link = ui.scope_builder(UiBuilder::new().max_rect(place).id_salt(("calendar-month-more", day)), |ui| {
        link_sized(ui, theme, keymap, &words, LinkSize::Small)
    });
    ui.ctx().data_mut(|data| data.insert_temp(super::more::anchor_id(day), link.inner.rect));
    if link.inner.clicked() {
        asked.push(Asked::More(Some(day)));
    }
}

/// The number of `day` at the top of its `cell`, a button that opens the day: the first of a month with the month's
/// name, today in a `cyan_edge` box with `today` beside it, the picked day on `cyan`, a day of another month muted.
fn day_number(ui: &mut Ui, theme: &Theme, calendar: &Calendar, day: Day, grid: &Grid, cell: Rect, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let ppp = ui.pixels_per_point();
    let line = grid.cols.width;
    let today = day == calendar.today();
    let picked = calendar.picked_day() == Some(day);
    let text = if day.date() == 1 { format!("{} {}", day.date(), day::month_short(day.month())) } else { day.date().to_string() };
    let colour = if picked {
        p.on_cyan
    } else if today {
        p.cyan_text
    } else if day.month() == grid.month {
        p.ink
    } else {
        p.muted
    };
    let font = theme.fonts.mono_s;
    let galley = crate::component::galley(ui, &text, font, colour);
    let wide = galley.size().x.ceil();
    let boxed = Rect::from_min_size(pos2(cell.left() + 4.0, snap(cell.top() + 4.0, ppp)), vec2(wide + 12.0, font.line + 2.0));
    let response = ui.interact(boxed, number_id(day), Sense::click());
    super::keys::item(ui.ctx(), &response);
    if super::keys::wanted(ui.ctx(), crate::calendar::Target::Day(day)) {
        response.request_focus();
    }
    if response.gained_focus() && !picked {
        asked.push(Asked::PickDay(day));
    }
    let spoken = if today { format!("{}, {}", day::long(day), tr!("today")) } else { day::long(day) };
    response.widget_info(|| WidgetInfo::selected(egui::accesskit::Role::Button, true, picked, &spoken));
    if picked {
        ui.painter().rect_filled(boxed, 0.0, p.cyan);
    } else if response.hovered() {
        ui.painter().rect_filled(boxed, 0.0, p.stage);
    }
    if today && !picked {
        super::stroke(ui.painter(), boxed, line, p.cyan_edge);
    }
    let x = if today || picked { boxed.left() + 6.0 } else { cell.left() + 9.0 };
    run_at(ui.painter(), pos2(x, boxed.top() + 1.0), galley, colour);
    if today {
        let note = crate::component::galley(ui, tr!("today"), font, p.muted);
        if boxed.right() + 6.0 + note.size().x <= cell.right() - 4.0 {
            run_at(ui.painter(), pos2(boxed.right() + 6.0, boxed.top() + 1.0), note, p.muted);
        }
    }
    if response.has_focus() && !picked {
        crate::draw::paint::focus_ring(ui.painter(), theme, boxed, crate::draw::paint::Ring::Outside);
    }
    if response.clicked() {
        asked.push(Asked::Open(day));
    }
}
