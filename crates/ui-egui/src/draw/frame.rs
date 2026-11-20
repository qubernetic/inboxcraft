// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The app's own window frame, drawn only when the native window has no title bar of the desktop's
//! ([`WindowFrame::Own`]): the command line is the title bar. A press and a drag on it where no control takes the
//! press moves the window, a double press maximises or restores it, and the window controls (minimise, maximise,
//! close) stand at its right end; the edges resize the window. Every control asks the window through a viewport
//! command; nothing here runs a command of the engine. The app's mark at the command line's left is drawn in every
//! frame ([`mark`]).

use egui::{
    Area, ColorImage, Context, CornerRadius, CursorIcon, Id, LayerId, Order, Pos2, Rect, ResizeDirection, Response, Sense, Stroke, StrokeKind,
    TextureHandle, TextureOptions, Ui, ViewportCommand, pos2, vec2,
};
use inboxcraft_engine::appearance::WindowFrame;

use crate::component::window_controls::{self, Control, Window};
use crate::theme::{Mode, Theme};

/// How far from the window's edge a press resizes it, in points.
const EDGE: f32 = 5.0;

/// The room the window controls keep at the command line's right end, in points: the 12 before them, their three
/// cells and the 4 after them.
pub(super) const CONTROLS_ROOM: f32 = GAP_BEFORE + window_controls::CELL * 3.0 + GAP_AFTER;

/// The room between the command field and the window controls, in points.
const GAP_BEFORE: f32 = 12.0;

/// The room between the window controls and the window's right edge, in points.
const GAP_AFTER: f32 = 4.0;

/// The room at the command line's right end when no window controls stand there: as much as before them.
pub(super) const NO_CONTROLS_ROOM: f32 = GAP_BEFORE;

/// The mark's side, and how far in from the command line's left it stands, in points.
const MARK: f32 = 22.0;
const MARK_IN: f32 = 16.0;

/// Whether the window is maximised now.
fn maximised(ctx: &Context) -> bool {
    ctx.input(|input| input.viewport().maximized.unwrap_or(false))
}

/// Whether the window is in front now; a window that does not say counts as in front.
fn focused(ctx: &Context) -> bool {
    ctx.input(|input| input.viewport().focused.unwrap_or(true))
}

/// How far the pointer goes with the button down on a handle before the window moves, in points; less is a click.
const MOVE_AFTER: f32 = 3.0;

/// Moves the window when the pointer goes a few points with the button pressed on `response`, and maximises or
/// restores it on a double click. The press is kept here rather than taken from egui's drag: while the desktop moves
/// the window it holds the pointer, the app never hears the button come up, and egui would take the next press for
/// the same drag still going on.
fn moves_the_window(ctx: &Context, response: &Response) {
    let id = response.id.with("pressed-at");
    let (pressed, released, at) = ctx.input(|input| (input.pointer.primary_pressed(), input.pointer.primary_released(), input.pointer.latest_pos()));
    // Hovered, not only under the pointer: where the field or a control stands over the handle, the press is theirs.
    if pressed && response.hovered() {
        ctx.data_mut(|data| data.insert_temp(id, at));
    } else if released {
        ctx.data_mut(|data| data.insert_temp(id, None::<Pos2>));
    } else if let (Some(from), Some(to)) = (ctx.data(|data| data.get_temp::<Option<Pos2>>(id)).flatten(), at)
        && from.distance(to) > MOVE_AFTER
    {
        ctx.data_mut(|data| data.insert_temp(id, None::<Pos2>));
        hand_over(ctx, ViewportCommand::StartDrag);
    }
    if response.double_clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximised(ctx)));
    }
}

/// The handle that makes the command line `line` the title bar: registered before the field and the controls, which
/// take the press where they stand, so a press anywhere else on the line (the mark's column, the gaps) moves the
/// window, and a double press maximises or restores it. By the pointer only: a handle is no Tab stop.
pub(super) fn title_bar(ui: &Ui, line: Rect) {
    let handle = ui.interact(line, title_bar_id(), Sense::CLICK | Sense::DRAG);
    moves_the_window(ui.ctx(), &handle);
}

/// The title bar's handle.
pub(super) fn title_bar_id() -> Id {
    Id::unique("window-title-bar")
}

