// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The brand's marks as the window draws them: the compact mark of the command line, the Qubernetic and the
//! InboxCraft wordmarks under Home's emblem, and the emblem itself (a grid of characters for the character field).
//! The marks are the brand's files under `assets/brand/` (CC BY-ND 4.0, see `ATTRIBUTION.md`), built into the binary
//! and drawn unchanged: read from their vector paths and filled pixel for pixel at the window's scale, in the brand's
//! own colours (light on the dark look, dark on the light one).
//!
//! The reader knows what the brand's files hold: `<path d=… fill=…>` and `<polygon points=… fill=…>`, in `<g>`
//! groups that may give the fill and a transform of `translate` and `scale`, and path data of M, L, H, V, C, Q and Z
//! (each also relative). Anything else makes the file unreadable, and nothing is drawn: the files are built in, so
//! that is a fault of the build, never a crash.

use std::sync::LazyLock;

use egui::{Color32, ColorImage, Context, Id, Pos2, Rect, TextureHandle, TextureOptions, Ui, Vec2, pos2, vec2};

use crate::field::Emblem;
use crate::theme::Mode;

/// The most paths (and polygons) a file is read for: the brand's have a few to a dozen.
const MOST_PATHS: usize = 32;

/// The most points all of a file's outlines hold, its curves cut into lines: the brand's have a few thousand.
const MOST_POINTS: usize = 16_384;

/// The deepest `<g>` groups go in a file.
const MOST_DEPTH: usize = 16;

/// The lines a curve is cut into, and the samples a pixel is measured at along each side.
const CURVE_STEPS: u16 = 12;
const SAMPLES: u16 = 4;

/// The parts of a pixel a picture is shifted by: a placement is rounded to an eighth of a pixel, so at most 64
/// pictures of one mark are kept for one scale.
const SHIFTS: f32 = 8.0;

/// The pixels a point a picture is drawn at, at least and at most.
const PPP: std::ops::RangeInclusive<f32> = 0.25..=8.0;

/// The most pixels a side of a picture has.
const MOST_PIXELS: f32 = 1024.0;

/// One of the brand's marks the window draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Mark {
    /// The InboxCraft compact mark, 22 points a side (the frames' `InboxCraft mark-compact · 22`).
    Compact,
    /// The Qubernetic wordmark, 88 × 14 points (the frames' `qubernetic wordmark`).
    Qubernetic,
    /// The InboxCraft wordmark, 102.17 × 13 points (the frames' `inboxcraft wordmark`).
    InboxCraft,
}

impl Mark {
    /// Its box, in points.
    pub(crate) const fn size(self) -> Vec2 {
        match self {
            Self::Compact => vec2(22.0, 22.0),
            Self::Qubernetic => vec2(88.0, 14.0),
            Self::InboxCraft => vec2(102.17, 13.0),
        }
    }

    /// The brand's file for `mode`: light ink on the dark look, dark ink on the light one.
    pub(crate) const fn file(self, mode: Mode) -> &'static str {
        let dark = matches!(mode, Mode::Dark);
        match self {
            Self::Compact if dark => include_str!("../../../assets/brand/inboxcraft/svg/mark-compact-dark.svg"),
            Self::Compact => include_str!("../../../assets/brand/inboxcraft/svg/mark-compact-light.svg"),
            Self::Qubernetic if dark => include_str!("../../../assets/brand/qubernetic/wordmark-dark.svg"),
            Self::Qubernetic => include_str!("../../../assets/brand/qubernetic/wordmark-light.svg"),
            Self::InboxCraft if dark => include_str!("../../../assets/brand/inboxcraft/svg/wordmark-dark.svg"),
            Self::InboxCraft => include_str!("../../../assets/brand/inboxcraft/svg/wordmark-light.svg"),
        }
    }

    /// How the file is laid into its box: the compact mark by its frame's bounds (the frames place the frame at 0, 2.62,
    /// 21.88 × 16.82 of the 22 point box), the wordmarks by their view box, which they fill.
    const fn fit(self) -> Fit {
        match self {
            Self::Compact => Fit::First(Rect { min: pos2(0.0, 2.62), max: pos2(21.88, 19.44) }),
            Self::Qubernetic | Self::InboxCraft => Fit::ViewBox,
        }
    }

    /// The mark for `mode` read into its box; none when the built-in file does not read.
    pub(crate) fn picture(self, mode: Mode) -> Option<Picture> {
        read(self.file(mode), self.size(), self.fit())
    }
}

