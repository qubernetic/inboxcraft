// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The event editor, a float as the design's event-editor frames draw it, 432 wide: a new event's hangs from New event,
//! its right edge on New event's; an edit's stands over the details at the calendar card's bottom right. In it: its
//! title with the Esc key, the event's title, Starts and Ends side by side, All day and Private, Repeat as a select
//! with Show as busy or free beside it, "this event only" for a series (coming soon), the calendar of a new event as a
//! select, the guests' addresses, the description (a press on a link in it with Ctrl or Cmd held copies the link and
//! the status line says so), and for an edit of an event with guests the question whether to tell them; then why
//! the last save was refused, and Cancel and Save. Someone else's event opens read-only, as the design's read-only
//! frame draws it: its title `Event`, every field read-only and every control disabled, a note saying whose event it is
//! and that only the organiser can change it, and Close alone; any press on a link in its description copies it. The fields are the project's field components with their input caps.
//! Esc or a press outside comes back as [`Asked::Leave`]: the editor closes when nothing was typed; else it stays and
//! says how to keep or throw away the typing (Save; Cancel or a second Esc), and a press outside never closes it. A
//! press outside reaches nothing under the editor.
//!
//! The same form shows in three hosts ([`Room`]): the small float; the full editor in the calendar's place
//! ([`full`]), 40 in from its sides, Starts, Ends and the two ticks on one row, Repeat, Show as and Alert on one row,
//! the calendar half wide; and a window of its own ([`window`]), 24 in from its sides, the same rows, its title and
//! Close the window's. Alert shows in the full editor and the window only. The full editor and the window give the
//! keyboard to the title when they open.

use egui::{Context, Id, Rect, Response, Ui, pos2, vec2};

use super::Asked;
use crate::calendar::Editor;
use crate::calendar::MAX_DESCRIPTION;
use crate::calendar::editor::{ALERTS, MAX_FIELD, MAX_GUESTS_TEXT, Menu, REPEATS, alert_text, repeat_text};
use crate::component::button::TextButton;
use crate::component::checkbox::{checkbox, radio};
use crate::component::field::{self, Field};
use crate::component::key::{ChipState, key_chip};
use crate::component::popup::{self, Float};
use crate::component::segmented::{self, segmented};
use crate::component::select::Select;
use crate::component::soon::{soon, soon_tag};
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

/// The editor's width, in points, its edge included, as the design draws it.
pub(super) const WIDTH: f32 = 432.0;

/// The full editor's room from its sides and its top, in points.
const FULL_PAD: (i8, i8) = (40, 20);

/// The window's room from its sides and its top, in points.
const WINDOW_PAD: (i8, i8) = (24, 20);

/// Starts' and Ends' width in the full editor and the window, in points.
const TIME_FIELD: f32 = 200.0;

/// The size of the editor's window of its own, in points: the composer's window, 80 taller.
pub(in crate::draw) const WINDOW: egui::Vec2 = egui::Vec2::new(704.0, 800.0);

/// Where the editor shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::draw) enum Room {
    /// The float over the calendar.
    Small,
    /// In the calendar's place.
    Full,
    /// In a window of its own.
    Window,
}

impl Room {
    /// Whether the form has the room of the full editor: the rows side by side, and Alert.
    fn wide(self) -> bool {
        self != Self::Small
    }
}

/// From the editor of an existing event to the calendar card's right edge, in points, as the edit frames draw it.
const EDIT_RIGHT: f32 = 3.0;

/// How far under its anchor a float hangs, in points ([`Float::anchor`]).
const HANG: f32 = 4.0;

/// The room inside the box of the question to the guests, in points, its edge not counted.
const NOTIFY_PAD: f32 = 12.0;

/// From the question to the guests to its answers, in points.
const NOTIFY_GAP: f32 = 10.0;

/// Between the editor's rows, in points.
const ROWS: f32 = 12.0;

/// From a label to its field, in points.
const LABEL_GAP: f32 = 8.0;