/// The window controls at the right end of the command line `line`, 4 from the window's edge and centred in its
/// height. On a layer of their own above everything, so a float or the event editor never covers them and Close
/// always closes; while a surface keeps the keyboard (a float, the editor) they are the pointer's only and `Tab` goes
/// round that surface. Pressed, by the pointer or by Enter or Space, they ask the window to minimise, maximise or
/// restore, and close.
pub(super) fn controls(ctx: &Context, theme: &Theme, line: Rect) {
    let high = window_controls::height(theme);
    let at = pos2(line.right() - GAP_AFTER - window_controls::CELL * 3.0, (line.center().y - high / 2.0).round());
    let keyboard = ctx.memory(|memory| memory.top_modal_layer()).is_none();
    let window = Window { maximised: maximised(ctx), focused: focused(ctx) };
    let shown = Area::new(Id::unique("window-controls"))
        .order(Order::Foreground)
        .fixed_pos(at)
        .show(ctx, |ui| window_controls::window_controls_keyed(ui, theme, Id::unique("window-control"), window, keyboard));
    ctx.move_to_top(shown.response.layer_id);
    let command = match shown.inner {
        Some(Control::Minimise) => ViewportCommand::Minimized(true),
        Some(Control::Maximise) => ViewportCommand::Maximized(!window.maximised),
        Some(Control::Close) => ViewportCommand::Close,
        None => return,
    };
    ctx.send_viewport_cmd(command);
}

/// The app's mark in a box of 22 points a side, centred in the command line's height: 16 in from its left, or centred
/// in the narrow index's column (`narrow`, the frames `size-1100/*` and `size-800/*`). The brand's compact mark, its flap
/// in cyan, drawn light on the dark look and dark on the light one from the brand's vector file, pixel for pixel at the
/// window's scale (the frames' `InboxCraft mark-compact · 22`). It takes no press (in the own frame the title bar's
/// handle under it does).
pub(super) fn mark(ui: &Ui, theme: &Theme, line: Rect, narrow: bool) {
    let left = if narrow { (super::shell::INDEX_NARROW + 1.0 - MARK) / 2.0 } else { MARK_IN };
    let at = pos2(line.left() + left, line.center().y - MARK / 2.0);
    let ppp = ui.ctx().pixels_per_point();
    // Where the box starts in physical pixels: its whole pixel, and the part of one the picture is drawn shifted by.
    let (x, y) = ((at.x * ppp).floor(), (at.y * ppp).floor());
    let shift = vec2(at.x * ppp - x, at.y * ppp - y);
    let Some(texture) = mark_texture(ui.ctx(), theme.mode, ppp, shift) else { return };
    let [wide, high] = texture.size();
    #[expect(clippy::cast_precision_loss, reason = "a picture of a few dozen pixels")]
    let rect = Rect::from_min_size(pos2(x / ppp, y / ppp), vec2(wide as f32 / ppp, high as f32 / ppp));
    if !ui.is_rect_visible(rect) {
        return;
    }
    let uv = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
    ui.painter().image(texture.id(), rect, uv, crate::theme::NO_TINT);
}

/// The mark's picture for `mode` at `ppp` pixels a point, shifted by `shift` of a pixel, drawn once and kept in the
/// context; none when the built-in file does not read.
fn mark_texture(ctx: &Context, mode: Mode, ppp: f32, shift: egui::Vec2) -> Option<TextureHandle> {
    let id = Id::unique("window-mark").with((mode == Mode::Dark, ppp.to_bits(), shift.x.to_bits(), shift.y.to_bits()));
    if let Some(kept) = ctx.data(|data| data.get_temp::<TextureHandle>(id)) {
        return Some(kept);
    }
    let file = if mode == Mode::Dark { MARK_DARK } else { MARK_LIGHT };
    let image = mark_image(&mark_shapes(file)?, ppp, shift)?;
    let texture = ctx.load_texture("window-mark", image, TextureOptions::NEAREST);
    ctx.data_mut(|data| data.insert_temp(id, texture.clone()));
    Some(texture)
}

