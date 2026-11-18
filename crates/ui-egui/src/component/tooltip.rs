// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tooltip: a float that takes no keys, the `surface` fill with the 1 px `line` edge and the one shadow, square.
//! The name of the control in mono 11 and, when its action has a key, the key as a chip; a control that cannot be used
//! adds why under the name, in `muted`. It wraps at 280 wide. The key is read from the [`Keymap`] each time the bubble
//! is drawn, so a changed key shows at once. It appears after egui's short hover delay.

use std::sync::Arc;

use egui::layers::ShapeIdx;
use egui::text::{LayoutJob, TextWrapping};
use egui::{Galley, Response, Role, Sense, Tooltip, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint::{self, FloatEdge};
use crate::keymap::Keymap;
use crate::theme::{Font, Theme};

use super::key::{Chip, ChipState};

/// The widest a tooltip gets, in points; its words wrap inside it.
const WIDEST: f32 = 280.0;

/// The room round a tooltip's content: 4 over and under, 8 beside.
const PAD: egui::Vec2 = vec2(8.0, 4.0);

/// The room between the name and the key chip, in points.
const GAP: f32 = 8.0;

/// The most lines a tooltip's name or reason takes before it ends with an ellipsis.
const MOST_ROWS: usize = 6;

/// `text` in `font` and `colour`, wrapped at `width`, at most [`MOST_ROWS`] lines.
fn wrapped(ui: &Ui, text: &str, font: Font, colour: egui::Color32, width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::single_section(font.text(text), font.format(colour));
    job.wrap = TextWrapping { max_width: width, max_rows: MOST_ROWS, break_anywhere: false, overflow_character: Some('…') };
    ui.painter().layout_job(job)
}

/// The width a galley takes in a tooltip: all of [`WIDEST`]'s room once it wraps, else its own on whole points.
fn width_of(galley: &Galley, room: f32) -> f32 {
    if galley.rows.len() > 1 { room } else { galley.size().x.ceil().min(room) }
}

/// A node for a screen reader over `rect` in `ui`, its id `id`: `text` in `role`.
fn spoken(ui: &Ui, rect: egui::Rect, id: egui::Id, role: Role, text: &str) {
    let response = ui.interact(rect, id, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(role, true, text));
}

/// The bubble: the name of the action and the chip of its key from `keymap`.
pub fn bubble(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, action: Option<&str>) {
    bubble_noted(ui, theme, keymap, name, action, None);
}

/// The bubble with `note`, why the control cannot be used, in `muted` under the name, when there is one.
pub fn bubble_noted(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, action: Option<&str>, note: Option<&str>) {
    bubble_in(ui, theme, keymap, name, action, note, None);
}

/// [`bubble_noted`] as a board draws it under a control: its shadow goes into `under`, a place kept on the same layer
/// before the control was drawn, so the control stands over the shadow and keeps its colours, as on the design's
/// boards.
pub(crate) fn bubble_noted_under(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, action: Option<&str>, note: Option<&str>, under: ShapeIdx) {
    bubble_in(ui, theme, keymap, name, action, note, Some(under));
}

/// [`bubble_noted`], its shadow in the kept place `under` when there is one.
fn bubble_in(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, action: Option<&str>, note: Option<&str>, under: Option<ShapeIdx>) {
    let p = &theme.palette;
    let font = theme.fonts.mono_s;
    let room = WIDEST - PAD.x * 2.0;
    let chip = action.and_then(|action| keymap.chip(ui.ctx(), action)).map(|key| (Chip::new(ui, theme, &key, ChipState::Default), key));
    let chip_room = chip.as_ref().map_or(0.0, |(chip, _)| chip.size().x + GAP);
    let title = wrapped(ui, name, font, p.ink, (room - chip_room).max(1.0));
    let reason = note.map(|note| wrapped(ui, note, font, p.muted, room));
    let title_width = width_of(&title, room - chip_room);
    let first = title.size().y.max(chip.as_ref().map_or(0.0, |(chip, _)| chip.size().y)).ceil();
    let first_width = title_width + chip_room;
    let reason_width = reason.as_ref().map_or(0.0, |reason| width_of(reason, room));
    let reason_height = reason.as_ref().map_or(0.0, |reason| reason.size().y.ceil());
    let size = vec2(first_width.max(reason_width) + PAD.x * 2.0, first + reason_height + PAD.y * 2.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Tooltip, true, name));
    let painter = ui.painter();
    let mut float = paint::float_shapes(theme, rect, FloatEdge::Quiet).into_iter();
    // The shadow comes first; on a board it goes into the place kept under the control.
    if let Some(under) = under
        && let Some(shadow) = float.next()
    {
        painter.set(under, shadow);
    }
    painter.extend(float);

    let left = rect.left() + PAD.x;
    let top = rect.top() + PAD.y;
    painter.galley(pos2(left, top + ((first - title.size().y) / 2.0).round()), title, p.ink);
    if let Some((chip, key)) = chip {
        let at = pos2(left + title_width + GAP, top + ((first - chip.size().y) / 2.0).round());
        spoken(ui, egui::Rect::from_min_size(at, chip.size()), response.id.with("key"), Role::Label, &key);
        chip.paint(ui.painter(), at);
    }
    if let (Some(reason), Some(note)) = (reason, note) {
        let at = pos2(left, top + first);
        spoken(ui, egui::Rect::from_min_size(at, reason.size()), response.id.with("note"), Role::Label, note);
        ui.painter().galley(at, reason, p.muted);
    }
}

/// Gives `response` a tooltip with `name` and the key of `action`, shown while it is hovered, enabled or not.
pub fn attach(response: Response, theme: &Theme, keymap: &Keymap, name: &str, action: Option<&str>) -> Response {
    attach_note(response, theme, keymap, name, action, None)
}

/// Gives `response` a tooltip with `name`, the key of `action` and `note` under them, shown while it is hovered.
pub fn attach_note(response: Response, theme: &Theme, keymap: &Keymap, name: &str, action: Option<&str>, note: Option<&str>) -> Response {
    let mut tooltip = Tooltip::for_widget(&response);
    let open = Tooltip::should_show_tooltip(&response, false);
    tooltip.popup = tooltip.popup.open(open).frame(egui::Frame::NONE);
    tooltip.show(|ui| bubble_noted(ui, theme, keymap, name, action, note));
    response
}

#[cfg(test)]
#[path = "tooltip_tests.rs"]
mod tests;
