// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The event editor, floating over the calendar's top right as the design's Calendar page shows it: its title, the
//! event's title, Starts and Ends side by side, All day and Private as check boxes, Repeat as a select with its popup
//! and Show as busy or free as a segmented control beside it, the calendar for a new event as a segmented control (a
//! select where the calendars do not fit side by side), the guests' addresses, and for an edit of an event with guests
//! the question whether to tell them (Don't tell them, the default, or Send them the update); then why the last save
//! was refused, and Cancel and Save. The fields and choices change the [`Editor`] as they are used; Save and Cancel
//! come back as [`Asked`].

use egui::{Align, Align2, Area, Frame, Id, Layout, Margin, Order, Rect, Response, RichText, TextEdit, Ui, vec2};

use super::Asked;
use crate::calendar::Editor;
use crate::calendar::editor::{MAX_FIELD, MAX_GUESTS_TEXT, Menu, REPEATS, repeat_text};
use crate::component::button::TextButton;
use crate::component::checkbox::{checkbox, radio};
use crate::component::margin;
use crate::component::popup;
use crate::component::segmented::{self, segmented};
use crate::component::select::Select;
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::theme::{Face, Font, Theme, Weight};
use crate::tr;

/// The editor's width, in points, as the design draws it.
const WIDTH: f32 = 430.0;

/// Between the editor's rows, in points.
const ROWS: f32 = 11.0;

/// From a label to its field, in points.
const LABEL_GAP: f32 = 6.0;

/// Between Repeat and Show as, in points.
const CHOICE_GAP: f32 = 14.0;

/// The room around the text of a field: the height of a [`select`] field, as the design draws it.
const FIELD_MARGIN: Margin = Margin { left: 10, right: 10, top: 9, bottom: 9 };

/// The editor over the top right of `screen`; what was pressed in it.
pub(in crate::draw) fn show(ctx: &egui::Context, theme: &Theme, keymap: &Keymap, editor: &mut Editor, screen: Rect) -> Vec<Asked> {
    let s = &theme.spacing;
    let width = WIDTH.min((screen.width() - s.gap * 4.0).max(1.0));
    let corner = screen.right_top() + vec2(-(s.gap + s.l), s.gap * 6.0 + s.xs + 3.0);
    let mut asked = Vec::new();
    Area::new(Id::unique("event-editor")).order(Order::Foreground).pivot(Align2::RIGHT_TOP).fixed_pos(corner).show(ctx, |ui| {
        popup::panel(ui, theme, width, |ui| {
            Frame::new().inner_margin(margin(s.m, s.m)).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = ROWS;
                form(ui, theme, keymap, editor, &mut asked);
            });
        });
    });
    asked
}

/// A label's font: the caption, SemiBold.
fn label_font(theme: &Theme) -> Font {
    Font { face: Face::Sans(Weight::SemiBold), ..theme.fonts.caption }
}

fn label(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    ui.label(RichText::new(text).font(label_font(theme).id()).color(theme.palette.muted))
}

/// A field under its `label` (the caption in the muted colour); `add` draws the field.
fn labelled(ui: &mut Ui, theme: &Theme, text: &str, width: f32, add: impl FnOnce(&mut Ui) -> Response) -> Response {
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = LABEL_GAP;
        let label = label(ui, theme, text);
        add(ui).labelled_by(label.id)
    })
    .inner
}

/// A control under its `label` that speaks for itself; `add` draws the control.
fn captioned(ui: &mut Ui, theme: &Theme, text: &str, width: f32, add: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = LABEL_GAP;
        label(ui, theme, text);
        add(ui);
    });
}

/// A one-line field `width` points wide holding `text`, `hint` while it is empty.
fn line<'t>(salt: &str, text: &'t mut String, hint: &str, width: f32) -> TextEdit<'t> {
    TextEdit::singleline(text).id_salt(("event-editor", salt)).hint_text(hint).char_limit(MAX_FIELD).margin(FIELD_MARGIN).desired_width(width)
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
    let s = &theme.spacing;
    let width = ui.available_width();
    let title = Font { size: theme.fonts.title.size * 0.9, ..theme.fonts.title };
    ui.label(RichText::new(editor.title()).font(title.id()).color(p.text));
    let hint = if editor.form().all_day { "2026-11-06" } else { "2026-11-06 13:00" };
    {
        let form = editor.form_mut();
        labelled(ui, theme, tr!("Title"), width, |ui| ui.add(line("title", &mut form.summary, tr!("(No title)"), width)));
        let gap = 10.0;
        let half = ((width - gap) / 2.0).max(1.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            labelled(ui, theme, tr!("Starts"), half, |ui| ui.add(line("starts", &mut form.start, hint, half)));
            labelled(ui, theme, tr!("Ends"), half, |ui| ui.add(line("ends", &mut form.end, hint, half)));
        });
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = s.l;
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
    calendar_choice(ui, theme, editor, width);
    labelled(ui, theme, tr!("Guests"), width, |ui| {
        ui.add(line("guests", &mut editor.form_mut().guests, "name@example.org, …", width).char_limit(MAX_GUESTS_TEXT))
    });
    if editor.asks_notify() {
        notify(ui, theme, editor);
    }
    if let Some(problem) = editor.problem() {
        ui.add(egui::Label::new(RichText::new(problem).font(theme.fonts.caption.id()).color(p.flag)).wrap());
    }
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = s.s;
        if TextButton::new(tr!("Save")).primary().show(ui, theme, keymap).clicked() {
            asked.push(Asked::Save);
        }
        if TextButton::new(tr!("Cancel")).secondary().show(ui, theme, keymap).clicked() {
            asked.push(Asked::Cancel);
        }
    });
}

