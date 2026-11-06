// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The app's own window frame, drawn only when the native window has no title bar of the desktop's
//! ([`WindowFrame::TopRow`] or [`WindowFrame::Strip`]): the window controls (minimise, maximise, close), a slim strip
//! with the window's title that moves the window, and the edges that resize it. Every control asks the window through
//! a viewport command; nothing here runs a command of the engine.

use egui::{
    Align2, Area, Context, CornerRadius, CursorIcon, Id, LayerId, Order, Rect, ResizeDirection, Response, Role, Sense, Stroke, StrokeKind, Ui,
    ViewportCommand, WidgetInfo, pos2, vec2,
};
use inboxcraft_engine::appearance::WindowFrame;

use crate::component::tooltip;
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

/// The strip's height, in points.
pub(super) const STRIP: f32 = 34.0;

/// A window control's width and height, in points.
const CONTROL: [f32; 2] = [34.0, 28.0];

/// How far from the window's edge a press resizes it, in points.
const EDGE: f32 = 5.0;

/// A window control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    Minimise,
    Maximise,
    Close,
}

impl Control {
    fn name(self, maximised: bool) -> &'static str {
        match self {
            Self::Minimise => tr!("Minimise"),
            Self::Maximise if maximised => tr!("Restore"),
            Self::Maximise => tr!("Maximise"),
            Self::Close => tr!("Close the window"),
        }
    }
}

/// Whether the window is maximised now.
fn maximised(ctx: &Context) -> bool {
    ctx.input(|input| input.viewport().maximized.unwrap_or(false))
}

/// One control: its lines in the text colour, a hover fill (the error colour behind Close).
fn control(ui: &mut Ui, theme: &Theme, keymap: &Keymap, which: Control) -> Response {
    let p = &theme.palette;
    let maximised = maximised(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(vec2(CONTROL[0], CONTROL[1]), Sense::click());
    let name = which.name(maximised);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, name));
    if ui.is_rect_visible(rect) {
        let lit = response.hovered() || response.has_focus();
        let ink = if lit && which == Control::Close { p.on_error } else { p.text };
        if lit {
            let fill = if which == Control::Close { p.error } else { p.hover };
            ui.painter().rect_filled(rect, theme.spacing.corner, fill);
        }
        let c = rect.center();
        let r = 5.0;
        let stroke = Stroke::new(1.2, ink);
        let painter = ui.painter();
        match which {
            Control::Minimise => {
                painter.line_segment([pos2(c.x - r, c.y), pos2(c.x + r, c.y)], stroke);
            }
            Control::Maximise if maximised => {
                painter.rect_stroke(Rect::from_center_size(c + vec2(-1.5, 1.5), vec2(r * 1.6, r * 1.6)), 1.5, stroke, egui::StrokeKind::Middle);
                painter.line_segment([pos2(c.x - r + 3.0, c.y - r), pos2(c.x + r, c.y - r)], stroke);
                painter.line_segment([pos2(c.x + r, c.y - r), pos2(c.x + r, c.y + r - 3.0)], stroke);
            }
            Control::Maximise => {
                painter.rect_stroke(Rect::from_center_size(c, vec2(r * 2.0, r * 2.0)), 1.5, stroke, egui::StrokeKind::Middle);
            }
            Control::Close => {
                painter.line_segment([pos2(c.x - r, c.y - r), pos2(c.x + r, c.y + r)], stroke);
                painter.line_segment([pos2(c.x - r, c.y + r), pos2(c.x + r, c.y - r)], stroke);
            }
        }
    }
    let response = tooltip::attach(response, theme, keymap, name, None);
    if response.clicked() {
        let command = match which {
            Control::Minimise => ViewportCommand::Minimized(true),
            Control::Maximise => ViewportCommand::Maximized(!maximised),
            Control::Close => ViewportCommand::Close,
        };
        ui.ctx().send_viewport_cmd(command);
    }
    response
}

/// The three controls in a row, Close last.
fn controls(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for which in [Control::Minimise, Control::Maximise, Control::Close] {
            control(ui, theme, keymap, which);
        }
    });
}

/// Moves the window while `response` is dragged, and maximises or restores it on a double click.
fn moves_the_window(ctx: &Context, response: &Response) {
    if response.drag_started_by(egui::PointerButton::Primary) {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }
    if response.double_clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximised(ctx)));
    }
}

/// The width of the three controls and the room between them.
const CONTROLS: f32 = CONTROL[0] * 3.0 + 4.0;

/// The strip over the whole of `ui`: a band of glass with a line under it, `title` in the middle in the muted
/// colour, the controls at the right; pressing anywhere else on it moves the window. Its top right corner follows the
/// window's `corner`.
pub(super) fn strip(ui: &mut Ui, theme: &Theme, keymap: &Keymap, title: &str, corner: f32) {
    let p = &theme.palette;
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, top_right_corner(corner), p.glass);
    let width = 1.0 / ui.ctx().pixels_per_point();
    ui.painter().hline(rect.x_range(), rect.bottom() - width / 2.0, Stroke::new(width, p.line));
    let handle = ui.interact(rect, Id::unique("window-strip"), Sense::click_and_drag());
    moves_the_window(ui.ctx(), &handle);
    let galley = ui.painter().layout_no_wrap(title.to_owned(), theme.fonts.caption.id(), p.muted);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, p.muted);
    let at = Rect::from_min_max(pos2(rect.right() - CONTROLS - theme.spacing.s, rect.top()), rect.max);
    ui.scope_builder(egui::UiBuilder::new().max_rect(at).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        controls(ui, theme, keymap);
    });
}

