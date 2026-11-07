// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The time picker, one for snooze, send later and reminders (`docs/design.md`, "Time picker"): its title, the
//! presets still ahead with their time (a preset behind is hidden), "Or type a time" with a field whose text counts as
//! soon as it parses ([`typed_time`]), without Enter, the time it reads shown under it, the alert switch when the
//! caller asks about one, Cancel and the confirm button.
//!
//! The time shown is always the time used: the line under the field and the confirm button read the same text at the
//! same now, every frame, so a typed `17:30` that falls behind while the picker is open shows, and is, tomorrow's.
//! The alert switch lives in the [`TimePicker`] the caller keeps, so it remembers its last value from one opening to
//! the next; [`TimePicker::open`] clears only the text.

use egui::{Align, Align2, Frame, Key, Layout, Margin, Response, Role, Sense, Stroke, StrokeKind, TextEdit, Ui, WidgetInfo, pos2, vec2};
use inboxcraft_time::{Offset, Timestamp};

use crate::iconset::{IconId, Style};
use crate::keymap::Keymap;
use crate::motion;
use crate::theme::Theme;
use crate::tr;

use super::button::TextButton;
use super::switch::switch;
use super::typed_time::{self, long, short};
use super::{control_side, cut, galley, line_height, margin};

pub use super::typed_time::MAX_TYPED;

/// A preset: its name and the time it stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset<'a> {
    /// What the row says ("Later today").
    pub name: &'a str,
    /// The time it picks.
    pub at: Timestamp,
}

/// What the picker asks: its words and its presets.
#[derive(Debug, Clone, Copy)]
pub struct Ask<'a> {
    /// The title over the presets ("Snooze until").
    pub title: &'a str,
    /// The confirm button ("Snooze", "Schedule", "Remind me").
    pub confirm: &'a str,
    /// The alert switch's name ("Notify me when it comes back"), or `None` for no switch.
    pub alert: Option<&'a str>,
    /// The presets, in the order shown; those not ahead of now are hidden.
    pub presets: &'a [Preset<'a>],
}

/// What the person did in this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Picked {
    /// Nothing yet: the picker stays open.
    Open,
    /// Cancel or Escape.
    Cancelled,
    /// A time, from a preset or typed, and whether to alert (always `false` without an alert switch).
    At {
        /// The time picked, ahead of now.
        at: Timestamp,
        /// Whether the switch asked for an alert.
        alert: bool,
    },
}

/// The picker's state, kept by the caller between frames and between openings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimePicker {
    typed: String,
    alert: bool,
}

impl TimePicker {
    /// A picker whose alert switch starts at `alert` (a value the caller saved).
    #[must_use]
    pub fn with_alert(alert: bool) -> Self {
        Self { typed: String::new(), alert }
    }

    /// A new opening: the typed text goes, the alert switch stays as it was left.
    pub fn open(&mut self) {
        self.typed.clear();
    }

    /// The text typed so far.
    #[must_use]
    pub fn typed(&self) -> &str {
        &self.typed
    }

    /// Puts `text` in the field, as if typed, cut to [`MAX_TYPED`] bytes at a character boundary.
    pub fn set_typed(&mut self, text: &str) {
        let mut end = text.len().min(MAX_TYPED);
        while !text.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        self.typed = text.get(..end).unwrap_or_default().to_owned();
    }

    /// Whether the alert switch is on.
    #[must_use]
    pub fn alert(&self) -> bool {
        self.alert
    }

    /// Sets the alert switch.
    pub fn set_alert(&mut self, on: bool) {
        self.alert = on;
    }

    /// The time the typed text names at `now` in `offset`: the one shown and the one the confirm button uses.
    #[must_use]
    pub fn chosen(&self, now: Timestamp, offset: Offset) -> Option<Timestamp> {
        typed_time::parse(&self.typed, now, offset)
    }
}

/// A preset row: the name on the left, the time on the right in the muted caption; clicked when chosen.
fn preset_row(ui: &mut Ui, theme: &Theme, name: &str, time: &str) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let height = control_side(ui, theme);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width().max(height), height), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::MenuItem, true, tr!("{name}, {time}", name = name, time = time)));
    let lit = response.hovered() || response.has_focus();
    let hover = motion::toggle(ui.ctx(), response.id.with("hover"), lit, theme.motion.hover_fade);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if hover > 0.0 {
            painter.rect_filled(rect, s.corner, p.hover.gamma_multiply(hover));
        }
        let time = galley(ui, time, theme.fonts.caption, p.muted);
        let right = rect.right() - s.m - time.size().x;
        painter.galley(pos2(right, rect.center().y - time.size().y / 2.0), time, p.muted);
        let left = rect.left() + s.m;
        let text = cut(ui, name, theme.fonts.body, p.text, right - s.m - left);
        painter.galley(pos2(left, rect.center().y - text.size().y / 2.0), text, p.text);
        if response.has_focus() {
            painter.rect_stroke(rect, s.corner, Stroke::new(2.0, p.focus), StrokeKind::Inside);
        }
    }
    response
}

