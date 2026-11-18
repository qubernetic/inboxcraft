// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The agenda: the days of the span that hold events, each under a head (its name in the `label` style, today's in
//! `cyan_text` with `today` beside it, the count of its events at the right) and a `panel_line` hairline; an event a
//! row 40 high with its own hairline under it: its start (or `All day`, or `Until` the end on the last day of a long
//! one) in a column of its own, its account's bar, its title, and at the right its place and length, the days a long
//! one runs, or the event's own zone with its start when that differs from the computer's. Without an event in the
//! span the empty state says so.

use egui::{Rect, ScrollArea, Sense, Ui, WidgetInfo, pos2, vec2};

use super::grid::{line as line_width, snap};
use super::{Asked, Kind, Words, block};
use crate::calendar::{AGENDA_DAYS, Calendar, Day, EventRow, day};
use crate::draw::paint::{run_at, run_right};
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

/// A row's height, in points.
const ROW: f32 = 40.0;

/// A day's head's height, in points.
const HEAD: f32 = 36.0;

/// Between two days, in points.
const DAY_GAP: f32 = 12.0;

/// Over the first day, in points.
const TOP: f32 = 8.0;

/// The most rows one day lists; the rest are counted in a last line that opens the day's float.
pub(super) const MOST_ROWS: usize = 50;

/// The agenda of the calendar's span.
pub(super) fn show(ui: &mut Ui, theme: &Theme, keymap: &Keymap, calendar: &Calendar, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let (first, last) = calendar.span();
    let events: Vec<&EventRow> = calendar.events().collect();
    // Laid out once for these events and days: at most a day's rows each, and its count.
    let laid = super::laid::agenda(ui.ctx(), calendar, &events, (first, usize::try_from(AGENDA_DAYS).unwrap_or(0)), MOST_ROWS);
    if laid.days.iter().all(|(_, count)| *count == 0) {
        let label = tr!("Agenda · {from} – {to}", from = day::short(first), to = day::short(last));
        let help = tr!("The arrows at the top step 30 days back or on. A day picked in the small month starts the agenda there.");
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() / 2.0 - 100.0).max(0.0));
            crate::component::empty::empty_state(ui, theme, &label, tr!("Nothing in these 30 days."), help);
        });
        return;
    }
    let row = super::fixed(theme, ROW);
    let head = super::fixed(theme, HEAD);
    crate::component::scroll::show_over(ui, theme, ScrollArea::vertical().id_salt("calendar-agenda").auto_shrink([false, false]), |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let ppp = ui.pixels_per_point();
        let line = line_width(ppp);
        ui.add_space(TOP);
        let mut any = false;
        for (offset, (held, count)) in (0_i64..).zip(laid.days.iter()) {
            let day = first.plus(offset);
            let rows: Vec<&EventRow> = held.iter().filter_map(|at| events.get(*at).copied()).collect();
            if rows.is_empty() {
                continue;
            }
            if any {
                ui.add_space(DAY_GAP);
            }
            any = true;
            let width = ui.available_width();
            let (rect, response) = ui.allocate_exact_size(vec2(width, head + line), Sense::hover());
            let rect = super::grid::whole(rect, ppp);
            let today = day == calendar.today();
            let name = crate::component::galley(ui, &day::dated(day), theme.fonts.label, if today { p.cyan_text } else { p.muted });
            let y = snap(rect.top() + head / 2.0 - theme.fonts.label.line / 2.0, ppp);
            let wide = name.size().x.ceil();
            run_at(ui.painter(), pos2(rect.left() + 16.0, y), name, if today { p.cyan_text } else { p.muted });
            if today {
                let note = crate::component::galley(ui, tr!("today"), theme.fonts.mono_s, p.muted);
                run_at(ui.painter(), pos2(rect.left() + 16.0 + wide + 8.0, y), note, p.muted);
            }
            let all = *count;
            let count = if all == 1 { tr!("1 event").to_owned() } else { tr!("{count} events", count = all) };
            let count_text = crate::component::galley(ui, &count, theme.fonts.mono_s, p.muted);
            run_right(ui.painter(), rect.right() - 16.0, y, count_text, p.muted);
            let under = snap(rect.top() + head, ppp);
            ui.painter().rect_filled(Rect::from_min_max(pos2(rect.left(), under), pos2(rect.right(), under + line)), 0.0, p.panel_line);
            let spoken = if today { format!("{}, {}, {count}", day::dated(day), tr!("today")) } else { format!("{}, {count}", day::dated(day)) };
            response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Label, true, &spoken));
            for event in &rows {
                let (rect, _) = ui.allocate_exact_size(vec2(width, row), Sense::hover());
                // On whole pixels, so its hairline and the picked row's bar never blur.
                let rect = super::grid::whole(rect, ppp);
                let (time, detail) = words(event, day);
                let salt = 3000_i64.saturating_add(offset);
                let area = Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - line));
                asked.extend(block(
                    ui,
                    theme,
                    calendar,
                    event,
                    area,
                    Kind::Row,
                    &Words { title: &event.summary, detail: &detail, time: &time },
                    salt,
                ));
                ui.painter().rect_filled(Rect::from_min_max(pos2(rect.left(), rect.bottom() - line), rect.max), 0.0, p.panel_line);
            }
            if let Some(more) = all.checked_sub(rows.len()).filter(|more| *more > 0) {
                let words = tr!("+{count} more", count = more);
                let (rect, _) = ui.allocate_exact_size(vec2(width, row), Sense::hover());
                let link = ui.scope_builder(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(16.0, 12.0))), |ui| {
                    crate::component::button::link_sized(ui, theme, keymap, &words, crate::component::button::LinkSize::Small)
                });
                super::more::keep_link(ui.ctx(), day, &link.inner);
                if link.inner.clicked() {
                    asked.push(Asked::More(Some(day)));
                }
            }
        }
    });
}