/// How high the band at the top of the window is that moves it in the top row: the panes' first rows.
const BAND: f32 = STRIP * 2.0;

/// How near the window's right edge a pane's first row ends when the controls of the top row sit at its end.
const REACH: f32 = 64.0;

/// Where the top row's controls go this frame, kept by the pane under them.
fn place_id() -> Id {
    Id::unique("window-controls-place")
}

/// Whether the top row is on, for the panes' first rows; set each frame before the panes are drawn.
fn on_id() -> Id {
    Id::unique("window-top-row")
}

/// Starts the top row's frame: tells the panes that it is on, and makes the band at the top of `ui` move the window
/// where no pane takes the press (between and above the panes, beside their titles).
pub(super) fn top_row_begins(ui: &mut Ui, on: bool) {
    ui.ctx().data_mut(|data| {
        data.insert_temp(on_id(), on);
        data.insert_temp(place_id(), None::<Rect>);
    });
    if !on {
        return;
    }
    let free = ui.available_rect_before_wrap();
    let band = Rect::from_min_max(free.min, pos2(free.right(), free.top() + BAND));
    let handle = ui.interact(band, Id::unique("window-top-band"), Sense::click_and_drag());
    moves_the_window(ui.ctx(), &handle);
}

/// Draws `add` as a pane's first row and, when the top row is on and the row ends at the window's right, leaves room
/// at its right end for the window controls, which are drawn there.
pub(super) fn beside<R>(ui: &mut Ui, theme: &Theme, add: impl FnOnce(&mut Ui) -> R) -> R {
    let on = ui.ctx().data(|data| data.get_temp::<bool>(on_id())).unwrap_or(false);
    let whole = ui.max_rect();
    if !on || whole.right() < ui.ctx().content_rect().right() - REACH {
        return add(ui);
    }
    let room = float_width() + theme.spacing.s;
    let inner = ui.scope(|ui| {
        ui.set_max_width((ui.available_width() - room).max(0.0));
        add(ui)
    });
    let row = inner.response.rect;
    let place = Rect::from_min_max(pos2(whole.right() - float_width(), row.top()), pos2(whole.right(), row.bottom()));
    ui.ctx().data_mut(|data| data.insert_temp(place_id(), Some(place)));
    inner.inner
}

/// The width of the controls on their float.
fn float_width() -> f32 {
    CONTROLS + 2.0 * (f32::from(FLOAT_MARGIN) + 1.0)
}

/// The controls in the top row, on a small float at the end of the right pane's first row (kept by [`beside`] this
/// frame), or at the window's top right when no pane kept a place.
pub(super) fn top_row(ctx: &Context, theme: &Theme, keymap: &Keymap) {
    let place = ctx.data(|data| data.get_temp::<Option<Rect>>(place_id())).flatten();
    let area = Area::new(Id::unique("window-controls")).order(Order::Foreground);
    let area = match place {
        Some(place) => area.pivot(Align2::RIGHT_CENTER).fixed_pos(place.right_center()),
        None => {
            let gap = theme.spacing.gap;
            area.anchor(Align2::RIGHT_TOP, vec2(-gap * 2.0, gap * 2.0))
        }
    };
    area.show(ctx, |ui| {
        Frame::float(theme).show(ui, |ui| controls(ui, theme, keymap));
    });
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
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
    }
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
    painter.rect_stroke(ctx.content_rect(), round(corner), Stroke::new(width, theme.palette.window_edge), StrokeKind::Inside);
}

/// `corner` as egui's radius, all four corners.
fn round(corner: f32) -> CornerRadius {
    // A few points; the clamp keeps the cast inside u8.
    CornerRadius::same(corner.clamp(0.0, 255.0) as u8)
}

/// `corner` on the left two corners only, for what touches the window's left edge (the rail).
#[must_use]
pub(super) fn left_corners(corner: f32) -> CornerRadius {
    let r = round(corner).nw;
    CornerRadius { nw: r, sw: r, ne: 0, se: 0 }
}

/// `corner` on the top right corner only, for what touches the window's top right (the strip).
fn top_right_corner(corner: f32) -> CornerRadius {
    CornerRadius { ne: round(corner).ne, ..CornerRadius::ZERO }
}

/// Whether the app draws its own frame for `frame` in a window that `owns` one.
#[must_use]
pub(super) fn drawn(owns: bool, frame: WindowFrame) -> bool {
    owns && frame != WindowFrame::System
}

/// The margin inside the float of the top row's controls, in points.
const FLOAT_MARGIN: i8 = 2;

/// A float's frame in `theme`, for the controls in the top row.
struct Frame;

impl Frame {
    fn float(theme: &Theme) -> egui::Frame {
        egui::Frame::new()
            .fill(theme.palette.float_over_text)
            .stroke(Stroke::new(1.0, theme.palette.glass_edge))
            // A few points; the clamp keeps the cast inside u8.
            .corner_radius(theme.spacing.corner.clamp(0.0, 255.0) as u8)
            .inner_margin(egui::Margin::same(FLOAT_MARGIN))
    }
}
