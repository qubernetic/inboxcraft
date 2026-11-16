// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The select: a closed field with the current choice and a chevron that opens a popup of the choices, for one of
//! many made seldom. The popup sits on the solid float, over the field, the current choice marked and holding the
//! keyboard focus; the arrow keys walk the choices, Enter takes one. A press beside it or Escape closes it. The caller
//! keeps whether it is open, so that a form can close it with itself.

use egui::{Id, Key, Modifiers, Rect, Response, Role, ScrollArea, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::iconset::{IconId, Style};
use crate::motion;
use crate::theme::Theme;

use super::{cut, line_height, popup};

/// The widest the popup gets, in points; narrower beside a narrower field.
const POPUP: f32 = 210.0;

/// The side of the chevron, in points.
const CARET: f32 = 14.0;

/// How much of the window the popup's rows may take before they scroll.
const MOST: f32 = 0.6;

/// The room to the left and right of the text, in points.
const SIDES: f32 = 10.0;

/// A field's height: a line of body text and the design's room above and below; the event editor's text fields match
/// it.
#[must_use]
pub fn height(ui: &Ui, theme: &Theme) -> f32 {
    line_height(ui, theme.fonts.body) + 18.0
}

/// What a select shows and offers.
#[derive(Debug, Clone, Copy)]
pub struct Select<'a, T> {
    /// What it is for: its name for a screen reader.
    pub name: &'a str,
    /// The current choice in words; shown even when it is none of the `options`.
    pub shown: &'a str,
    /// The choices and their names.
    pub options: &'a [(T, &'a str)],
    /// The choice now, when it is one of the `options`.
    pub current: Option<T>,
    /// The icon of the rows that are not the current choice.
    pub icon: IconId,
    /// The closed field's width, in points.
    pub width: f32,
}

impl<T: Copy + PartialEq> Select<'_, T> {
    /// Draws the field and, while `open`, the popup; the field's response and the choice just taken.
    pub fn show(self, ui: &mut Ui, theme: &Theme, open: &mut bool) -> (Response, Option<T>) {
        let high = height(ui, theme);
        let width = if self.width.is_finite() { self.width.clamp(high, 2048.0) } else { high };
        let (rect, _) = ui.allocate_exact_size(vec2(width, high), Sense::hover());
        let id = ui.make_persistent_id(("select", self.name));
        let response = ui.interact(rect, id, Sense::click());
        response
            .widget_info(|| WidgetInfo { current_text_value: Some(self.shown.to_owned()), ..WidgetInfo::labeled(Role::ComboBox, true, self.name) });
        if response.clicked() {
            *open = !*open;
            if *open {
                ui.data_mut(|data| data.insert_temp(id.with("opened"), ()));
            }
        }
        self.paint_field(ui, theme, &response);
        if !*open {
            return (response, None);
        }
        let opened_now = ui.data_mut(|data| data.remove_temp::<()>(id.with("opened"))).is_some();
        let (inside, chosen) = self.popup(ui, theme, id, rect, opened_now);
        let (escaped, beside) = ui.input_mut(|input| {
            let beside = input.pointer.primary_pressed() && input.pointer.interact_pos().is_some_and(|at| !inside.contains(at) && !rect.contains(at));
            (input.consume_key(Modifiers::NONE, Key::Escape), beside)
        });
        if escaped || chosen.is_some() {
            *open = false;
            response.request_focus();
        } else if beside {
            *open = false;
        }
        (response, chosen)
    }

    fn paint_field(&self, ui: &Ui, theme: &Theme, response: &Response) {
        let p = &theme.palette;
        let s = &theme.spacing;
        let rect = response.rect;
        if !ui.is_rect_visible(rect) {
            return;
        }
        let painter = ui.painter();
        painter.rect_filled(rect, s.corner, p.field);
        let hover = motion::toggle(ui.ctx(), response.id.with("hover"), response.hovered(), theme.motion.hover_fade);
        if hover > 0.0 {
            painter.rect_filled(rect, s.corner, p.hover.gamma_multiply(hover));
        }
        let edge = if response.has_focus() { Stroke::new(2.0, p.focus) } else { Stroke::new(1.0, p.field_border) };
        painter.rect_stroke(rect, s.corner, edge, StrokeKind::Inside);
        let caret = Rect::from_center_size(pos2(rect.right() - SIDES - CARET / 2.0, rect.center().y), vec2(CARET, CARET));
        theme.icons.paint(painter, caret, IconId::Caret, Style::Regular, p.muted);
        let text = cut(ui, self.shown, theme.fonts.body, p.text, caret.left() - rect.left() - SIDES * 2.0);
        painter.galley(pos2(rect.left() + SIDES, rect.center().y - text.size().y / 2.0), text, p.text);
    }

    /// The popup over the field at `rect`; where it is and the choice taken in it. `opened_now` gives the current
    /// choice, or else the first, the keyboard focus.
    fn popup(&self, ui: &Ui, theme: &Theme, id: Id, rect: Rect, opened_now: bool) -> (Rect, Option<T>) {
        let ctx = ui.ctx().clone();
        let anchor = rect.left_top() + vec2(0.0, theme.spacing.s);
        let most = (ctx.content_rect().height() * MOST).max(rect.height());
        popup::show(&ctx, theme, id.with("popup"), anchor, POPUP.min(rect.width()), |ui| {
            let mut taken = None;
            let scrolled = ScrollArea::vertical().id_salt("select-rows").max_height(most).show(ui, |ui| {
                for (index, (value, name)) in self.options.iter().enumerate() {
                    let chosen = self.current == Some(*value);
                    let row = popup::option(ui, theme, self.icon, name, chosen);
                    if opened_now && (chosen || (self.current.is_none() && index == 0)) {
                        row.request_focus();
                    }
                    if row.clicked() {
                        taken = Some(*value);
                    }
                }
            });
            (scrolled.inner_rect.union(ui.min_rect()), taken)
        })
    }
}

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;
