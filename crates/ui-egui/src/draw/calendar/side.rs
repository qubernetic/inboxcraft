// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The side panel of the calendar, 252 points wide at the right (H7): the mini month (its name, the steps to the month
//! before and after, a button per day that puts the view there; today in a `cyan_edge` box, the day the arrows picked
//! on `cyan`), the calendars by account (the account's bar and name, then a box per calendar that shows or hides its
//! events), and the details of the picked event: its title beside its account's bar, both dates with the times, the
//! event's own zone when its clock differs from the computer's, the place, the description, the calendar, busy or
//! free, private and the repeat as tags, the guests with their answers, and Edit and Delete; every text whole, the
//! panel scrolling to the rest. At the foot the sync line: when the calendars were last read, and the owner's sync
//! (`calendar.sync`) as it goes, ends or fails, with one control that runs it (see [`crate::calendar::sync`]).

use egui::{Rect, ScrollArea, Sense, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use super::grid::{line as line_width, snap};
use super::{Asked, bar_colour, stroke};
use crate::calendar::{Calendar, Day, EventRow, day};
use crate::component::avatar::avatar;
use crate::component::button::{IconButton, TextButton};
use crate::component::calendar_sync;
use crate::component::checkbox::checkbox;
use crate::component::tag::{TagKind, tag};
use crate::draw::paint::run_at;
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

/// The room around the panel's content, in points: 15 at the top and the sides.
const PAD: f32 = 15.0;

/// Between the panel's parts, in points.
const GAP: f32 = 12.0;

/// The most rows a text of the details lays out: more than its longest text takes, so nothing is cut; the panel
/// scrolls to the rest.
const WHOLE: usize = 4096;

/// The most guests the details list by name.
const GUESTS: usize = 8;

/// A day's cell in the mini month, in points.
const MINI_CELL: (f32, f32) = (28.0, 24.0);

/// Between two weeks of the mini month, in points.
const MINI_GAP: f32 = 4.0;

/// The id of `day`'s button in the mini month: the keyboard's focus while the arrows pick days there.
pub(in crate::draw) fn mini_id(day: Day) -> egui::Id {
    egui::Id::unique(("calendar-mini", day))
}

/// The side panel.
pub(super) fn show(ui: &mut Ui, theme: &Theme, keymap: &Keymap, calendar: &Calendar, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let ppp = ui.pixels_per_point();
    let line = line_width(ppp);
    let rect = ui.max_rect();
    let view = calendar.sync_view();
    let foot = snap(rect.bottom() - calendar_sync::height(&view) - line, ppp);
    // On whole pixels: the mini month's box and the foot's hairline never blur.
    let inner = super::grid::whole(Rect::from_min_max(pos2(rect.left() + PAD, rect.top() + PAD), pos2(rect.right() - PAD, foot)), ppp);
    ui.scope_builder(UiBuilder::new().max_rect(inner).id_salt("calendar-side-scroll"), |ui| {
        let area = ScrollArea::vertical().id_salt("calendar-side").auto_shrink([false, false]);
        crate::component::scroll::show(ui, theme, area, |ui| {
            // The scrolling part pads its content 1 point at the top and the sides, as the frame does.
            egui::Frame::new()
                .inner_margin(egui::Margin { left: 1, right: 1, top: 1, bottom: 0 })
                .show(ui, |ui| side_content(ui, theme, keymap, calendar, asked));
        });
    });
    ui.painter().rect_filled(Rect::from_min_max(pos2(inner.left(), foot), pos2(inner.right(), foot + line)), 0.0, p.panel_line);
    let sync = Rect::from_min_max(pos2(inner.left(), foot + line), pos2(inner.right(), rect.bottom()));
    // Under the event editor the control rests: the editor is the surface the keys and the pointer are for.
    let resting = calendar.editor().is_some();
    let drawn = calendar_sync::Drawn { id: sync_id(), resting, still: false };
    let pressed = ui
        .scope_builder(UiBuilder::new().max_rect(sync).id_salt("calendar-sync"), |ui| calendar_sync::calendar_sync(ui, theme, keymap, &view, drawn))
        .inner;
    // The press is noted for the mailbox, which runs `calendar.sync` on its next round; a frame is asked for it.
    if pressed && calendar.ask_sync() {
        ui.ctx().request_repaint();
    }
}

/// The id of the sync control: one Tab stop in every state of the sync, so the keyboard stays on it from one to the
/// next.
pub(in crate::draw) fn sync_id() -> egui::Id {
    egui::Id::unique("calendar-sync-control")
}

/// What the side panel scrolls: the mini month, the calendars and the picked event's details.
fn side_content(ui: &mut Ui, theme: &Theme, keymap: &Keymap, calendar: &Calendar, asked: &mut Vec<Asked>) {
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    mini_month(ui, theme, &Mini::of(calendar), asked);
    ui.add_space(GAP);
    calendars(ui, theme, calendar, asked);
    if let Some(event) = calendar.picked() {
        ui.add_space(GAP);
        crate::draw::paint::rule(ui, theme.palette.panel_line);
        ui.add_space(GAP);
        details(ui, theme, keymap, calendar, event, asked);
    }
    ui.add_space(GAP);
}

/// What the mini month shows: its month, today, the picked day, and a day drawn hovered (the gallery's board).
struct Mini {
    month: Day,
    today: Day,
    picked: Option<Day>,
    hover: Option<Day>,
    /// The board it is drawn on in the gallery, which tells its buttons apart from another board's.
    board: Option<u8>,
}

impl Mini {
    fn of(calendar: &Calendar) -> Self {
        Self { month: calendar.month(), today: calendar.today(), picked: calendar.picked_day(), hover: None, board: None }
    }
}

/// The mini-month sheet's board number `board` for the gallery: the month of `month`, `today`, the `picked` day and a
/// `hover`ed one, 220 wide.
pub(crate) fn mini_month_board(ui: &mut Ui, theme: &Theme, board: u8, month: Day, today: Day, (picked, hover): (Option<Day>, Option<Day>)) {
    ui.allocate_ui(vec2(220.0, 260.0), |ui| {
        ui.set_width(220.0);
        let mut asked = Vec::new();
        mini_month(ui, theme, &Mini { month: month.first_of_month(), today, picked, hover, board: Some(board) }, &mut asked);
    });
}

/// The mini month as the mini-month sheet draws it: a box with its name and steps, the weekdays' letters and the weeks
/// the month touches.
fn mini_month(ui: &mut Ui, theme: &Theme, mini: &Mini, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let ppp = ui.pixels_per_point();
    let line = line_width(ppp);
    let month = mini.month;
    let first = month.monday();
    let last = month.months_later(1).plus(-1);
    let weeks = usize::try_from(last.since(first) / 7 + 1).unwrap_or(6).clamp(4, 6);
    let width = ui.available_width();
    let (cell_w, cell_h) = MINI_CELL;
    let height = 8.0 + 28.0 + 4.0 + 20.0 + 4.0 + cell_h * weeks as f32 + MINI_GAP * (weeks - 1) as f32 + 12.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    // On whole pixels, its box and today's never blur (the layout rounds to a 64th of a point).
    let rect = super::grid::whole(rect, ppp);
    ui.painter().rect_filled(rect, 0.0, p.surface);
    stroke(ui.painter(), rect, line, p.line);
    let left = rect.left() + 12.0;
    // The head: the month's name and the steps.
    let head = Rect::from_min_size(pos2(left, rect.top() + 8.0), vec2(width - 24.0, 28.0));
    let name = tr!("{month} {year}", month = day::month_name(month.month()), year = month.year());
    let text = crate::component::cut(ui, &name, theme.fonts.label, p.ink, head.width() - 40.0);
    run_at(ui.painter(), pos2(head.left(), snap(head.center().y - theme.fonts.label.line / 2.0, ppp)), text, p.ink);
    let response = ui.interact(head, ui.scope_id().with(("mini-name", mini.board)), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Label, true, &name));
    // Each sign's 8-wide box as the sheet places it (+168 and +188 in a head 196 wide), in the middle of a 16-wide spot.
    for (forward, sign, spoken, x) in [(false, "‹", tr!("Month before"), head.right() - 28.0), (true, "›", tr!("Month after"), head.right() - 8.0)]
    {
        let spot = Rect::from_min_size(pos2(x - 4.0, head.top() + 4.0), vec2(16.0, 20.0));
        let response = ui.interact(spot, ui.scope_id().with(("mini-step", forward, mini.board)), Sense::click());
        response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Button, true, spoken));
        let colour = if response.hovered() { p.ink } else { p.muted };
        let galley = crate::component::galley(ui, sign, theme.fonts.mono, colour);
        crate::draw::paint::run_centred(ui.painter(), spot, galley, colour);
        if response.has_focus() {
            crate::draw::paint::focus_ring(ui.painter(), theme, spot, crate::draw::paint::Ring::Outside);
        }
        if response.clicked() {
            asked.push(Asked::Month(forward));
        }
    }
    let letters = rect.top() + 8.0 + 28.0 + 4.0;
    for index in 0..7_i64 {
        let cell = Rect::from_min_size(pos2(left + cell_w * index as f32, letters), vec2(cell_w, 20.0));
        let galley = crate::component::galley(ui, day::weekday_letter(index), theme.fonts.label, p.muted);
        crate::draw::paint::run_centred(ui.painter(), cell, galley, p.muted);
    }
    let top = letters + 20.0 + 4.0;
    for week in 0..weeks {
        for index in 0..7_usize {
            let day = first.plus(i64::try_from(week * 7 + index).unwrap_or(0));
            let spot = Rect::from_min_size(pos2(left + cell_w * index as f32, top + (cell_h + MINI_GAP) * week as f32), vec2(cell_w, cell_h));
            mini_day(ui, theme, mini, day, spot, asked);
        }
    }
}

