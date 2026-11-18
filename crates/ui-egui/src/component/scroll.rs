// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scroll bar: a strip 12 px wide beside what scrolls, never over it, so a thumb never covers a row or its focus
//! edge. The mail list is the one exception ([`show_viewport_over`]): its rows keep the design's 440 whether they
//! scroll or not, so its bar lies over their right edge, and a focus edge the thumb passes is painted over the thumb. At rest it shows only a 4 px `muted` thumb in its middle; under the pointer the strip takes the `stage` fill
//! and the thumb grows to 8 px, 2 from each side; while dragged the thumb is `ink`. Square, no fade, no movement of
//! its own: the presentation gives it none.
//!
//! egui keeps the scrolling and the bar's pointer ([`style`] makes it paint nothing of its own); [`show`] and
//! [`show_viewport`] show an area and paint its bar as the sheet has it ([`bar`]). [`show_over`], whose bar stands over
//! the content, keeps the pointer itself: only its thumb takes a press.

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
    bar_with_ends(ui, theme, output, 0.0);
}

/// [`bar`], its thumb travelling `ends` clear of the strip's top and bottom (a field's bar on its board: 4).
pub fn bar_with_ends<R>(ui: &Ui, theme: &Theme, output: &ScrollAreaOutput<R>, ends: f32) {
    paint_bar(ui, theme, output, ends, false);
}

/// Where the bar of the area `output` came from stands: the strip egui senses, the thumb's range down it, and how far
/// the content scrolls for a point of the thumb's travel; nothing when it shows no bar. `over` the content when the
/// area gives it no room of its own.
fn thumb_of<R>(ui: &Ui, output: &ScrollAreaOutput<R>, ends: f32, over: bool) -> Option<(Rect, Rangef, f32)> {
    let id = output.id.with(1);
    let strip = ui.ctx().viewport(|viewport| viewport.this_pass.widgets.get(id).map(|widget| widget.rect))?;
    // egui starts showing a bar a frame before it gives it room beside the content: until the strip stands clear of
    // the content, nothing is painted, so no frame has the bar over a row.
    if !over && strip.left() < output.inner_rect.right() - 0.01 {
        return None;
    }
    let (inner, content) = (output.inner_rect.height(), output.content_size.y);
    // max then min, never `clamp`: a bound from a rect's height may be negative or NaN, and `min` and `max` keep the
    // other number then.
    let ends = if ends.is_finite() { ends.max(0.0).min(strip.height().max(0.0) / 4.0) } else { 0.0 };
    let length = strip.height() - ends * 2.0;
    if !(content > inner && inner > 0.0 && length > 0.0 && content.is_finite()) {
        return None;
    }
    let size = (inner / content * length).max(ui.spacing().scroll.handle_min_length).min(length);
    // A thumb kept clear of the ends is a whole number of points long, rounded down: the field's board's 148, not 149.
    let size = if ends > 0.0 { size.floor() } else { size };
    let most = content - inner;
    let share = (output.state.clamped_offset().y / most).clamp(0.0, 1.0);
    let top = strip.top() + ends + share * (length - size);
    let rate = if length - size > 0.0 { most / (length - size) } else { 0.0 };
    Some((strip, Rangef::new(top, top + size), rate))
}

/// Paints the bar of the area `output` came from beside its content.
fn paint_bar<R>(ui: &Ui, theme: &Theme, output: &ScrollAreaOutput<R>, ends: f32, over: bool) {
    let Some((strip, thumb, _)) = thumb_of(ui, output, ends, over) else { return };
    let response = ui.ctx().read_response(output.id.with(1));
    let state = match response {
        Some(response) if response.dragged() || response.is_pointer_button_down_on() => Bar::Dragging,
        Some(response) if response.hovered() => Bar::Hover,
        _ => Bar::Rest,
    };
    paint(ui.painter(), theme, strip, thumb, state);
}

/// Shows `area` in `ui` with `add` drawing its content, and paints its bar.
pub fn show<R>(ui: &mut Ui, theme: &Theme, area: ScrollArea, add: impl FnOnce(&mut Ui) -> R) -> ScrollAreaOutput<R> {
    show_with_ends(ui, theme, area, 0.0, add)
}