/// The brand's compact mark for the dark look and for the light one: one path for the frame, one for the flap.
pub(super) const MARK_DARK: &str = include_str!("../../../../assets/brand/inboxcraft/svg/mark-compact-dark.svg");
pub(super) const MARK_LIGHT: &str = include_str!("../../../../assets/brand/inboxcraft/svg/mark-compact-light.svg");

/// Where the mark's frame stands in its box of [`MARK`], from the frames: at 0, 2.62, 21.88 × 16.82.
const MARK_FRAME: Rect = Rect { min: pos2(0.0, 2.62), max: pos2(21.88, 19.44) };

/// The most paths and points the mark's file is read for: the brand's has two paths of a few dozen curves.
const MARK_MOST_PATHS: usize = 8;
const MARK_MOST_POINTS: usize = 4096;

/// The lines a curve is cut into, and the samples a pixel is measured at along each side.
const CURVE_STEPS: u16 = 12;
const SAMPLES: u16 = 4;

/// A filled shape of the mark: its closed outlines, in the box's points, and its colour.
pub(super) struct MarkShape {
    pub(super) outlines: Vec<Vec<Pos2>>,
    pub(super) colour: egui::Color32,
}

/// The mark's shapes from `file` (an SVG of `<path d=… fill=…>` with absolute M, C, L and Z), in paint order, laid
/// out so that the first one's bounds are [`MARK_FRAME`]; none when the file does not read as that.
pub(super) fn mark_shapes(file: &str) -> Option<Vec<MarkShape>> {
    let mut shapes = Vec::new();
    let mut points = 0_usize;
    for part in file.split("<path").skip(1).take(MARK_MOST_PATHS) {
        let d = attribute(part, "d")?;
        let colour = fill(attribute(part, "fill")?)?;
        let outlines = outlines(d)?;
        points = points.saturating_add(outlines.iter().map(Vec::len).sum());
        if points > MARK_MOST_POINTS {
            return None;
        }
        shapes.push(MarkShape { outlines, colour });
    }
    let first = shapes.first()?;
    let bounds = Rect::from_points(&first.outlines.iter().flatten().copied().collect::<Vec<_>>());
    if !(bounds.width() > 0.0 && bounds.height() > 0.0) {
        return None;
    }
    let scale = vec2(MARK_FRAME.width() / bounds.width(), MARK_FRAME.height() / bounds.height());
    for point in shapes.iter_mut().flat_map(|shape| shape.outlines.iter_mut().flatten()) {
        *point = MARK_FRAME.min + (*point - bounds.min) * scale;
    }
    Some(shapes)
}

/// The colour of a `fill` of the brand's file, `#rrggbb`: the brand's own, not a token.
fn fill(hex: &str) -> Option<egui::Color32> {
    let hex = hex.strip_prefix('#').filter(|hex| hex.len() == 6)?;
    let byte = |at: usize| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok();
    let (red, green, blue) = (byte(0)?, byte(2)?, byte(4)?);
    Some(egui::Color32::from_rgba_premultiplied(red, green, blue, u8::MAX))
}

/// The value of `name="…"` in `part`.
fn attribute<'a>(part: &'a str, name: &str) -> Option<&'a str> {
    let start = part.find(&format!(" {name}=\""))?.checked_add(name.len() + 3)?;
    let rest = part.get(start..)?;
    rest.get(..rest.find('"')?)
}

/// A word of path data: a command's letter or a number.
enum Word {
    Command(char),
    Number(f32),
}

/// The words of the path data `d` ("M244.9 2.04 C261.89 …": a letter may lead a number); none when one does not read.
fn words(d: &str) -> Option<Vec<Word>> {
    let mut words = Vec::new();
    for word in d.split_ascii_whitespace() {
        let mut rest = word;
        if let Some(letter) = rest.chars().next().filter(char::is_ascii_alphabetic) {
            words.push(Word::Command(letter));
            rest = rest.get(1..)?;
        }
        if !rest.is_empty() {
            words.push(Word::Number(rest.parse::<f32>().ok().filter(|value| value.is_finite())?));
        }
    }
    Some(words)
}

