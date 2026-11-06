// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The one place that paints a gradient: on shapes ([`fill`]), on lines ([`line`]) and on text ([`text`]). Every
//! colour transition in the window goes through here, so its quality is decided once (a test refuses gradients
//! painted anywhere else).
//!
//! A [`Gradient`] says what colour each point of a shape has: along a line through stops, as soft colour fields
//! over a base (a gradient mesh), or as the soft shadow around a layer. Shapes get the gradient as a picture,
//! rasterised once per gradient and size and kept as a texture, with every pixel dithered: a noise below one step of
//! eight-bit colour, added before rounding, so a wide gradient over few colour steps shows no bands. Lines and text
//! are coloured per vertex of their mesh, dithered the same way.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use egui::epaint::{PathShape, PathStroke, RectShape, TextShape};
use egui::{Color32, ColorImage, CornerRadius, Galley, Painter, Pos2, Rect, Shape, TextureHandle, TextureId, TextureOptions, Vec2, pos2, vec2};

use crate::theme::NO_TINT;

/// The largest side of a gradient's picture in pixels; larger shapes stretch it.
pub const MAX_SIDE: usize = 2048;

/// Soft gradients are rasterised for sizes rounded up to this many pixels, so resizing the window does not
/// rasterise them again for every pixel of the drag.
const SOFT_STEP: usize = 128;

/// How many pictures are kept for reuse.
const KEEP: usize = 32;

/// The longest piece of a gradient line between two vertices, in points: a line is coloured at its vertices.
const LINE_STEP: f32 = 4.0;

/// The most vertices a gradient line gets.
const MAX_LINE_POINTS: usize = 4096;

/// A colour at a place along a [`Gradient::Linear`]: 0 is the start, 1 the end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stop {
    /// Where along the gradient, 0 to 1.
    pub at: f32,
    /// The colour there.
    pub colour: Color32,
}

impl Stop {
    /// `colour` at `at`.
    #[must_use]
    pub const fn new(at: f32, colour: Color32) -> Self {
        Self { at, colour }
    }
}

/// One soft colour field of a [`Gradient::Fields`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Field {
    /// Its centre, in the unit square of the shape (0, 0 top left, 1, 1 bottom right).
    pub centre: Pos2,
    /// Where it has faded out, as a part of the shape's longer side.
    pub radius: f32,
    /// Its colour at the centre; the alpha is how strongly it covers the base there.
    pub colour: Color32,
}

/// The colour of every point of a shape.
#[derive(Debug, Clone, PartialEq)]
pub enum Gradient {
    /// Along the line from `from` to `to` (in the unit square of the shape), through `stops` in order; before the
    /// first stop its colour, after the last stop its colour.
    Linear {
        /// Where the gradient starts.
        from: Pos2,
        /// Where it ends.
        to: Pos2,
        /// Its colours, in order of `at`.
        stops: Vec<Stop>,
    },
    /// Soft colour fields over an opaque base: a gradient mesh, the large colour behind the window.
    Fields {
        /// The colour under the fields.
        base: Color32,
        /// The fields, painted in order.
        fields: Vec<Field>,
    },
    /// The soft shadow of a layer: the shape is the layer grown by `blur` on every side (and by `drop` downwards);
    /// the shadow is `colour` at the layer's edge, fading out over `blur` points, falling `drop` points down, and
    /// clear under the layer itself so a translucent layer shows no shadow through it.
    Shadow {
        /// The colour at the layer's edge.
        colour: Color32,
        /// How far the shadow reaches, in points.
        blur: f32,
        /// How far it falls downwards, in points.
        drop: f32,
        /// The layer's corner radius, in points.
        corner: f32,
    },
}

/// `colour` as premultiplied channels, 0 to 1.
fn unit(colour: Color32) -> [f32; 4] {
    colour.to_array().map(|channel| f32::from(channel) / 255.0)
}

