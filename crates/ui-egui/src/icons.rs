// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Art drawn in code: the app icon. The icons inside the window come from an icon set, see [`crate::iconset`].
//!
//! The icon is the brand's mark (`plan/design-gen/brand`, decided in #104): two stepped cards entering a notched tray,
//! in the Electric gradient, on a white rounded tile with a faint edge. It is a list of [`Part`]s on a square of
//! [`SIDE`] units, with the master's coordinates. The same parts give the SVG the desktop shows ([`svg`], exported to
//! [`EXPORTED`] by the `app_icon` example) and the window icon ([`window_icon`]), so the two never drift apart; a test
//! checks the exported file against the code. Every part is convex, so egui fills it as it is.

use egui::epaint::{CornerRadiusF32, RectShape};
use egui::{Color32, IconData, Pos2, Rect, Shape, Vec2, pos2, vec2};

use crate::UiError;
use crate::gradient;
use crate::shot::{self, Picture};
use crate::theme::Mode;
use crate::theme::app_icon::{EDGE, GRADIENT, GRADIENT_LINE, TILE};

/// The side of the square the parts are drawn on, in units.
pub const SIDE: f32 = 256.0;

/// Where the exported SVG lives, relative to the workspace root.
pub const EXPORTED: &str = "assets/app-icon/inboxcraft.svg";

/// The side of the window icon in pixels.
pub const WINDOW_ICON_SIDE: usize = 256;

/// What a part is filled with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fill {
    /// One colour.
    Solid(Color32),
    /// The Electric gradient along [`GRADIENT_LINE`].
    Gradient,
}

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
        /// The fill.
        fill: Fill,
    },
    /// A convex polygon, its points clockwise on the screen.
    Polygon {
        /// The corners.
        points: Vec<[f32; 2]>,
        /// The fill.
        fill: Fill,
    },
}

const fn round(radius: f32) -> CornerRadiusF32 {
    CornerRadiusF32 { nw: radius, ne: radius, sw: radius, se: radius }
}

const fn top(radius: f32) -> CornerRadiusF32 {
    CornerRadiusF32 { nw: radius, ne: radius, sw: 0.0, se: 0.0 }
}

/// InboxCraft's icon: two cards entering a notched tray, in the Electric gradient, on a white tile.
#[must_use]
pub fn app_icon() -> Vec<Part> {
    vec![
        // The tile's edge, then the tile: the master's 2-unit edge around the tile from 8 to 248.
        Part::Rect { min: [7.0, 7.0], max: [249.0, 249.0], corners: round(61.0), fill: Fill::Solid(EDGE) },
        Part::Rect { min: [9.0, 9.0], max: [247.0, 247.0], corners: round(59.0), fill: Fill::Solid(TILE) },
        // The two cards, the upper one narrower.
        Part::Rect { min: [71.0, 53.0], max: [185.0, 77.0], corners: top(8.0), fill: Fill::Gradient },
        Part::Rect { min: [51.0, 94.0], max: [205.0, 118.0], corners: top(8.0), fill: Fill::Gradient },
        // The tray: two shoulders around the notch, reaching a unit into the base so no seam shows, and the base.
        Part::Polygon { points: vec![[41.0, 135.0], [95.0, 135.0], [111.7, 159.0], [41.0, 159.0]], fill: Fill::Gradient },
        Part::Polygon { points: vec![[161.0, 135.0], [215.0, 135.0], [215.0, 159.0], [144.3, 159.0]], fill: Fill::Gradient },
        Part::Rect {
            min: [41.0, 158.0],
            max: [215.0, 201.0],
            corners: CornerRadiusF32 { nw: 0.0, ne: 0.0, sw: 16.0, se: 16.0 },
            fill: Fill::Gradient,
        },
    ]
}

/// The gradient's colour at `point` of the [`SIDE`] square.
#[must_use]
pub fn gradient_at(point: [f32; 2]) -> Color32 {
    let [[x0, y0], [x1, y1]] = GRADIENT_LINE;
    let (dx, dy) = (x1 - x0, y1 - y0);
    let along = ((point[0] - x0) * dx + (point[1] - y0) * dy) / (dx * dx + dy * dy);
    let t = if along.is_finite() { along.clamp(0.0, 1.0) } else { 0.0 };
    let [first, middle, last] = GRADIENT;
    if t < 0.5 { first.lerp_to_gamma(middle, t * 2.0) } else { middle.lerp_to_gamma(last, (t - 0.5) * 2.0) }
}

/// `value` as an SVG number: no trailing `.0`, no exponent.
fn number(value: f32) -> String {
    let value = if value.is_finite() { value } else { 0.0 };
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" { "0".to_owned() } else { text.to_owned() }
}