/// One day of the mini month over `spot`: a button that puts the view on it.
fn mini_day(ui: &mut Ui, theme: &Theme, mini: &Mini, day: Day, spot: Rect, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let line = line_width(ui.pixels_per_point());
    let spot = super::grid::whole(spot, ui.pixels_per_point());
    let today = day == mini.today;
    let picked = mini.picked == Some(day);
    let month = mini.month.month();
    let id = mini.board.map_or_else(|| mini_id(day), |board| mini_id(day).with(board));
    let response = ui.interact(spot, id, Sense::click());
    super::keys::item(ui.ctx(), &response);
    if super::keys::wanted(ui.ctx(), crate::calendar::Target::Mini(day)) {
        response.request_focus();
    }
    if response.gained_focus() && !picked {
        asked.push(Asked::PickDay(day));
    }
    let spoken = if today { format!("{}, {}", day::long(day), tr!("today")) } else { day::long(day) };
    response.widget_info(|| WidgetInfo::selected(egui::accesskit::Role::Button, true, picked, &spoken));
    let colour = if picked {
        p.on_cyan
    } else if today {
        p.cyan_text
    } else if day.month() == month {
        p.ink
    } else {
        p.muted
    };
    if picked {
        ui.painter().rect_filled(spot, 0.0, p.cyan);
    } else if response.hovered() || mini.hover == Some(day) {
        ui.painter().rect_filled(spot, 0.0, p.stage);
    }
    if today && !picked {
        stroke(ui.painter(), spot, line, p.cyan_edge);
    }
    let galley = crate::component::galley(ui, &day.date().to_string(), theme.fonts.mono_s, colour);
    crate::draw::paint::run_centred(ui.painter(), spot, galley, colour);
    if response.has_focus() && !picked {
        crate::draw::paint::focus_ring(ui.painter(), theme, spot, crate::draw::paint::Ring::Inside);
    }
    if response.clicked() {
        asked.push(Asked::Go(day));
    }
}