/// How a file's shapes are laid into a box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Fit {
    /// The first shape's bounds become this rect of the box (each way on its own), the rest follow.
    First(Rect),
    /// The file's view box fills the box (each way on its own).
    ViewBox,
}

/// A filled shape: its closed outlines, in the box's points, their bounds and its colour.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Shape {
    pub(crate) outlines: Vec<Vec<Pos2>>,
    pub(crate) bounds: Rect,
    pub(crate) colour: Color32,
}

/// A mark read into its box: its shapes in paint order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Picture {
    pub(crate) size: Vec2,
    pub(crate) shapes: Vec<Shape>,
}

/// A transform of `translate` and `scale`: a point goes to `scale × point + offset`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Affine {
    scale: Vec2,
    offset: Vec2,
}

impl Affine {
    const NONE: Self = Self { scale: vec2(1.0, 1.0), offset: Vec2::ZERO };

    fn apply(self, point: Pos2) -> Pos2 {
        pos2(self.scale.x * point.x + self.offset.x, self.scale.y * point.y + self.offset.y)
    }

    /// `self` after `inner`: the point goes through `inner` first.
    fn then(self, inner: Self) -> Self {
        Self { scale: self.scale * inner.scale, offset: self.scale * inner.offset + self.offset }
    }
}

/// `file` read and laid into a box of `size` by `fit`; none when the file does not read as the brand's files do, holds
/// too much, or has nothing to fill.
pub(crate) fn read(file: &str, size: Vec2, fit: Fit) -> Option<Picture> {
    let mut shapes = raw_shapes(file)?;
    let from = match fit {
        Fit::First(_) => bounds(&shapes.first()?.outlines),
        Fit::ViewBox => view_box(file)?,
    };
    let into = match fit {
        Fit::First(rect) => rect,
        Fit::ViewBox => Rect::from_min_size(Pos2::ZERO, size),
    };
    if !(from.width() > 0.0 && from.height() > 0.0 && into.width() > 0.0 && into.height() > 0.0) {
        return None;
    }
    let scale = vec2(into.width() / from.width(), into.height() / from.height());
    for shape in &mut shapes {
        for point in shape.outlines.iter_mut().flatten() {
            *point = into.min + (*point - from.min) * scale;
        }
        shape.bounds = bounds(&shape.outlines);
    }
    let finite = shapes.iter().flat_map(|shape| shape.outlines.iter().flatten()).all(|point| point.x.is_finite() && point.y.is_finite());
    finite.then_some(Picture { size, shapes })
}

/// The bounds of `outlines`.
fn bounds(outlines: &[Vec<Pos2>]) -> Rect {
    outlines.iter().flatten().fold(Rect::NOTHING, |all, point| all.union(Rect::from_min_max(*point, *point)))
}

/// The file's view box (`viewBox="x y width height"` of its `<svg>`).
fn view_box(file: &str) -> Option<Rect> {
    let svg = file.split('<').find(|tag| tag.starts_with("svg"))?;
    let numbers = numbers(attribute(svg, "viewBox")?)?;
    match numbers.as_slice() {
        [x, y, width, height] => Some(Rect::from_min_size(pos2(*x, *y), vec2(*width, *height))),
        _ => None,
    }
}