fn hex(colour: Color32) -> String {
    let [r, g, b, _] = colour.to_srgba_unmultiplied();
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// The id of the gradient in the SVG.
const GRADIENT_ID: &str = "electric";

/// The `fill` attribute (with its opacity where the colour lets some through) of `fill`.
fn fill_attribute(fill: Fill) -> String {
    match fill {
        Fill::Gradient => format!("fill=\"url(#{GRADIENT_ID})\""),
        Fill::Solid(colour) if colour.a() == u8::MAX => format!("fill=\"{}\"", hex(colour)),
        Fill::Solid(colour) => format!("fill=\"{}\" fill-opacity=\"{}\"", hex(colour), number(f32::from(colour.a()) / 255.0)),
    }
}

/// `corners` with each radius between 0 and half the shorter side of a `width` by `height` rectangle.
fn clamp(corners: CornerRadiusF32, width: f32, height: f32) -> CornerRadiusF32 {
    let most = (width.min(height) / 2.0).max(0.0);
    let one = |radius: f32| if radius.is_finite() { radius.clamp(0.0, most) } else { 0.0 };
    CornerRadiusF32 { nw: one(corners.nw), ne: one(corners.ne), sw: one(corners.sw), se: one(corners.se) }
}

fn svg_rect(min: [f32; 2], max: [f32; 2], corners: CornerRadiusF32, fill: Fill) -> String {
    let [x0, y0] = min;
    let [x1, y1] = max;
    let c = clamp(corners, x1 - x0, y1 - y0);
    let fill = fill_attribute(fill);
    if c.nw == c.ne && c.ne == c.sw && c.sw == c.se {
        let rx = if c.nw > 0.0 { format!(" rx=\"{}\"", number(c.nw)) } else { String::new() };
        return format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rx} {fill}/>", number(x0), number(y0), number(x1 - x0), number(y1 - y0));
    }
    let n = number;
    let arc = |r: f32, x: f32, y: f32| if r > 0.0 { format!(" A{} {} 0 0 1 {} {}", n(r), n(r), n(x), n(y)) } else { String::new() };
    format!(
        "<path d=\"M{} {} H{}{} V{}{} H{}{} V{}{} Z\" {fill}/>",
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

/// The gradient's definition, on one line.
fn svg_gradient() -> String {
    let [[x0, y0], [x1, y1]] = GRADIENT_LINE;
    let [first, middle, last] = GRADIENT;
    format!(
        "<defs><linearGradient id=\"{GRADIENT_ID}\" gradientUnits=\"userSpaceOnUse\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"><stop stop-color=\"{}\"/><stop offset=\"0.5\" stop-color=\"{}\"/><stop offset=\"1\" stop-color=\"{}\"/></linearGradient></defs>",
        number(x0),
        number(y0),
        number(x1),
        number(y1),
        hex(first),
        hex(middle),
        hex(last)
    )
}

/// `parts` as an SVG document of [`SIDE`] by [`SIDE`] units; the gradient's definition first when a part uses it.
#[must_use]
pub fn svg(parts: &[Part]) -> String {
    let side = number(SIDE);
    let mut out = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{side}\" height=\"{side}\" viewBox=\"0 0 {side} {side}\">\n");
    let graded = parts.iter().any(|part| matches!(part, Part::Rect { fill: Fill::Gradient, .. } | Part::Polygon { fill: Fill::Gradient, .. }));
    if graded {
        out.push_str("  ");
        out.push_str(&svg_gradient());
        out.push('\n');
    }
    for part in parts {
        let line = match part {
            Part::Rect { min, max, corners, fill } => svg_rect(*min, *max, *corners, *fill),
            Part::Polygon { points, fill } => {
                let points: Vec<String> = points.iter().map(|[x, y]| format!("{},{}", number(*x), number(*y))).collect();
                format!("<polygon points=\"{}\" {}/>", points.join(" "), fill_attribute(*fill))
            }
        };
        out.push_str("  ");
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str("</svg>\n");
    out
}

/// `parts` as egui shapes filling `rect` (the [`SIDE`] square scaled to its width and height); a gradient part is a
/// mesh.
#[must_use]
pub fn shapes(parts: &[Part], rect: Rect) -> Vec<Shape> {
    let scale = vec2(rect.width() / SIDE, rect.height() / SIDE);
    let at = |[x, y]: [f32; 2]| -> Pos2 { rect.min + Vec2::new(x * scale.x, y * scale.y) };
    let size = scale.x.min(scale.y);
    let solid = |fill: Fill| match fill {
        Fill::Solid(colour) => colour,
        // Any opaque colour: the mesh takes the gradient's colours, and only the coverage stays.
        Fill::Gradient => TILE,
    };
    parts
        .iter()
        .map(|part| {
            let (shape, fill) = match part {
                Part::Rect { min, max, corners, fill } => {
                    let shown = Rect::from_min_max(at(*min), at(*max));
                    let c = clamp(*corners, max[0] - min[0], max[1] - min[1]);
                    let c = CornerRadiusF32 { nw: c.nw * size, ne: c.ne * size, sw: c.sw * size, se: c.se * size };
                    (Shape::Rect(RectShape::filled(shown, c, solid(*fill))), *fill)
                }
                Part::Polygon { points, fill } => {
                    (Shape::convex_polygon(points.iter().map(|point| at(*point)).collect(), solid(*fill), egui::Stroke::NONE), *fill)
                }
            };
            match fill {
                Fill::Solid(_) => shape,
                Fill::Gradient => gradient::recoloured(shape, |pos| {
                    gradient_at([(pos.x - rect.min.x) / scale.x.max(f32::EPSILON), (pos.y - rect.min.y) / scale.y.max(f32::EPSILON)])
                }),
            }
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