/// The calendars by account: the account's bar and name, then a box per calendar.
fn calendars(ui: &mut Ui, theme: &Theme, calendar: &Calendar, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let ppp = ui.pixels_per_point();
    crate::component::label(ui, theme.fonts.label, tr!("Calendars"), p.muted);
    ui.add_space(4.0);
    let row = super::fixed(theme, 24.0);
    let mut accounts: Vec<u64> = Vec::new();
    for entry in calendar.calendars() {
        if !accounts.contains(&entry.account) {
            accounts.push(entry.account);
        }
    }
    for account in accounts {
        let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), row), Sense::hover());
        let colour = theme.account_colour(calendar.account_index(account));
        let mark = Rect::from_min_size(pos2(rect.left(), snap(rect.center().y - 4.0, ppp)), vec2(super::grid::pixels(3.0, ppp), 12.0));
        ui.painter().rect_filled(mark, 0.0, colour);
        let name = calendar.account_label(account);
        let text = crate::component::cut(ui, &name, theme.fonts.label, p.muted, (rect.width() - 11.0).max(1.0));
        run_at(ui.painter(), pos2(rect.left() + 11.0, snap(rect.center().y - theme.fonts.label.line / 2.0 + 2.0, ppp)), text, p.muted);
        response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Label, true, &name));
        for entry in calendar.calendars().iter().filter(|entry| entry.account == account) {
            ui.scope_builder(UiBuilder::new().max_rect(Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), row))), |ui| {
                ui.set_min_height(row);
                ui.horizontal_centered(|ui| {
                    if checkbox(ui, theme, &entry.name, calendar.is_shown(entry.id)).clicked() {
                        asked.push(Asked::Toggle(entry.id));
                    }
                });
            });
        }
    }
}