/// [`show`], the thumb travelling `ends` clear of the strip's ends ([`bar_with_ends`]).
pub fn show_with_ends<R>(ui: &mut Ui, theme: &Theme, area: ScrollArea, ends: f32, add: impl FnOnce(&mut Ui) -> R) -> ScrollAreaOutput<R> {
    let output = area.show(ui, add);
    bar_with_ends(ui, theme, &output, ends);
    output
}

/// Shows `area` in `ui` with `add` drawing its content as wide as the area, and paints its bar over the content's
/// right edge (a view whose rows run under the bar, as the calendar's agenda and hours). Over the content the strip
/// takes no press of its own: only the thumb (its 8 wide under the pointer, as long as it is) takes one, to be dragged;
/// a press anywhere else in the strip goes to the content under it. The wheel scrolls as everywhere.
pub fn show_over<R>(ui: &mut Ui, theme: &Theme, area: ScrollArea, add: impl FnOnce(&mut Ui) -> R) -> ScrollAreaOutput<R> {
    let ctx = ui.ctx().clone();
    let held = ui.scope_id().with("scroll-over-thumb");
    let mut area = area.scroll_source(egui::scroll_area::ScrollSource { scroll_bar: false, ..egui::scroll_area::ScrollSource::default() });
    // The thumb held since the last frame: the content follows the pointer's move along its travel.
    if ctx.dragged_id() == Some(held)
        && let Some((offset, rate, most)) = ctx.data(|data| data.get_temp::<(f32, f32, f32)>(held))
    {
        let to = offset + ctx.input(|input| input.pointer.delta().y) * rate;
        if to.is_finite() && most.is_finite() {
            area = area.vertical_scroll_offset(to.min(most).max(0.0));
        }
    }
    let output = ui
        .scope(|ui| {
            ui.spacing_mut().scroll.floating_allocated_width = 0.0;
            area.show(ui, add)
        })
        .inner;
    let Some((strip, thumb, rate)) = thumb_of(ui, &output, 0.0, true) else { return output };
    let most = output.content_size.y - output.inner_rect.height();
    ctx.data_mut(|data| data.insert_temp(held, (output.state.clamped_offset().y, rate, most)));
    let inset = theme.spacing.s1 / 2.0;
    let target = Rect::from_x_y_ranges(Rangef::new(strip.left() + inset, strip.right() - inset), thumb);
    // Not reached by Tab: the keys scroll the view as they did.
    let response = ui.interact(target, held, egui::Sense::CLICK | egui::Sense::DRAG);
    let over = ctx.read_response(output.id.with(1)).is_some_and(|strip| strip.hovered());
    let state = if response.dragged() || response.is_pointer_button_down_on() {
        Bar::Dragging
    } else if over || response.hovered() {
        Bar::Hover
    } else {
        Bar::Rest
    };
    paint(ui.painter(), theme, strip, thumb, state);
    output
}

/// Shows `area` in `ui` with `add` drawing its content and no bar of egui's: a thin thumb, `thumb` (width, length)
/// in `muted`, `inset` in from the area's right edge and travelling `inset[1]` clear of its top and bottom, over the
/// content (a float's rows, which keep the room clear).
pub fn show_thin<R>(
    ui: &mut Ui,
    theme: &Theme,
    area: ScrollArea,
    thumb: [f32; 2],
    inset: [f32; 2],
    add: impl FnOnce(&mut Ui) -> R,
) -> ScrollAreaOutput<R> {
    let output = area.scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, add);
    let (inner, content) = (output.inner_rect, output.content_size.y);
    let ends = if inset[1].is_finite() { inset[1].max(0.0).min(inner.height().max(0.0) / 4.0) } else { 0.0 };
    let room = inner.height() - ends * 2.0;
    if content > inner.height() && room > 0.0 && content.is_finite() {
        let length = thumb[1].min(room);
        let share = (output.state.clamped_offset().y / (content - inner.height())).clamp(0.0, 1.0);
        let top = inner.top() + ends + share * (room - length);
        let rect = Rect::from_min_size(egui::pos2(inner.right() - inset[0] - thumb[0], top), egui::vec2(thumb[0], length));
        ui.painter().rect_filled(rect, 0.0, theme.palette.muted);
    }
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
    let held = ui.scope_id().with("scroll-over-held");
    let mut area = area.scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
    // A thumb held: the content goes where the pointer puts the grab point, in this frame.
    if let Some(offset) = ui.data(|data| data.get_temp::<Held>(held)).and_then(|grip| grip.offset(ui.ctx())) {
        area = area.vertical_scroll_offset(offset);
    }
    let output = area.show_viewport(ui, add);
    let grip = over_bar(ui, theme, &output, ui.data(|data| data.get_temp::<Held>(held)));
    ui.data_mut(|data| match grip {
        Some(grip) => {
            data.insert_temp(held, grip);
        }
        None => data.remove::<Held>(held),
    });
    super::row::repaint_focus_edge(ui, theme, output.inner_rect);
    output
}

