// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Badges: a count in `mono_s`, 16 high: `cyan_text` on the `cyan_muted` wash, `muted` with no fill, or `on_cyan` on
//! `cyan` for the one count a surface is about ([`badge`], [`BadgeKind`]); a group's badge, its outline (Regular) icon in
//! the muted colour without a background; and a chip, a short name in the same colours as the count. All square.

use std::sync::Arc;

use egui::text::LayoutJob;
use egui::{Color32, Galley, Rect, Response, Role, Sense, Ui, Vec2, WidgetInfo, pos2, vec2};

use crate::iconset::IconId;
use crate::theme::{Font, Theme};

use super::galley;
use super::label::count as written;

/// What a badge is about, as its sheet names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeKind {
    /// A count that asks to be read.
    Count,
    /// A count that asks for nothing.
    Muted,
    /// The one count a surface is about.
    Accent,
}

/// A badge for `n` of `kind`: the written count ([`count`](super::label::count): thin groups, `9 999+` at most) in
/// `mono_s`, 4 beside it, at least as wide as high; nothing for 0. A screen reader reads the count.
pub fn badge(ui: &mut Ui, theme: &Theme, n: u64, kind: BadgeKind) -> Response {
    let Some((size, text)) = lay(ui, theme, n) else {
        return ui.allocate_response(Vec2::ZERO, Sense::hover());
    };
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, text.text()));
    if ui.is_rect_visible(rect) {
        paint(ui, theme, rect, text, kind);
    }
    response
}

/// The size of the badge for `n` and its text laid out in the placeholder colour; `None` for 0.
fn lay(ui: &Ui, theme: &Theme, n: u64) -> Option<(Vec2, Arc<Galley>)> {
    let shown = written(n);
    if shown.is_empty() {
        return None;
    }
    let text = in_columns(ui, &shown, theme.fonts.mono_s);
    let height = theme.boxes().badge;
    let width = (text.size().x.ceil() + theme.spacing.s1 * 2.0).max(height);
    Some((vec2(width, height), text))
}

/// `text` in `font` and the placeholder colour, each thin group space (U+202F) as wide as a digit, as the sheet sets
/// it: Plex Mono has no such space, and the face egui falls back on draws a narrower one.
fn in_columns(ui: &Ui, text: &str, font: Font) -> Arc<Galley> {
    let wide = |part: &str| galley(ui, part, font, Color32::PLACEHOLDER).size().x;
    let pad = (wide("0") - wide(GROUP)).max(0.0);
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    job.break_on_newline = false;
    for (at, part) in text.split(GROUP).enumerate() {
        if at > 0 {
            let mut format = font.format(Color32::PLACEHOLDER);
            format.extra_letter_spacing += pad;
            job.append(GROUP, 0.0, format);
        }
        job.append(part, 0.0, font.format(Color32::PLACEHOLDER));
    }
    ui.painter().layout_job(job)
}

/// The thin space between groups of digits.
const GROUP: &str = "\u{202f}";

/// Paints a badge of `kind` over `rect` with `text` in the middle.
fn paint(ui: &Ui, theme: &Theme, rect: Rect, text: Arc<Galley>, kind: BadgeKind) {
    let p = &theme.palette;
    let (fill, ink) = match kind {
        BadgeKind::Count => (Some(p.cyan_muted), p.cyan_text),
        BadgeKind::Muted => (None, p.muted),
        BadgeKind::Accent => (Some(p.cyan), p.on_cyan),
    };
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, 0.0, fill);
    }
    ui.painter().galley(rect.center() - text.size() / 2.0, text, ink);
}

/// The width of a count badge for `count`; 0 for 0.
#[must_use]
pub fn count_width(ui: &Ui, theme: &Theme, count: u64) -> f32 {
    lay(ui, theme, count).map_or(0.0, |(size, _)| size.x)
}

/// Paints a count badge (the sheet's count) with `count` in it, its right edge at `right` (on the nearest pixel
/// boundary), centred on `centre_y`, every edge on whole physical pixels; returns its left edge. A count of 0 is not
/// shown: its left edge is `right`.
pub fn count(ui: &Ui, theme: &Theme, count: u64, right: f32, centre_y: f32) -> f32 {
    let Some((size, text)) = lay(ui, theme, count) else { return right };
    let ppp = ui.ctx().pixels_per_point();
    let ppp = if ppp.is_finite() && ppp > 0.0 { ppp } else { 1.0 };
    let pixels = |at: f32| (at * ppp).round();
    let (end, top) = (pixels(right), pixels(centre_y - size.y / 2.0));
    let rect = Rect::from_min_max(pos2((end - pixels(size.x)) / ppp, top / ppp), pos2(end / ppp, (top + pixels(size.y)) / ppp));
    paint(ui, theme, rect, text, BadgeKind::Count);
    rect.left()
}

/// The ink of a group badge's icon in `theme`: the muted colour.
#[must_use]
pub fn group_ink(theme: &Theme) -> egui::Color32 {
    theme.palette.muted
}

/// A group's badge of `side` points: `icon` as an outline in the muted colour, without a background; a screen reader
/// reads `name`.
pub fn group(ui: &mut Ui, theme: &Theme, icon: IconId, name: &str, side: f32) -> Response {
    let side = if side.is_finite() { side.clamp(1.0, 512.0) } else { 1.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Image, true, name));
    if ui.is_rect_visible(rect) {
        crate::iconset::paint(ui.painter(), rect.shrink(side * 0.15), icon, group_ink(theme));
    }
    response
}

/// A chip: `name` in the caption font, `cyan_text` on a `cyan_muted` box; a screen reader reads `spoken`.
pub fn chip(ui: &mut Ui, theme: &Theme, name: &str, spoken: &str) -> Response {
    let s = &theme.spacing;
    let p = theme.palette;
    let ink = p.cyan_text;
    let text = galley(ui, name, theme.fonts.mono_s, ink);
    let size = vec2(text.size().x + s.s2 * 2.0, theme.boxes().chip);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, spoken));
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, 0.0, p.cyan_muted);
        ui.painter().galley(rect.center() - text.size() / 2.0, text, ink);
    }
    response
}

#[cfg(test)]
#[path = "badge_tests.rs"]
mod tests;
