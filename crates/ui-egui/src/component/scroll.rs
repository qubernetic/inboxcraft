// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scroll bar: a strip 12 px wide beside what scrolls, never over it, so a thumb never covers a row or its focus
//! edge. The mail list is the one exception ([`show_viewport_over`]): its rows keep the design's 440 whether they
//! scroll or not, so its bar lies over their right edge, and a focus edge the thumb passes is painted over the thumb. At rest it shows only a 4 px `muted` thumb in its middle; under the pointer the strip takes the `stage` fill
//! and the thumb grows to 8 px, 2 from each side; while dragged the thumb is `ink`. Square, no fade, no movement of
//! its own: the presentation gives it none.
//!
//! egui keeps the scrolling and the bar's pointer ([`style`] makes it paint nothing of its own); [`show`] and
//! [`show_viewport`] show an area and paint its bar as the sheet has it ([`bar`]).

use egui::scroll_area::ScrollAreaOutput;
use egui::style::{ScrollFadeStyle, ScrollStyle};
use egui::{Margin, Painter, Rangef, Rect, ScrollArea, Ui};

use crate::theme::Theme;

/// The shortest a thumb gets, in points: the sheet's thumb.
const THUMB_LEAST: f32 = 32.0;

/// egui's scroll style for `theme`: a bar [`Sizes::scrollbar`](crate::theme::Sizes) wide, allocated beside the
/// content, as wide at rest as under the pointer, whose own paint egui leaves out (every opacity 0: [`bar`] paints
/// it), and no fade over the content's ends.
#[must_use]
pub fn style(theme: &Theme) -> ScrollStyle {
    let width = theme.sizes.scrollbar;
    ScrollStyle {
        floating: true,
        content_margin: Margin::ZERO,
        bar_width: width,
        handle_min_length: THUMB_LEAST,
        bar_inner_margin: 0.0,
        bar_outer_margin: 0.0,
        floating_width: width,
        floating_allocated_width: width,
        foreground_color: false,
        dormant_background_opacity: 0.0,
        active_background_opacity: 0.0,
        interact_background_opacity: 0.0,
        dormant_handle_opacity: 0.0,
        active_handle_opacity: 0.0,
        interact_handle_opacity: 0.0,
        fade: ScrollFadeStyle { strength: 0.0, ..ScrollFadeStyle::default() },
        ..ScrollStyle::solid()
    }
}

/// What the pointer does to a scroll bar, as its sheet names the states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bar {
    /// Nothing on it.
    Rest,
    /// The pointer over the strip.
    Hover,
    /// The thumb held and moved.
    Dragging,
}

/// Paints a scroll bar over `strip` (12 wide) with its thumb over `thumb` (a range down the strip), in `bar`'s state;
/// every edge on whole pixels.
pub fn paint(painter: &Painter, theme: &Theme, strip: Rect, thumb: Rangef, bar: Bar) {
    let p = &theme.palette;
    let ppp = painter.pixels_per_point();
    let whole = |at: f32| if ppp > 0.0 && ppp.is_finite() { (at * ppp).round() / ppp } else { at.round() };
    let strip = Rect::from_min_max(egui::pos2(whole(strip.left()), whole(strip.top())), egui::pos2(whole(strip.right()), whole(strip.bottom())));
    let (inset, ink) = match bar {
        Bar::Rest => (theme.spacing.s1, p.muted),
        Bar::Hover => (theme.spacing.s1 / 2.0, p.muted),
        Bar::Dragging => (theme.spacing.s1 / 2.0, p.ink),
    };
    if bar != Bar::Rest {
        painter.rect_filled(strip, 0.0, p.stage);
    }
    let across = Rangef::new(strip.left() + inset, strip.right() - inset);
    let down = Rangef::new(whole(thumb.min).max(strip.top()), whole(thumb.max).min(strip.bottom()));
    if across.span() > 0.0 && down.span() > 0.0 {
        painter.rect_filled(Rect::from_x_y_ranges(across, down), 0.0, ink);
    }
}

/// Paints the vertical bar of the area `output` came from, as egui placed it this frame (nothing when it showed
/// none, or showed it over the content): the strip egui senses, the thumb where egui's drag puts it. Only the
/// vertical bar: a horizontal one is not painted.
pub fn bar<R>(ui: &Ui, theme: &Theme, output: &ScrollAreaOutput<R>) {
    paint_bar(ui, theme, output);
}