/// Between two fields side by side, in points.
const SIDE_GAP: f32 = 12.0;

/// The editor's float.
fn float_id() -> Id {
    Id::unique("event-editor")
}

/// The editor of a new event hanging from `new_event` (New event's rect, if it was drawn), and of an existing one over
/// the details at the bottom right of `card` (the calendar card's room, if it was drawn); what was pressed in it.
/// While `quiet` another surface lies over it (the palette, the settings sheet) and has the keys and the pointer.
pub(in crate::draw) fn show(
    ctx: &Context,
    theme: &Theme,
    keymap: &Keymap,
    editor: &mut Editor,
    (new_event, card): (Option<Rect>, Option<Rect>),
    quiet: bool,
) -> Vec<Asked> {
    let screen = ctx.content_rect();
    let button = new_event.unwrap_or_else(|| Rect::from_min_size(pos2(screen.right() - 16.0, screen.top() + 48.0), vec2(0.0, 0.0)));
    // Its right edge on New event's: the float hangs from a place as wide as it, ending where the button ends.
    let mut anchor = Rect::from_min_max(pos2(button.right() - WIDTH, button.top()), button.max);
    if let Some(card) = card.filter(|_| editor.id().is_some()) {
        // An edit, as the design's edit frames: its right edge 3 inside the card's, its bottom edge on the card's bottom
        // edge, as tall as it was the frame before (the first frame the window's room keeps it in sight).
        let high = ctx.memory(|memory| memory.area_rect(float_id())).map_or(0.0, |rect| rect.height());
        let bottom = card.bottom() + 1.0 - high - HANG;
        anchor = Rect::from_min_max(pos2(card.right() - EDIT_RIGHT - WIDTH, bottom), pos2(card.right() - EDIT_RIGHT, bottom));
    }
    let float = Float { id: float_id(), anchor, width: WIDTH, returns_to: Id::NULL };
    let mut asked = Vec::new();
    let shown = popup::held(ctx, theme, &float, quiet, |ui| {
        egui::Frame::new().inner_margin(egui::Margin { left: 16, right: 16, top: 12, bottom: 16 }).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, ROWS);
            form(ui, theme, keymap, editor, &mut asked, Room::Small);
        });
    });
    if let Some((_, Some(leave))) = shown {
        asked.push(Asked::Leave(leave));
    }
    asked
}

/// The full editor in `rect`, the calendar's place: what was pressed in it. Its `Esc` comes back as [`Asked::Leave`]
/// when no list of its own is open over it and no other surface (`quiet`) has the keys; nothing is outside it.
pub(in crate::draw) fn full(ui: &mut Ui, theme: &Theme, keymap: &Keymap, editor: &mut Editor, rect: Rect, quiet: bool) -> Vec<Asked> {
    let mut asked = Vec::new();
    let ctx = ui.ctx().clone();
    if !quiet && !popup::any_open(&ctx) && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        asked.push(Asked::Leave(popup::Leave::Escape));
    }
    let mut inside = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt("event-editor-full"));
    inside.set_clip_rect(rect.intersect(ui.clip_rect()));
    inside.painter().rect_filled(rect, 0.0, theme.palette.surface);
    hosted(&mut inside, theme, keymap, editor, &mut asked, (Room::Full, FULL_PAD));
    asked
}

/// The editor in its window of its own, `ui` its content: what was pressed in it. Its `Esc` and its close are the
/// window's ([`super::super::host`]).
pub(in crate::draw) fn window(ui: &mut Ui, theme: &Theme, keymap: &Keymap, editor: &mut Editor) -> Vec<Asked> {
    let mut asked = Vec::new();
    ui.painter().rect_filled(ui.max_rect(), 0.0, theme.palette.surface);
    hosted(ui, theme, keymap, editor, &mut asked, (Room::Window, WINDOW_PAD));
    asked
}

