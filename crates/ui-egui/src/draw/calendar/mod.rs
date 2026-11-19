// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendar screen: the view on the left (a toolbar with the view's title, the steps back, to today and on, the
//! views as a segmented control and New event; under it the day, week, month or agenda) and the side panel standing
//! 252 points wide at the right, a `line` hairline between them (the mini month, the calendars by account, the picked
//! event's details, and the sync line). Below a view of [`VIEW_LEAST`] points the side panel gives the view its room.
//! Without a calendar the screen's empty state says there are none yet and what will show here.
//!
//! The day and week views lay the hours out on a grid of `panel_line` hairlines under a lane of bands over whole days
//! (`hours`), the month view the six weeks of the month (`month`), the agenda the days that hold events (`agenda`).
//! Every event is a cell on its grid ([`grid`]): its edges take the pixels of the grid's lines. Every event is a button
//! named after its summary and its days and times; pressing it picks it, and the keyboard's focus and the picked event
//! are one thing ([`keys`]). Drawing only reads the [`Calendar`]; what was pressed comes back as [`Asked`] and is
//! applied after the frame. New event, and Edit under the picked event's details, open the event editor (`editor`) as a
//! float (a new event's hanging from New event, an edit's over the details); "+N more" opens the day's events in a
//! float (`more`).

mod agenda;
mod editor;
mod glide;
mod grid;
mod hours;
pub(super) mod keys;
mod laid;
pub(super) use self::side::mini_id;
pub(crate) use self::side::mini_month_board;
mod month;
mod more;
mod side;

use std::sync::Arc;

use egui::{CentralPanel, Color32, Frame, Galley, Id, Margin, Pos2, Rect, Response, Sense, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use self::grid::{line as line_width, pixels, snap};
use crate::calendar::{Calendar, Day, EventRow, Picked, View};
use crate::component::button::{IconButton, TextButton};
use crate::component::segmented::segmented;
use crate::draw::paint::{Panes, Ring, focus_ring, run_at};
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

/// The side panel's width, in points, as the design draws it (H7: at the right).
const SIDE: f32 = 252.0;

/// The narrowest the view gets with the side panel beside it, in points: below, the panel gives the view its room.
const VIEW_LEAST: f32 = 600.0;

/// The room left and right in the toolbar, in points.
const TOOLBAR_PAD: f32 = 16.0;

/// Between the toolbar's controls, in points.
const TOOLBAR_GAP: f32 = 8.0;

/// Between the toolbar's groups (the steps, the views, New event), in points, as the design draws it.
const GROUP_GAP: f32 = 24.0;

/// From the toolbar's title to its note (the week's number, the agenda's days), in points, as the design draws it.
const NOTE_GAP: f32 = 20.0;

/// The width of an event's account bar, in points (H8: an event is coloured by its account's bar only).
const BAR: f32 = 3.0;

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
    /// An event pressed or reached by the keyboard, or the details closed.
    Pick(Option<Picked>),
    /// A day's button in the month or the mini month reached by the keyboard: it is picked.
    PickDay(Day),
    /// "+N more" of a day: its events in a float; `None` closes it.
    More(Option<Day>),
    /// New event: the editor on a new event.
    New,
    /// Edit: the editor on the picked event.
    Edit,
    /// Delete: the picked event goes.
    Delete,
    /// The editor's Save.
    Save,
    /// The editor's Cancel: what was typed is thrown away.
    Cancel,
    /// Esc or a press outside the editor: it closes when nothing was typed, else it says how to keep the typing.
    Leave(crate::component::popup::Leave),
}

/// The calendar's fixed heights and widths at the text scale (each through [`Theme::size`], as the design's 130 %
/// frames scale them).
#[derive(Debug, Clone, Copy)]
pub(super) struct Sizes {
    /// The toolbar.
    pub toolbar: f32,
    /// A day's head over the hours.
    pub head: f32,
    /// The gutter of the hours' labels.
    pub gutter: f32,
    /// An hour.
    pub hour: f32,
    /// A band over whole days.
    pub lane: f32,
}

