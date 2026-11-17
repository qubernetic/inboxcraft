// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Segmented choices: a few options side by side, 32 high, on `term` like a field, a 1 px `line` round them and a
//! hairline between two. The chosen option is the `stage` fill, its word in `ink` and a 2 px `cyan_edge` bar at its
//! foot, so it stays the chosen one on a `stage` ground (a hovered row). The others are `muted`; hovered, the word turns
//! `ink` and takes no fill (a fill says chosen); pressed, the option is the accent wash with its word in `cyan_text`.
//! The control is one stop for Tab with one ring round it, and the arrow keys move the choice. Disabled: a dashed edge,
//! no `term` ground, every word `muted`, the chosen option keeping its fill with its bar in `muted`. An option's name is
//! cut with an ellipsis at 136; where the room is too small the options share it equally. Each option is a node of its
//! own for a screen reader, selected or not; nothing moves (the design gives it no transition).

use egui::{Color32, EventFilter, Key, Rect, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint::{self, Ring};
use crate::theme::{Font, Theme};
use crate::tr;

use super::{Hover, Shown, cut, galley, lit, solid, soon};

/// The room left and right of an option's name, in points.
const PAD: f32 = 12.0;

/// The widest an option's name gets before it ends with an ellipsis, in points.
const WIDEST: f32 = 136.0;

/// The height of the foot bar under the chosen option, in points.
const BAR: f32 = 2.0;

/// What the track takes around and between its `count` options: its 1 px edge on each side and a hairline between two.
fn chrome(count: usize) -> f32 {
    2.0 + count.saturating_sub(1) as f32
}

/// An option's width at its full name, cut at [`WIDEST`], on whole points.
fn natural<T>(ui: &Ui, theme: &Theme, (_, name, font): &(T, &str, Font)) -> f32 {
    galley(ui, name, *font, theme.palette.ink).size().x.min(WIDEST).ceil() + PAD * 2.0
}

/// The width the options take with their full names, in points.
#[must_use]
pub fn width<T>(ui: &Ui, theme: &Theme, options: &[(T, &str, Font)]) -> f32 {
    options.iter().map(|option| natural(ui, theme, option)).sum::<f32>() + chrome(options.len())
}

/// The `options`, `current` on the selection, each name in its own font; the option pressed or moved to.
pub fn segmented<T: Copy + PartialEq>(ui: &mut Ui, theme: &Theme, options: &[(T, &str, Font)], current: T) -> Option<T> {
    segmented_with(ui, theme, options, current, None, None)
}

/// [`segmented`], each option spoken as "<option> for `about`" where several rows offer the same options.
pub fn segmented_for<T: Copy + PartialEq>(ui: &mut Ui, theme: &Theme, options: &[(T, &str, Font)], current: T, about: Option<&str>) -> Option<T> {
    segmented_with(ui, theme, options, current, about, None)
}

/// [`segmented_for`] with the option at the index of `shown` drawn hovered or pressed, or the control focused, whatever
/// the pointer and the keyboard do: a board of the gallery.
pub(crate) fn segmented_with<T: Copy + PartialEq>(
    ui: &mut Ui,
    theme: &Theme,
    options: &[(T, &str, Font)],
    current: T,
    about: Option<&str>,
    shown: Option<(Shown, usize)>,
) -> Option<T> {
    if options.is_empty() {
        return None;
    }
    let p = &theme.palette;
    let enabled = ui.is_enabled() && !soon::is_soon(ui);
    let room = ui.available_width();
    let full = width(ui, theme, options);
    // Half a point of leeway: a row sized from this very control must not count as too small.
    let squeezed = full > room + 0.5;
    let shared = ((room - chrome(options.len())) / options.len() as f32).floor().max(1.0);
    let high = theme.boxes().button;
    let size = vec2(if squeezed { room.max(1.0) } else { full }, high);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (track, response) = ui.allocate_exact_size(size, sense);
    let names: Vec<&str> = options.iter().map(|(_, name, _)| *name).collect();
    let group = about.map_or_else(|| names.join(" / "), str::to_owned);
    response.widget_info(|| WidgetInfo::labeled(Role::RadioGroup, enabled, &group));
    let now = options.iter().position(|(value, ..)| *value == current);
    // The arrows move the choice, not the focus.
    if response.has_focus() {
        ui.memory_mut(|memory| memory.set_focus_lock_filter(response.id, EventFilter { horizontal_arrows: true, ..EventFilter::default() }));
    }
    let lit = lit(ui, theme, &response, enabled, shown.map(|(shown, _)| shown), Hover::AtOnce);
    // The options' rects, left to right.
    let mut rects = Vec::with_capacity(options.len());
    let mut left = track.left() + 1.0;
    for option in options {
        let wide = if squeezed { shared } else { natural(ui, theme, option) };
        rects.push(Rect::from_min_size(pos2(left, track.top() + 1.0), vec2(wide, high - 2.0)));
        left += wide + 1.0;
    }
    let under = |at: Option<egui::Pos2>| at.and_then(|at| rects.iter().position(|rect| rect.contains(at)));
    let (hovered, pressed) = match shown {
        Some((Shown::Hover, index)) => (Some(index), None),
        Some((Shown::Pressed, index)) => (None, Some(index)),
        Some(_) => (None, None),
        None if lit.pressed && response.is_pointer_button_down_on() => (None, under(ui.input(|input| input.pointer.interact_pos()))),
        None if lit.pressed => (None, now),
        None => (if lit.hover { under(response.hover_pos()) } else { None }, None),
    };
    let mut chosen = None;
    if enabled && response.clicked() {
        chosen = under(response.interact_pointer_pos());
    }
    if enabled && response.has_focus() {
        let (back, on) = ui.input(|input| (input.key_pressed(Key::ArrowLeft), input.key_pressed(Key::ArrowRight)));
        let at = now.unwrap_or(0);
        if back {
            chosen = Some(at.saturating_sub(1));
        } else if on {
            chosen = Some((at + 1).min(options.len() - 1));
        }
    }
    let visible = ui.is_rect_visible(track);
    let painter = solid(ui);
    if visible {
        if enabled {
            painter.rect(track, 0.0, p.term, Stroke::new(1.0, p.line), StrokeKind::Inside);
        } else {
            paint::dashed(&painter, track, p.line);
        }
    }
    for (index, ((value, name, font), rect)) in options.iter().zip(&rects).enumerate() {
        let on = *value == current;
        let id = response.id.with(("segment", index));
        // A node for a screen reader; the track takes the press.
        let segment = ui.interact(*rect, id, Sense::hover());
        let spoken = match about {
            Some(about) => tr!("{option} for {about}", option = name, about = about),
            None => (*name).to_owned(),
        };
        segment.widget_info(|| WidgetInfo::selected(Role::Button, enabled, on, &spoken));
        if !visible {
            continue;
        }
        let pushed = enabled && pressed == Some(index);
        let (fill, ink) = match (enabled, on, pushed) {
            (false, true, _) => (p.stage, p.muted),
            (false, false, _) => (Color32::TRANSPARENT, p.muted),
            (true, _, true) => (p.cyan_muted, if on { p.ink } else { p.cyan_text }),
            (true, true, false) => (p.stage, p.ink),
            (true, false, false) => (Color32::TRANSPARENT, if hovered == Some(index) { p.ink } else { p.muted }),
        };
        if fill != Color32::TRANSPARENT {
            painter.rect_filled(*rect, 0.0, fill);
        }
        if on {
            let bar = if enabled { p.cyan_edge } else { p.muted };
            painter.rect_filled(Rect::from_min_max(pos2(rect.left(), rect.bottom() - BAR), rect.max), 0.0, bar);
        }
        if index + 1 < options.len() {
            painter.rect_filled(Rect::from_min_size(pos2(rect.right(), rect.top()), vec2(1.0, rect.height())), 0.0, p.line);
        }
        let pad = if squeezed { theme.spacing.s1 } else { PAD };
        // A point more than the name needs, so that rounding never cuts a name that fits.
        let text = cut(ui, name, *font, ink, (rect.width() - pad * 2.0 + 1.0).min(WIDEST));
        painter.galley(rect.center() - text.size() / 2.0, text, ink);
    }
    if visible && lit.focus {
        paint::focus_ring(&painter, theme, track, Ring::Outside);
    }
    chosen.filter(|index| Some(*index) != now).and_then(|index| options.get(index)).map(|(value, ..)| *value)
}

#[cfg(test)]
#[path = "segmented_tests.rs"]
mod tests;
