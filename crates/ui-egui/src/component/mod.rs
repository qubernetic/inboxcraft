// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The components every view is built from (`docs/design.md`), each drawn only from the [`crate::theme`] tokens,
//! moving only through [`crate::motion`] and showing keys only from the [`crate::keymap`]:
//!
//! - [`card`]: a rounded card with a header that opens and closes it (grouped inbox groups, the screener, messages).
//! - [`button`]: an icon button with its tooltip, active or disabled.
//! - [`field`]: the search pill and the margin of a form field.
//! - [`pill`]: a round, selectable chip (filters).
//! - [`tooltip`]: the bubble with the action's name and its key chip.
//! - [`avatar`]: a person's initials on a colour picked from their address.
//! - [`badge`]: an unread count, or a group's icon on the preset's gradient.
//! - [`hover`]: the action group that fades in over a hovered or selected row.
//! - [`toast`]: a message that rises from the bottom edge, with Undo and its key chip.
//! - [`popup`]: a float with menu rows, over sharp text or content that fades.
//! - [`switch`]: a track with a knob, on or off.
//! - [`time_picker`]: presets and a typed time for snooze, send later and reminders ([`typed_time`] reads the text).
//!
//! [`gallery`] draws every one of them on one page for `ui_shot`.

pub mod avatar;
pub mod badge;
pub mod button;
pub mod card;
pub mod field;
pub mod gallery;
pub mod hover;
pub mod pill;
pub mod popup;
pub mod switch;
pub mod time_picker;
pub mod toast;
pub mod tooltip;
pub mod typed_time;

use std::sync::Arc;

use egui::{CornerRadius, Galley, Margin, Ui};

use crate::theme::{Font, Theme};

/// The height of one line of `font`, from the font's own metrics.
pub(crate) fn line_height(ui: &Ui, font: Font) -> f32 {
    ui.fonts_mut(|fonts| fonts.row_height(&font.id()))
}

/// The side of a control (an icon button, a pill's height): a line of button text and a little room.
#[must_use]
pub fn control_side(ui: &Ui, theme: &Theme) -> f32 {
    line_height(ui, theme.fonts.button) + theme.spacing.s * 1.5
}

/// `text` in `font` and `colour`, on one line.
pub(crate) fn galley(ui: &Ui, text: &str, font: Font, colour: egui::Color32) -> Arc<Galley> {
    ui.painter().layout_no_wrap(text.to_owned(), font.id(), colour)
}

/// `text` in `font` and `colour` on one line, cut with an ellipsis at `width`.
pub(crate) fn cut(ui: &Ui, text: &str, font: Font, colour: egui::Color32, width: f32) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font.id(), colour);
    job.wrap = egui::text::TextWrapping { max_width: width.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    ui.painter().layout_job(job)
}

/// A margin of `x` points left and right and `y` above and below.
pub(crate) fn margin(x: f32, y: f32) -> Margin {
    // Distances are a few points; the clamp keeps the casts inside i8.
    let points = |value: f32| value.clamp(0.0, 100.0) as i8;
    Margin::symmetric(points(x), points(y))
}

/// A corner radius of `points`.
pub(crate) fn corner(points: f32) -> CornerRadius {
    // A few points; the clamp keeps the cast inside u8.
    CornerRadius::same(points.clamp(0.0, 255.0) as u8)
}

#[cfg(test)]
mod tests;