/// The file's shapes in its own units, in paint order: each `<path>` and `<polygon>`, its fill its own or its group's,
/// through its own transform and its groups'.
fn raw_shapes(file: &str) -> Option<Vec<Shape>> {
    // What each open group gives: its transform (already after its parents') and its fill.
    let mut groups: Vec<(Affine, Option<&str>)> = Vec::new();
    let mut shapes = Vec::new();
    let mut points = 0_usize;
    for tag in file.split('<').skip(1) {
        let tag = tag.split('>').next().unwrap_or_default();
        let (outer, inherited) = groups.last().copied().unwrap_or((Affine::NONE, None));
        if tag.starts_with("/g") {
            groups.pop();
        } else if tag.starts_with("g ") || tag == "g" || tag.starts_with("g/") {
            if tag.ends_with('/') {
                continue;
            }
            if groups.len() >= MOST_DEPTH {
                return None;
            }
            let own = attribute(tag, "transform").map_or(Some(Affine::NONE), transform)?;
            groups.push((outer.then(own), attribute(tag, "fill").or(inherited)));
        } else if tag.starts_with("path ") || tag.starts_with("polygon ") {
            if shapes.len() >= MOST_PATHS {
                return None;
            }
            let own = attribute(tag, "transform").map_or(Some(Affine::NONE), transform)?;
            let whole = outer.then(own);
            let colour = fill(attribute(tag, "fill").or(inherited)?)?;
            let mut outlines = if tag.starts_with("path ") { outlines(attribute(tag, "d")?)? } else { polygon(attribute(tag, "points")?)? };
            points = points.saturating_add(outlines.iter().map(Vec::len).sum());
            if points > MOST_POINTS {
                return None;
            }
            for point in outlines.iter_mut().flatten() {
                *point = whole.apply(*point);
            }
            shapes.push(Shape { bounds: bounds(&outlines), outlines, colour });
        }
    }
    (!shapes.is_empty()).then_some(shapes)
}

/// A transform list of `translate(x y)`, `translate(x)`, `scale(x y)` and `scale(s)`, applied right to left as SVG
/// does; none for anything else.
fn transform(list: &str) -> Option<Affine> {
    let mut whole = Affine::NONE;
    let mut rest = list.trim();
    while !rest.is_empty() {
        let open = rest.find('(')?;
        let close = rest.find(')')?;
        let name = rest.get(..open)?.trim().trim_start_matches(',').trim();
        let values = numbers(rest.get(open.checked_add(1)?..close)?)?;
        let step = match (name, values.as_slice()) {
            ("translate", [x]) => Affine { scale: vec2(1.0, 1.0), offset: vec2(*x, 0.0) },
            ("translate", [x, y]) => Affine { scale: vec2(1.0, 1.0), offset: vec2(*x, *y) },
            ("scale", [s]) => Affine { scale: Vec2::splat(*s), offset: Vec2::ZERO },
            ("scale", [x, y]) => Affine { scale: vec2(*x, *y), offset: Vec2::ZERO },
            _ => return None,
        };
        whole = whole.then(step);
        rest = rest.get(close.checked_add(1)?..)?.trim();
    }
    Some(whole)
}

/// The colour of a `fill`, `#rrggbb` (either case): the brand's own, not a token. `none` and the rest do not read.
fn fill(hex: &str) -> Option<Color32> {
    let hex = hex.strip_prefix('#').filter(|hex| hex.len() == 6 && hex.is_ascii())?;
    let byte = |at: usize| u8::from_str_radix(hex.get(at..at.checked_add(2)?)?, 16).ok();
    Some(Color32::from_rgb(byte(0)?, byte(2)?, byte(4)?))
}

/// The value of `name="…"` in the tag `tag`.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let found = tag.match_indices(&key).next()?.0;
    let rest = tag.get(found.checked_add(key.len())?..)?;
    rest.get(..rest.find('"')?)
}

/// A word of path data: a command's letter or a number.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Word {
    Command(char),
    Number(f32),
}