/// The closed outlines of the path data `d` (absolute M, L, C and Z), its curves cut into [`CURVE_STEPS`] lines each.
fn outlines(d: &str) -> Option<Vec<Vec<Pos2>>> {
    let words = words(d)?;
    let mut words = words.iter();
    let mut found: Vec<Vec<Pos2>> = Vec::new();
    let mut outline: Vec<Pos2> = Vec::new();
    let close = |outline: &mut Vec<Pos2>, found: &mut Vec<Vec<Pos2>>| {
        if outline.len() > 2 {
            found.push(std::mem::take(outline));
        }
        outline.clear();
    };
    while let Some(word) = words.next() {
        let Word::Command(command) = word else { return None };
        let mut point = || match (words.next(), words.next()) {
            (Some(Word::Number(x)), Some(Word::Number(y))) => Some(pos2(*x, *y)),
            _ => None,
        };
        match command {
            'M' => {
                close(&mut outline, &mut found);
                outline.push(point()?);
            }
            'L' => outline.push(point()?),
            'C' => {
                let from = *outline.last()?;
                let (one, two, to) = (point()?, point()?, point()?);
                for step in 1..=CURVE_STEPS {
                    let t = f32::from(step) / f32::from(CURVE_STEPS);
                    let u = 1.0 - t;
                    let at = from.to_vec2() * (u * u * u)
                        + one.to_vec2() * (3.0 * u * u * t)
                        + two.to_vec2() * (3.0 * u * t * t)
                        + to.to_vec2() * (t * t * t);
                    outline.push(at.to_pos2());
                }
            }
            'Z' => close(&mut outline, &mut found),
            _ => return None,
        }
    }
    close(&mut outline, &mut found);
    Some(found)
}

/// The mark's `shapes` drawn at `ppp` pixels a point, shifted right and down by `shift` of a pixel: each pixel is the
/// share of its [`SAMPLES`] × [`SAMPLES`] points that each shape covers (the later shape over the earlier one), each
/// outline filled even-odd as the file's holes want.
pub(super) fn mark_image(shapes: &[MarkShape], ppp: f32, shift: egui::Vec2) -> Option<ColorImage> {
    if !(ppp.is_finite() && (0.25..=8.0).contains(&ppp)) {
        return None;
    }
    // A few dozen pixels a side at most: 22 points at 8 pixels a point and one for the shift.
    let side = |extra: f32| (MARK * ppp + extra).ceil().clamp(1.0, 200.0);
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to 1 ..= 200 above")]
    let (wide, high) = (side(shift.x.clamp(0.0, 1.0)) as usize, side(shift.y.clamp(0.0, 1.0)) as usize);
    let mut pixels = Vec::with_capacity(wide.saturating_mul(high));
    let samples = f32::from(SAMPLES);
    for row in 0..high {
        for column in 0..wide {
            let mut sum = [0.0_f32; 4];
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    #[expect(clippy::cast_precision_loss, reason = "a pixel's place in a picture of a few dozen pixels")]
                    let at = pos2(
                        (column as f32 + (f32::from(sx) + 0.5) / samples - shift.x) / ppp,
                        (row as f32 + (f32::from(sy) + 0.5) / samples - shift.y) / ppp,
                    );
                    if let Some(shape) = shapes.iter().rev().find(|shape| inside(&shape.outlines, at)) {
                        let [r, g, b, a] = shape.colour.to_array();
                        for (into, value) in sum.iter_mut().zip([r, g, b, a]) {
                            *into += f32::from(value);
                        }
                    }
                }
            }
            let share = samples * samples;
            #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a mean of bytes, rounded")]
            let [r, g, b, a] = sum.map(|value| (value / share).round().clamp(0.0, 255.0) as u8);
            pixels.push(egui::Color32::from_rgba_premultiplied(r, g, b, a));
        }
    }
    Some(ColorImage::new([wide, high], pixels))
}