/// 0 for nonsense, otherwise `value` itself.
fn sane(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

/// Smooth from 0 at `t` = 0 to 1 at `t` = 1.
fn smoothstep(t: f32) -> f32 {
    let t = sane(t).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The signed distance from `at` to a rounded rectangle from `min` to `max`: negative inside.
fn rounded_distance(at: Pos2, min: Pos2, max: Pos2, corner: f32) -> f32 {
    let centre = pos2((min.x + max.x) / 2.0, (min.y + max.y) / 2.0);
    let half = vec2((max.x - min.x).max(0.0) / 2.0, (max.y - min.y).max(0.0) / 2.0);
    let corner = sane(corner).clamp(0.0, half.x.min(half.y));
    let q = vec2((at.x - centre.x).abs() - half.x + corner, (at.y - centre.y).abs() - half.y + corner);
    vec2(q.x.max(0.0), q.y.max(0.0)).length() + q.x.max(q.y).min(0.0) - corner
}

impl Gradient {
    /// The premultiplied colour (0 to 1) at `at`, in points from the top left of a shape of `size` points.
    #[must_use]
    pub fn colour_at(&self, at: Pos2, size: Vec2) -> [f32; 4] {
        let unit_at = |value: f32, side: f32| if side > 0.0 { sane(value / side) } else { 0.0 };
        let local = pos2(unit_at(at.x, size.x), unit_at(at.y, size.y));
        match self {
            Self::Linear { from, to, stops } => {
                let along = *to - *from;
                let length = along.dot(along);
                let t = if length > f32::EPSILON { sane((local - *from).dot(along) / length) } else { 0.0 };
                linear(stops, t)
            }
            Self::Fields { base, fields } => {
                let mut out = unit(*base);
                let longer = size.x.max(size.y);
                for field in fields {
                    let reach = field.radius * longer;
                    let distance = (at - pos2(field.centre.x * size.x, field.centre.y * size.y)).length();
                    let x = if reach > 0.0 { sane(distance / reach) } else { 1.0 };
                    let fall = if x < 1.0 { (1.0 - x * x) * (1.0 - x * x) } else { 0.0 };
                    let [r, g, b, a] = field.colour.to_srgba_unmultiplied().map(|channel| f32::from(channel) / 255.0);
                    let cover = a * fall;
                    out = [
                        r * cover + out[0] * (1.0 - cover),
                        g * cover + out[1] * (1.0 - cover),
                        b * cover + out[2] * (1.0 - cover),
                        cover + out[3] * (1.0 - cover),
                    ];
                }
                out
            }
            Self::Shadow { colour, blur, drop, corner } => {
                let (blur, drop) = (sane(*blur).max(0.0), sane(*drop).max(0.0));
                let layer = (pos2(blur, blur), pos2(size.x - blur, size.y - blur - drop));
                if rounded_distance(at, layer.0, layer.1, *corner) <= 0.0 {
                    return [0.0; 4];
                }
                let outside = rounded_distance(at, pos2(blur, blur + drop), pos2(size.x - blur, size.y - blur), *corner).max(0.0);
                let strength = if blur > 0.0 {
                    1.0 - smoothstep(outside / blur)
                } else if outside > 0.0 {
                    0.0
                } else {
                    1.0
                };
                unit(*colour).map(|channel| channel * strength)
            }
        }
    }

    /// Whether the gradient is soft enough to be stretched a little or cut at an edge without anyone seeing it.
    fn soft(&self) -> bool {
        matches!(self, Self::Fields { .. })
    }

    /// A key for the picture of this gradient at `size` points in `pixels`.
    fn key(&self, size: Vec2, pixels: [usize; 2]) -> u64 {
        let mut hasher = DefaultHasher::new();
        let mut float = |value: f32| value.to_bits().hash(&mut hasher);
        float(size.x);
        float(size.y);
        match self {
            Self::Linear { from, to, stops } => {
                for value in [from.x, from.y, to.x, to.y] {
                    float(value);
                }
                for stop in stops {
                    float(stop.at);
                }
                0u8.hash(&mut hasher);
                stops.iter().for_each(|stop| stop.colour.hash(&mut hasher));
            }
            Self::Fields { base, fields } => {
                for field in fields {
                    for value in [field.centre.x, field.centre.y, field.radius] {
                        float(value);
                    }
                }
                1u8.hash(&mut hasher);
                base.hash(&mut hasher);
                fields.iter().for_each(|field| field.colour.hash(&mut hasher));
            }
            Self::Shadow { colour, blur, drop, corner } => {
                for value in [*blur, *drop, *corner] {
                    float(value);
                }
                2u8.hash(&mut hasher);
                colour.hash(&mut hasher);
            }
        }
        pixels.hash(&mut hasher);
        hasher.finish()
    }

    /// The colour at `at` (in points from the top left of a shape of `size`) for a vertex at `pixel` on screen,
    /// dithered.
    fn vertex(&self, at: Pos2, size: Vec2, pixel: Pos2) -> Color32 {
        let index = |value: f32| if value.is_finite() && value > 0.0 { value as u32 } else { 0 };
        quantise(self.colour_at(at, size), index(pixel.x), index(pixel.y), true)
    }
}

/// The colour of `stops` at `t`, interpolated between the two around it.
fn linear(stops: &[Stop], t: f32) -> [f32; 4] {
    let (Some(first), Some(last)) = (stops.first(), stops.last()) else { return [0.0; 4] };
    if t <= first.at {
        return unit(first.colour);
    }
    if t >= last.at {
        return unit(last.colour);
    }
    for pair in stops.windows(2) {
        if let [a, b] = pair
            && t <= b.at
        {
            let span = b.at - a.at;
            let f = if span > f32::EPSILON { (t - a.at) / span } else { 1.0 };
            let (ca, cb) = (unit(a.colour), unit(b.colour));
            return std::array::from_fn(|i| ca[i] + (cb[i] - ca[i]) * f);
        }
    }
    unit(last.colour)
}

/// A threshold between 0 and 1 for the pixel at `x`, `y`: interleaved gradient noise, which spreads the thresholds
/// evenly along rows, columns and small blocks.
fn threshold(x: u32, y: u32) -> f32 {
    // Only the fractions matter; the casts of large coordinates lose nothing that counts.
    let (x, y) = ((x % 4096) as f32, (y % 4096) as f32);
    let inner = (0.067_110_56 * x + 0.005_837_15 * y).fract();
    (52.982_918 * inner).fract()
}

/// `value` (0 to 1) as an eight-bit channel, dithered at the pixel `x`, `y`: rounding with a threshold that moves
/// from pixel to pixel, so the mean of a few pixels is the value itself rather than the nearest step.
#[must_use]
pub fn dither(value: f32, x: u32, y: u32) -> u8 {
    let scaled = sane(value).clamp(0.0, 1.0) * 255.0;
    // Between 0 and 255 after the clamp, so the cast loses nothing.
    (scaled + threshold(x, y)).floor().clamp(0.0, 255.0) as u8
}

/// `colour` (premultiplied, 0 to 1) as a [`Color32`] at the pixel `x`, `y`, dithered or rounded.
fn quantise(colour: [f32; 4], x: u32, y: u32, dithered: bool) -> Color32 {
    let channel = |value: f32| {
        if dithered {
            dither(value, x, y)
        } else {
            // Between 0 and 255 after the clamp.
            (sane(value).clamp(0.0, 1.0) * 255.0).round() as u8
        }
    };
    let [r, g, b, a] = colour.map(channel);
    // Premultiplied: no channel above the alpha.
    Color32::from_rgba_premultiplied(r.min(a), g.min(a), b.min(a), a)
}

/// The picture of `gradient` on a shape of `size` points, `pixels` wide and high (each side 1 to [`MAX_SIDE`]),
/// dithered or not.
pub(crate) fn raster(gradient: &Gradient, size: Vec2, pixels: [usize; 2], dithered: bool) -> ColorImage {
    let [width, height] = pixels.map(|side| side.clamp(1, MAX_SIDE));
    let step = vec2(size.x / width as f32, size.y / height as f32);
    let mut out = Vec::with_capacity(width.saturating_mul(height));
    for y in 0..height {
        for x in 0..width {
            let at = pos2((x as f32 + 0.5) * step.x, (y as f32 + 0.5) * step.y);
            // At most MAX_SIDE, so the casts lose nothing.
            out.push(quantise(gradient.colour_at(at, size), x as u32, y as u32, dithered));
        }
    }
    ColorImage::new([width, height], out)
}

/// The dithered picture of `gradient` on a shape of `size` points, `pixels` wide and high (each side 1 to
/// [`MAX_SIDE`]).
#[must_use]
pub fn image(gradient: &Gradient, size: Vec2, pixels: [usize; 2]) -> ColorImage {
    raster(gradient, size, pixels, true)
}

/// The pictures kept for reuse: key, texture, the pass it was last used in.
#[derive(Clone, Default)]
struct Cache {
    entries: Vec<(u64, TextureHandle, u64)>,
}

fn cache_id() -> egui::Id {
    egui::Id::unique("inboxcraft-gradients")
}

/// The texture of `gradient` on a shape of `size` points in `pixels`, rasterised when it is not kept yet.
fn texture(ctx: &egui::Context, gradient: &Gradient, size: Vec2, pixels: [usize; 2]) -> TextureId {
    let key = gradient.key(size, pixels);
    let pass = ctx.cumulative_pass_nr();
    let kept = ctx.data_mut(|data| {
        let cache = data.get_temp_mut_or_default::<Cache>(cache_id());
        cache.entries.iter_mut().find(|entry| entry.0 == key).map(|entry| {
            entry.2 = pass;
            entry.1.id()
        })
    });
    if let Some(id) = kept {
        return id;
    }
    let handle = ctx.load_texture("gradient", image(gradient, size, pixels), TextureOptions::NEAREST);
    let id = handle.id();
    ctx.data_mut(|data| {
        let cache = data.get_temp_mut_or_default::<Cache>(cache_id());
        cache.entries.push((key, handle, pass));
        // The oldest go first, never one this frame paints.
        while cache.entries.len() > KEEP {
            let oldest = cache.entries.iter().enumerate().filter(|(_, entry)| entry.2 < pass).min_by_key(|(_, entry)| entry.2).map(|(at, _)| at);
            let Some(at) = oldest else { break };
            drop(cache.entries.remove(at));
        }
    });
    id
}

/// How many pictures are kept now.
#[must_use]
pub fn kept(ctx: &egui::Context) -> usize {
    ctx.data_mut(|data| data.get_temp::<Cache>(cache_id()).map_or(0, |cache| cache.entries.len()))
}

/// The pixels of `points` at `scale`, at least 1 and at most [`MAX_SIDE`].
fn pixels_of(points: f32, scale: f32) -> usize {
    let pixels = (points * scale).round();
    // Clamped before the cast.
    if pixels.is_finite() { pixels.clamp(1.0, MAX_SIDE as f32) as usize } else { 1 }
}

/// Paints `gradient` over `rect`, with corners rounded by `corner`.
pub fn fill(painter: &Painter, rect: Rect, corner: impl Into<CornerRadius>, gradient: &Gradient) {
    painter.add(filled(painter, rect, corner, gradient));
}

/// The shape of `gradient` over `rect`, with corners rounded by `corner`, for a painter to add later (its picture
/// is rasterised now when it is not kept yet).
pub fn filled(painter: &Painter, rect: Rect, corner: impl Into<CornerRadius>, gradient: &Gradient) -> Shape {
    let scale = painter.pixels_per_point();
    let wanted = [pixels_of(rect.width(), scale), pixels_of(rect.height(), scale)];
    let (pixels, uv) = if gradient.soft() {
        let up = |side: usize| side.div_ceil(SOFT_STEP).saturating_mul(SOFT_STEP).clamp(1, MAX_SIDE);
        let pixels = wanted.map(up);
        (pixels, Rect::from_min_max(Pos2::ZERO, pos2(wanted[0] as f32 / pixels[0] as f32, wanted[1] as f32 / pixels[1] as f32)))
    } else {
        (wanted, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)))
    };
    let size = if gradient.soft() { vec2(pixels[0] as f32 / scale, pixels[1] as f32 / scale) } else { rect.size() };
    let id = texture(painter.ctx(), gradient, size, pixels);
    Shape::Rect(RectShape::filled(rect, corner, NO_TINT).with_texture(id, uv))
}

