// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Art drawn in code: the app icon. The icons inside the window come from an icon set, see [`crate::iconset`].
//!
//! The icon is a list of [`Part`]s on a square of [`SIDE`] units: a harbour teal tile, a letter standing in an inbox
//! tray and an unread dot. The same parts give the SVG the desktop shows ([`svg`], exported to [`EXPORTED`] by the
//! `app_icon` example) and the window icon ([`window_icon`]), so the two never drift apart; a test checks the
//! exported file against the code. Every part is convex, so egui fills it as it is.

use egui::epaint::{CornerRadiusF32, RectShape};
use egui::{Color32, IconData, Pos2, Rect, Shape, Vec2, pos2, vec2};

use crate::UiError;
use crate::shot::{self, Picture};
use crate::theme::Mode;
use crate::theme::app_icon::{DOT, INK, PAPER, TILE, TRAY};

/// The side of the square the parts are drawn on, in units.
pub const SIDE: f32 = 256.0;

/// Where the exported SVG lives, relative to the workspace root.
pub const EXPORTED: &str = "assets/app-icon/inboxcraft.svg";

/// The side of the window icon in pixels.
pub const WINDOW_ICON_SIDE: usize = 256;

/// One filled shape of an icon, in units of the [`SIDE`] square.
#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    /// A rectangle from `min` to `max`, each corner rounded by its own radius.
    Rect {
        /// The top left corner.
        min: [f32; 2],
        /// The bottom right corner.
        max: [f32; 2],
        /// The corner radii.
        corners: CornerRadiusF32,
        /// The colour.
        fill: Color32,
    },
    /// A convex polygon, its points clockwise on the screen.
    Polygon {
        /// The corners.
        points: Vec<[f32; 2]>,
        /// The colour.
        fill: Color32,
    },
    /// A circle.
    Circle {
        /// The centre.
        centre: [f32; 2],
        /// The radius.
        radius: f32,
        /// The colour.
        fill: Color32,
    },
}

const fn round(radius: f32) -> CornerRadiusF32 {
    CornerRadiusF32 { nw: radius, ne: radius, sw: radius, se: radius }
}

/// InboxCraft's icon: a letter in an inbox tray, with an unread dot, on a rounded teal tile.
#[must_use]
pub fn app_icon() -> Vec<Part> {
    vec![
        // The tile, with a margin so it sits among other icons at the same size.
        Part::Rect { min: [16.0, 16.0], max: [240.0, 240.0], corners: round(52.0), fill: TILE },
        // The letter and two lines of text on it.
        Part::Rect { min: [76.0, 48.0], max: [180.0, 150.0], corners: round(8.0), fill: PAPER },
        Part::Rect { min: [94.0, 72.0], max: [162.0, 84.0], corners: round(6.0), fill: INK },
        Part::Rect { min: [94.0, 96.0], max: [146.0, 108.0], corners: round(6.0), fill: INK },
        // The tray in front of it: two shoulders around the slot and the base below.
        Part::Polygon { points: vec![[44.0, 124.0], [100.0, 124.0], [114.0, 152.0], [44.0, 152.0]], fill: TRAY },
        Part::Polygon { points: vec![[156.0, 124.0], [212.0, 124.0], [212.0, 152.0], [142.0, 152.0]], fill: TRAY },
        Part::Rect { min: [44.0, 150.0], max: [212.0, 204.0], corners: CornerRadiusF32 { nw: 0.0, ne: 0.0, sw: 16.0, se: 16.0 }, fill: TRAY },
        // The unread dot, cut free from the letter by a ring of the tile colour.
        Part::Circle { centre: [180.0, 52.0], radius: 28.0, fill: TILE },
        Part::Circle { centre: [180.0, 52.0], radius: 20.0, fill: DOT },
    ]
}

/// `value` as an SVG number: no trailing `.0`, no exponent.
fn number(value: f32) -> String {
    let value = if value.is_finite() { value } else { 0.0 };
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".to_owned() } else { text.to_owned() }
}

fn hex(colour: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", colour.r(), colour.g(), colour.b())
}

/// `corners` with each radius between 0 and half the shorter side of a `width` by `height` rectangle.
fn clamp(corners: CornerRadiusF32, width: f32, height: f32) -> CornerRadiusF32 {
    let most = (width.min(height) / 2.0).max(0.0);
    let one = |radius: f32| if radius.is_finite() { radius.clamp(0.0, most) } else { 0.0 };
    CornerRadiusF32 { nw: one(corners.nw), ne: one(corners.ne), sw: one(corners.sw), se: one(corners.se) }
}