/// A row's time and detail on `day`: `10:00` with its place and length (`Room 3 · 1 h`), `All day` with the days a
/// long one runs (`since Tuesday, until Thursday`), `Until 13:00` on the last day of a long timed one, and the event's
/// own zone with its start before its length when its clock differs.
pub(super) fn words(event: &EventRow, day: Day) -> (String, String) {
    let weekday = |day: Day| crate::mailbox::weekday_text(day.weekday());
    let runs = || match (event.first < day, event.last > day) {
        (true, true) => tr!("since {first}, until {last}", first = weekday(event.first), last = weekday(event.last)),
        (true, false) => tr!("since {first}", first = weekday(event.first)),
        (false, true) => tr!("until {last}", last = weekday(event.last)),
        (false, false) => String::new(),
    };
    if event.all_day {
        return (tr!("All day").to_owned(), runs());
    }
    let mut parts: Vec<String> = Vec::new();
    let time = if event.start.day == day {
        event.start.clock()
    } else if event.end.day == day || event.last == day {
        tr!("Until {time}", time = event.end.clock())
    } else {
        tr!("All day").to_owned()
    };
    if event.start.day != day || event.end.day > day && event.last > day {
        let runs = runs();
        if !runs.is_empty() {
            return (time, runs);
        }
    }
    if !event.location.is_empty() {
        parts.push(event.location.clone());
    }
    if let Some(zone) = event.zone.as_ref() {
        parts.push(format!("{} {}", zone.name, zone.start.clock()));
    }
    if let Some(length) = length(event) {
        parts.push(length);
    }
    (time, parts.join(" · "))
}

/// How long a timed event lasts: `30 min`, `1 h`, `1 h 30`; nothing for one without length or longer than a day.
fn length(event: &EventRow) -> Option<String> {
    let days = event.end.day.since(event.start.day);
    let minutes = days.checked_mul(1440)?.checked_add(i64::from(event.end.minute))?.checked_sub(i64::from(event.start.minute))?;
    if minutes <= 0 || minutes > 1440 {
        return None;
    }
    let (hours, rest) = (minutes / 60, minutes % 60);
    Some(match (hours, rest) {
        (0, minutes) => tr!("{minutes} min", minutes = minutes),
        (hours, 0) => tr!("{hours} h", hours = hours),
        (hours, minutes) => tr!("{hours} h {minutes}", hours = hours, minutes = format!("{minutes:02}")),
    })
}
