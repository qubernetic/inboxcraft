// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The side panel of the calendar: the mini month (its name, the steps to the month before and after, a button per
//! day that puts the view there; today in the accent, the days shown on the selection), the calendars with a box
//! each that shows or hides its events, and the details of the picked event: its summary, both dates with the times,
//! the event's own zone, labelled, when its clock differs from the computer's, the place, the description, the
//! calendar with busy or free, private and the repeat, and the guests with their answers.

use egui::{Align, Layout, Rect, RichText, ScrollArea, Sense, Ui, WidgetInfo, pos2, vec2};

use super::{Asked, caption_strong, tint};
use crate::calendar::{Calendar, Day, EventRow, day};
use crate::component::button::IconButton;
use crate::draw::paint::{line, line_height, selection};
use crate::iconset::{IconId, Style};
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

/// The most guests the details list by name.
const GUESTS: usize = 8;

/// The side panel.
pub(super) fn show(ui: &mut Ui, theme: &Theme, keymap: &Keymap, calendar: &Calendar, asked: &mut Vec<Asked>) {
    ScrollArea::vertical().id_salt("calendar-side").auto_shrink([false, false]).show(ui, |ui| {
        mini_month(ui, theme, keymap, calendar, asked);
        calendars(ui, theme, calendar, asked);
        if let Some(event) = calendar.picked() {
            details(ui, theme, keymap, calendar, event, asked);
        }
    });
}

fn mini_month(ui: &mut Ui, theme: &Theme, keymap: &Keymap, calendar: &Calendar, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let month = calendar.month();
    ui.horizontal(|ui| {
        let name = tr!("{month} {year}", month = day::month_name(month.month()), year = month.year());
        ui.label(RichText::new(name).font(theme.fonts.strong.id()).color(p.text));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if IconButton::new(IconId::Next, tr!("Month after")).show(ui, theme, keymap).clicked() {
                asked.push(Asked::Month(true));
            }
            if IconButton::new(IconId::Previous, tr!("Month before")).show(ui, theme, keymap).clicked() {
                asked.push(Asked::Month(false));
            }
        });
    });
    ui.add_space(s.xs);
    let width = ui.available_width();
    let cell = (width / 7.0).max(1.0);
    let high = (line_height(ui, theme.fonts.caption) + s.s * 1.5).min(cell);
    let (rect, _) = ui.allocate_exact_size(vec2(width, high), Sense::hover());
    for index in 0..7_i64 {
        let galley = line(ui, &[(day::weekday_letter(index), theme.fonts.caption, p.muted)], cell);
        let centre = pos2(rect.left() + cell * (index as f32 + 0.5), rect.center().y);
        ui.painter().galley(centre - galley.size() / 2.0, galley, p.muted);
    }
    let (shown_first, shown_last) = calendar.span();
    let first = month.monday();
    for week in 0..6_i64 {
        let (rect, _) = ui.allocate_exact_size(vec2(width, high), Sense::hover());
        for index in 0..7_i64 {
            let day = first.plus(week * 7 + index);
            let spot = Rect::from_center_size(pos2(rect.left() + cell * (index as f32 + 0.5), rect.center().y), vec2(high - 2.0, high - 2.0));
            mini_day(
                ui,
                theme,
                day,
                month.month(),
                spot,
                (shown_first..=shown_last).contains(&day) && day != calendar.today(),
                calendar.today(),
                asked,
            );
        }
    }
}

/// One day of the mini month over `spot`: a button that puts the view on it.
#[expect(clippy::too_many_arguments, reason = "the day, the month it is shown in, its place and its states")]
fn mini_day(ui: &mut Ui, theme: &Theme, day: Day, month: u8, spot: Rect, shown: bool, today: Day, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let response = ui.interact(spot, ui.scope_id().with(("calendar-mini", day)), Sense::click());
    response.widget_info(|| WidgetInfo::selected(egui::accesskit::Role::Button, true, shown || day == today, day::long(day)));
    let colour = if day == today {
        p.on_accent
    } else if day.month() == month {
        p.text
    } else {
        p.muted
    };
    if day == today {
        ui.painter().circle_filled(spot.center(), spot.height() / 2.0, p.accent);
    } else if shown {
        selection(ui.painter(), theme, spot, spot.height() / 2.0);
    } else if response.hovered() {
        ui.painter().circle_filled(spot.center(), spot.height() / 2.0, p.hover);
    }
    let font = if day == today { caption_strong(theme) } else { theme.fonts.caption };
    let galley = line(ui, &[(&day.date().to_string(), font, colour)], spot.width() + 8.0);
    ui.painter().galley(spot.center() - galley.size() / 2.0, galley, colour);
    if response.clicked() {
        asked.push(Asked::Go(day));
    }
}

