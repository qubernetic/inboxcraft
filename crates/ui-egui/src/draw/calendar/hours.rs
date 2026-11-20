// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The day and week views: a head per day (its name in the `label` style; today's in `cyan_text` with `today` beside
//! it and a 1 px `cyan_edge` line under it), a lane of bands over whole days under "All day" when there are any (at
//! most [`MOST_LANES`]; what does not fit is counted in a "+N more" under the day), then the day's hours on a grid of
//! `panel_line` hairlines, scrolled to the morning at first. A timed event is a cell on the grid: from its start's
//! line to its end's, from its day's line to the next, side by side with the events it overlaps (at most
//! [`MOST_COLUMNS`] columns; the rest are counted in the day's "+N more"). The now line (`cyan_edge`) runs across
//! today's column with the time in the gutter, and the view asks for a repaint when the minute turns, never sooner.

use egui::{Id, Rect, ScrollArea, Sense, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use super::grid::{Lines, lane_pitch, line as line_width, pixels, snap};
use super::{Asked, Kind, Sizes, Words, block};
use crate::calendar::layout;
use crate::calendar::{Calendar, Day, EventRow, day};
use crate::component::button::{LinkSize, link_sized};
use crate::draw::paint::run_at;
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

/// The room at the right of the last day's line, for the scroll bar, in points.
const SCROLL_ROOM: f32 = 12.0;

/// The hour the view is scrolled to at first, unless an event starts earlier.
const MORNING: u16 = 7;

/// The most lanes of bands shown over the hours; the others are counted in their days' "+N more".
pub(super) const MOST_LANES: usize = 2;

/// The most columns of overlapping events side by side in a day; the events past them are counted in the day's
/// "+N more", so 500 events at one hour lay out four cells and one link.
pub(super) const MOST_COLUMNS: usize = 4;

/// The day or week view of `days` days from the first day of the calendar's span.
pub(super) fn show(ui: &mut Ui, theme: &Theme, keymap: &Keymap, calendar: &Calendar, days: usize, asked: &mut Vec<Asked>) {
    show_from(ui, theme, keymap, calendar, (calendar.span().0, days), asked);
}

/// The hours of `days` days from `first` (the full event editor's day beside it: its own day, wherever the view is).
pub(super) fn show_from(ui: &mut Ui, theme: &Theme, keymap: &Keymap, calendar: &Calendar, (first, days): (Day, usize), asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let z = Sizes::of(theme);
    let ppp = ui.pixels_per_point();
    let line = line_width(ppp);
    let rect = ui.max_rect();
    let events: Vec<&EventRow> = calendar.events().collect();
    let cols = Lines::cut(rect.left() + z.gutter, rect.right() - SCROLL_ROOM, days, ppp);
    let day_of = |index: usize| first.plus(i64::try_from(index).unwrap_or(0));

    let head_bottom = snap(rect.top() + z.head, ppp);
    head(ui, theme, calendar, &cols, (rect.top(), head_bottom), (first, days), asked);
    ui.painter().rect_filled(Rect::from_min_max(pos2(rect.left(), head_bottom), pos2(rect.right(), head_bottom + line)), 0.0, p.line);

    // The timed events of each day, and what is left out of each: bands past the lanes, events past the columns. Laid
    // out once for these events and days, within the bounds.
    let laid = super::laid::hours(ui.ctx(), calendar, &events, (first, days), (MOST_COLUMNS, MOST_LANES));
    let (pieces, bars) = (&laid.pieces, &laid.bars);
    let left_out: Vec<usize> = (0..days)
        .map(|index| {
            let bands = laid.bands_left.get(index).copied().unwrap_or(0);
            let timed = pieces.get(index).map_or(0, |(_, left)| *left);
            bands.saturating_add(timed)
        })
        .collect();
    let lanes = layout::lanes(bars).min(MOST_LANES);
    let more = left_out.iter().any(|count| *count > 0);
    let rows = lanes + usize::from(more);
    let pitch = lane_pitch(z.lane, ppp);
    let mut hours_top = head_bottom + line;
    if rows > 0 {
        let top = head_bottom;
        let bottom = top + pitch * rows as f32;
        let label = crate::component::galley(ui, tr!("All day"), theme.fonts.mono_s, p.muted);
        run_at(ui.painter(), pos2(rect.left() + 6.0, top + 5.0), label, p.muted);
        for index in 0..=days {
            let x = cols.line(index);
            ui.painter().rect_filled(Rect::from_min_max(pos2(x, top + line), pos2(x + line, bottom)), 0.0, p.panel_line);
        }
        ui.painter().rect_filled(Rect::from_min_max(pos2(rect.left(), bottom), pos2(rect.right(), bottom + line)), 0.0, p.line);
        for bar in bars.iter().filter(|bar| bar.lane < lanes) {
            let Some(event) = events.get(bar.event) else { continue };
            let (left, right) = cols.cell(bar.from, bar.to.saturating_add(1));
            let top = top + pitch * bar.lane as f32;
            let area = Rect::from_min_max(pos2(left, top), pos2(right, top + pitch + line));
            let days_text = if event.last > event.first {
                format!("{} – {}", day::weekday_short(event.first.weekday()), day::weekday_short(event.last.weekday()))
            } else {
                String::new()
            };
            let words = Words { title: &event.summary, detail: &days_text, time: "" };
            asked.extend(block(ui, theme, calendar, event, area, Kind::Band, &words, -1));
        }
        if more {
            for (index, count) in left_out.iter().enumerate().filter(|(_, count)| **count > 0) {
                let day = day_of(index);
                let top = top + pitch * lanes as f32 + 4.0;
                let place =
                    Rect::from_min_size(pos2(cols.line(index) + 8.0, top), vec2((cols.inside(index).1 - cols.line(index) - 8.0).max(1.0), 16.0));
                let words = tr!("+{count} more", count = count);
                let link = ui.scope_builder(UiBuilder::new().max_rect(place).id_salt(("calendar-more", index)), |ui| {
                    link_sized(ui, theme, keymap, &words, LinkSize::Small)
                });
                super::more::keep_link(ui.ctx(), day, &link.inner);
                if link.inner.clicked() {
                    asked.push(Asked::More(Some(day)));
                }
            }
        }
        hours_top = bottom + line;
    }

    let area = Rect::from_min_max(pos2(rect.left(), hours_top), rect.max);
    ui.scope_builder(UiBuilder::new().max_rect(area).id_salt("calendar-hours-area"), |ui| {
        // Each hours' own: the view's, and the day beside the full editor's, each starts at the morning once.
        let scrolled = ui.scope_id().with("inboxcraft-calendar-hours-scrolled");
        let mut scroll = ScrollArea::vertical().id_salt("calendar-hours").auto_shrink([false, false]);
        if !ui.ctx().data(|data| data.get_temp::<bool>(scrolled).unwrap_or(false)) {
            let earliest = events.iter().filter(|event| !event.is_block()).map(|event| event.start.minute / 60).fold(MORNING, u16::min);
            scroll = scroll.vertical_scroll_offset(f32::from(earliest) * z.hour);
            ui.ctx().data_mut(|data| data.insert_temp(scrolled, true));
        }
        crate::component::scroll::show_over(ui, theme, scroll, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let (content, _) = ui.allocate_exact_size(vec2(rect.width(), z.hour * 24.0 + line), Sense::hover());
            let top = content.top();
            let at = |minute: u16| snap(top + f32::from(minute) / 60.0 * z.hour, ppp);
            let painter = ui.painter().clone();
            // An hour line at the top of what is in sight would double the `line` over the hours: the design has none.
            let sight = ui.clip_rect().top() + line;
            for hour in 1..24_u16 {
                let y = at(hour * 60);
                if y < sight {
                    continue;
                }
                painter.rect_filled(Rect::from_min_max(pos2(cols.line(0), y), pos2(cols.line(days), y + line)), 0.0, p.panel_line);
            }
            for index in 0..=days {
                let x = cols.line(index);
                painter.rect_filled(Rect::from_min_max(pos2(x, top), pos2(x + line, content.bottom())), 0.0, p.panel_line);
            }
            for hour in 0..24_u16 {
                let label = crate::component::galley(ui, &format!("{hour:02}:00"), theme.fonts.mono_s, p.muted);
                run_at(&painter, pos2(rect.left() + 10.0, at(hour * 60) + 4.0), label, p.muted);
            }
            for (index, (pieces, _)) in pieces.iter().enumerate() {
                for piece in pieces {
                    let Some(event) = events.get(piece.event) else { continue };
                    let shared = piece.columns.clamp(1, MOST_COLUMNS);
                    let sub = Lines::cut(cols.line(index), cols.line(index + 1), shared, ppp);
                    let (left, right) = sub.cell(piece.column, piece.column + 1);
                    let cell = Rect::from_min_max(pos2(left, at(piece.from)), pos2(right, at(piece.to) + line));
                    let from_top = cell.translate(-content.min.to_vec2());
                    let glided = super::glide::glide(ui.ctx(), event, (calendar.span(), i64::try_from(index).unwrap_or(0)), from_top);
                    let moved = glided.translate(content.min.to_vec2());
                    // On its way it stands on whole pixels too; landed, it is its cell.
                    let area = if glided == from_top {
                        cell
                    } else {
                        Rect::from_min_max(
                            pos2(snap(moved.left(), ppp), snap(moved.top(), ppp)),
                            pos2(snap(moved.right(), ppp), snap(moved.bottom(), ppp)),
                        )
                    };
                    let detail = detail(event, days == 1);
                    let words = Words { title: &event.summary, detail: &detail, time: "" };
                    let salt = i64::try_from(index).unwrap_or(0);
                    asked.extend(block(ui, theme, calendar, event, area, Kind::Timed { narrow: shared > 1 }, &words, salt));
                }
            }
            if let Some(now) = calendar.now() {
                let index = usize::try_from(now.day.since(first)).ok().filter(|index| *index < days);
                if let Some(index) = index {
                    let y = at(now.minute);
                    let (left, right) = cols.inside(index);
                    painter.rect_filled(Rect::from_min_max(pos2(left, y), pos2(right, y + line)), 0.0, p.cyan_edge);
                    let mark = pixels(7.0, ppp);
                    let gutter_line = cols.line(0);
                    let half = snap(3.0, ppp);
                    painter.rect_filled(Rect::from_min_max(pos2(gutter_line - mark, y - half), pos2(gutter_line, y - half + mark)), 0.0, p.cyan_edge);
                    let time = crate::component::galley(ui, &now.clock(), theme.fonts.mono_s, p.cyan_text);
                    let height = theme.fonts.mono_s.line;
                    let boxed = Rect::from_min_size(pos2(rect.left() + 6.0, y - 3.0 - height), vec2(time.size().x.ceil() + 8.0, height));
                    painter.rect_filled(boxed, 0.0, p.surface);
                    run_at(&painter, pos2(boxed.left() + 4.0, boxed.top()), time, p.cyan_text);
                    ui.ctx().request_repaint_after(calendar.until_next_minute());
                }
            }
        });
    });
}