impl Sizes {
    pub(super) fn of(theme: &Theme) -> Self {
        let size = |points| fixed(theme, points);
        Self { toolbar: size(48.0), head: size(36.0), gutter: size(56.0), hour: size(48.0), lane: size(24.0) }
    }
}

/// A fixed box size of the design, `points` at 100 %, at the text scale ([`Theme::size`]): a row's or a lane's height,
/// never a font's.
pub(super) fn fixed(theme: &Theme, points: f32) -> f32 {
    Theme::size(theme, points)
}

/// Under the empty state the design keeps the room of its "Open accounts" button, in points (a secondary button).
const EMPTY_ACTION: f32 = 32.0;

/// The calendar screen while there is no calendar: the calendar's card, as with calendars, and in its middle what will
/// show here, where the design places it (over the room of a button the app has no command for yet).
pub(super) fn empty(ui: &mut Ui, theme: &Theme, layers: &mut Panes) {
    let about = tr!("The calendars of your accounts show here, by day, week, month and agenda, once an account has a CalDAV address.");
    CentralPanel::no_frame().frame(Frame::NONE).show(ui, |ui| {
        layers.card(ui, Margin::ZERO, |ui| {
            let all = ui.max_rect();
            let words = Rect::from_min_max(all.min, pos2(all.right(), (all.bottom() - fixed(theme, EMPTY_ACTION)).max(all.top())));
            ui.scope_builder(UiBuilder::new().max_rect(words).id_salt("calendar-empty"), |ui| {
                crate::component::empty::empty_state(ui, theme, tr!("Calendar · no calendars"), tr!("No calendars yet."), about);
            });
        });
    });
}

/// The calendar screen with its calendars: the view and the side panel; what was pressed. While `over` the palette or
/// the settings sheet lies over the screen's floats and has the keys.
pub(super) fn screen(ui: &mut Ui, theme: &Theme, keymap: &Keymap, calendar: &Calendar, layers: &mut Panes, over: bool) -> Vec<Asked> {
    let mut asked = Vec::new();
    CentralPanel::no_frame().frame(Frame::NONE).show(ui, |ui| {
        layers.card(ui, Margin::ZERO, |ui| {
            let all = ui.max_rect();
            ui.ctx().data_mut(|data| data.insert_temp(card_id(), all));
            let ppp = ui.pixels_per_point();
            let line = line_width(ppp);
            let with_side = all.width() - SIDE - line >= VIEW_LEAST;
            let side_left = if with_side { snap(all.right() - SIDE, ppp) } else { all.right() + line };
            let view = Rect::from_min_max(all.min, pos2((side_left - line).max(all.left()), all.bottom()));
            if with_side {
                let edge = Rect::from_min_max(pos2(side_left - line, all.top()), pos2(side_left, all.bottom()));
                ui.painter().rect_filled(edge, 0.0, theme.palette.line);
            }
            ui.scope_builder(UiBuilder::new().max_rect(view).id_salt("calendar-view"), |ui| {
                ui.set_clip_rect(view.intersect(ui.clip_rect()));
                view_part(ui, theme, keymap, calendar, over, &mut asked);
            });
            if with_side {
                let side = Rect::from_min_max(pos2(side_left, all.top()), all.max);
                ui.scope_builder(UiBuilder::new().max_rect(side).id_salt("calendar-side"), |ui| {
                    ui.set_clip_rect(side.intersect(ui.clip_rect()));
                    side::show(ui, theme, keymap, calendar, &mut asked);
                });
            }
        });
    });
    asked
}

