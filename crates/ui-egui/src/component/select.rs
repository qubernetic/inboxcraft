// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The select: a closed field with the current choice and a caret (▾, drawn as a shape, 8 × 5) that opens a popup of
//! the choices, for one of many made seldom. 36 high like a text field, so the two line up side by side: the `term`
//! fill, a 1 px `line` edge, the choice in mono 13 cut with an ellipsis, the caret `muted`. Hovered: a `muted` edge and
//! an `ink` caret; pressed: the `stage` fill with an `ink` edge and caret; open: a `cyan_edge` edge and ▴ in
//! `cyan_text`. Focus is the ring 2 px clear. Disabled: a dashed `line` edge, no fill, the choice `muted` and the caret
//! `dim`; unavailable looks disabled and its tooltip says why. Nothing in it moves (the design gives it no transition).
//!
//! The popup opens under the field, on the solid float, the current choice marked and holding the keyboard focus; the
//! arrow keys walk the choices, Enter takes one. A press beside it or Escape closes it. The caller keeps whether it is
//! open, so that a form can close it with itself.

use egui::{Color32, Id, Key, Rect, Response, Role, ScrollArea, Sense, Shape, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint::{self, Ring};
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::theme::Theme;

use super::{Hover, Lit, Shown, cut, lit, popup, scroll, solid, soon, tooltip};

/// The widest the popup gets, in points; narrower beside a narrower field.
const POPUP: f32 = 210.0;

/// The caret's width and height, in points.
const CARET: [f32; 2] = [8.0, 5.0];

/// How much of the window the popup's rows may take before they scroll.
const MOST: f32 = 0.6;

/// The room left and right inside the field, in points.
const PAD: f32 = 12.0;

/// Between the choice and the caret, in points.
const GAP: f32 = 8.0;

/// A field's height: a line of body text and the design's room above and below; the event editor's text fields match
/// it.
#[must_use]
pub fn height(theme: &Theme) -> f32 {
    theme.boxes().field
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
        self.show_with(ui, theme, open, None, None)
    }

    /// [`Self::show`], unavailable when there is a `note`: drawn disabled, never opening, its tooltip saying why.
    pub fn show_noted(self, ui: &mut Ui, theme: &Theme, open: &mut bool, note: Option<&str>) -> (Response, Option<T>) {
        self.show_with(ui, theme, open, note, None)
    }

    /// [`Self::show_noted`] drawn in `shown` whatever the pointer and the keyboard do: a board of the gallery.
    pub(crate) fn show_with(self, ui: &mut Ui, theme: &Theme, open: &mut bool, note: Option<&str>, shown: Option<Shown>) -> (Response, Option<T>) {
        self.draw(ui, theme, open, note, shown, true)
    }

    /// The closed field drawn as if it were `open`, without its popup: the open state on a board of the gallery.
    pub(crate) fn field_open(self, ui: &mut Ui, theme: &Theme) -> Response {
        self.draw(ui, theme, &mut true, None, None, false).0
    }

    /// [`Self::show_with`], the popup drawn only with `list`.
    fn draw(self, ui: &mut Ui, theme: &Theme, open: &mut bool, note: Option<&str>, shown: Option<Shown>, list: bool) -> (Response, Option<T>) {
        let high = height(theme);
        let width = if self.width.is_finite() { self.width.max(high).min(2048.0) } else { high };
        let (rect, _) = ui.allocate_exact_size(vec2(width, high), Sense::hover());
        let id = ui.make_persistent_id(("select", self.name));
        // Disabled, unavailable or coming soon, it never opens.
        let enabled = ui.is_enabled() && !soon::is_soon(ui) && note.is_none();
        if !enabled {
            *open = false;
        }
        let response = ui.interact(rect, id, if enabled { Sense::click() } else { Sense::hover() });
        response.widget_info(|| WidgetInfo {
            current_text_value: Some(self.shown.to_owned()),
            ..WidgetInfo::labeled(Role::ComboBox, enabled, self.name)
        });
        // Enter and Space open it when they come up, so that the field shows itself pressed while they are down; a press
        // of the pointer or a screen reader's opens it at once. Only a key that went down on the closed field opens it:
        // the key that takes a choice in the list goes down on a row, and comes up after the focus is back here.
        let keys = [Key::Enter, Key::Space];
        let (went_down, held, released) = ui.input(|input| {
            // A key held down repeats its press; only the first goes down, so a key held on from a row never opens it.
            let fresh = input.events.iter().any(|event| {
                matches!(event, egui::Event::Key { key, pressed: true, repeat: false, .. } if keys.contains(key))
            });
            (
                fresh,
                keys.iter().any(|key| input.key_down(*key)),
                keys.iter().any(|key| input.key_released(*key)),
            )
        });
        let armed = id.with("key-down");
        let focused = response.has_focus() && !*open;
        if focused && went_down {
            ui.data_mut(|data| data.insert_temp(armed, ()));
        }
        let was_armed = if !focused || released || !held { ui.data_mut(|data| data.remove_temp::<()>(armed)).is_some() } else { false };
        // A press that closed the float (it began outside it, on the field) does not open it again when it comes up.
        let closing = id.with("closing-press");
        let stale = ui.data(|data| data.get_temp::<()>(closing)).is_some();
        if stale && (response.clicked() || !ui.input(|input| input.pointer.any_down())) {
            ui.data_mut(|data| data.remove::<()>(closing));
        }
        let pressed = (response.clicked() && !held && !stale) || (focused && released && was_armed);
        if enabled && pressed {
            *open = !*open;
            if *open {
                ui.data_mut(|data| data.insert_temp(id.with("opened"), ()));
            }
        }
        let refocus = id.with("refocus");
        if let Some(left) = ui.data(|data| data.get_temp::<u8>(refocus)) {
            if response.has_focus() || left == 0 || *open {
                ui.data_mut(|data| data.remove::<u8>(refocus));
            } else {
                response.request_focus();
                ui.data_mut(|data| data.insert_temp(refocus, left.saturating_sub(1)));
            }
        }
        let lit = lit(ui, theme, &response, enabled, shown, Hover::AtOnce);
        self.paint_field(ui, theme, &response, enabled, *open, lit);
        let response = match note {
            Some(note) => tooltip::attach_note(response, theme, &Keymap::empty(), self.name, None, Some(note)),
            None => response,
        };
        if !*open || !list {
            return (response, None);
        }
        let opened_now = ui.data_mut(|data| data.remove_temp::<()>(id.with("opened"))).is_some();
        // The float closes itself on Esc and on a press outside it, and gives the field the keyboard back.
        match self.popup(ui, theme, id, rect, opened_now) {
            None => {
                *open = false;
                if ui.input(|input| input.pointer.any_down()) {
                    ui.data_mut(|data| data.insert_temp(id.with("closing-press"), ()));
                }
            }
            Some(Some(chosen)) => {
                *open = false;
                response.request_focus();
                // The float held the keyboard this frame and the next: the field asks again until it has it.
                ui.data_mut(|data| data.insert_temp(id.with("refocus"), 3_u8));
                return (response, Some(chosen));
            }
            Some(None) => {}
        }
        (response, None)
    }

    fn paint_field(&self, ui: &Ui, theme: &Theme, response: &Response, enabled: bool, open: bool, lit: Lit) {
        let p = &theme.palette;
        let rect = response.rect;
        if !ui.is_rect_visible(rect) {
            return;
        }
        let painter = solid(ui);
        let (fill, edge, value, mark) = if !enabled {
            (Color32::TRANSPARENT, None, p.muted, p.dim)
        } else if open {
            (p.term, Some(p.cyan_edge), p.ink, p.cyan_text)
        } else if lit.pressed {
            (p.stage, Some(p.ink), p.ink, p.ink)
        } else if lit.hover {
            (p.term, Some(p.muted), p.ink, p.ink)
        } else {
            (p.term, Some(p.line), p.ink, p.muted)
        };
        match edge {
            Some(colour) => {
                painter.rect(rect, 0.0, fill, Stroke::new(1.0, colour), StrokeKind::Inside);
            }
            None => paint::dashed(&painter, rect, p.line),
        }
        let caret = Rect::from_min_size(pos2(rect.right() - PAD - CARET[0], rect.center().y - CARET[1] / 2.0), vec2(CARET[0], CARET[1]));
        // ▾ closed, ▴ open.
        let points = if open {
            vec![caret.left_bottom(), caret.right_bottom(), caret.center_top()]
        } else {
            vec![caret.left_top(), caret.right_top(), caret.center_bottom()]
        };
        painter.add(Shape::convex_polygon(points, mark, Stroke::NONE));
        let text = cut(ui, self.shown, theme.fonts.mono, value, caret.left() - GAP - rect.left() - PAD);
        painter.galley(pos2(rect.left() + PAD, rect.center().y - text.size().y / 2.0), text, value);
        if lit.focus {
            paint::focus_ring(&painter, theme, rect, Ring::Outside);
        }
    }

    /// The float under the field at `rect`: the choice taken in it, or `None` once it closed itself. `opened_now`
    /// gives the current choice, or else the first, the keyboard focus.
    fn popup(&self, ui: &Ui, theme: &Theme, id: Id, rect: Rect, opened_now: bool) -> Option<Option<T>> {
        let ctx = ui.ctx().clone();
        let most = (ctx.content_rect().height() * MOST).max(rect.height());
        let float = popup::Float { id: id.with("popup"), anchor: rect, width: POPUP.min(rect.width()), returns_to: id };
        popup::float(&ctx, theme, &float, |ui| {
            let mut taken = None;
            let area = ScrollArea::vertical().id_salt("select-rows").max_height(most);
            scroll::show(ui, theme, area, |ui| {
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
            taken
        })
    }
}

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;
