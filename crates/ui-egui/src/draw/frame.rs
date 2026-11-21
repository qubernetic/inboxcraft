// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The app's own window frame, drawn only when the native window has no title bar of the desktop's
//! ([`WindowFrame::Own`]): the command line is the title bar. A press and a drag on it where no control takes the
//! press moves the window, a double press maximises or restores it, and the window controls (minimise, maximise,
//! close) stand at its right end; the edges resize the window. Every control asks the window through a viewport
//! command; nothing here runs a command of the engine. The app's mark at the command line's left is drawn in every
//! frame ([`mark`]).

use egui::{
    Area, Context, CornerRadius, CursorIcon, Id, LayerId, Order, Pos2, Rect, ResizeDirection, Response, Sense, Stroke, StrokeKind, Ui,
    ViewportCommand, ViewportId, pos2,
};
use inboxcraft_engine::appearance::WindowFrame;

use crate::component::window_controls::{self, Control, Window};
use crate::theme::Theme;

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
/// Where `free`, the pointer is on the handle's free part under a held float's ground, which egui's hover does not see.
fn moves_the_window(ctx: &Context, response: &Response, free: bool) {
    let id = response.id.with("pressed-at");
    let (pressed, released, at) = ctx.input(|input| (input.pointer.primary_pressed(), input.pointer.primary_released(), input.pointer.latest_pos()));
    // Hovered, not only under the pointer: where the field or a control stands over the handle, the press is theirs.
    if pressed && (response.hovered() || free) {
        ctx.data_mut(|data| data.insert_temp(id, at));
    } else if released {
        ctx.data_mut(|data| data.insert_temp(id, None::<Pos2>));
    } else if let (Some(from), Some(to)) = (ctx.data(|data| data.get_temp::<Option<Pos2>>(id)).flatten(), at)
        && from.distance(to) > MOVE_AFTER
    {
        ctx.data_mut(|data| data.insert_temp(id, None::<Pos2>));
        hand_over(ctx, ViewportCommand::StartDrag);
    }
    let doubled = free && ctx.input(|input| input.pointer.button_double_clicked(egui::PointerButton::Primary));
    if response.double_clicked() || doubled {
        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximised(ctx)));
    }
}

/// The handle that makes the command line `line` the title bar: registered before the field and the controls, which
/// take the press where they stand, so a press anywhere else on the line (the mark's column, the gaps) moves the
/// window, and a double press maximises or restores it. By the pointer only: a handle is no Tab stop. Under the event
/// editor too, whose ground lies over the window though the frames draw none: there the handle's free part (neither
/// the editor, the field nor a control under the pointer) moves the window, and the press is no press outside the
/// editor. The field under it stays the editor's press outside.
pub(super) fn title_bar(ui: &Ui, line: Rect) {
    let ctx = ui.ctx();
    let handle = ui.interact(line, title_bar_id(), Sense::CLICK | Sense::DRAG);
    let free = crate::component::popup::ground(ctx).is_some_and(|float| {
        let field = ctx.data(|data| data.get_temp::<Rect>(crate::command::field_id().with("box")));
        let controls = Rect::from_min_max(pos2(line.right() - CONTROLS_ROOM + GAP_BEFORE, line.top()), line.max);
        ctx.input(|input| input.pointer.interact_pos())
            .is_some_and(|at| line.contains(at) && !float.contains(at) && !controls.contains(at) && !field.is_some_and(|field| field.contains(at)))
    });
    if free && ctx.input(|input| input.pointer.any_pressed()) {
        crate::component::popup::press_is_the_window_s(ctx);
    }
    moves_the_window(ctx, &handle, free);
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
    crate::brand::paint(ui, crate::brand::Mark::Compact, theme.mode, at);
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

/// Marks that the desktop has the pointer of the window `viewport`: a move or a resize of that window it does itself.
fn grabbed_id(viewport: ViewportId) -> Id {
    Id::unique("window-grabbed").with(viewport)
}

/// Sends `command`, which hands the pointer to the desktop until the button comes up, and marks it so the next input
/// of the same window gets the release the desktop keeps ([`release_after_grab`]): the main window's through its input
/// hook, a window of its own's through [`GrabRelease`], which egui runs before every pass of every window.
fn hand_over(ctx: &Context, command: ViewportCommand) {
    let id = grabbed_id(ctx.viewport_id());
    ctx.data_mut(|data| data.insert_temp(id, true));
    ctx.add_plugin(GrabRelease);
    ctx.send_viewport_cmd(command);
}

/// Adds to `raw` the release of the primary button when the desktop took the pointer of `raw`'s window since that
/// window's last pass: while it moves or resizes the window it keeps the release to itself, and egui would think the
/// button still down, showing no hover anywhere until the next click. Only that window's input gets it.
pub(super) fn release_after_grab(ctx: &Context, raw: &mut egui::RawInput) {
    let viewport = raw.viewport_id;
    let grabbed = ctx.data_mut(|data| data.remove_temp::<bool>(grabbed_id(viewport))).unwrap_or(false);
    if !grabbed {
        return;
    }
    let (pos, modifiers) = ctx.input_for(viewport, |input| (input.pointer.latest_pos().unwrap_or(Pos2::ZERO), input.modifiers));
    raw.events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers });
}

/// Gives a window of its own's input the release the desktop kept ([`release_after_grab`]): eframe's input hook reaches
/// the main window only; egui runs its plugins before every window's pass.
struct GrabRelease;

impl egui::Plugin for GrabRelease {
    fn debug_name(&self) -> &'static str {
        "inboxcraft-grab-release"
    }

    fn input_hook(&mut self, ctx: &Context, input: &mut egui::RawInput) {
        // The main window's is its input hook's ([`release_after_grab`] there).
        if input.viewport_id != ViewportId::ROOT {
            release_after_grab(ctx, input);
        }
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
