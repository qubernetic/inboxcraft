// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The event editor, floating over the calendar's top right as the design's Calendar page shows it: its title, the
//! event's title, Starts and Ends side by side, All day and Private, the repeat presets and Show as busy or free as
//! pills, the calendar for a new event, the guests' addresses, and for an edit of an event with guests the question
//! whether to tell them (Don't tell them, the default, or Send them the update); then why the last save was refused,
//! and Cancel and Save. The fields and choices change the [`Editor`] as they are used; Save and Cancel come back as
//! [`Asked`].

use egui::{Align, Align2, Area, Frame, Id, Layout, Order, Rect, Response, RichText, TextEdit, Ui, vec2};

use super::Asked;
use crate::calendar::Editor;
use crate::calendar::editor::{MAX_FIELD, MAX_GUESTS_TEXT, REPEATS, repeat_text};
use crate::component::button::TextButton;
use crate::component::pill::pill;
use crate::component::popup;
use crate::component::{field as field_style, margin};
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

/// The editor's width, in points, as the design draws it.
const WIDTH: f32 = 440.0;

/// The editor over the top right of `screen`; what was pressed in it.
pub(in crate::draw) fn show(ctx: &egui::Context, theme: &Theme, keymap: &Keymap, editor: &mut Editor, screen: Rect) -> Vec<Asked> {
    let s = &theme.spacing;
    let width = WIDTH.min((screen.width() - s.gap * 4.0).max(1.0));
    let corner = screen.right_top() + vec2(-s.gap * 2.0, s.gap * 7.0);
    let mut asked = Vec::new();
    Area::new(Id::unique("event-editor")).order(Order::Foreground).pivot(Align2::RIGHT_TOP).fixed_pos(corner).show(ctx, |ui| {
        popup::panel(ui, theme, width, |ui| {
            Frame::new().inner_margin(margin(s.m, s.m)).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = s.m;
                form(ui, theme, keymap, editor, &mut asked);
            });
        });
    });
    asked
}

/// A field under its `label` (the caption in the muted colour); `add` draws the field.
fn labelled(ui: &mut Ui, theme: &Theme, label: &str, width: f32, add: impl FnOnce(&mut Ui) -> Response) -> Response {
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = theme.spacing.xs;
        let label = ui.label(RichText::new(label).font(theme.fonts.caption.id()).color(theme.palette.muted));
        add(ui).labelled_by(label.id)
    })
    .inner
}

/// A one-line field `width` points wide holding `text`, `hint` while it is empty.
fn line<'t>(theme: &Theme, salt: &str, text: &'t mut String, hint: &str, width: f32) -> TextEdit<'t> {
    TextEdit::singleline(text)
        .id_salt(("event-editor", salt))
        .hint_text(hint)
        .char_limit(MAX_FIELD)
        .margin(field_style::margin(theme))
        .desired_width(width)
}

/// A row of pills after its `label`.
fn choices(ui: &mut Ui, theme: &Theme, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(label).font(theme.fonts.caption.id()).color(theme.palette.muted));
        add(ui);
    });
}

fn form(ui: &mut Ui, theme: &Theme, keymap: &Keymap, editor: &mut Editor, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let width = ui.available_width();
    ui.label(RichText::new(editor.title()).font(theme.fonts.heading.id()).color(p.text));
    let hint = if editor.form().all_day { "2026-11-06" } else { "2026-11-06 13:00" };
    {
        let form = editor.form_mut();
        labelled(ui, theme, tr!("Title"), width, |ui| ui.add(line(theme, "title", &mut form.summary, tr!("(No title)"), width)));
        let half = ((width - s.s) / 2.0).max(1.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = s.s;
            labelled(ui, theme, tr!("Starts"), half, |ui| ui.add(line(theme, "starts", &mut form.start, hint, half)));
            labelled(ui, theme, tr!("Ends"), half, |ui| ui.add(line(theme, "ends", &mut form.end, hint, half)));
        });
    }
    ui.horizontal(|ui| {
        let mut all_day = editor.form().all_day;
        if ui.checkbox(&mut all_day, tr!("All day")).changed() {
            editor.set_all_day(all_day);
        }
        ui.checkbox(&mut editor.form_mut().private, tr!("Private"));
    });
    choices(ui, theme, tr!("Repeat"), |ui| {
        let form = editor.form_mut();
        for id in REPEATS {
            if pill(ui, theme, repeat_text(id), form.repeat == id).clicked() {
                id.clone_into(&mut form.repeat);
            }
        }
        if !REPEATS.contains(&form.repeat.as_str()) {
            // A repeat the presets do not name stays as it is until a preset is picked.
            pill(ui, theme, repeat_text(&form.repeat), true);
        }
    });
    choices(ui, theme, tr!("Show as"), |ui| {
        let form = editor.form_mut();
        for (busy, name) in [(true, tr!("Busy")), (false, tr!("Free"))] {
            if pill(ui, theme, name, form.busy == busy).clicked() {
                form.busy = busy;
            }
        }
    });
    if !editor.choices().is_empty() {
        let names: Vec<(u64, String)> = editor.choices().to_vec();
        choices(ui, theme, tr!("Calendar"), |ui| {
            let form = editor.form_mut();
            for (id, name) in &names {
                if pill(ui, theme, name, form.calendar == *id).clicked() {
                    form.calendar = *id;
                }
            }
        });
    }
    labelled(ui, theme, tr!("Guests"), width, |ui| {
        ui.add(line(theme, "guests", &mut editor.form_mut().guests, "name@example.org, …", width).char_limit(MAX_GUESTS_TEXT))
    });
    if editor.asks_notify() {
        notify(ui, theme, editor);
    }
    if let Some(problem) = editor.problem() {
        ui.add(egui::Label::new(RichText::new(problem).font(theme.fonts.caption.id()).color(p.flag)).wrap());
    }
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if TextButton::new(tr!("Save")).primary().show(ui, theme, keymap).clicked() {
            asked.push(Asked::Save);
        }
        if TextButton::new(tr!("Cancel")).secondary().show(ui, theme, keymap).clicked() {
            asked.push(Asked::Cancel);
        }
    });
}

/// The question whether to tell the guests of the change, on a tinted box.
fn notify(ui: &mut Ui, theme: &Theme, editor: &mut Editor) {
    let p = &theme.palette;
    let s = &theme.spacing;
    Frame::new().fill(p.hover).corner_radius(s.corner).inner_margin(margin(s.s, s.s)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(editor.notify_question()).font(theme.fonts.body.id()).color(p.text));
        let mut notify = editor.notify();
        ui.horizontal(|ui| {
            ui.radio_value(&mut notify, false, tr!("Don't tell them"));
            ui.radio_value(&mut notify, true, tr!("Send them the update"));
        });
        editor.set_notify(notify);
    });
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