/// Whether `at` lies inside `outlines`, even-odd: a line from it to the right crosses their sides an odd number of times.
fn inside(outlines: &[Vec<Pos2>], at: Pos2) -> bool {
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

/// The edges of the window resize it, its corners in both directions; the pointer shows which way. The window's rect
/// comes from the context: on Wayland the desktop keeps the window's place to itself, and egui's `inner_rect` stays
/// empty there.
pub(super) fn edges(ctx: &Context) {
    if maximised(ctx) || full_screen(ctx) {
        return;
    }
    let screen = ctx.content_rect();
    let Some(at) = ctx.input(|input| input.pointer.hover_pos()) else { return };
    let (left, right, top, bottom) =
        (at.x <= screen.left() + EDGE, at.x >= screen.right() - EDGE, at.y <= screen.top() + EDGE, at.y >= screen.bottom() - EDGE);
    let direction = match (left, right, top, bottom) {
        (true, _, true, _) => ResizeDirection::NorthWest,
        (_, true, true, _) => ResizeDirection::NorthEast,
        (true, _, _, true) => ResizeDirection::SouthWest,
        (_, true, _, true) => ResizeDirection::SouthEast,
        (true, ..) => ResizeDirection::West,
        (_, true, ..) => ResizeDirection::East,
        (_, _, true, _) => ResizeDirection::North,
        (.., true) => ResizeDirection::South,
        _ => return,
    };
    let cursor = match direction {
        ResizeDirection::North | ResizeDirection::South => CursorIcon::ResizeVertical,
        ResizeDirection::East | ResizeDirection::West => CursorIcon::ResizeHorizontal,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::ResizeNwSe,
        ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::ResizeNeSw,
    };
    ctx.set_cursor_icon(cursor);
    if ctx.input(|input| input.pointer.primary_pressed()) {
        hand_over(ctx, ViewportCommand::BeginResize(direction));
    }
}

/// Marks that the desktop has the pointer: a move or a resize of the window it does itself.
fn grabbed_id() -> Id {
    Id::unique("window-grabbed")
}

/// Sends `command`, which hands the pointer to the desktop until the button comes up, and marks it so the next
/// frame's input gets the release the desktop keeps ([`release_after_grab`]).
fn hand_over(ctx: &Context, command: ViewportCommand) {
    ctx.data_mut(|data| data.insert_temp(grabbed_id(), true));
    ctx.send_viewport_cmd(command);
}

/// Adds to `raw` the release of the primary button when the desktop took the pointer since the last frame: while it
/// moves or resizes the window it keeps the release to itself, and egui would think the button still down, showing
/// no hover anywhere until the next click.
pub(super) fn release_after_grab(ctx: &Context, raw: &mut egui::RawInput) {
    let grabbed = ctx.data_mut(|data| data.remove_temp::<bool>(grabbed_id())).unwrap_or(false);
    if !grabbed {
        return;
    }
    let pos = ctx.input(|input| input.pointer.latest_pos()).unwrap_or(Pos2::ZERO);
    raw.events.push(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: ctx.input(|input| input.modifiers),
    });
}

/// Whether the window fills the screen now.
fn full_screen(ctx: &Context) -> bool {
    ctx.input(|input| input.viewport().fullscreen.unwrap_or(false))
}

/// The window's corner radius when it is round, in points; about what the desktops give their own windows.
const CORNER: f32 = 12.0;

/// The window's corner radius: round while the app draws the frame, square when the window is maximised or full
/// screen, or when the desktop draws the frame.
pub(super) fn corner(ctx: &Context, drawn: bool) -> f32 {
    if drawn && !maximised(ctx) && !full_screen(ctx) { CORNER } else { 0.0 }
}

/// The window's outline, a line one pixel wide inside its edge with its corners, over everything; nothing when the
/// corners are square (the window is maximised, or the desktop draws the frame).
pub(super) fn outline(ctx: &Context, theme: &Theme, corner: f32) {
    if corner <= 0.0 {
        return;
    }
    let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::unique("window-outline")));
    let width = 1.0 / ctx.pixels_per_point();
    painter.rect_stroke(ctx.content_rect(), round(corner), Stroke::new(width, theme.palette.line), StrokeKind::Inside);
}

/// `corner` as egui's radius, all four corners.
fn round(corner: f32) -> CornerRadius {
    // A few points; the clamp keeps the cast inside u8.
    CornerRadius::same(corner.clamp(0.0, 255.0) as u8)
}

/// Whether the app draws its own frame for `frame` in a window that `owns` one.
#[must_use]
pub(super) fn drawn(owns: bool, frame: WindowFrame) -> bool {
    owns && frame != WindowFrame::System
}
