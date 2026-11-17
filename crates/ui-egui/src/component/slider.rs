// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The text size slider: a hairline track with five ticks, the sizes 90 % to 130 % named under them, and a 9 × 17
//! handle that rests on a tick only. The size in use is named in `ink`, the others in `muted`. Hovered, the handle is
//! the accent (`cyan_edge`); held (by the pointer, or by Enter or Space while it has the focus) it grows to 25 high in
//! the accent and the size it will take is named in `cyan_text`, the size is set when it is let go. A press beside the
//! handle sets the size under it. Focus is the ring round the handle, and the arrow keys move it one tick. Disabled:
//! the ticks and the handle `dim`, every size `muted`. The track and its ticks keep their size at every text size, the
//! names grow with the text. Nothing slides: the handle jumps from tick to tick (the design gives it no transition).

use egui::{EventFilter, Key, Pos2, Rect, Sense, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint::{self, Ring};
use crate::theme::{TextScale, Theme};
use crate::tr;

use super::{Hover, Shown, galley, lit, solid, soon};

/// The slider's width, in points.
const WIDTH: f32 = 240.0;

/// From the left edge to the first tick, in points.
const FIRST: f32 = 20.0;

/// From one tick to the next, in points.
const STEP: f32 = 50.0;

/// The track's height from the top, in points.
const TRACK_Y: f32 = 12.0;

/// A tick's top and height, in points.
const TICK: [f32; 2] = [8.0, 9.0];

/// The handle's width, its top and height at rest, its top and height held, in points.
const HANDLE: [f32; 5] = [9.0, 4.0, 17.0, 0.0, 25.0];

/// The top of the sizes' names, in points. The sheet gives each a 40 wide slot at 100 %; the names are never cut, and
/// at 120 % and 130 % they are wider than that slot.
const NAMES: f32 = 28.0;

/// The tick of `index` from the left edge of `rect`: its left side.
fn tick(rect: Rect, index: usize) -> f32 {
    // Five ticks: the cast is of a small index.
    rect.left() + FIRST + STEP * index as f32
}

/// The tick nearest `at` within `rect`.
fn nearest(rect: Rect, at: Pos2) -> usize {
    let last = TextScale::ALL.len().saturating_sub(1);
    // A share of the track rounded to a tick; the cast is of a small, clamped number.
    ((at.x - rect.left() - FIRST) / STEP).round().clamp(0.0, last as f32) as usize
}

/// The text size slider at `current`; the size chosen, when one is.
pub fn text_size(ui: &mut Ui, theme: &Theme, current: TextScale) -> Option<TextScale> {
    text_size_with(ui, theme, current, None, None)
}

/// [`text_size`] drawn in `shown` whatever the pointer and the keyboard do, held `towards` a size when it is
/// [`Shown::Pressed`]: a board of the gallery.
pub(crate) fn text_size_with(ui: &mut Ui, theme: &Theme, current: TextScale, shown: Option<Shown>, towards: Option<TextScale>) -> Option<TextScale> {
    let enabled = ui.is_enabled() && !soon::is_soon(ui);
    let names = theme.fonts.mono_s;
    let size = vec2(WIDTH, NAMES + names.line.ceil());
    let sense = if enabled { Sense::click_and_drag() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let name = tr!("Text size");
    response.widget_info(|| WidgetInfo::slider(enabled, f64::from(current.percent()), name));
    let now = TextScale::ALL.iter().position(|scale| *scale == current).unwrap_or(1);
    let last = TextScale::ALL.len().saturating_sub(1);
    let lit = lit(ui, theme, &response, enabled, shown, Hover::AtOnce);
    // The tick a held handle stands on: under the pointer, or where it is when a key holds it.
    let pointer = response.interact_pointer_pos().or_else(|| ui.input(|input| input.pointer.latest_pos()));
    let held = match (shown, towards) {
        (Some(Shown::Pressed), Some(scale)) => TextScale::ALL.iter().position(|step| *step == scale),
        (Some(Shown::Pressed), None) => Some(now),
        (Some(_), _) => None,
        (None, _) if enabled && response.is_pointer_button_down_on() => pointer.map(|at| nearest(rect, at)),
        (None, _) if lit.pressed => Some(now),
        (None, _) => None,
    };
    let mut wanted = None;
    if enabled && (response.clicked() || response.drag_stopped()) && response.interact_pointer_pos().is_some() {
        wanted = pointer.map(|at| nearest(rect, at));
        // A press takes the focus, so the arrow keys go on from where it set the size.
        response.request_focus();
    }
    if enabled && response.has_focus() {
        ui.memory_mut(|memory| memory.set_focus_lock_filter(response.id, EventFilter { horizontal_arrows: true, ..EventFilter::default() }));
        let (back, on) = ui.input(|input| (input.key_pressed(Key::ArrowLeft), input.key_pressed(Key::ArrowRight)));
        if back {
            wanted = Some(now.saturating_sub(1));
        } else if on {
            wanted = Some((now + 1).min(last));
        }
    }
    if ui.is_rect_visible(rect) {
        paint_slider(ui, theme, rect, (now, held), enabled, lit);
    }
    wanted.filter(|index| *index != now).and_then(|index| TextScale::ALL.get(index).copied())
}

/// Paints the slider over `rect`: at the tick `now`, held on the tick `held` if it is.
fn paint_slider(ui: &Ui, theme: &Theme, rect: Rect, (now, held): (usize, Option<usize>), enabled: bool, lit: super::Lit) {
    let p = &theme.palette;
    let painter = solid(ui);
    // Whole pixels whatever the slider's origin: every block's corner is rounded to the pixel grid, and its size to
    // whole pixels, at least one.
    let ppp = ui.ctx().pixels_per_point().max(0.1);
    let on_grid = |value: f32| (value * ppp).round() / ppp;
    let block = |left: f32, top: f32, wide: f32, high: f32| {
        let size = |points: f32| (points * ppp).round().max(1.0) / ppp;
        Rect::from_min_size(pos2(on_grid(left), on_grid(top)), vec2(size(wide), size(high)))
    };
    let ticks = if enabled { p.muted } else { p.dim };
    let last = TextScale::ALL.len().saturating_sub(1);
    painter.rect_filled(block(tick(rect, 0), rect.top() + TRACK_Y, tick(rect, last) - tick(rect, 0) + 1.0, 1.0), 0.0, p.line);
    for (index, scale) in TextScale::ALL.iter().enumerate() {
        let x = tick(rect, index);
        let mark = block(x, rect.top() + TICK[0], 1.0, TICK[1]);
        painter.rect_filled(mark, 0.0, ticks);
        let colour = match (enabled, held) {
            (false, _) => p.muted,
            (true, Some(at)) if at == index => p.cyan_text,
            (true, Some(_)) => p.muted,
            (true, None) if index == now => p.ink,
            (true, None) => p.muted,
        };
        // Never cut: centred on its tick, kept inside the slider at the ends. Ticks are 50 apart and the widest name is
        // about 43 at 130 %, so two never meet.
        let text = galley(ui, &tr!("{percent} %", percent = scale.percent()), theme.fonts.mono_s, colour);
        let wide = text.size().x;
        let left = (mark.center().x - wide / 2.0).clamp(rect.left(), (rect.right() - wide).max(rect.left()));
        painter.galley(pos2(on_grid(left), rect.top() + NAMES), text, colour);
    }
    let (at, top, high) = match held {
        Some(at) => (at, HANDLE[3], HANDLE[4]),
        None => (now, HANDLE[1], HANDLE[2]),
    };
    let colour = match (enabled, held.is_some() || lit.hover) {
        (false, _) => p.dim,
        (true, true) => p.cyan_edge,
        (true, false) => p.ink,
    };
    let handle = block(tick(rect, at) - (HANDLE[0] - 1.0) / 2.0, rect.top() + top, HANDLE[0], high);
    painter.rect_filled(handle, 0.0, colour);
    if lit.focus {
        paint::focus_ring(&painter, theme, handle, Ring::Outside);
    }
}

#[cfg(test)]
#[path = "slider_tests.rs"]
mod tests;