/// A timed event's second line: its start and place in the week (or its times without a place), its times and place
/// in the day.
fn detail(event: &EventRow, one_day: bool) -> String {
    let times = event.times();
    match (one_day, event.location.is_empty()) {
        (true, true) => times,
        (true, false) => format!("{times} · {}", event.location),
        (false, true) => times,
        (false, false) => format!("{} · {}", event.start.clock(), event.location),
    }
}

/// The id of `day`'s head over the hours in the week: the same each frame, so the keyboard can be handed to it.
pub(super) fn head_id(day: crate::calendar::Day) -> Id {
    Id::unique(("calendar-day-head", day))
}

/// The days' heads over the hours: each a button that opens its day in the week (the day view's own is a label).
fn head(
    ui: &mut Ui,
    theme: &Theme,
    calendar: &Calendar,
    cols: &Lines,
    (top, bottom): (f32, f32),
    (first, days): (Day, usize),
    asked: &mut Vec<Asked>,
) {
    let p = &theme.palette;
    let ppp = ui.pixels_per_point();
    let line = cols.width;
    for index in 0..=days {
        let x = cols.line(index);
        ui.painter().rect_filled(Rect::from_min_max(pos2(x, top), pos2(x + line, bottom)), 0.0, p.panel_line);
    }
    for index in 0..days {
        let day = first.plus(i64::try_from(index).unwrap_or(0));
        let (left, right) = cols.inside(index);
        let cell = Rect::from_min_max(pos2(left, top), pos2(right, bottom));
        let today = day == calendar.today();
        let name = if days == 1 { day::dated(day) } else { format!("{} {}", day::weekday_short(day.weekday()), day.date()) };
        let font = theme.fonts.label;
        let colour = if today { p.cyan_text } else { p.muted };
        let text = crate::component::cut(ui, &name, font, colour, (cell.width() - 20.0).max(1.0));
        let wide = text.size().x;
        let y = snap(cell.center().y - font.line / 2.0, ppp);
        run_at(ui.painter(), pos2(left + 10.0, y), text, colour);
        if today {
            let note = crate::component::galley(ui, tr!("today"), theme.fonts.mono_s, p.muted);
            if wide + 8.0 + note.size().x <= cell.width() - 20.0 {
                run_at(ui.painter(), pos2(left + 10.0 + wide.ceil() + 8.0, y), note, p.muted);
            }
            let under = pixels(1.0, ppp);
            ui.painter().rect_filled(Rect::from_min_max(pos2(left, bottom - under), pos2(right, bottom)), 0.0, p.cyan_edge);
        }
        let spoken = format!("{}{}", day::long(day), if today { format!(", {}", tr!("today")) } else { String::new() });
        if days > 1 {
            let response = ui.interact(cell, head_id(day), Sense::click());
            response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Button, true, &spoken));
            if response.clicked() {
                asked.push(Asked::Open(day));
            }
        } else {
            let response = ui.interact(cell, ui.scope_id().with(("calendar-day", index)), Sense::hover());
            response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Label, true, &spoken));
        }
    }
}