/// The view: its toolbar, a `line` hairline, and the days.
fn view_part(ui: &mut Ui, theme: &Theme, keymap: &Keymap, calendar: &Calendar, over: bool, asked: &mut Vec<Asked>) {
    let z = Sizes::of(theme);
    let ppp = ui.pixels_per_point();
    let line = line_width(ppp);
    let rect = ui.max_rect();
    let bar = Rect::from_min_size(rect.min, vec2(rect.width(), z.toolbar));
    ui.scope_builder(UiBuilder::new().max_rect(bar).id_salt("calendar-toolbar"), |ui| toolbar(ui, theme, keymap, calendar, asked));
    let under = snap(rect.top() + z.toolbar, ppp);
    ui.painter().rect_filled(Rect::from_min_max(pos2(rect.left(), under), pos2(rect.right(), under + line)), 0.0, theme.palette.line);
    let days = Rect::from_min_max(pos2(rect.left(), under + line), rect.max);
    ui.scope_builder(UiBuilder::new().max_rect(days).id_salt("calendar-days"), |ui| match calendar.view() {
        View::Day => hours::show(ui, theme, keymap, calendar, 1, asked),
        View::Week => hours::show(ui, theme, keymap, calendar, 7, asked),
        View::Month => month::show(ui, theme, keymap, calendar, asked),
        View::Agenda => agenda::show(ui, theme, keymap, calendar, asked),
    });
    if let Some(day) = calendar.more() {
        more::show(ui.ctx(), theme, keymap, calendar, (day, over), asked);
    } else {
        more::refocus(ui.ctx());
    }
}

/// Where the editor hangs from: New event's rect, kept from the toolbar for the float drawn after the screen.
pub(super) fn new_event_id() -> Id {
    Id::unique("calendar-new-event-button")
}

/// Where the editor of an existing event stands: the calendar card's room, kept from the screen for the float.
fn card_id() -> Id {
    Id::unique("calendar-card-room")
}

/// Where the control that opened the editor is kept (New event, or Edit), and whether the editor was open last frame.
fn opener_id() -> Id {
    Id::unique("calendar-editor-opener")
}

/// Notes `opener` as the control that opened the editor: it gets the keyboard back when the editor closes.
pub(super) fn opened_from(ctx: &egui::Context, opener: Id) {
    ctx.data_mut(|data| data.insert_temp(opener_id(), (opener, false)));
}

/// The toolbar over the view: its title and note (the week's number, the agenda's days), the steps back, to today and
/// on, the views as a segmented control, and New event at the right.
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
    let inner = ui.max_rect().shrink2(vec2(TOOLBAR_PAD, 0.0));
    ui.scope_builder(UiBuilder::new().max_rect(inner).layout(egui::Layout::right_to_left(egui::Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = TOOLBAR_GAP;
        // Pressed while the editor hangs from it, as the design draws it.
        let new = TextButton::new(tr!("New event")).primary().arrow();
        let new = if calendar.editor().is_some() { new.shown(crate::component::Shown::Pressed) } else { new };
        let new = new.show(ui, theme, keymap);
        ui.ctx().data_mut(|data| {
            data.insert_temp(new_event_id(), new.rect);
            data.insert_temp(new_event_id().with("id"), new.id);
        });
        if new.clicked() {
            opened_from(ui.ctx(), new.id);
            asked.push(Asked::New);
        }
        // An editor opened from elsewhere (a script) gives the keyboard to New event when it closes.
        if ui.ctx().data(|data| data.get_temp::<(Id, bool)>(opener_id())).is_none() {
            opened_from(ui.ctx(), new.id);
        }
        ui.add_space(GROUP_GAP - TOOLBAR_GAP);
        if let Some(view) = segmented(ui, theme, &views, calendar.view()) {
            asked.push(Asked::View(view));
        }
        ui.add_space(GROUP_GAP - TOOLBAR_GAP);
        if IconButton::new(IconId::Next, on).show(ui, theme, keymap).clicked() {
            asked.push(Asked::Step(true));
        }
        if TextButton::new(tr!("Today")).secondary().show(ui, theme, keymap).clicked() {
            asked.push(Asked::Today);
        }
        if IconButton::new(IconId::Previous, back).show(ui, theme, keymap).clicked() {
            asked.push(Asked::Step(false));
        }
        ui.add_space(TOOLBAR_GAP);
        // The title and its note take what is left at the left, the title cut first.
        let left = inner.left();
        let room = (ui.available_rect_before_wrap().right() - left).max(0.0);
        let note = match calendar.view() {
            View::Day | View::Week => calendar.week_number().map(|number| tr!("week {number}", number = number)),
            View::Agenda => Some(tr!("{count} days", count = crate::calendar::AGENDA_DAYS)),
            View::Month => None,
        };
        let note = note.map(|note| crate::component::galley(ui, &note, theme.fonts.mono_s, p.muted));
        let note_room = note.as_ref().map_or(0.0, |note| note.size().x + NOTE_GAP);
        let words = calendar.title();
        let title = crate::component::cut(ui, &words, theme.fonts.heading, p.ink, (room - note_room).max(1.0));
        let middle = inner.center().y;
        let wide = title.size().x;
        let ppp = ui.pixels_per_point();
        let at = pos2(left, snap(middle - title.size().y / 2.0, ppp));
        spoken_text(ui, at, title, p.ink, &words);
        if let Some(note) = note {
            let at = pos2(left + wide + NOTE_GAP, snap(middle - note.size().y / 2.0, ppp));
            let said = note.text().to_owned();
            spoken_text(ui, at, note, p.muted, &said);
        }
    });
}