fn calendars(ui: &mut Ui, theme: &Theme, calendar: &Calendar, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let s = &theme.spacing;
    ui.add_space(s.l);
    ui.label(RichText::new(tr!("Calendars").to_uppercase()).font(theme.fonts.caption.id()).color(p.muted).extra_letter_spacing(0.6));
    ui.add_space(s.xs);
    for row in calendar.calendars() {
        ui.horizontal(|ui| {
            let mut shown = calendar.is_shown(row.id);
            if ui.checkbox(&mut shown, RichText::new(&row.name).font(theme.fonts.body.id()).color(p.text)).changed() {
                asked.push(Asked::Toggle(row.id));
            }
            let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
            ui.painter().circle_filled(dot.center(), 4.0, tint(theme, calendar, row.id));
        });
    }
}

/// The details of the picked `event`.
fn details(ui: &mut Ui, theme: &Theme, keymap: &Keymap, calendar: &Calendar, event: &EventRow, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let s = &theme.spacing;
    ui.add_space(s.l);
    ui.separator();
    ui.add_space(s.s);
    ui.horizontal(|ui| {
        let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
        ui.painter().circle_filled(dot.center(), 5.0, tint(theme, calendar, event.calendar));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if IconButton::new(IconId::Close, tr!("Close the details")).show(ui, theme, keymap).clicked() {
                asked.push(Asked::Pick(None));
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add(egui::Label::new(RichText::new(&event.summary).font(theme.fonts.heading.id()).color(p.text)).wrap());
            });
        });
    });
    ui.add_space(s.xs);
    let body = |ui: &mut Ui, text: &str, muted: bool| {
        let colour = if muted { p.muted } else { p.text };
        ui.add(egui::Label::new(RichText::new(text).font(theme.fonts.body.id()).color(colour)).wrap());
    };
    body(ui, &event.when(), false);
    if let Some(zone) = event.zone_line() {
        ui.horizontal(|ui| {
            let side = line_height(ui, theme.fonts.caption);
            let (spot, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
            theme.icons.paint(ui.painter(), spot, IconId::Globe, Style::Regular, p.muted);
            ui.add(egui::Label::new(RichText::new(zone).font(theme.fonts.caption.id()).color(p.muted)).wrap());
        });
    }
    if !event.location.is_empty() {
        body(ui, &event.location, true);
    }
    if !event.description.is_empty() {
        body(ui, &event.description, true);
    }
    body(ui, &facts(calendar, event), true);
    for guest in event.guests.iter().take(GUESTS) {
        let answer = answer(&guest.status);
        let text = if answer.is_empty() { guest.name.clone() } else { tr!("{name} · {answer}", name = guest.name, answer = answer) };
        body(ui, &text, true);
    }
    if let Some(more) = event.guests.len().checked_sub(GUESTS).filter(|more| *more > 0) {
        body(ui, &tr!("and {count} more", count = more), true);
    }
}

/// The calendar, busy or free, private and the repeat: `Personal · busy · private · repeats weekly`.
fn facts(calendar: &Calendar, event: &EventRow) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(row) = calendar.calendar(event.calendar) {
        parts.push(row.name.clone());
    }
    parts.push(if event.busy { tr!("busy") } else { tr!("free") }.to_owned());
    if event.private {
        parts.push(tr!("private").to_owned());
    }
    if let Some(repeat) = event.repeat.as_deref() {
        parts.push(
            match repeat {
                "daily" => tr!("repeats daily"),
                "weekdays" => tr!("repeats on weekdays"),
                "weekly" => tr!("repeats weekly"),
                "monthly" => tr!("repeats monthly"),
                "yearly" => tr!("repeats yearly"),
                _ => tr!("repeats"),
            }
            .to_owned(),
        );
    }
    parts.join(" · ")
}

/// A guest's answer in words.
fn answer(status: &str) -> &'static str {
    match status {
        "accepted" => tr!("going"),
        "declined" => tr!("not going"),
        "tentative" => tr!("maybe"),
        "needs_action" => tr!("no answer yet"),
        "delegated" => tr!("passed on"),
        _ => "",
    }
}
