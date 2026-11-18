// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The event editor, a float as the design's event-editor frames draw it, 432 wide: a new event's hangs from New event,
//! its right edge on New event's; an edit's stands over the details at the calendar card's bottom right. In it: its
//! title with the Esc key, the event's title, Starts and Ends side by side, All day and Private, Repeat as a select
//! with Show as busy or free beside it, "this event only" for a series (coming soon), the calendar of a new event as a
//! select, the guests' addresses, and for an edit of an event with guests the question whether to tell them; then why
//! the last save was refused, and Cancel and Save. The fields are the project's field components with their input caps.
//! Esc or a press outside comes back as [`Asked::Leave`]: the editor closes when nothing was typed; else it stays and
//! says how to keep or throw away the typing (Save; Cancel or a second Esc), and a press outside never closes it. A
//! press outside reaches nothing under the editor.

use egui::{Context, Id, Rect, Response, Ui, pos2, vec2};

use super::Asked;
use crate::calendar::Editor;
use crate::calendar::editor::{MAX_FIELD, MAX_GUESTS_TEXT, Menu, REPEATS, repeat_text};
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
const WIDTH: f32 = 432.0;

/// From the editor of an existing event to the calendar card's right edge, in points, as the edit frames draw it.
const EDIT_RIGHT: f32 = 3.0;

/// How far under its anchor a float hangs, in points ([`Float::anchor`]).
const HANG: f32 = 4.0;

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
            form(ui, theme, keymap, editor, &mut asked);
        });
    });
    if let Some((_, Some(leave))) = shown {
        asked.push(Asked::Leave(leave));
    }
    asked
}

/// A one-line field `width` points wide with its `label`, holding `text` (at most `limit` characters), `hint` while
/// it is empty.
fn line(ui: &mut Ui, theme: &Theme, (salt, label): (&str, &str), text: &mut String, hint: &str, (width, limit): (f32, usize)) -> Response {
    ui.vertical(|ui| {
        ui.set_width(width);
        let id = ui.make_persistent_id(("event-editor", salt));
        field::text_field_limited(ui, theme, id, text, &Field::new(label, hint), limit)
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

fn form(ui: &mut Ui, theme: &Theme, keymap: &Keymap, editor: &mut Editor, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let width = ui.available_width();
    let head = theme.boxes().button;
    ui.allocate_ui_with_layout(vec2(width, head), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.set_min_height(head);
        key_chip(ui, theme, tr!("Esc"), ChipState::Default);
        ui.add_space(8.0);
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            crate::component::label(ui, theme.fonts.heading, editor.title(), p.ink);
        });
    });
    let hint = if editor.form().all_day { "2026-11-06" } else { "2026-11-06 13:00" };
    {
        let form = editor.form_mut();
        line(ui, theme, ("title", tr!("Title")), &mut form.summary, tr!("(No title)"), (width, MAX_FIELD));
        let half = ((width - SIDE_GAP) / 2.0).max(1.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = SIDE_GAP;
            line(ui, theme, ("starts", tr!("Starts")), &mut form.start, hint, (half, MAX_FIELD));
            line(ui, theme, ("ends", tr!("Ends")), &mut form.end, hint, (half, MAX_FIELD));
        });
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 24.0;
        let all_day = editor.form().all_day;
        if checkbox(ui, theme, tr!("All day"), all_day).clicked() {
            editor.set_all_day(!all_day);
        }
        let form = editor.form_mut();
        if checkbox(ui, theme, tr!("Private"), form.private).clicked() {
            form.private = !form.private;
        }
    });
    repeat_and_show_as(ui, theme, editor, width);
    if editor.id().is_some() && editor.form().repeat != REPEATS[0] {
        // Changing one occurrence of a series is not built yet: the whole series changes.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            soon(ui, theme, |ui| {
                checkbox(ui, theme, tr!("This event only"), false);
                soon_tag(ui, theme);
            });
        });
    }
    calendar_choice(ui, theme, editor, width);
    line(ui, theme, ("guests", tr!("Guests")), &mut editor.form_mut().guests, "name@example.org, …", (width, MAX_GUESTS_TEXT));
    if editor.asks_notify() {
        notify(ui, theme, editor);
    }
    if let Some(problem) = editor.problem() {
        let galley = crate::component::wrapped(ui, problem, theme.fonts.caption, p.ink, width, 4, egui::Align::Min);
        let (rect, response) = ui.allocate_exact_size(vec2(width, galley.size().y), egui::Sense::hover());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Label, true, problem));
        crate::draw::paint::rows_at(ui.painter(), rect.min, width, galley, p.ink);
    }
    ui.allocate_ui_with_layout(vec2(width, theme.boxes().button), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if TextButton::new(tr!("Save")).primary().show(ui, theme, keymap).clicked() {
            asked.push(Asked::Save);
        }
        if TextButton::new(tr!("Cancel")).plain().show(ui, theme, keymap).clicked() {
            asked.push(Asked::Cancel);
        }
    });
}

/// Repeat as a select, filling what Show as, the segmented control beside it, leaves.
fn repeat_and_show_as(ui: &mut Ui, theme: &Theme, editor: &mut Editor, width: f32) {
    let font = theme.fonts.button;
    let shows = [(true, tr!("Busy"), font), (false, tr!("Free"), font)];
    let show_as = segmented::width(ui, theme, &shows).min(width);
    let repeat = (width - show_as - SIDE_GAP).max(1.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = SIDE_GAP;
        let mut open = editor.menu() == Some(Menu::Repeat);
        let names = REPEATS.map(repeat_text);
        let options: Vec<(usize, &str)> = names.iter().copied().enumerate().collect();
        let current = REPEATS.iter().position(|id| *id == editor.form().repeat);
        // A repeat the presets do not name stays as it is until a preset is picked.
        let shown = repeat_text(&editor.form().repeat);
        let chosen = labelled(ui, theme, tr!("Repeat"), repeat, |ui| {
            let (_, taken) =
                Select { name: tr!("Repeat"), shown, options: &options, current, icon: IconId::Changed, width: repeat }.show(ui, theme, &mut open);
            taken
        });
        keep_menu(editor, Menu::Repeat, open);
        if let Some(id) = chosen.and_then(|index| REPEATS.get(index)) {
            (*id).clone_into(&mut editor.form_mut().repeat);
        }
        let form = editor.form_mut();
        labelled(ui, theme, tr!("Show as"), show_as, |ui| {
            ui.add_space(2.0);
            if let Some(busy) = segmented(ui, theme, &shows, form.busy) {
                form.busy = busy;
            }
        });
    });
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

/// The question whether to tell the guests of the change, with its two answers.
fn notify(ui: &mut Ui, theme: &Theme, editor: &mut Editor) {
    let p = &theme.palette;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        crate::component::label(ui, theme.fonts.body, &editor.notify_question(), p.ink);
        let notify = editor.notify();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 24.0;
            if radio(ui, theme, tr!("Don't tell them"), !notify).clicked() {
                editor.set_notify(false);
            }
            if radio(ui, theme, tr!("Send them the update"), notify).clicked() {
                editor.set_notify(true);
            }
        });
    });
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