/// Paints `galley` at `at` as a label a screen reader hears as `spoken`.
fn spoken_text(ui: &mut Ui, at: Pos2, galley: Arc<Galley>, colour: Color32, spoken: &str) -> Response {
    let rect = Rect::from_min_size(at, galley.size());
    let response = ui.interact(rect, ui.auto_id_with(("calendar-text", spoken)), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Label, true, spoken));
    if ui.is_rect_visible(rect) {
        run_at(ui.painter(), at, galley, colour);
    }
    response
}

/// The colour of an event's bar: its calendar's account's colour (H8), by the account's place among the accounts.
fn bar_colour(theme: &Theme, calendar: &Calendar, id: u64) -> Color32 {
    theme.account_colour(calendar.account_place(id))
}

/// How an event is drawn where it shows, as the event-block sheet draws each place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// A timed event in the hours, `narrow` where it shares its day with others side by side.
    Timed {
        /// Whether it shares its day's width.
        narrow: bool,
    },
    /// An event over whole days, in a lane.
    Band,
    /// A line in a month's cell: its time and its title.
    Chip,
    /// A row of the agenda or of a day's float.
    Row,
}

/// What an event's block says.
pub(super) struct Words<'a> {
    /// Its title.
    pub title: &'a str,
    /// Its second line (timed), its days (band), its place and length (row), or nothing.
    pub detail: &'a str,
    /// Its time (chip, row), or nothing.
    pub time: &'a str,
}

/// The id of the place `salt` an occurrence shows in: the same each frame, so the keyboard's focus stays on it.
pub(super) fn event_id(event: &EventRow, salt: i64) -> Id {
    Id::unique(("calendar-event", event.id, event.start, salt))
}

/// An event drawn over `rect` as `kind`: a button named after the event, selected while it is picked. `salt` tells apart
/// the places one event shows in. A press picks it, or lets it go when it was picked; reaching it with Tab picks it.
#[expect(clippy::too_many_arguments, reason = "the event, its place, its look and its words")]
pub(super) fn block(
    ui: &mut Ui,
    theme: &Theme,
    calendar: &Calendar,
    event: &EventRow,
    rect: Rect,
    kind: Kind,
    words: &Words<'_>,
    salt: i64,
) -> Option<Asked> {
    let picked = calendar.picked().is_some_and(|picked| picked.key() == event.key());
    let response = ui.interact(rect, event_id(event, salt), Sense::click());
    keys::item(ui.ctx(), &response);
    if keys::wanted(ui.ctx(), crate::calendar::Target::Event(event.key())) {
        response.request_focus();
        response.scroll_to_me(None);
    }
    response.widget_info(|| WidgetInfo::selected(egui::accesskit::Role::Button, true, picked, event.spoken()));
    if ui.is_rect_visible(rect) {
        let ground =
            crate::motion::fade_eased(ui.ctx(), response.id.with("ground"), picked || response.hovered(), theme.motion.fast, crate::motion::css_ease);
        let lit = Lit { focus: response.has_focus(), ground };
        paint_block(ui, theme, bar_colour(theme, calendar, event.calendar), rect, lit, picked, kind, words);
    }
    if response.clicked() {
        return Some(Asked::Pick(if picked { None } else { Some(event.key()) }));
    }
    (response.gained_focus() && !picked).then_some(Asked::Pick(Some(event.key())))
}