/// The form in the full editor or the window, `pad` in from the sides and the top, scrolling where it does not fit.
fn hosted(ui: &mut Ui, theme: &Theme, keymap: &Keymap, editor: &mut Editor, asked: &mut Vec<Asked>, (room, pad): (Room, (i8, i8))) {
    let fresh = Id::unique(("event-editor-opened", room));
    let opened = ui.ctx().data(|data| data.get_temp::<bool>(fresh)).unwrap_or(false);
    let area = egui::ScrollArea::vertical().id_salt(("event-editor-scroll", room)).auto_shrink([false, false]);
    crate::component::scroll::show(ui, theme, area, |ui| {
        let margin = egui::Margin { left: pad.0, right: pad.0, top: pad.1, bottom: pad.1 };
        egui::Frame::new().inner_margin(margin).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, ROWS);
            form(ui, theme, keymap, editor, asked, room);
        });
    });
    if !opened {
        // Opened in this host: the keyboard goes to the title, never left on a control no longer drawn.
        ui.ctx().data_mut(|data| data.insert_temp(fresh, true));
        let title = ui.ctx().data(|data| data.get_temp::<Id>(title_id())).unwrap_or(Id::NULL);
        ui.ctx().memory_mut(|memory| memory.request_focus(title));
    }
}

/// Where the id of the title's field is kept, for the full editor and the window to give it the keyboard.
fn title_id() -> Id {
    Id::unique("event-editor-title-field")
}

/// Forgets that the full editor and the window were open, so that each gives the keyboard to the title when it opens
/// next: the editor closed, or moved to another host.
pub(in crate::draw) fn forget_opened(ctx: &Context, but: Option<Room>) {
    for room in [Room::Full, Room::Window].into_iter().filter(|room| Some(*room) != but) {
        ctx.data_mut(|data| data.remove_temp::<bool>(Id::unique(("event-editor-opened", room))));
    }
}

/// A one-line field `width` points wide with its `label`, holding `text` (at most `limit` characters), `hint` while
/// it is empty; read and copied but not changed while `read_only`.
fn line(
    ui: &mut Ui,
    theme: &Theme,
    (salt, label): (&str, &str),
    text: &mut String,
    hint: &str,
    (width, limit, read_only): (f32, usize, bool),
) -> Response {
    ui.vertical(|ui| {
        ui.set_width(width);
        let id = ui.make_persistent_id(("event-editor", salt));
        if salt == "title" {
            ui.ctx().data_mut(|data| data.insert_temp(title_id(), id));
        }
        field::text_field_limited(ui, theme, id, text, &Field { read_only, ..Field::new(label, hint) }, limit)
    })
    .inner
}

/// A control under its `label`; `add` draws it.
fn labelled<R>(ui: &mut Ui, theme: &Theme, text: &str, width: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = LABEL_GAP;
        crate::component::label(ui, theme.fonts.label, text, theme.palette.muted);
        add(ui)
    })
    .inner
}

/// Writes back the state of a select's list: `open` for `menu`, leaving another list that is open alone.
fn keep_menu(editor: &mut Editor, menu: Menu, open: bool) {
    if open {
        editor.set_menu(Some(menu));
    } else if editor.menu() == Some(menu) {
        editor.set_menu(None);
    }
}

