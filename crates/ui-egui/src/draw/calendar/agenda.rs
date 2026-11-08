// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The agenda: the days of the span that hold events, each under its name ("Today" first when it is today), an event
//! a row: its start (or "All day", or "Until" the end on a later day of a long one), its summary and place, and at the
//! right, as plain text, the event's own zone with its start when that differs from the computer's.

use egui::{Rect, RichText, ScrollArea, Sense, Ui, vec2};

use super::{Asked, Shape, Text, block, caption_strong};
use crate::calendar::{AGENDA_DAYS, Calendar, Day, EventRow, day};
use crate::draw::paint::{line, line_height};
use crate::theme::Theme;
use crate::tr;

/// The width of the time at the left of a row, in points.
const TIME: f32 = 64.0;

/// Between a day's name and its rows, and from row to row, in points.
const ROWS: f32 = 10.0;

/// The room above and below an event's text in a row, in points.
const BAR: f32 = 7.0;

/// The room at the right of the zone note and between it and the bar, in points.
const NOTE: f32 = 10.0;

/// The agenda of the calendar's span.
pub(super) fn show(ui: &mut Ui, theme: &Theme, calendar: &Calendar, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let (first, _) = calendar.span();
    let events: Vec<&EventRow> = calendar.events().collect();
    ScrollArea::vertical().id_salt("calendar-agenda").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = ROWS;
        let mut any = false;
        for offset in 0..AGENDA_DAYS {
            let day = first.plus(offset);
            let held: Vec<&EventRow> = events.iter().copied().filter(|event| event.covers(day)).collect();
            if held.is_empty() {
                continue;
            }
            any = true;
            let name = if day == calendar.today() { tr!("Today · {day}", day = day::dated(day)) } else { day::dated(day) };
            ui.label(RichText::new(name).font(theme.fonts.heading.id()).color(p.text));
            for event in held {
                row(ui, theme, calendar, event, day, offset, asked);
            }
        }
        if !any {
            ui.add_space(s.l);
            let text = tr!("Nothing in these {count} days.", count = AGENDA_DAYS);
            ui.label(RichText::new(text).font(theme.fonts.body.id()).color(p.muted));
        }
    });
}

/// One event of `day` in the agenda.
fn row(ui: &mut Ui, theme: &Theme, calendar: &Calendar, event: &EventRow, day: Day, salt: i64, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let height = line_height(ui, theme.fonts.caption) + BAR * 2.0;
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let time = if event.all_day {
        tr!("All day").to_owned()
    } else if event.start.day == day {
        event.start.clock()
    } else {
        tr!("Until {time}", time = event.end.clock())
    };
    let galley = line(ui, &[(&time, theme.fonts.caption, p.muted)], TIME);
    ui.painter().galley(rect.left_center() - vec2(0.0, galley.size().y / 2.0), galley, p.muted);
    let zone = event.zone.as_ref().map(|zone| format!("{} {}", zone.name, zone.start.clock()));
    let zone_width = zone.as_ref().map_or(0.0, |zone| {
        let galley = line(ui, &[(zone, caption_strong(theme), p.text)], width / 3.0);
        let wide = galley.size().x;
        let at = rect.right_center() - vec2(wide + NOTE, galley.size().y / 2.0);
        ui.painter().galley(at, galley, p.text);
        wide + NOTE * 2.0 + 12.0
    });
    let area = Rect::from_min_max(rect.min + vec2(TIME, 0.0), rect.max - vec2(zone_width, 0.0));
    let title = if event.location.is_empty() { event.summary.clone() } else { format!("{} · {}", event.summary, event.location) };
    asked.extend(block(ui, theme, calendar, event, area, &Text { title: &title, detail: None, shape: Shape::Row }, salt));
}