/// What a block shows of the pointer and the keyboard.
#[derive(Debug, Clone, Copy, Default)]
struct Lit {
    focus: bool,
    /// How far the ground has faded to `stage`, 0 to 1 (hovered or picked).
    ground: f32,
}

/// One state of the event-block sheet for the gallery's board, named as the sheet names it (`timed`, `band`, `chip`,
/// `row`, `picked`, `hover`, `focus`), with the sheet's words, in the second account's colour.
pub(crate) fn event_board(ui: &mut Ui, theme: &Theme, state: &str) -> Response {
    let (size, kind, words) = match state {
        "band" => (vec2(320.0, 24.0), Kind::Band, Words { title: "Open day", detail: "Sat – Sun", time: "" }),
        "chip" => (vec2(152.0, 20.0), Kind::Chip, Words { title: "Design review", detail: "", time: "15:00" }),
        "row" => (vec2(440.0, 40.0), Kind::Row, Words { title: "Design review", detail: "Room 3 · 1 h", time: "15:00" }),
        _ => (vec2(160.0, 60.0), Kind::Timed { narrow: false }, Words { title: "Design review", detail: "15:00 · Room 3", time: "" }),
    };
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let name = format!("{} {state}", tr!("Event"));
    response.widget_info(|| WidgetInfo::labeled(egui::accesskit::Role::Button, true, &name));
    let hover = state == "hover" || response.hovered();
    let lit = Lit { focus: state == "focus" || response.has_focus(), ground: if hover || state == "picked" { 1.0 } else { 0.0 } };
    paint_block(ui, theme, theme.account_colour(1), rect, lit, state == "picked", kind, &words);
    if kind == Kind::Row {
        let line = line_width(ui.pixels_per_point());
        ui.painter().rect_filled(Rect::from_min_max(pos2(rect.left(), rect.bottom() - line), rect.max), 0.0, theme.palette.panel_line);
    }
    response
}