/// The words of path data: letters, and numbers that may follow a letter or a sign with no room between
/// (`M386 1303H126`, `1-2`, `.5.5`); none when a number does not read or is not finite.
fn words(d: &str) -> Option<Vec<Word>> {
    let mut words = Vec::new();
    let mut chars = d.char_indices().peekable();
    while let Some(&(at, ch)) = chars.peek() {
        if ch.is_ascii_whitespace() || ch == ',' {
            chars.next();
        } else if ch.is_ascii_alphabetic() {
            words.push(Word::Command(ch));
            chars.next();
        } else if ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.') {
            let mut end = at;
            let (mut dot, mut exponent, mut first) = (false, false, true);
            while let Some(&(here, ch)) = chars.peek() {
                let sign_ok = first || d.get(..here).is_some_and(|before| before.ends_with(['e', 'E']));
                let takes = match ch {
                    '0'..='9' => true,
                    '-' | '+' => sign_ok,
                    '.' => !dot && !exponent,
                    'e' | 'E' => !exponent && !first,
                    _ => false,
                };
                if !takes {
                    break;
                }
                dot |= ch == '.';
                exponent |= matches!(ch, 'e' | 'E');
                first = false;
                end = here.checked_add(ch.len_utf8())?;
                chars.next();
            }
            words.push(Word::Number(d.get(at..end)?.parse::<f32>().ok().filter(|value| value.is_finite())?));
        } else {
            return None;
        }
    }
    Some(words)
}

/// The numbers of a list (`0 0 750.84 95.54`, `1,2 3,4`).
fn numbers(list: &str) -> Option<Vec<f32>> {
    words(list)?.into_iter().map(|word| if let Word::Number(value) = word { Some(value) } else { None }).collect()
}

/// The one closed outline of a `<polygon>`'s `points`.
fn polygon(list: &str) -> Option<Vec<Vec<Pos2>>> {
    let values = numbers(list)?;
    if values.len() % 2 != 0 {
        return None;
    }
    let outline: Vec<Pos2> = values.as_chunks::<2>().0.iter().map(|[x, y]| pos2(*x, *y)).collect();
    Some(if outline.len() > 2 { vec![outline] } else { Vec::new() })
}

/// The closed outlines of the path data `d`, its curves cut into [`CURVE_STEPS`] lines each. A command's numbers may
/// repeat it (after M, as L); a lower-case command is relative to the current point.
fn outlines(d: &str) -> Option<Vec<Vec<Pos2>>> {
    let words = words(d)?;
    let mut at = 0_usize;
    let mut found: Vec<Vec<Pos2>> = Vec::new();
    let mut outline: Vec<Pos2> = Vec::new();
    let mut current = Pos2::ZERO;
    let mut start = Pos2::ZERO;
    let mut command: Option<char> = None;
    let close = |outline: &mut Vec<Pos2>, found: &mut Vec<Vec<Pos2>>| {
        if outline.len() > 2 {
            found.push(std::mem::take(outline));
        }
        outline.clear();
    };
    while let Some(word) = words.get(at) {
        let letter = match word {
            Word::Command(letter) => {
                at = at.saturating_add(1);
                *letter
            }
            // Numbers after a command's own repeat it; after M they draw lines.
            Word::Number(_) => match command? {
                'M' => 'L',
                'm' => 'l',
                'Z' | 'z' => return None,
                other => other,
            },
        };
        let relative = letter.is_ascii_lowercase();
        let base = if relative { current.to_vec2() } else { Vec2::ZERO };
        let mut number = || match words.get(at) {
            Some(Word::Number(value)) => {
                at = at.saturating_add(1);
                Some(*value)
            }
            _ => None,
        };
        match letter.to_ascii_uppercase() {
            'M' => {
                close(&mut outline, &mut found);
                current = pos2(number()?, number()?) + base;
                start = current;
                outline.push(current);
            }
            'L' => {
                current = pos2(number()?, number()?) + base;
                outline.push(current);
            }
            'H' => {
                current.x = number()? + base.x;
                outline.push(current);
            }
            'V' => {
                current.y = number()? + base.y;
                outline.push(current);
            }
            'C' => {
                let one = pos2(number()?, number()?) + base;
                let two = pos2(number()?, number()?) + base;
                let to = pos2(number()?, number()?) + base;
                cubic(&mut outline, current, one, two, to);
                current = to;
            }
            'Q' => {
                let one = pos2(number()?, number()?) + base;
                let to = pos2(number()?, number()?) + base;
                // A quadratic curve is the cubic with its two controls two thirds of the way to its one.
                let first = current + (one - current) * (2.0 / 3.0);
                let second = to + (one - to) * (2.0 / 3.0);
                cubic(&mut outline, current, first, second, to);
                current = to;
            }
            'Z' => {
                close(&mut outline, &mut found);
                current = start;
            }
            _ => return None,
        }
        command = Some(letter);
    }
    close(&mut outline, &mut found);
    Some(found)
}

