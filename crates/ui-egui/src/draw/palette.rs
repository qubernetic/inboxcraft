// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command palette over the dimmed window: a float near the top with the field, the actions that match it (each
//! with its icon, its name and its key) and a line on the keys that work in it. Typing narrows the list, the arrow
//! keys move the pick, Enter or a press runs an action; Escape or a press beside the float closes it. An action that
//! cannot run here (Done with no message selected) is listed faded and runs nothing.

use egui::{Area, Frame, Id, Key, Modifiers, Order, Rect, Role, ScrollArea, Sense, Shape, Ui, WidgetInfo, pos2, vec2};

use crate::component::field;
use crate::component::{control_side, cut, galley, margin};
use crate::glass::{self, Under};
use crate::iconset::Style;
use crate::keymap::{Action, Keymap};
use crate::palette::{MAX_CHARS, Palette};
use crate::theme::Theme;
use crate::tr;

/// The float's width where the window has room, in points.
const WIDTH: f32 = 640.0;

/// The most rows shown at once; more scroll.
const ROWS: f32 = 8.0;

/// What the owner asked for in the palette this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Asked {
    /// Run the action.
    Run(&'static str),
    /// Move the pick down (`true`) or up.
    Step(bool),
    /// The text changed.
    Typed,
    Close,
}

/// The id of the palette's field.
pub(super) fn field_id() -> Id {
    Id::unique("palette-field")
}

/// One action's row: its icon, its name and its key; `picked` on the selection fill, faded when it cannot `run`.
fn row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, action: &Action, picked: bool, run: bool) -> egui::Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let height = control_side(ui, theme);
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width().max(height), height), if run { Sense::click() } else { Sense::hover() });
    response.widget_info(|| WidgetInfo::selected(Role::MenuItem, run, picked, action.name));
    if picked {
        ui.scroll_to_rect(rect, None);
    }
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if picked || response.hovered() {
            painter.rect_filled(rect, s.corner, p.hover);
        }
        let ink = if run { p.text } else { p.muted };
        let side = theme.fonts.body.size * 1.2;
        let icon = Rect::from_center_size(pos2(rect.left() + s.s + side / 2.0, rect.center().y), vec2(side, side));
        theme.icons.paint(painter, icon, action.icon, Style::Regular, ink);
        let mut right = rect.right() - s.s;
        if let Some(chip) = keymap.chip(ui.ctx(), action.id) {
            let chip = galley(ui, &chip, theme.fonts.caption, p.muted);
            right -= chip.size().x;
            painter.galley(pos2(right, rect.center().y - chip.size().y / 2.0), chip, p.muted);
            right -= s.l;
        }
        let left = icon.right() + s.s;
        let name = cut(ui, action.name, theme.fonts.body, ink, right - left);
        painter.galley(pos2(left, rect.center().y - name.size().y / 2.0), name, ink);
    }
    response
}

/// Draws the palette over the window; `runnable` holds the ids of the actions that can run now. What was asked for,
/// applied by the caller after the frame's drawing.
pub(super) fn show(ctx: &egui::Context, theme: &Theme, keymap: &Keymap, palette: &mut Palette, runnable: &[&str]) -> Option<Asked> {
    let p = &theme.palette;
    let s = &theme.spacing;
    let screen = ctx.content_rect();
    // The palette's keys are its own: the field never sees them.
    let (escape, enter, down, up) = ctx.input_mut(|input| {
        (
            input.consume_key(Modifiers::NONE, Key::Escape),
            input.consume_key(Modifiers::NONE, Key::Enter),
            input.consume_key(Modifiers::NONE, Key::ArrowDown),
            input.consume_key(Modifiers::NONE, Key::ArrowUp),
        )
    });
    let matches = palette.matches();
    let can = |action: &Action| runnable.contains(&action.id);
    let mut asked = if escape {
        Some(Asked::Close)
    } else if enter {
        palette.chosen().filter(|action| can(action)).map(|action| Asked::Run(action.id))
    } else if down || up {
        Some(Asked::Step(down))
    } else {
        None
    };
    // A press beside the float closes it, and reaches nothing under it.
    let beside = Area::new(Id::unique("palette-dim"))
        .order(Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, 0.0, p.scrim);
            response.clicked() || (response.contains_pointer() && ui.input(|input| input.pointer.primary_pressed()))
        })
        .inner;
    if beside {
        asked = Some(Asked::Close);
    }
    let width = WIDTH.min(screen.width() - s.gap * 2.0).max(1.0);
    let top = screen.top() + (screen.height() * 0.12).max(s.gap);
    let at = pos2(screen.center().x - width / 2.0, top);
    Area::new(Id::unique("palette")).order(Order::Foreground).fixed_pos(at).show(ctx, |ui| {
        // The float's shadow and edge, then a solid surface: nothing of the list shows through the names.
        let slot = ui.painter().add(Shape::Noop);
        let inner = Frame::new().inner_margin(margin(s.xs, s.xs)).show(ui, |ui| {
            ui.set_width((width - s.xs * 2.0).max(1.0));
            ui.spacing_mut().item_spacing.y = 0.0;
            let (_, response) = ui.allocate_exact_size(vec2(ui.available_width(), 0.0), Sense::hover());
            response.widget_info(|| WidgetInfo::labeled(Role::Dialog, true, tr!("Command palette")));
            ui.add_space(s.xs);
            let typed = field::search(ui, theme, field_id(), palette.query_mut(), tr!("Type a command"), ui.available_width());
            ui.ctx().accesskit_node_builder(typed.id, |node| node.set_label(tr!("Command")));
            if !typed.has_focus() {
                typed.request_focus();
            }
            if typed.changed() {
                let cut: String = palette.query().chars().take(MAX_CHARS).collect();
                *palette.query_mut() = cut;
                asked = asked.or(Some(Asked::Typed));
            }
            ui.add_space(s.xs);
            let edge = ui.cursor().top();
            ui.painter().hline(ui.max_rect().x_range(), edge, egui::Stroke::new(1.0, p.line));
            ui.add_space(s.xs);
            if matches.is_empty() {
                let none = tr!("No command matches “{text}”.", text = palette.query());
                ui.label(egui::RichText::new(none).font(theme.fonts.body.id()).color(p.muted));
            } else {
                let height = control_side(ui, theme) * ROWS;
                ScrollArea::vertical().max_height(height).auto_shrink([false, true]).show(ui, |ui| {
                    for (index, action) in matches.iter().enumerate() {
                        if row(ui, theme, keymap, action, index == palette.picked(), can(action)).clicked() {
                            asked = Some(Asked::Run(action.id));
                        }
                    }
                });
            }
            ui.add_space(s.xs);
            ui.horizontal(|ui| {
                ui.add_space(s.s);
                ui.label(egui::RichText::new(tr!("↑↓ to move · Enter to run · Esc to close")).font(theme.fonts.caption.id()).color(p.muted));
            });
            ui.add_space(s.xs);
        });
        let rect = inner.response.rect;
        let mut shapes = glass::float_shapes(ui.painter(), theme, rect, Under::Fading);
        shapes.push(Shape::rect_filled(rect.shrink(1.0), s.card, p.surface));
        ui.painter().set(slot, Shape::Vec(shapes));
    });
    asked
}