/// Paints an event's block: see the event-block sheet. The stroke is painted last, over the bar, as the design does.
#[expect(clippy::too_many_arguments, reason = "the block's place, its state and its words")]
fn paint_block(ui: &Ui, theme: &Theme, bar: Color32, rect: Rect, shown: Lit, picked: bool, kind: Kind, words: &Words<'_>) {
    let p = &theme.palette;
    let ppp = ui.pixels_per_point();
    let line = line_width(ppp);
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let lit = shown.ground > 0.0;
    let ground = crate::theme::mix(p.surface, p.stage, shown.ground);
    match kind {
        Kind::Timed { narrow } => {
            painter.rect_filled(rect, 0.0, ground);
            painter.rect_filled(Rect::from_min_size(rect.min, vec2(pixels(BAR, ppp), rect.height())), 0.0, bar);
            let gap = if narrow { 3.0 } else { 8.0 };
            let left = rect.left() + pixels(BAR, ppp) + gap;
            let width = (rect.right() - left - 4.0).max(1.0);
            let font = theme.fonts.event_cell();
            let title = crate::component::cut(ui, words.title, font, p.ink, width);
            run_at(&painter, pos2(left, rect.top() + 4.0), title, p.ink);
            let room = rect.height() - 4.0 - font.line;
            if !narrow && !words.detail.is_empty() && room >= theme.fonts.mono_s.line {
                let detail = crate::component::cut(ui, words.detail, theme.fonts.mono_s, p.muted, width);
                run_at(&painter, pos2(left, rect.top() + 4.0 + font.line), detail, p.muted);
            }
            stroke(&painter, rect, line, if picked { p.cyan_edge } else { p.line });
        }
        Kind::Band => {
            painter.rect_filled(rect, 0.0, ground);
            painter.rect_filled(Rect::from_min_size(rect.min, vec2(pixels(BAR, ppp), rect.height())), 0.0, bar);
            let left = rect.left() + pixels(BAR, ppp) + 8.0;
            let right = rect.right() - 9.0;
            let detail = (!words.detail.is_empty()).then(|| crate::component::galley(ui, words.detail, theme.fonts.mono_s, p.muted));
            let detail = detail.filter(|detail| detail.size().x + 8.0 < (right - left) / 2.0);
            let detail_room = detail.as_ref().map_or(0.0, |detail| detail.size().x + 8.0);
            let font = theme.fonts.caption;
            let title = crate::component::cut(ui, words.title, font, p.ink, (right - left - detail_room).max(1.0));
            let middle = rect.center().y;
            run_at(&painter, pos2(left, snap(middle - font.line / 2.0, ppp)), title, p.ink);
            if let Some(detail) = detail {
                crate::draw::paint::run_right(&painter, right, snap(middle - theme.fonts.mono_s.line / 2.0, ppp), detail, p.muted);
            }
            stroke(&painter, rect, line, if picked { p.cyan_edge } else { p.line });
        }
        Kind::Chip => {
            if lit {
                let frame = grid::whole(Rect::from_min_max(rect.min - vec2(3.0, 1.0), rect.max + vec2(4.0, 1.0)), ppp);
                let painter = ui.painter().with_clip_rect(frame.intersect(ui.clip_rect()));
                painter.rect_filled(frame, 0.0, ground);
                stroke(&painter, frame, line, if picked { p.cyan_edge } else { p.line });
            }
            let middle = rect.center().y;
            let mark = Rect::from_min_size(pos2(rect.left(), snap(middle - 6.0, ppp)), vec2(pixels(BAR, ppp), 12.0));
            painter.rect_filled(mark, 0.0, bar);
            let mut left = rect.left() + 9.0;
            if !words.time.is_empty() {
                let time = crate::component::galley(ui, words.time, theme.fonts.mono_s, p.muted);
                let wide = time.size().x;
                run_at(&painter, pos2(left, snap(middle - theme.fonts.mono_s.line / 2.0, ppp)), time, p.muted);
                left += wide + 6.0;
            }
            let font = theme.fonts.caption;
            let title = crate::component::cut(ui, words.title, font, p.ink, (rect.right() - left).max(1.0));
            run_at(&painter, pos2(left, snap(middle - font.line / 2.0, ppp)), title, p.ink);
        }
        Kind::Row => {
            if lit {
                painter.rect_filled(rect, 0.0, ground);
            }
            if picked {
                painter.rect_filled(Rect::from_min_size(rect.min, vec2(pixels(2.0, ppp), rect.height())), 0.0, p.cyan_edge);
            }
            row_words(ui, &painter, theme, rect, bar, words);
        }
    }
    if shown.focus && !picked {
        focus_ring(ui.painter(), theme, rect, Ring::Outside);
    }
}

/// The words of a row: the time in `mono` in a column of its own, the account's bar, the title in `body`, and the
/// detail in `mono_s` at the right.
fn row_words(ui: &Ui, painter: &egui::Painter, theme: &Theme, rect: Rect, bar: Color32, words: &Words<'_>) {
    let p = &theme.palette;
    let ppp = ui.pixels_per_point();
    let middle = rect.center().y;
    let left = rect.left() + 16.0;
    let column = time_column(ui, theme);
    let time = crate::component::cut(ui, words.time, theme.fonts.mono, p.muted, column);
    run_at(painter, pos2(left, snap(middle - theme.fonts.mono.line / 2.0, ppp)), time, p.muted);
    let mark = Rect::from_min_size(pos2(left + column + 12.0, snap(middle - 6.0, ppp)), vec2(pixels(BAR, ppp), 12.0));
    painter.rect_filled(mark, 0.0, bar);
    let title_left = mark.right() + 12.0;
    let right = rect.right() - 16.0;
    let detail = (!words.detail.is_empty())
        .then(|| crate::component::cut(ui, words.detail, theme.fonts.mono_s, p.muted, ((right - title_left) / 2.0).max(1.0)));
    let detail_room = detail.as_ref().map_or(0.0, |detail| detail.size().x + 12.0);
    let title = crate::component::cut(ui, words.title, theme.fonts.body, p.ink, (right - title_left - detail_room).max(1.0));
    run_at(painter, pos2(title_left, snap(middle - theme.fonts.body.line / 2.0, ppp)), title, p.ink);
    if let Some(detail) = detail {
        crate::draw::paint::run_right(painter, right, snap(middle - theme.fonts.mono_s.line / 2.0, ppp), detail, p.muted);
    }
}