fn svg_rect(min: [f32; 2], max: [f32; 2], corners: CornerRadiusF32, fill: Color32) -> String {
    let [x0, y0] = min;
    let [x1, y1] = max;
    let c = clamp(corners, x1 - x0, y1 - y0);
    let fill = hex(fill);
    if c.nw == c.ne && c.ne == c.sw && c.sw == c.se {
        let rx = if c.nw > 0.0 { format!(" rx=\"{}\"", number(c.nw)) } else { String::new() };
        return format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rx} fill=\"{fill}\"/>",
            number(x0),
            number(y0),
            number(x1 - x0),
            number(y1 - y0)
        );
    }
    let n = number;
    let arc = |r: f32, x: f32, y: f32| if r > 0.0 { format!(" A{} {} 0 0 1 {} {}", n(r), n(r), n(x), n(y)) } else { String::new() };
    format!(
        "<path d=\"M{} {} H{}{} V{}{} H{}{} V{}{} Z\" fill=\"{fill}\"/>",
        n(x0 + c.nw),
        n(y0),
        n(x1 - c.ne),
        arc(c.ne, x1, y0 + c.ne),
        n(y1 - c.se),
        arc(c.se, x1 - c.se, y1),
        n(x0 + c.sw),
        arc(c.sw, x0, y1 - c.sw),
        n(y0 + c.nw),
        arc(c.nw, x0 + c.nw, y0),
    )
}

/// `parts` as an SVG document of [`SIDE`] by [`SIDE`] units.
#[must_use]
pub fn svg(parts: &[Part]) -> String {
    let side = number(SIDE);
    let mut out = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{side}\" height=\"{side}\" viewBox=\"0 0 {side} {side}\">\n");
    for part in parts {
        let line = match part {
            Part::Rect { min, max, corners, fill } => svg_rect(*min, *max, *corners, *fill),
            Part::Polygon { points, fill } => {
                let points: Vec<String> = points.iter().map(|[x, y]| format!("{},{}", number(*x), number(*y))).collect();
                format!("<polygon points=\"{}\" fill=\"{}\"/>", points.join(" "), hex(*fill))
            }
            Part::Circle { centre: [x, y], radius, fill } => {
                format!("<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>", number(*x), number(*y), number(radius.max(0.0)), hex(*fill))
            }
        };
        out.push_str("  ");
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str("</svg>\n");
    out
}

/// `parts` as egui shapes filling `rect` (the [`SIDE`] square scaled to its width and height).
#[must_use]
pub fn shapes(parts: &[Part], rect: Rect) -> Vec<Shape> {
    let scale = vec2(rect.width() / SIDE, rect.height() / SIDE);
    let at = |[x, y]: [f32; 2]| -> Pos2 { rect.min + Vec2::new(x * scale.x, y * scale.y) };
    let size = scale.x.min(scale.y);
    parts
        .iter()
        .map(|part| match part {
            Part::Rect { min, max, corners, fill } => {
                let shown = Rect::from_min_max(at(*min), at(*max));
                let c = clamp(*corners, max[0] - min[0], max[1] - min[1]);
                let c = CornerRadiusF32 { nw: c.nw * size, ne: c.ne * size, sw: c.sw * size, se: c.se * size };
                Shape::Rect(RectShape::filled(shown, c, *fill))
            }
            Part::Polygon { points, fill } => Shape::convex_polygon(points.iter().map(|point| at(*point)).collect(), *fill, egui::Stroke::NONE),
            Part::Circle { centre, radius, fill } => Shape::circle_filled(at(*centre), radius.max(0.0) * size, *fill),
        })
        .collect()
}

/// `parts` filled into a picture of `side` by `side` pixels on a transparent background.
///
/// # Errors
///
/// [`UiError::Shot`] for a side of nothing or over [`shot::MAX_SIDE`].
pub fn picture(parts: &[Part], side: usize) -> Result<Picture, UiError> {
    let points = side as f32;
    let shown = shapes(parts, Rect::from_min_size(pos2(0.0, 0.0), vec2(points, points)));
    shot::paint(vec2(points, points), 1.0, Mode::Light, |ui| {
        ui.painter().extend(shown.iter().cloned());
    })
}

/// The app icon for the window, [`WINDOW_ICON_SIDE`] pixels a side, with straight alpha as the window system wants.
///
/// # Errors
///
/// The errors of [`picture`]; none for the built-in side.
pub fn window_icon() -> Result<IconData, UiError> {
    let picture = picture(&app_icon(), WINDOW_ICON_SIDE)?;
    let mut rgba = Vec::with_capacity(picture.width().saturating_mul(picture.height()).saturating_mul(4));
    for y in 0..picture.height() {
        for x in 0..picture.width() {
            rgba.extend_from_slice(&picture.pixel(x, y).unwrap_or_default().to_srgba_unmultiplied());
        }
    }
    let side = |pixels: usize| u32::try_from(pixels).map_err(|_| UiError::Shot("the icon is too large".to_owned()));
    Ok(IconData { rgba, width: side(picture.width())?, height: side(picture.height())? })
}

#[cfg(test)]
#[path = "icons_tests.rs"]
mod tests;
