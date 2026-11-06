// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The small pieces the panes are drawn from: one line of text cut to a width, the selection, a navigation row, a
//! badge.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{Color32, Galley, Painter, Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::gradient::{self, Gradient, Stop};
use crate::theme::{Font, Theme};

/// One line of `sections` (text, font token and colour), cut with an ellipsis at `width`.
pub(super) fn line(ui: &Ui, sections: &[(&str, Font, Color32)], width: f32) -> Arc<Galley> {
    let mut job = LayoutJob {
        wrap: TextWrapping { max_width: width.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') },
        break_on_newline: false,
        ..LayoutJob::default()
    };
    for (text, font, colour) in sections {
        job.append(text, 0.0, TextFormat::simple(font.id(), *colour));
    }
    ui.painter().layout_job(job)
}

/// The height of one line of `font`, from the font's own metrics.
pub(super) fn line_height(ui: &Ui, font: Font) -> f32 {
    ui.fonts_mut(|fonts| fonts.row_height(&font.id()))
}

/// The selection over `rect`: the selection gradient from left to right.
pub(super) fn selection(painter: &Painter, theme: &Theme, rect: Rect, corner: f32) {
    let p = &theme.palette;
    let gradient =
        Gradient::Linear { from: pos2(0.0, 0.5), to: pos2(1.0, 0.5), stops: vec![Stop::new(0.0, p.selection), Stop::new(1.0, p.selection_end)] };
    gradient::fill(painter, rect, corner, &gradient);
}

/// The focus ring around a control that has the keyboard focus.
pub(super) fn focus_ring(ui: &Ui, theme: &Theme, response: &Response) {
    if response.has_focus() {
        ui.painter().rect_stroke(response.rect.shrink(1.0), theme.spacing.corner, Stroke::new(2.0, theme.palette.focus), StrokeKind::Inside);
    }
}

/// A pill with `count` in it, its right edge at `right`, centred on `centre_y`; returns its left edge.
pub(super) fn badge(ui: &Ui, theme: &Theme, count: u64, right: f32, centre_y: f32) -> f32 {
    crate::component::badge::count(ui, theme, count, right, centre_y)
}

/// What a navigation row shows.
pub(super) struct NavRow<'a> {
    pub(super) text: &'a str,
    /// What a screen reader says for it.
    pub(super) spoken: String,
    pub(super) badge: u64,
    pub(super) selected: bool,
    pub(super) depth: usize,
}

/// A row of the folder pane: a unified view or a folder, with its unread badge.
pub(super) fn nav_row(ui: &mut Ui, theme: &Theme, row: &NavRow<'_>) -> Response {
    let s = &theme.spacing;
    let p = &theme.palette;
    let height = line_height(ui, theme.fonts.body) + s.s * 1.5;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    response.widget_info(|| WidgetInfo::selected(Role::Button, true, row.selected, &row.spoken));
    if ui.is_rect_visible(rect) {
        if row.selected {
            selection(ui.painter(), theme, rect, s.corner);
        } else if response.hovered() {
            ui.painter().rect_filled(rect, s.corner, p.hover);
        }
        let depth = u16::try_from(row.depth.min(8)).unwrap_or(8);
        let left = rect.left() + s.s + f32::from(depth) * s.m;
        let right = if row.badge > 0 { badge(ui, theme, row.badge, rect.right() - s.s, rect.center().y) - s.s } else { rect.right() - s.s };
        let galley = line(ui, &[(row.text, theme.fonts.body, p.text)], right - left);
        ui.painter().galley(pos2(left, rect.center().y - galley.size().y / 2.0), galley, p.text);
        focus_ring(ui, theme, &response);
    }
    response
}