/// The details of the picked `event`.
fn details(ui: &mut Ui, theme: &Theme, keymap: &Keymap, calendar: &Calendar, event: &EventRow, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let ppp = ui.pixels_per_point();
    let width = ui.available_width();
    let wrapped = |ui: &mut Ui, text: &str, font: crate::theme::Font, colour, rows: usize| {
        let galley = crate::component::wrapped(ui, text, font, colour, width, rows, egui::Align::Min);
        let height = galley.rows.len().max(1) as f32 * font.line;
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        // A line broken on purpose is heard as one.
        let spoken = text.replace('\n', " ");
        response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Label, true, &spoken));
        crate::draw::paint::rows_at(ui.painter(), rect.min, width, galley, colour);
    };
    // The head: the title beside its account's bar, Close at the right.
    let close_side = theme.boxes().button;
    let title = crate::component::wrapped(
        ui,
        &event.summary,
        theme.fonts.heading,
        p.ink,
        (width - close_side - 8.0 - 11.0).max(1.0),
        WHOLE,
        egui::Align::Min,
    );
    let rows = title.rows.len().max(1) as f32;
    let head_height = (rows * theme.fonts.heading.line + 10.0).max(close_side);
    let (head, head_response) = ui.allocate_exact_size(vec2(width, head_height), Sense::hover());
    ui.painter().rect_filled(
        Rect::from_min_size(head.min, vec2(super::grid::pixels(3.0, ppp), head_height)),
        0.0,
        bar_colour(theme, calendar, event.calendar),
    );
    let title_width = (width - close_side - 8.0 - 11.0).max(1.0);
    crate::draw::paint::rows_at(ui.painter(), pos2(head.left() + 11.0, head.top() + 5.0), title_width, title, p.ink);
    head_response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Label, true, &event.summary));
    let close = Rect::from_min_size(pos2(head.right() - close_side, head.top()), vec2(close_side, close_side));
    // A child of its own: the head took its room already, so the cursor stays under it.
    let closed = IconButton::new(IconId::Close, tr!("Close the details")).show(&mut ui.new_child(UiBuilder::new().max_rect(close)), theme, keymap);
    if super::keys::wanted(ui.ctx(), crate::calendar::Target::Details) {
        closed.request_focus();
    }
    if closed.clicked() {
        asked.push(Asked::Pick(None));
    }
    ui.add_space(8.0);
    // One day's times on a line of their own, under the day, as the design breaks them.
    let when = if !event.all_day && event.first == event.last { event.when().replacen(", ", ",\n", 1) } else { event.when() };
    wrapped(ui, &when, theme.fonts.body, p.ink, WHOLE);
    if let Some(zone) = event.zone_line() {
        ui.add_space(8.0);
        wrapped(ui, &zone, theme.fonts.caption, p.muted, WHOLE);
    }
    if !event.location.is_empty() {
        ui.add_space(8.0);
        wrapped(ui, &event.location, theme.fonts.body, p.muted, WHOLE);
    }
    if !event.description.is_empty() {
        ui.add_space(8.0);
        wrapped(ui, &event.description, theme.fonts.caption, p.muted, WHOLE);
    }
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        ui.add_space(0.0);
        for fact in facts(calendar, event) {
            tag(ui, theme, &fact, TagKind::Default);
        }
    });
    if !event.guests.is_empty() {
        ui.add_space(8.0 + 2.0);
        crate::component::label(ui, theme.fonts.label, &tr!("Guests · {count}", count = event.guests.len()), p.muted);
        ui.add_space(4.0);
        for guest in event.guests.iter().take(GUESTS) {
            guest_row(ui, theme, guest);
        }
        if let Some(more) = event.guests.len().checked_sub(GUESTS).filter(|more| *more > 0) {
            wrapped(ui, &tr!("and {count} more", count = more), theme.fonts.caption, p.muted, 1);
        }
    }
    ui.add_space(8.0 + 4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let edit = TextButton::new(tr!("Edit")).secondary().icon(IconId::Compose).show(ui, theme, keymap);
        if edit.clicked() {
            super::opened_from(ui.ctx(), edit.id);
            asked.push(Asked::Edit);
        }
        if TextButton::new(tr!("Delete")).secondary().icon(IconId::Delete).show(ui, theme, keymap).clicked() {
            asked.push(Asked::Delete);
        }
    });
}