/// The cubic curve from `from` through the controls `one` and `two` to `to`, cut into [`CURVE_STEPS`] lines, added to
/// `outline` (without `from`, already there).
fn cubic(outline: &mut Vec<Pos2>, from: Pos2, one: Pos2, two: Pos2, to: Pos2) {
    for step in 1..=CURVE_STEPS {
        let t = f32::from(step) / f32::from(CURVE_STEPS);
        let u = 1.0 - t;
        let at = from.to_vec2() * (u * u * u) + one.to_vec2() * (3.0 * u * u * t) + two.to_vec2() * (3.0 * u * t * t) + to.to_vec2() * (t * t * t);
        outline.push(at.to_pos2());
    }
}

/// `picture` drawn at `ppp` pixels a point, shifted right and down by `shift` of a pixel (0 to 1 each way): each pixel
/// is the share of its [`SAMPLES`] × [`SAMPLES`] points each shape covers (a later shape over an earlier one), each
/// shape filled even-odd as the files' holes want. None at a scale that is not a number or out of [`PPP`].
pub(crate) fn image(picture: &Picture, ppp: f32, shift: Vec2) -> Option<ColorImage> {
    if !(ppp.is_finite() && PPP.contains(&ppp) && shift.x.is_finite() && shift.y.is_finite()) {
        return None;
    }
    let side = |length: f32, extra: f32| (length * ppp + extra.clamp(0.0, 1.0)).ceil().clamp(1.0, MOST_PIXELS);
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to 1 ..= 1024 above")]
    let (wide, high) = (side(picture.size.x, shift.x) as usize, side(picture.size.y, shift.y) as usize);
    let mut pixels = Vec::with_capacity(wide.saturating_mul(high));
    let samples = f32::from(SAMPLES);
    for row in 0..high {
        for column in 0..wide {
            let mut sum = [0.0_f32; 4];
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    #[expect(clippy::cast_precision_loss, reason = "a pixel's place in a picture of at most 1024 pixels a side")]
                    let at = pos2(
                        (column as f32 + (f32::from(sx) + 0.5) / samples - shift.x) / ppp,
                        (row as f32 + (f32::from(sy) + 0.5) / samples - shift.y) / ppp,
                    );
                    let covering = picture.shapes.iter().rev().find(|shape| shape.bounds.contains(at) && inside(&shape.outlines, at));
                    if let Some(shape) = covering {
                        for (into, value) in sum.iter_mut().zip(shape.colour.to_array()) {
                            *into += f32::from(value);
                        }
                    }
                }
            }
            let share = samples * samples;
            #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a mean of bytes, rounded and clamped")]
            let [r, g, b, a] = sum.map(|value| (value / share).round().clamp(0.0, 255.0) as u8);
            pixels.push(Color32::from_rgba_premultiplied(r, g, b, a));
        }
    }
    Some(ColorImage::new([wide, high], pixels))
}

/// Whether `at` lies inside `outlines`, even-odd: a line from it to the right crosses their sides an odd number of times.
pub(crate) fn inside(outlines: &[Vec<Pos2>], at: Pos2) -> bool {
    let mut odd = false;
    for outline in outlines {
        let mut last = outline.last().copied();
        for &point in outline {
            if let Some(from) = last
                && (from.y > at.y) != (point.y > at.y)
                && at.x < from.x + (at.y - from.y) / (point.y - from.y) * (point.x - from.x)
            {
                odd = !odd;
            }
            last = Some(point);
        }
    }
    odd
}

