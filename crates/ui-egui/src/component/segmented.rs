// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Segmented choices: a few options side by side in one fully rounded track with a quiet edge, the chosen one on the
//! selection with an edge in the full gradient and in SemiBold. Each option is a button of its own for a screen
//! reader, selected or not. An option keeps its width whichever is chosen. Where the room is too small the options
//! share it equally and their names are cut, so the track never runs out of its place.

use egui::{Rect, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::motion;
use crate::theme::{Face, Font, Theme, Weight};
use crate::tr;

use super::{cut, galley, line_height, pill};

/// From the track's outside to an option: its edge and the room inside it.
const TRACK: f32 = 4.0;

/// Between two options.
const GAP: f32 = 2.0;

/// `font` in the SemiBold weight.
fn semibold(font: Font) -> Font {
    Font { face: Face::Sans(Weight::SemiBold), ..font }
}

/// What the track takes around and between its `count` options.
fn chrome(count: usize) -> f32 {
    TRACK * 2.0 + GAP * count.saturating_sub(1) as f32
}

/// An option's width at its full name: the name in SemiBold, so that it does not change when the option is chosen.
fn natural<T>(ui: &Ui, theme: &Theme, (_, name, font): &(T, &str, Font)) -> f32 {
    galley(ui, name, semibold(*font), theme.palette.text).size().x + theme.spacing.m * 2.0
}

/// The width the options take with their full names, in points.
#[must_use]
pub fn width<T>(ui: &Ui, theme: &Theme, options: &[(T, &str, Font)]) -> f32 {
    options.iter().map(|option| natural(ui, theme, option)).sum::<f32>() + chrome(options.len())
}

/// The `options`, `current` on the selection, each name in its own font; the option pressed.
pub fn segmented<T: Copy + PartialEq>(ui: &mut Ui, theme: &Theme, options: &[(T, &str, Font)], current: T) -> Option<T> {
    segmented_for(ui, theme, options, current, None)
}

/// [`segmented`], each option spoken as "<option> for `about`" where several rows offer the same options.
pub fn segmented_for<T: Copy + PartialEq>(ui: &mut Ui, theme: &Theme, options: &[(T, &str, Font)], current: T, about: Option<&str>) -> Option<T> {
    if options.is_empty() {
        return None;
    }
    let p = &theme.palette;
    let s = &theme.spacing;
    let room = ui.available_width();
    let full = width(ui, theme, options);
    // Half a point of leeway: a row sized from this very control must not count as too small.
    let squeezed = full > room + 0.5;
    let shared = ((room - chrome(options.len())) / options.len() as f32).max(1.0);
    let tallest = options.iter().map(|(_, _, font)| line_height(ui, *font)).fold(0.0, f32::max);
    let high = tallest + s.s + 2.0;
    let size = vec2(if squeezed { room.max(1.0) } else { full }, high + TRACK * 2.0);
    let (track, response) = ui.allocate_exact_size(size, Sense::hover());
    let visible = ui.is_rect_visible(track);
    if visible {
        ui.painter().rect_stroke(track, track.height() / 2.0, Stroke::new(1.0, p.control_edge), StrokeKind::Inside);
    }
    let mut chosen = None;
    let mut left = track.left() + TRACK;
    for (index, option) in options.iter().enumerate() {
        let (value, name, font) = option;
        let on = *value == current;
        let wide = if squeezed { shared } else { natural(ui, theme, option) };
        let rect = Rect::from_min_size(pos2(left, track.top() + TRACK), vec2(wide, high));
        left += wide + GAP;
        let id = response.id.with(("segment", index));
        let segment = ui.interact(rect, id, Sense::click());
        let spoken = match about {
            Some(about) => tr!("{option} for {about}", option = name, about = about),
            None => (*name).to_owned(),
        };
        segment.widget_info(|| WidgetInfo::selected(Role::Button, true, on, &spoken));
        let lit = segment.hovered() || segment.has_focus();
        let hover = motion::toggle(ui.ctx(), id.with("hover"), lit && !on, theme.motion.hover_fade);
        if visible {
            let round = rect.height() / 2.0;
            if on {
                pill::selected_fill(ui.painter(), theme, rect, round);
            } else if hover > 0.0 {
                ui.painter().rect_filled(rect, round, p.hover.gamma_multiply(hover));
            }
            let pad = if squeezed { s.xs } else { s.m };
            // A point more than the name needs, so that rounding never cuts a name that fits.
            let text = cut(ui, name, if on { semibold(*font) } else { *font }, p.text, wide - pad * 2.0 + 1.0);
            ui.painter().galley(rect.center() - text.size() / 2.0, text, p.text);
            if segment.has_focus() {
                ui.painter().rect_stroke(rect.shrink(1.0), round, Stroke::new(2.0, p.focus), StrokeKind::Inside);
            }
        }
        if segment.clicked() {
            chosen = Some(*value);
        }
    }
    chosen
}

#[cfg(test)]
#[path = "segmented_tests.rs"]
mod tests;
