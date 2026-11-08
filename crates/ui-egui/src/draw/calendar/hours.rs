// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The day and week views: a column per day under its name (today on the selection), a lane of blocks over whole
//! days, then the day's hours, scrolled to the morning at first, with the timed events placed by their minutes and
//! side by side where they overlap, and a line at the time now across today.

use egui::{Id, Rect, ScrollArea, Sense, Stroke, Ui, WidgetInfo, pos2, vec2};

use super::{Asked, Text, block, caption_strong};
use crate::calendar::{Calendar, DAY_MINUTES, EventRow, day, layout};
use crate::draw::paint::{line, line_height, selection};
use crate::mailbox::weekday_text;
use crate::theme::Theme;

/// The height of an hour, in points.
const HOUR: f32 = 44.0;

/// The width of the hours' labels at the left, in points.
const GUTTER: f32 = 52.0;

/// The hour the view is scrolled to at first.
const MORNING: f32 = 7.5;

/// The day or week view of `days` days from the first day of the calendar's span.
pub(super) fn show(ui: &mut Ui, theme: &Theme, calendar: &Calendar, days: i64, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let (first, _) = calendar.span();
    let events: Vec<&EventRow> = calendar.events().collect();
    let width = ui.available_width();
    // Days are one or seven.
    let count = days.max(1) as f32;
    let column = ((width - GUTTER) / count).max(1.0);
    let x = |left: f32, index: i64| left + GUTTER + column * index as f32;

    let head = line_height(ui, theme.fonts.caption) + s.s * 2.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, head), Sense::hover());
    for index in 0..days {
        let day = first.plus(index);
        let cell = Rect::from_min_size(pos2(x(rect.left(), index), rect.top()), vec2(column, head));
        let text = if days == 1 { weekday_text(day.weekday()).to_owned() } else { format!("{} {}", day::weekday_short(day.weekday()), day.date()) };
        let galley = line(ui, &[(&text, theme.fonts.caption, p.text)], column);
        let at = cell.center() - galley.size() / 2.0;
        if day == calendar.today() {
            let pill = Rect::from_center_size(cell.center(), galley.size() + vec2(s.m * 2.0, s.xs * 2.0));
            selection(ui.painter(), theme, pill, pill.height() / 2.0);
        }
        ui.painter().galley(at, galley, p.text);
        // In the week a day's name opens the day; the day view's own name is no button.
        if days > 1 {
            let response = ui.interact(cell, ui.scope_id().with(("calendar-day", index)), Sense::click());
            response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Button, true, day::long(day)));
            if response.clicked() {
                asked.push(Asked::Open(day));
            }
        }
    }

    let bars = layout::bars(&events, first, usize::try_from(days).unwrap_or(1));
    let lanes = layout::lanes(&bars);
    if lanes > 0 {
        let lane = line_height(ui, caption_strong(theme)) + s.xs * 2.0;
        let (rect, _) = ui.allocate_exact_size(vec2(width, lane * lanes as f32 + s.xs), Sense::hover());
        for bar in &bars {
            let Some(event) = events.get(bar.event) else { continue };
            let left = x(rect.left(), i64::try_from(bar.from).unwrap_or(0)) + 1.0;
            let right = x(rect.left(), i64::try_from(bar.to).unwrap_or(0).saturating_add(1)) - 1.0;
            let top = rect.top() + lane * bar.lane as f32;
            let area = Rect::from_min_max(pos2(left, top), pos2(right, top + lane - 2.0));
            asked.extend(block(ui, theme, calendar, event, area, &Text { title: &event.summary, detail: None }, -1));
        }
    }
    ui.add_space(s.xs);

    let scrolled = Id::unique("inboxcraft-calendar-hours-scrolled");
    let mut area = ScrollArea::vertical().id_salt("calendar-hours").auto_shrink([false, false]);
    if !ui.ctx().data(|data| data.get_temp::<bool>(scrolled).unwrap_or(false)) {
        let earliest = events.iter().filter(|event| !event.is_block()).map(|event| f32::from(event.start.minute) / 60.0).fold(MORNING, f32::min);
        // A few points above the hour, so its label shows whole.
        area = area.vertical_scroll_offset((earliest.floor() * HOUR - 8.0).max(0.0));
        ui.ctx().data_mut(|data| data.insert_temp(scrolled, true));
    }
    area.show(ui, |ui| {
        let width = ui.available_width();
        let column = ((width - GUTTER) / count).max(1.0);
        let (rect, _) = ui.allocate_exact_size(vec2(width, HOUR * 24.0), Sense::hover());
        let painter = ui.painter().clone();
        let at = |minute: u16| rect.top() + f32::from(minute) / 60.0 * HOUR;
        for hour in 0..24_u16 {
            let y = at(hour * 60);
            painter.hline(rect.left() + GUTTER..=rect.right(), y, Stroke::new(1.0, p.line));
            if hour > 0 {
                let label = line(ui, &[(&format!("{hour:02}:00"), theme.fonts.caption, p.muted)], GUTTER);
                painter.galley(pos2(rect.left(), y - label.size().y / 2.0), label, p.muted);
            }
        }
        for index in 0..=days {
            let x = rect.left() + GUTTER + column * index as f32;
            painter.vline(x, rect.top()..=rect.bottom(), Stroke::new(1.0, p.line));
        }
        for index in 0..days {
            let day = first.plus(index);
            let left = rect.left() + GUTTER + column * index as f32;
            if let Some(now) = calendar.now().filter(|now| now.day == day) {
                let y = at(now.minute);
                painter.hline(left..=left + column, y, Stroke::new(2.0, p.accent));
                painter.circle_filled(pos2(left, y), 4.0, p.accent);
            }
            for piece in layout::pieces(&events, day) {
                let Some(event) = events.get(piece.event) else { continue };
                let share = column / piece.columns.max(1) as f32;
                let top = at(piece.from);
                let bottom = at(piece.to.min(DAY_MINUTES)).max(top + 14.0);
                let area = Rect::from_min_max(
                    pos2(left + share * piece.column as f32 + 1.0, top + 1.0),
                    pos2(left + share * (piece.column + 1) as f32 - 2.0, bottom - 1.0),
                );
                let times = event.times();
                asked.extend(block(ui, theme, calendar, event, area, &Text { title: &event.summary, detail: Some(&times) }, index));
            }
        }
    });
}