fn form(ui: &mut Ui, theme: &Theme, keymap: &Keymap, editor: &mut Editor, asked: &mut Vec<Asked>, room: Room) {
    let p = &theme.palette;
    let width = ui.available_width();
    let head = theme.boxes().button;
    if room != Room::Window {
        // The window's title bar names it, and its Esc is the window's.
        ui.allocate_ui_with_layout(vec2(width, head), egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.set_min_height(head);
            key_chip(ui, theme, tr!("Esc"), ChipState::Default);
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                crate::component::label(ui, theme.fonts.heading, editor.title(), p.ink);
            });
        });
    }
    let hint = if editor.form().all_day { "2026-11-06" } else { "2026-11-06 13:00" };
    // Someone else's event: every field read-only, every control disabled, the note saying why, and Close alone.
    let read_only = editor.read_only();
    {
        let form = editor.form_mut();
        line(ui, theme, ("title", tr!("Title")), &mut form.summary, tr!("(No title)"), (width, MAX_FIELD, read_only));
    }
    if room.wide() {
        // Starts, Ends and the two ticks on one row, the ticks under a label's room.
        let time = TIME_FIELD.min(((width - SIDE_GAP * 2.0) / 3.0).max(1.0));
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = SIDE_GAP;
            let form = editor.form_mut();
            line(ui, theme, ("starts", tr!("Starts")), &mut form.start, hint, (time, MAX_FIELD, read_only));
            line(ui, theme, ("ends", tr!("Ends")), &mut form.end, hint, (time, MAX_FIELD, read_only));
            let rest = (width - (time + SIDE_GAP) * 2.0).max(1.0);
            labelled(ui, theme, " ", rest, |ui| {
                ui.add_space(8.0);
                ticks(ui, theme, editor, read_only);
            });
        });
    } else {
        let form = editor.form_mut();
        let half = ((width - SIDE_GAP) / 2.0).max(1.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = SIDE_GAP;
            line(ui, theme, ("starts", tr!("Starts")), &mut form.start, hint, (half, MAX_FIELD, read_only));
            line(ui, theme, ("ends", tr!("Ends")), &mut form.end, hint, (half, MAX_FIELD, read_only));
        });
        ticks(ui, theme, editor, read_only);
    }
    repeat_and_show_as(ui, theme, editor, width, room.wide());
    if editor.id().is_some() && !read_only && editor.form().repeat != REPEATS[0] {
        // Changing one occurrence of a series is not built yet: the whole series changes.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            soon(ui, theme, |ui| {
                checkbox(ui, theme, tr!("This event only"), false);
                soon_tag(ui, theme);
            });
        });
    }
    calendar_choice(ui, theme, editor, if room.wide() { ((width - SIDE_GAP) / 2.0).max(1.0) } else { width });
    line(ui, theme, ("guests", tr!("Guests")), &mut editor.form_mut().guests, "name@example.org, …", (width, MAX_GUESTS_TEXT, read_only));
    description(ui, theme, editor, asked);
    if editor.asks_notify() {
        notify(ui, theme, editor);
    }
    if let Some(note) = editor.read_only_note() {
        let galley = crate::component::wrapped(ui, &note, theme.fonts.caption, p.muted, width, 4, egui::Align::Min);
        let (rect, response) = ui.allocate_exact_size(vec2(width, galley.size().y), egui::Sense::hover());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Label, true, &note));
        crate::draw::paint::rows_at(ui.painter(), rect.min, width, galley, p.muted);
    }
    if let Some(problem) = editor.problem() {
        let galley = crate::component::wrapped(ui, problem, theme.fonts.caption, p.ink, width, 4, egui::Align::Min);
        let (rect, response) = ui.allocate_exact_size(vec2(width, galley.size().y), egui::Sense::hover());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Label, true, problem));
        crate::draw::paint::rows_at(ui.painter(), rect.min, width, galley, p.ink);
    }
    ui.allocate_ui_with_layout(vec2(width, theme.boxes().button), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if read_only {
            if TextButton::new(tr!("Close")).secondary().icon(IconId::Close).show(ui, theme, keymap).clicked() {
                asked.push(Asked::Cancel);
            }
            return;
        }
        if TextButton::new(tr!("Save")).primary().show(ui, theme, keymap).clicked() {
            asked.push(Asked::Save);
        }
        if TextButton::new(tr!("Cancel")).plain().show(ui, theme, keymap).clicked() {
            asked.push(Asked::Cancel);
        }
    });
}

