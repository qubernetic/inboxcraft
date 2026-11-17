// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The components every view is built from (`docs/design.md`), each drawn only from the [`crate::theme`] tokens,
//! moving only through [`crate::motion`] and showing keys only from the [`crate::keymap`]:
//!
//! - [`card`]: a rounded card with a header that opens and closes it (grouped inbox groups, the screener, messages).
//! - [`button`]: an icon button with its tooltip, active or disabled.
//! - [`field`]: the search pill and the margin of a form field.
//! - [`pill`]: a round, selectable chip (filters).
//! - [`segmented`]: a few choices side by side in one track, one of them on the selection.
//! - [`select`]: a closed field with the choice in it and a popup of the choices.
//! - [`checkbox`]: a filled check box and a radio button.
//! - [`tooltip`]: the bubble with the action's name and its key chip.
//! - [`key`]: the chip of a key, its words read from a shortcut.
//! - [`tag`]: a short mark in a 1 px box, and the coming-soon mark.
//! - [`label`]: the bracketed section label and the written form of a count.
//! - [`avatar`]: a person's initials, `muted` on a `stage` square.
//! - [`badge`]: an unread count, or a group's outline icon.
//! - [`hover`]: the action group that fades in over a hovered or selected row.
//! - [`toast`]: a message that rises from the bottom edge, with Undo and its key chip.
//! - [`popup`]: a float with menu rows, over sharp text or content that fades.
//! - [`switch`]: a track with a knob, on or off.
//! - [`soon`]: what is designed and not built yet, drawn unavailable, and its "coming soon" tag.
//! - [`time_picker`]: presets and a typed time for snooze, send later and reminders ([`typed_time`] reads the text).
//!
//! [`gallery`] draws every one of them on one page for `ui_shot`.

pub mod avatar;
pub mod badge;
pub mod button;
pub mod card;
pub mod checkbox;
pub mod field;
pub mod gallery;
pub mod hover;
pub mod key;
pub mod label;
pub mod pill;
pub mod popup;
pub mod segmented;
pub mod select;
pub mod soon;
pub mod switch;
pub mod tag;
pub mod time_picker;
pub mod toast;
pub mod tooltip;
pub mod typed_time;

use std::sync::Arc;

use egui::{CornerRadius, Galley, Key, Margin, Response, Ui};

use crate::theme::{Font, Theme};

/// The side of a control (an icon button, a text button, a hover action): the design's fixed button height at the
/// text scale; its text is centred in it.
#[must_use]
pub fn control_side(theme: &Theme) -> f32 {
    theme.boxes().button
}

/// The state a pressable control is drawn in, as its sheet names it: what the pointer and the keyboard do to it, or
/// what a board of the gallery asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    /// Nothing on it.
    Rest,
    /// The pointer over it.
    Hover,
    /// The keyboard focus on it.
    Focus,
    /// Held down by the pointer, or by Enter or Space while it has the focus.
    Pressed,
}

/// What a pressable control shows of the pointer and the keyboard: the hover fill, the pressed look, the focus ring.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Lit {
    pub hover: bool,
    pub pressed: bool,
    pub focus: bool,
}

/// What `response` shows, `enabled` or not, in `theme`; `forced` draws a state whatever the pointer does (the
/// gallery's boards). Hover keeps [`crate::motion::toggle`], which lands at once when motion is reduced. Pressed is
/// the pointer held down on it, or Enter or Space held down while it has the focus.
pub(crate) fn lit(ui: &Ui, theme: &crate::theme::Theme, response: &Response, enabled: bool, forced: Option<Shown>) -> Lit {
    if let Some(shown) = forced {
        return Lit { hover: shown == Shown::Hover, pressed: shown == Shown::Pressed && enabled, focus: shown == Shown::Focus };
    }
    let hovered = enabled && response.hovered();
    let hover = crate::motion::toggle(ui.ctx(), response.id.with("hover"), hovered, theme.motion.hover_fade) > 0.0;
    let focus = response.has_focus();
    let keyed = focus && ui.input(|input| input.key_down(Key::Enter) || input.key_down(Key::Space));
    let pressed = enabled && (response.is_pointer_button_down_on() || keyed);
    Lit { hover: hover && enabled, pressed, focus }
}

/// `text` in `font` and `colour`, on one line as high as the style's line height, with its tracking and, if it asks for
/// them, in capitals.
pub(crate) fn galley(ui: &Ui, text: &str, font: Font, colour: egui::Color32) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::single_section(font.text(text), font.format(colour));
    job.wrap.max_width = f32::INFINITY;
    job.break_on_newline = false;
    ui.painter().layout_job(job)
}

/// [`galley`], cut with an ellipsis at `width`.
pub(crate) fn cut(ui: &Ui, text: &str, font: Font, colour: egui::Color32, width: f32) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::single_section(font.text(text), font.format(colour));
    job.wrap = egui::text::TextWrapping { max_width: width.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    job.break_on_newline = false;
    ui.painter().layout_job(job)
}

/// `text` as a label in `font` and `colour`. A style in capitals paints capitals, but a screen reader hears `text` as
/// written.
pub(crate) fn label(ui: &mut Ui, font: Font, text: &str, colour: egui::Color32) -> egui::Response {
    let response = ui.label(font.rich(text).color(colour));
    if font.caps {
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, ui.is_enabled(), text));
    }
    response
}

/// A margin of `x` points left and right and `y` above and below.
pub(crate) fn margin(x: f32, y: f32) -> Margin {
    // Distances are a few points; the clamp keeps the casts inside i8.
    let points = |value: f32| value.clamp(0.0, 100.0) as i8;
    Margin::symmetric(points(x), points(y))
}

/// A plain tick in `colour` over `rect`, drawn in code: the mark of a ticked box and of a select's current choice.
pub(crate) fn tick(painter: &egui::Painter, rect: egui::Rect, colour: egui::Color32) {
    let at = |x: f32, y: f32| rect.lerp_inside(egui::vec2(x, y));
    painter.line(vec![at(0.15, 0.55), at(0.4, 0.8), at(0.85, 0.2)], egui::Stroke::new(1.5, colour));
}

/// A corner radius of `points`.
pub(crate) fn corner(points: f32) -> CornerRadius {
    // A few points; the clamp keeps the cast inside u8.
    CornerRadius::same(points.clamp(0.0, 255.0) as u8)
}

#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
#[path = "walk_tests.rs"]
mod walk_tests;