/// The thumb of a [`show_viewport_over`] bar while it is held: its id, how far below its top it was taken, and the
/// bar it moves on (where its travel begins, how far it travels, the content's whole scroll).
#[derive(Debug, Clone, Copy)]
struct Held {
    thumb: egui::Id,
    grab: f32,
    top: f32,
    travel: f32,
    most: f32,
}

impl Held {
    /// The offset that puts the grab point under the pointer, while the thumb is still dragged: past an end the
    /// content stays at that end, and the thumb waits there for the pointer to come back to its grab point.
    fn offset(self, ctx: &egui::Context) -> Option<f32> {
        let dragged = ctx.read_response(self.thumb).is_some_and(|response| response.dragged());
        let at = ctx.pointer_interact_pos()?;
        if !dragged || self.travel <= 0.0 || self.most <= 0.0 {
            return None;
        }
        let share = (at.y - self.grab - self.top) / self.travel;
        share.is_finite().then(|| share.clamp(0.0, 1.0) * self.most)
    }
}

/// The bar of [`show_viewport_over`], over the content's right 12: the thumb alone senses the pointer, registered
/// after the content so it wins over a row there, and a drag on it moves the content as egui's bar does, the point
/// it was taken by staying under the pointer ([`Held`]); the strip shows its hover while the pointer is over it.
/// Nothing when the content fits. The thumb held, `was` while it was already held.
fn over_bar<R>(ui: &Ui, theme: &Theme, output: &ScrollAreaOutput<R>, was: Option<Held>) -> Option<Held> {
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
    #[cfg(test)]
    ui.ctx().data_mut(|data| data.insert_temp(egui::Id::unique("over-thumb-rect"), thumb));
    let response = ui.interact(thumb, output.id.with("over-thumb"), egui::Sense::drag());
    let held = (response.dragged() || response.is_pointer_button_down_on()).then(|| {
        // Taken where the press began; kept while held, as the bar moves under it.
        let grab = was
            .filter(|was| was.thumb == response.id)
            .map_or_else(|| response.interact_pointer_pos().map_or(0.0, |at| (at.y - top).clamp(0.0, size)), |was| was.grab);
        Held { thumb: response.id, grab, top: strip.top(), travel, most }
    });
    let over_strip = ui.ctx().pointer_hover_pos().is_some_and(|at| strip.contains(at));
    let bar = if response.dragged() || response.is_pointer_button_down_on() {
        Bar::Dragging
    } else if response.hovered() || over_strip {
        Bar::Hover
    } else {
        Bar::Rest
    };
    paint(ui.painter(), theme, strip, Rangef::new(top, top + size), bar);
    held
}

/// The thumb of the last [`show_viewport_over`] bar drawn, as it was laid out.
#[cfg(test)]
pub(crate) fn over_thumb(ctx: &egui::Context) -> Option<Rect> {
    ctx.data(|data| data.get_temp::<Rect>(egui::Id::unique("over-thumb-rect")))
}

#[cfg(test)]
#[path = "scroll_tests.rs"]
mod tests;