/// All day and Private, side by side; disabled in someone else's event.
fn ticks(ui: &mut Ui, theme: &Theme, editor: &mut Editor, read_only: bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 24.0;
        if read_only {
            ui.disable();
        }
        let all_day = editor.form().all_day;
        if checkbox(ui, theme, tr!("All day"), all_day).clicked() {
            editor.set_all_day(!all_day);
        }
        let form = editor.form_mut();
        if checkbox(ui, theme, tr!("Private"), form.private).clicked() {
            form.private = !form.private;
        }
    });
}

/// The description, its lines kept: read-only in someone else's event and in one read cut short. A press on a link in
/// it with Ctrl (Cmd on a Mac) held, or any press on one in someone else's event, copies the link, as the reader does.
fn description(ui: &mut Ui, theme: &Theme, editor: &mut Editor, asked: &mut Vec<Asked>) {
    let id = ui.make_persistent_id(("event-editor", "description"));
    // A description that cannot be changed asks for nothing while it is empty.
    let read_only = !editor.description_editable();
    let field = Field { read_only, ..Field::new(tr!("Description"), if read_only { "" } else { tr!("optional") }) };
    field::multiline_field_holding(ui, theme, id, &mut editor.form_mut().description, &field, field::Cap::Chars(MAX_DESCRIPTION));
    if let Some((at, command)) = field::take_press(ui.ctx(), id)
        && let Some(link) = editor.press_description(at, command)
    {
        ui.ctx().copy_text(link);
        asked.push(Asked::Copied);
    }
}

/// Repeat as a select, filling what Show as, the segmented control beside it, leaves; with `alert` (the full editor
/// and the window) Alert after them, Repeat and Alert sharing what is left.
fn repeat_and_show_as(ui: &mut Ui, theme: &Theme, editor: &mut Editor, width: f32, alert: bool) {
    let font = theme.fonts.button;
    let shows = [(true, tr!("Busy"), font), (false, tr!("Free"), font)];
    let show_as = segmented::width(ui, theme, &shows).min(width);
    let rest = (width - show_as - SIDE_GAP).max(1.0);
    let repeat = if alert { ((rest - SIDE_GAP) / 2.0).floor().max(1.0) } else { rest };
    let alert_width = (rest - repeat - SIDE_GAP).max(1.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = SIDE_GAP;
        let mut open = editor.menu() == Some(Menu::Repeat);
        let names = REPEATS.map(repeat_text);
        let options: Vec<(usize, &str)> = names.iter().copied().enumerate().collect();
        let current = REPEATS.iter().position(|id| *id == editor.form().repeat);
        // A repeat the presets do not name stays as it is until a preset is picked.
        let shown = repeat_text(&editor.form().repeat);
        let note = editor.read_only().then(|| tr!("Only the organiser can change this event."));
        let chosen = labelled(ui, theme, tr!("Repeat"), repeat, |ui| {
            let select = Select { name: tr!("Repeat"), shown, options: &options, current, icon: IconId::Changed, width: repeat };
            select.show_noted(ui, theme, &mut open, note).1
        });
        keep_menu(editor, Menu::Repeat, open);
        if let Some(id) = chosen.and_then(|index| REPEATS.get(index)) {
            (*id).clone_into(&mut editor.form_mut().repeat);
        }
        let read_only = editor.read_only();
        let form = editor.form_mut();
        labelled(ui, theme, tr!("Show as"), show_as, |ui| {
            if read_only {
                ui.disable();
            }
            ui.add_space(2.0);
            if let Some(busy) = segmented(ui, theme, &shows, form.busy) {
                form.busy = busy;
            }
        });
        if alert {
            alert_choice(ui, theme, editor, alert_width);
        }
    });
}