/// [`bar`]: beside the content.
fn paint_bar<R>(ui: &Ui, theme: &Theme, output: &ScrollAreaOutput<R>) {
    let id = output.id.with(1);
    let Some(strip) = ui.ctx().viewport(|viewport| viewport.this_pass.widgets.get(id).map(|widget| widget.rect)) else { return };
    // egui starts showing a bar a frame before it gives it room beside the content: until the strip stands clear of
    // the content, nothing is painted, so no frame has the bar over a row.
    if strip.left() < output.inner_rect.right() - 0.01 {
        return;
    }
    let (inner, content) = (output.inner_rect.height(), output.content_size.y);
    let length = strip.height();
    if !(content > inner && inner > 0.0 && length > 0.0 && content.is_finite()) {
        return;
    }
    let size = (inner / content * length).max(ui.spacing().scroll.handle_min_length).min(length);
    let most = content - inner;
    let share = (output.state.clamped_offset().y / most).clamp(0.0, 1.0);
    let top = strip.top() + share * (length - size);
    let response = ui.ctx().read_response(id);
    let state = match response {
        Some(response) if response.dragged() || response.is_pointer_button_down_on() => Bar::Dragging,
        Some(response) if response.hovered() => Bar::Hover,
        _ => Bar::Rest,
    };
    paint(ui.painter(), theme, strip, Rangef::new(top, top + size), state);
}

/// Shows `area` in `ui` with `add` drawing its content, and paints its bar.
pub fn show<R>(ui: &mut Ui, theme: &Theme, area: ScrollArea, add: impl FnOnce(&mut Ui) -> R) -> ScrollAreaOutput<R> {
    let output = area.show(ui, add);
    bar(ui, theme, &output);
    output
}

/// Shows `area` in `ui` with `add` drawing what of the content meets the viewport it is given (the content's part in
/// sight, from the content's top), and paints its bar: the list that builds only the lines in sight.
pub fn show_viewport<R>(ui: &mut Ui, theme: &Theme, area: ScrollArea, add: impl FnOnce(&mut Ui, Rect) -> R) -> ScrollAreaOutput<R> {
    let output = area.show_viewport(ui, add);
    bar(ui, theme, &output);
    output
}

/// [`show_viewport`] with the bar over the content's right edge, not beside it: the content keeps the area's whole
/// width whether it scrolls or not (the mail list's 440 rows). egui shows no bar of its own here: only the thumb takes
/// the pointer, so a press in the strip beside it reaches the row under it ([`over_bar`]). A row's focus edge the
/// thumb passes is painted again over the thumb ([`super::row::Row::finish`]), so the thumb never covers it.
pub fn show_viewport_over<R>(ui: &mut Ui, theme: &Theme, area: ScrollArea, add: impl FnOnce(&mut Ui, Rect) -> R) -> ScrollAreaOutput<R> {
    super::row::forget_focus_edge(ui.ctx());
    // Where a drag of the thumb put the content in the last frame.
    let dragged = ui.scope_id().with("scroll-over-dragged");
    let mut area = area.scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
    if let Some(offset) = ui.data_mut(|data| data.remove_temp::<f32>(dragged)) {
        area = area.vertical_scroll_offset(offset);
    }
    let output = area.show_viewport(ui, add);
    if let Some(offset) = over_bar(ui, theme, &output) {
        ui.data_mut(|data| data.insert_temp(dragged, offset));
    }
    super::row::repaint_focus_edge(ui, theme, output.inner_rect);
    output
}

/// The bar of [`show_viewport_over`], over the content's right 12: the thumb alone senses the pointer, registered
/// after the content so it wins over a row there, and a drag on it moves the content as egui's bar does; the strip
/// shows its hover while the pointer is over it. Nothing when the content fits. The offset a drag asks for, which the
/// next frame takes.
fn over_bar<R>(ui: &Ui, theme: &Theme, output: &ScrollAreaOutput<R>) -> Option<f32> {
    let inner = output.inner_rect;
    let (length, content) = (inner.height(), output.content_size.y);
    if !(content > length && length > 0.0 && content.is_finite()) {
        return None;
    }
    let strip = Rect::from_x_y_ranges(Rangef::new(inner.right() - theme.sizes.scrollbar, inner.right()), inner.y_range());
    let size = (length / content * length).max(ui.spacing().scroll.handle_min_length).min(length);
    let (most, travel) = (content - length, length - size);
    let offset = output.state.clamped_offset().y;
    let share = (offset / most).clamp(0.0, 1.0);
    let top = strip.top() + share * travel;
    let thumb = Rect::from_x_y_ranges(strip.x_range(), Rangef::new(top, top + size));
    let response = ui.interact(thumb, output.id.with("over-thumb"), egui::Sense::drag());
    let mut asked = None;
    if response.dragged() && travel > 0.0 {
        let moved = offset + response.drag_delta().y * most / travel;
        if moved.is_finite() && moved != offset {
            asked = Some(moved.clamp(0.0, most));
            ui.ctx().request_repaint();
        }
    }
    let over_strip = ui.ctx().pointer_hover_pos().is_some_and(|at| strip.contains(at));
    let bar = if response.dragged() || response.is_pointer_button_down_on() {
        Bar::Dragging
    } else if response.hovered() || over_strip {
        Bar::Hover
    } else {
        Bar::Rest
    };
    paint(ui.painter(), theme, strip, Rangef::new(top, top + size), bar);
    asked
}

#[cfg(test)]
#[path = "scroll_tests.rs"]
mod tests;
