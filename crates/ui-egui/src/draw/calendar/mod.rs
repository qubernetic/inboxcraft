// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendar screen: one card in the window's middle. Until calendar sync brings calendars it says there are none
//! yet and what will show here.

use egui::{RichText, Sense, Ui, vec2};

use super::paint::line_height;
use crate::iconset::{IconId, Style};
use crate::theme::Theme;
use crate::tr;

/// The calendar card's content while there is no calendar: what will show here.
pub(super) fn empty(ui: &mut Ui, theme: &Theme) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let side = line_height(ui, theme.fonts.body) * 3.6;
    let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    theme.icons.paint(ui.painter(), rect, IconId::Calendar, Style::Duotone, p.accent);
    ui.add_space(s.m);
    ui.label(RichText::new(tr!("No calendars yet")).font(theme.fonts.title.id()).color(p.text));
    ui.add_space(s.xs);
    let about = tr!("The calendars of your accounts show here, by day, week, month and agenda, once calendar sync is built.");
    ui.add(egui::Label::new(RichText::new(about).font(theme.fonts.body.id()).color(p.muted)).wrap());
}

#[cfg(test)]
mod tests;