/// The line under the field: the time read, with a tick, in the accent colour; or why there is none.
fn reading(ui: &mut Ui, theme: &Theme, chosen: Option<Timestamp>, now: Timestamp, offset: Offset) {
    let p = &theme.palette;
    let s = &theme.spacing;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = s.xs;
        match chosen {
            Some(at) => {
                let side = line_height(ui, theme.fonts.caption);
                let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
                theme.icons.paint(ui.painter(), rect, IconId::Done, Style::Regular, p.accent);
                ui.label(galley(ui, &long(at, now, offset), theme.fonts.caption, p.accent));
            }
            None => {
                ui.label(galley(ui, tr!("Not a time yet: try thu 9, 12 nov or in 3 days"), theme.fonts.caption, p.muted));
            }
        }
    });
}

/// Draws the picker into `ui` (a popup's panel, see [`super::popup`]) for `state` and `ask`, at `now` on the wall
/// clock of `offset`; what the person did in this frame.
pub fn show(ui: &mut Ui, theme: &Theme, state: &mut TimePicker, ask: &Ask<'_>, now: Timestamp, offset: Offset) -> Picked {
    let p = &theme.palette;
    let s = &theme.spacing;
    let keys = Keymap::empty();
    let mut picked = Picked::Open;
    Frame::new().inner_margin(margin(s.s, s.s)).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = s.xs;
        ui.label(galley(ui, ask.title, theme.fonts.strong, p.text));
        for preset in ask.presets.iter().filter(|preset| preset.at > now) {
            if preset_row(ui, theme, preset.name, &short(preset.at, now, offset)).clicked() {
                picked = Picked::At { at: preset.at, alert: false };
            }
        }
        let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter().rect_filled(line, 0.0, p.line);
        let caption = ui.label(galley(ui, tr!("Or type a time"), theme.fonts.caption, p.muted));
        let id = ui.make_persistent_id("time-picker-typed");
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), control_side(ui, theme) + s.xs), Sense::hover());
        let edge = if ui.memory(|memory| memory.has_focus(id)) { Stroke::new(2.0, p.focus) } else { Stroke::new(1.0, p.field_border) };
        ui.painter().rect_filled(rect, s.corner, p.field);
        ui.painter().rect_stroke(rect, s.corner, edge, StrokeKind::Inside);
        let inner = rect.shrink2(vec2(s.s, 0.0));
        let edit = TextEdit::singleline(&mut state.typed)
            .id(id)
            .char_limit(MAX_TYPED)
            .hint_text(tr!("thu 9, 12 nov 17:00, in 3 days"))
            .frame(Frame::NONE)
            .margin(Margin::ZERO)
            .align(Align2::LEFT_CENTER)
            .desired_width(inner.width());
        let field = ui.put(inner, edit).labelled_by(caption.id);
        // Read after the edit, so the line and the button see this frame's text.
        let chosen = state.chosen(now, offset);
        let entered = field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
        if !state.typed.trim().is_empty() {
            reading(ui, theme, chosen, now, offset);
        }
        if let Some(name) = ask.alert {
            ui.horizontal(|ui| {
                ui.label(galley(ui, name, theme.fonts.body, p.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if switch(ui, theme, name, state.alert).clicked() {
                        state.alert = !state.alert;
                    }
                });
            });
        }
        ui.add_space(s.xs);
        let row = vec2(ui.available_width(), control_side(ui, theme));
        ui.allocate_ui_with_layout(row, Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = s.s;
            let confirm = match chosen {
                Some(_) => TextButton::new(ask.confirm).primary(),
                None => TextButton::new(ask.confirm).primary().unavailable(tr!("Pick a time above or type one first")),
            };
            if (confirm.show(ui, theme, &keys).clicked() || entered)
                && let Some(at) = chosen
            {
                picked = Picked::At { at, alert: false };
            }
            if TextButton::new(tr!("Cancel")).secondary().show(ui, theme, &keys).clicked() {
                picked = Picked::Cancelled;
            }
        });
    });
    if ui.input(|input| input.key_pressed(Key::Escape)) {
        picked = Picked::Cancelled;
    }
    match picked {
        Picked::At { at, .. } => Picked::At { at, alert: ask.alert.is_some() && state.alert },
        other => other,
    }
}

#[cfg(test)]
#[path = "time_picker_tests.rs"]
mod tests;