/// The width of a row's time column: eleven `mono` letters, as `Until 13:00` takes.
pub(super) fn time_column(ui: &Ui, theme: &Theme) -> f32 {
    crate::component::galley(ui, "00000000000", theme.fonts.mono, Color32::PLACEHOLDER).size().x.ceil()
}

/// A 1 px (one point in whole pixels) edge inside `rect` in `colour`.
fn stroke(painter: &egui::Painter, rect: Rect, line: f32, colour: Color32) {
    let (l, t, r, b) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    for edge in [
        Rect::from_min_max(pos2(l, t), pos2(r, t + line)),
        Rect::from_min_max(pos2(l, b - line), pos2(r, b)),
        Rect::from_min_max(pos2(l, t), pos2(l + line, b)),
        Rect::from_min_max(pos2(r - line, t), pos2(r, b)),
    ] {
        painter.rect_filled(edge, 0.0, colour);
    }
}

/// The event editor over the screen while it is open, hanging from New event (an edit: over the details, at the card's
/// bottom right); what was pressed in it.
/// While `quiet` another surface lies over it and has the keys.
/// The frame after it closed, the keyboard goes back to the control that opened it; when that is gone (Edit hidden by
/// a narrow window, or with its event deleted), to the grid's picked day (the month's number, the week's head), and
/// where the view draws no day's button (the day, the agenda), to New event. It is never left nowhere.
pub(super) fn editor(ctx: &egui::Context, theme: &Theme, keymap: &Keymap, calendar: &mut Calendar, quiet: bool) -> Vec<Asked> {
    let anchor = ctx.data(|data| data.get_temp::<Rect>(new_event_id()));
    let open = calendar.editor().is_some();
    if let Some((opener, was_open)) = ctx.data(|data| data.get_temp::<(Id, bool)>(opener_id())) {
        if was_open && !open {
            // The screen was drawn before this: what it drew this frame can take the keyboard.
            let day = if calendar.view() == View::Month { calendar.picked_day().unwrap_or_else(|| calendar.focus()) } else { calendar.focus() };
            let new_event = ctx.data(|data| data.get_temp::<Id>(new_event_id().with("id")));
            let to = [Some(opener), Some(month::number_id(day)), Some(hours::head_id(day)), new_event]
                .into_iter()
                .flatten()
                .find(|id| ctx.read_response(*id).is_some());
            if let Some(to) = to {
                ctx.memory_mut(|memory| memory.request_focus(to));
            }
        }
        ctx.data_mut(|data| data.insert_temp(opener_id(), (opener, open)));
    }
    let card = ctx.data(|data| data.get_temp::<Rect>(card_id()));
    calendar.editor_mut().map(|editor| editor::show(ctx, theme, keymap, editor, (anchor, card), quiet)).unwrap_or_default()
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "grid_screen_tests.rs"]
mod grid_screen_tests;

#[cfg(test)]
#[path = "leave_tests.rs"]
mod leave_tests;

#[cfg(test)]
#[path = "esc_tests.rs"]
mod esc_tests;

#[cfg(test)]
#[path = "focus_tests.rs"]
mod focus_tests;

#[cfg(test)]
#[path = "look_tests.rs"]
mod look_tests;

#[cfg(test)]
#[path = "compose_tests.rs"]
mod compose_tests;

#[cfg(test)]
#[path = "sheet_tests.rs"]
mod sheet_tests;