/// Paints a line through `points`, `width` points wide, coloured by `gradient` over the line's bounding box.
pub fn line(painter: &Painter, points: Vec<Pos2>, width: f32, gradient: &Gradient) {
    painter.add(stroked(painter, points, width, gradient));
}

/// The shape of a line through `points`, `width` points wide, coloured by `gradient` over the line's bounding box,
/// for a painter to add later.
pub fn stroked(painter: &Painter, points: Vec<Pos2>, width: f32, gradient: &Gradient) -> Shape {
    let points = subdivided(&points);
    let gradient = gradient.clone();
    let scale = painter.pixels_per_point();
    let stroke =
        PathStroke::new_uv(width, move |bounds, at| gradient.vertex((at - bounds.min).to_pos2(), bounds.size(), (at.to_vec2() * scale).to_pos2()));
    Shape::Path(PathShape::line(points, stroke))
}

/// `points` with more in between, so no piece is longer than [`LINE_STEP`] (up to [`MAX_LINE_POINTS`] in all).
fn subdivided(points: &[Pos2]) -> Vec<Pos2> {
    let mut out: Vec<Pos2> = points.first().copied().into_iter().collect();
    for pair in points.windows(2) {
        let [a, b] = pair else { continue };
        let length = (*b - *a).length();
        let room = MAX_LINE_POINTS.saturating_sub(out.len()).max(1);
        // At least one piece, at most the room left; the clamp keeps the cast in range.
        let pieces = if length.is_finite() { (length / LINE_STEP).ceil().clamp(1.0, room as f32) as usize } else { 1 };
        out.extend((1..=pieces).map(|piece| a.lerp(*b, piece as f32 / pieces as f32)));
    }
    out
}

/// Paints `galley` at `pos`, its glyphs coloured by `gradient` over the galley's box.
pub fn text(painter: &Painter, pos: Pos2, galley: &Arc<Galley>, gradient: &Gradient) {
    let scale = painter.pixels_per_point();
    let size = galley.size();
    let mut coloured = Galley::clone(galley);
    for placed in &mut coloured.rows {
        let offset = placed.pos.to_vec2();
        let row = Arc::make_mut(&mut placed.row);
        let glyphs = row.visuals.glyph_vertex_range.clone();
        for (index, vertex) in row.visuals.mesh.vertices.iter_mut().enumerate() {
            if glyphs.contains(&index) {
                let at = vertex.pos + offset;
                vertex.color = gradient.vertex(at, size, ((pos + at.to_vec2()).to_vec2() * scale).to_pos2());
            }
        }
    }
    painter.add(TextShape::new(pos, Arc::new(coloured), NO_TINT));
}

#[cfg(test)]
#[path = "gradient_tests.rs"]
mod tests;