/// Repeat as a select, filling what Show as, the segmented control beside it, leaves.
fn repeat_and_show_as(ui: &mut Ui, theme: &Theme, editor: &mut Editor, width: f32) {
    let font = theme.fonts.button;
    let shows = [(true, tr!("Busy"), font), (false, tr!("Free"), font)];
    let show_as = segmented::width(ui, theme, &shows).min(width);
    let repeat = (width - show_as - CHOICE_GAP).max(1.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = CHOICE_GAP;
        let mut open = editor.menu() == Some(Menu::Repeat);
        let names = REPEATS.map(repeat_text);
        let options: Vec<(usize, &str)> = names.iter().copied().enumerate().collect();
        let current = REPEATS.iter().position(|id| *id == editor.form().repeat);
        // A repeat the presets do not name stays as it is until a preset is picked.
        let shown = repeat_text(&editor.form().repeat);
        let mut chosen = None;
        labelled(ui, theme, tr!("Repeat"), repeat, |ui| {
            let (field, taken) =
                Select { name: tr!("Repeat"), shown, options: &options, current, icon: IconId::Changed, width: repeat }.show(ui, theme, &mut open);
            chosen = taken;
            field
        });
        keep_menu(editor, Menu::Repeat, open);
        if let Some(id) = chosen.and_then(|index| REPEATS.get(index)) {
            (*id).clone_into(&mut editor.form_mut().repeat);
        }
        let form = editor.form_mut();
        captioned(ui, theme, tr!("Show as"), show_as, |ui| {
            if let Some(busy) = segmented(ui, theme, &shows, form.busy) {
                form.busy = busy;
            }
        });
    });
}

/// The calendar of a new event: a segmented control, or a select where the calendars do not fit side by side.
fn calendar_choice(ui: &mut Ui, theme: &Theme, editor: &mut Editor, width: f32) {
    if editor.choices().is_empty() {
        return;
    }
    let names: Vec<(u64, String)> = editor.choices().to_vec();
    let font = theme.fonts.button;
    let wide: Vec<(u64, &str, Font)> = names.iter().map(|(id, name)| (*id, name.as_str(), font)).collect();
    let current = editor.form().calendar;
    if segmented::width(ui, theme, &wide) <= width {
        captioned(ui, theme, tr!("Calendar"), width, |ui| {
            if let Some(calendar) = segmented(ui, theme, &wide, current) {
                editor.form_mut().calendar = calendar;
            }
        });
        return;
    }
    let mut open = editor.menu() == Some(Menu::Calendar);
    let options: Vec<(u64, &str)> = names.iter().map(|(id, name)| (*id, name.as_str())).collect();
    let shown = options.iter().find(|(id, _)| *id == current).map_or("", |(_, name)| *name);
    let mut chosen = None;
    labelled(ui, theme, tr!("Calendar"), width, |ui| {
        let select = Select { name: tr!("Calendar"), shown, options: &options, current: Some(current), icon: IconId::Calendar, width };
        let (field, taken) = select.show(ui, theme, &mut open);
        chosen = taken;
        field
    });
    keep_menu(editor, Menu::Calendar, open);
    if let Some(calendar) = chosen {
        editor.form_mut().calendar = calendar;
    }
}

/// The question whether to tell the guests of the change, on a tinted box.
fn notify(ui: &mut Ui, theme: &Theme, editor: &mut Editor) {
    let p = &theme.palette;
    let s = &theme.spacing;
    Frame::new().fill(p.hover).corner_radius(10).inner_margin(margin(s.m, s.m)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = s.s;
        ui.label(RichText::new(editor.notify_question()).font(theme.fonts.body.id()).color(p.text));
        let notify = editor.notify();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = s.l;
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