/// A guest's row: the avatar, the name, and the answer in its colour at the right.
fn guest_row(ui: &mut Ui, theme: &Theme, guest: &crate::calendar::Guest) {
    let p = &theme.palette;
    let ppp = ui.pixels_per_point();
    let (word, colour) = answer(theme, &guest.status);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    ui.scope_builder(UiBuilder::new().max_rect(Rect::from_min_size(pos2(rect.left(), rect.top() + 1.0), vec2(24.0, 24.0))), |ui| {
        avatar(ui, theme, &guest.name, &guest.address, 24.0);
    });
    let answer_galley = (!word.is_empty()).then(|| crate::component::galley(ui, word, theme.fonts.mono_s, colour));
    let answer_width = answer_galley.as_ref().map_or(0.0, |galley| galley.size().x.ceil() + 8.0);
    let name = crate::component::cut(ui, &guest.name, theme.fonts.caption, p.ink, (rect.width() - 32.0 - answer_width).max(1.0));
    run_at(ui.painter(), pos2(rect.left() + 32.0, snap(rect.top() + 4.0, ppp)), name, p.ink);
    if let Some(galley) = answer_galley {
        crate::draw::paint::run_right(ui.painter(), rect.right(), snap(rect.top() + 5.0, ppp), galley, colour);
    }
    let spoken = if word.is_empty() { guest.name.clone() } else { format!("{}, {word}", guest.name) };
    response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Label, true, &spoken));
}

/// The calendar, busy or free, private and the repeat, a tag each.
fn facts(calendar: &Calendar, event: &EventRow) -> Vec<String> {
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
    parts
}

/// A guest's answer in words and its colour: going in `plus`, maybe in `warn`, not going in `minus`, the rest `muted`.
fn answer(theme: &Theme, status: &str) -> (&'static str, egui::Color32) {
    let p = &theme.palette;
    match status {
        "accepted" => (tr!("going"), p.plus),
        "declined" => (tr!("not going"), p.minus),
        "tentative" => (tr!("maybe"), p.warn),
        "needs_action" => (tr!("no answer yet"), p.muted),
        "delegated" => (tr!("passed on"), p.muted),
        _ => ("", p.muted),
    }
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod sync_tests;
