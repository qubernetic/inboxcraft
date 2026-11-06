// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The app's own window frame, drawn only when the native window has no title bar of the desktop's
//! ([`WindowFrame::TopRow`] or [`WindowFrame::Strip`]): the window controls (minimise, maximise, close), a slim strip
//! with the window's title that moves the window, and the edges that resize it. Every control asks the window through
//! a viewport command; nothing here runs a command of the engine.

use egui::{
    Align2, Area, Context, CursorIcon, Id, Order, Pos2, Rect, ResizeDirection, Response, Role, Sense, Stroke, Ui, ViewportCommand, WidgetInfo, pos2,
    vec2,
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

/// The strip over the whole of `ui`: `title` in the middle in the muted colour, the controls at the right; pressing
/// anywhere else on it moves the window.
pub(super) fn strip(ui: &mut Ui, theme: &Theme, keymap: &Keymap, title: &str) {
    let rect = ui.max_rect();
    let handle = ui.interact(rect, Id::unique("window-strip"), Sense::click_and_drag());
    moves_the_window(ui.ctx(), &handle);
    let galley = ui.painter().layout_no_wrap(title.to_owned(), theme.fonts.caption.id(), theme.palette.muted);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, theme.palette.muted);
    // Three controls and the room between them, at the right edge.
    let width = CONTROL[0] * 3.0 + 4.0;
    let at = Rect::from_min_max(pos2(rect.right() - width - theme.spacing.s, rect.top()), rect.max);
    ui.scope_builder(egui::UiBuilder::new().max_rect(at).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        controls(ui, theme, keymap);
    });
}

/// The controls floating at the window's top right, over the top row of the panes, with room to move the window by
/// pressing beside them.
pub(super) fn top_row(ctx: &Context, theme: &Theme, keymap: &Keymap, gap: f32) {
    Area::new(Id::unique("window-controls")).order(Order::Foreground).anchor(Align2::RIGHT_TOP, vec2(-gap * 2.0, gap * 2.0)).show(ctx, |ui| {
        Frame::float(theme).show(ui, |ui| controls(ui, theme, keymap));
    });
}

/// The edges of the window resize it, its corners in both directions; the pointer shows which way.
pub(super) fn edges(ctx: &Context) {
    let Some(screen) = ctx.input(|input| input.viewport().inner_rect.map(|rect| Rect::from_min_size(Pos2::ZERO, rect.size()))) else {
        return;
    };
    if maximised(ctx) {
        return;
    }
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

/// Whether the app draws its own frame for `frame` in a window that `owns` one.
#[must_use]
pub(super) fn drawn(owns: bool, frame: WindowFrame) -> bool {
    owns && frame != WindowFrame::System
}

/// A float's frame in `theme`, for the controls in the top row.
struct Frame;

impl Frame {
    fn float(theme: &Theme) -> egui::Frame {
        egui::Frame::new()
            .fill(theme.palette.float_over_text)
            .stroke(Stroke::new(1.0, theme.palette.glass_edge))
            // A few points; the clamp keeps the cast inside u8.
            .corner_radius(theme.spacing.corner.clamp(0.0, 255.0) as u8)
            .inner_margin(egui::Margin::same(2))
    }
}