/// Where a box at `at` (points) lands at `ppp`: the whole pixel it starts in and the part of a pixel it is shifted by,
/// rounded to [`SHIFTS`] parts (a whole one carries into the pixel).
pub(crate) fn placed(at: Pos2, ppp: f32) -> (Pos2, Vec2) {
    let one = |value: f32| {
        let pixels = value * ppp;
        let whole = pixels.floor();
        let part = ((pixels - whole) * SHIFTS).round() / SHIFTS;
        if part >= 1.0 { (whole + 1.0, 0.0) } else { (whole, part) }
    };
    let ((x, sx), (y, sy)) = (one(at.x), one(at.y));
    (pos2(x, y), vec2(sx, sy))
}

/// `mark` for `mode` at `ppp` pixels a point, shifted by `shift` of a pixel, drawn once and kept in the context; none
/// when the built-in file does not read or the scale is not usable.
fn texture(ctx: &Context, mark: Mark, mode: Mode, ppp: f32, shift: Vec2) -> Option<TextureHandle> {
    let id = Id::unique("brand-mark").with((mark, mode == Mode::Dark, ppp.to_bits(), shift.x.to_bits(), shift.y.to_bits()));
    if let Some(kept) = ctx.data(|data| data.get_temp::<TextureHandle>(id)) {
        return Some(kept);
    }
    let image = image(&mark.picture(mode)?, ppp, shift)?;
    #[cfg(test)]
    ctx.data_mut(|data| *data.get_temp_mut_or_default::<u32>(rasterised_id()) += 1);
    let texture = ctx.load_texture("brand-mark", image, TextureOptions::NEAREST);
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    Some(texture)
}

/// Where the tests count the pictures drawn in a context.
#[cfg(test)]
fn rasterised_id() -> Id {
    Id::unique("brand-rasterised")
}

/// How many pictures of a mark were drawn (rasterised and uploaded) in `ctx`: once each, then kept.
#[cfg(test)]
pub(crate) fn rasterised(ctx: &Context) -> u32 {
    ctx.data(|data| data.get_temp::<u32>(rasterised_id())).unwrap_or(0)
}

/// Paints `mark` for `mode` with its box's top left corner at `at`, pixel for pixel; the rect of the picture painted
/// (from the whole pixel the box starts in), or none where nothing was painted.
pub(crate) fn paint(ui: &Ui, mark: Mark, mode: Mode, at: Pos2) -> Option<Rect> {
    let ppp = ui.ctx().pixels_per_point();
    let (pixel, shift) = placed(at, ppp);
    let texture = texture(ui.ctx(), mark, mode, ppp, shift)?;
    let [wide, high] = texture.size();
    #[expect(clippy::cast_precision_loss, reason = "a picture of at most 1024 pixels a side")]
    let rect = Rect::from_min_size(pos2(pixel.x / ppp, pixel.y / ppp), vec2(wide as f32 / ppp, high as f32 / ppp));
    if !ui.is_rect_visible(rect) {
        return None;
    }
    let uv = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
    ui.painter().image(texture.id(), rect, uv, crate::theme::NO_TINT);
    Some(rect)
}

/// The Qubernetic emblem's grid of characters, 76 × 40 cells (547 × 500 points), as the character field draws it:
/// the ramp's characters in ink, the digits in cyan. Read once from the built-in file.
pub(crate) fn emblem() -> &'static Emblem {
    static EMBLEM: LazyLock<Emblem> = LazyLock::new(|| {
        let rows: Vec<&str> = EMBLEM_ROWS.split('\n').map(|row| row.trim_end_matches('\r')).collect();
        Emblem::from_rows(&rows)
    });
    &EMBLEM
}

/// The emblem's file: one line a row.
pub(crate) const EMBLEM_ROWS: &str = include_str!("../../../assets/brand/qubernetic/emblem.txt");

#[cfg(test)]
#[path = "brand_tests.rs"]
mod tests;