/// Alert as a select of [`ALERTS`], the event's own shown as it is (the default lead, or one the list does not name);
/// unavailable in someone else's event.
fn alert_choice(ui: &mut Ui, theme: &Theme, editor: &mut Editor, width: f32) {
    let all_day = editor.form().all_day;
    let names = ALERTS.map(|alert| alert_text(alert, all_day));
    let options: Vec<(usize, &str)> = names.iter().map(String::as_str).enumerate().collect();
    let now = editor.form().alert;
    let current = ALERTS.iter().position(|alert| *alert == now);
    let shown = alert_text(now, all_day);
    let mut open = editor.menu() == Some(Menu::Alert);
    let note = editor.read_only().then(|| tr!("Only the organiser can change this event."));
    let chosen = labelled(ui, theme, tr!("Alert"), width, |ui| {
        let select = Select { name: tr!("Alert"), shown: &shown, options: &options, current, icon: IconId::Notify, width };
        select.show_noted(ui, theme, &mut open, note).1
    });
    keep_menu(editor, Menu::Alert, open);
    if let Some(alert) = chosen.and_then(|index| ALERTS.get(index)) {
        editor.form_mut().alert = *alert;
    }
}

/// The calendar of a new event, as a select.
fn calendar_choice(ui: &mut Ui, theme: &Theme, editor: &mut Editor, width: f32) {
    if editor.choices().is_empty() {
        return;
    }
    let names: Vec<(u64, String)> = editor.choices().to_vec();
    let current = editor.form().calendar;
    let mut open = editor.menu() == Some(Menu::Calendar);
    let options: Vec<(u64, &str)> = names.iter().map(|(id, name)| (*id, name.as_str())).collect();
    let shown = options.iter().find(|(id, _)| *id == current).map_or("", |(_, name)| *name);
    let chosen = labelled(ui, theme, tr!("Calendar"), width, |ui| {
        let select = Select { name: tr!("Calendar"), shown, options: &options, current: Some(current), icon: IconId::Calendar, width };
        select.show(ui, theme, &mut open).1
    });
    keep_menu(editor, Menu::Calendar, open);
    if let Some(calendar) = chosen {
        editor.form_mut().calendar = calendar;
    }
}

/// The question whether to tell the guests of the change, with its two answers, in a box as the design's tell-guests
/// frames draw it: `panel`, a 1 px `cyan_edge`, 12 inside, the answers 10 under the question.
fn notify(ui: &mut Ui, theme: &Theme, editor: &mut Editor) {
    let p = &theme.palette;
    let ppp = ui.pixels_per_point();
    let line = super::grid::line(ppp);
    let width = ui.available_width();
    let inset = line + NOTIFY_PAD;
    let start = ui.available_rect_before_wrap().min;
    let start = pos2(super::grid::snap(start.x, ppp), super::grid::snap(start.y, ppp));
    let ground = ui.painter().add(egui::Shape::Noop);
    let inner = Rect::from_min_size(start + vec2(inset, inset), vec2((width - inset * 2.0).max(1.0), f32::INFINITY));
    let mut inside = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    inside.spacing_mut().item_spacing.y = NOTIFY_GAP;
    let question = editor.notify_question();
    let galley = crate::component::wrapped(&inside, &question, theme.fonts.body, p.ink, inner.width(), 4, egui::Align::Min);
    let (rect, response) = inside.allocate_exact_size(vec2(inner.width(), galley.size().y), egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Label, true, &question));
    crate::draw::paint::rows_at(inside.painter(), rect.min, inner.width(), galley, p.ink);
    let notify = editor.notify();
    inside.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 24.0;
        if radio(ui, theme, tr!("Don't tell them"), !notify).clicked() {
            editor.set_notify(false);
        }
        if radio(ui, theme, tr!("Send them the update"), notify).clicked() {
            editor.set_notify(true);
        }
    });
    let bottom = super::grid::snap(inside.min_rect().bottom() + inset, ppp);
    let boxed = Rect::from_min_max(start, pos2(start.x + width, bottom.max(start.y)));
    ui.allocate_rect(boxed, egui::Sense::hover());
    ui.painter().set(ground, egui::Shape::rect_filled(boxed, 0.0, p.panel));
    super::stroke(ui.painter(), boxed, line, p.cyan_edge);
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
