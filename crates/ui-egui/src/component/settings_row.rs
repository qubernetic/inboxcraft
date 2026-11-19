// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A row of a settings page: its title in `strong` and an optional line of help in `caption` and `muted` on the left,
//! its control on the right, 640 wide, at least 56 high, 12 above and below, 16 left and right, 24 between the words
//! and the control, both centred on the row, a `panel_line` hairline under it. Under the pointer it takes the `stage`
//! fill at once (the presentation's `.set` has no transition); while its control has the keyboard focus it draws the
//! focus edge 2 px inside itself.
//!
//! [`settings_row_soon`] is a setting that is not built yet: its title `muted` with the coming-soon tag, its reason as
//! the help, and an unavailable switch.

use egui::{Align, Layout, Rect, Role, Sense, Shape, Ui, UiBuilder, Vec2, WidgetInfo, pos2, vec2};

use crate::draw::paint::{self, Ring};
use crate::theme::Theme;

use super::Shown;
use super::soon::{soon, soon_tag};
use super::switch::{SwitchState, switch_in};

/// The row's widest, in points.
const WIDTH: f32 = 640.0;

/// The room above and below the row's content, in points.
const PAD_Y: f32 = 12.0;

/// The room left and right of the row's content, in points.
const PAD_X: f32 = 16.0;

/// The room between the words and the control, in points.
const GAP: f32 = 24.0;

/// The room between the title and the help, in points.
const UNDER: f32 = 2.0;

/// The room between the title and its tag, in points.
const TAG_GAP: f32 = 8.0;

/// The least width the words keep beside the control; with less, the control stands under them, in points.
const WORDS_LEAST: f32 = 160.0;

/// The room between the words and a control under them, in points.
const STACKED_GAP: f32 = 8.0;

/// The most lines the help takes.
const ROWS: usize = 4;

/// The tag on the [`Ui`] that [`page`] opens.
const PAGE: &str = "inboxcraft-settings-page";

/// Draws `add` as a settings page: its rows take the page's whole width (the design's 720-wide column) in place of the
/// component's 640. What `add` gave.
pub fn page<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope_builder(UiBuilder::new().ui_stack_info(egui::UiStackInfo::default().with_tag(PAGE)), add).inner
}

/// The width of a row drawn in `ui`: the room there on a [`page`], at most 640 elsewhere; on whole points.
fn width_in(ui: &Ui) -> f32 {
    let on_page = ui.stack().iter().any(|frame| frame.tags().contains(PAGE));
    let room = ui.available_width();
    let width = if on_page { room } else { WIDTH.min(room) };
    if width.is_finite() { width.max(PAD_X * 2.0 + 1.0).floor() } else { WIDTH }
}

/// A settings row called `title` with `help` under it and the control `control` draws; what `control` gave.
pub fn settings_row<R>(ui: &mut Ui, theme: &Theme, title: &str, help: Option<&str>, control: impl FnOnce(&mut Ui) -> R) -> R {
    settings_row_with(ui, theme, title, help, false, None, control)
}

/// A settings row for what is not built yet: `title` with the coming-soon tag, `reason` as its help, an unavailable
/// switch.
pub fn settings_row_soon(ui: &mut Ui, theme: &Theme, title: &str, reason: &str) {
    soon(ui, theme, |ui| {
        settings_row_with(ui, theme, title, Some(reason), true, None, |ui| {
            switch_in(ui, theme, title, SwitchState::Unavailable);
        });
    });
}

/// [`settings_row_soon`] with the control `control` draws in place of the switch (a choice of a segmented control, a
/// link), drawn unavailable as everything inside [`soon`] is; what `control` gave.
pub fn settings_row_soon_with<R>(ui: &mut Ui, theme: &Theme, title: &str, reason: &str, control: impl FnOnce(&mut Ui) -> R) -> R {
    soon(ui, theme, |ui| settings_row_with(ui, theme, title, Some(reason), true, None, control))
}

/// [`settings_row`], with the coming-soon tag after the title when `tagged`, drawn hovered or focused as `shown` says
/// (the gallery's board).
pub(crate) fn settings_row_with<R>(
    ui: &mut Ui,
    theme: &Theme,
    title: &str,
    help: Option<&str>,
    tagged: bool,
    shown: Option<Shown>,
    control: impl FnOnce(&mut Ui) -> R,
) -> R {
    let p = &theme.palette;
    let f = &theme.fonts;
    let width = width_in(ui);
    let id = ui.scope_id().with("settings-row").with(title);
    // The control's size from the frame before: the words wrap in the room it leaves. A change of it redraws the
    // frame before it is shown.
    let size_id = id.with("control");
    let control_size: Vec2 = ui.data(|data| data.get_temp(size_id)).unwrap_or(Vec2::ZERO);
    let control_room = if control_size.x > 0.0 { GAP + control_size.x } else { 0.0 };
    // In a page too narrow for the words beside the control (a narrow window, large text), the control stands under
    // the words, at the row's right end, and the words take the row's width.
    let stacked = control_size.x > 0.0 && width - PAD_X * 2.0 - control_room < WORDS_LEAST;
    let words_width = if stacked { width - PAD_X * 2.0 } else { width - PAD_X * 2.0 - control_room }.max(1.0);
    let ink = super::soon::ink(ui, theme, p.ink);
    let tag_width = if tagged { TAG_GAP + super::galley(ui, crate::tr!("coming soon"), f.mono_s, p.muted).size().x.ceil() + 12.0 } else { 0.0 };
    let title_galley = super::cut(ui, title, f.strong, ink, (words_width - tag_width).max(1.0));
    let title_width = title_galley.size().x.ceil();
    let help_galley = help.map(|help| super::wrapped(ui, &super::plain(help), f.caption, p.muted, words_width, ROWS, Align::LEFT));
    let words_high = f.strong.line + help_galley.as_ref().map_or(0.0, |help| UNDER + help.size().y);
    let content = if stacked { words_high + STACKED_GAP + control_size.y } else { words_high.max(control_size.y) };
    let high = (PAD_Y + content + PAD_Y).max(theme.boxes().settings_row).ceil();
    let slot = ui.painter().add(Shape::Noop);
    let (_, rect) = ui.allocate_space(vec2(width, high));
    let response = ui.interact(rect, id, Sense::hover());
    // A screen reader hears the row's help with its title.
    response.widget_info(|| WidgetInfo::labeled(Role::Group, ui.is_enabled(), spoken(title, help)));
    let top = if stacked { rect.top() + PAD_Y } else { (rect.center().y - words_high / 2.0).round() };
    let left = rect.left() + PAD_X;
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.galley(pos2(left, top), title_galley, ink);
        if let Some(help) = help_galley {
            paint::rows_at(painter, pos2(left, top + f.strong.line + UNDER), words_width, help, p.muted);
        }
    }
    if tagged {
        let tag_height = theme.boxes().chip;
        let at =
            Rect::from_min_size(pos2(left + title_width + TAG_GAP, (top + (f.strong.line - tag_height) / 2.0).round()), vec2(tag_width, tag_height));
        let mut child = ui.new_child(UiBuilder::new().max_rect(at).layout(Layout::left_to_right(Align::Min)));
        soon_tag(&mut child, theme);
    }
    // The control at the right end, centred on the row, in a room as high as it was the frame before (a horizontal
    // layout gives each widget the room's whole height, so a room as high as the row would measure the row).
    let room_top = if stacked { top + words_high + STACKED_GAP } else { (rect.center().y - control_size.y / 2.0).round() };
    let room = Rect::from_min_max(pos2(left, room_top), pos2(rect.right() - PAD_X, room_top + control_size.y));
    let mut child = ui.new_child(UiBuilder::new().id_salt(id.with("control-ui")).max_rect(room).layout(Layout::right_to_left(Align::Center)));
    let given = control(&mut child);
    let drawn = child.min_rect().size();
    if (drawn - control_size).length() > 0.01 {
        ui.data_mut(|data| data.insert_temp(size_id, drawn));
        ui.ctx().request_discard("a settings row's control changed its size");
    }
    let hovered = shown.map_or_else(|| ui.rect_contains_pointer(rect), |shown| shown == Shown::Hover);
    let focused = shown.map_or_else(
        || {
            let focus = ui.ctx().memory(|memory| memory.focused());
            focus.and_then(|focus| ui.ctx().read_response(focus)).is_some_and(|focus| rect.contains_rect(focus.rect))
        },
        |shown| shown == Shown::Focus,
    );
    if ui.is_rect_visible(rect) {
        if hovered {
            ui.painter().set(slot, Shape::rect_filled(rect, 0.0, p.stage));
        }
        let painter = ui.painter();
        let y = rect.bottom() - 1.0;
        paint::hairline(painter, pos2(rect.left(), y), pos2(rect.right() - 1.0, y), p.panel_line);
        if focused {
            paint::focus_ring(painter, theme, Rect::from_min_max(rect.min, pos2(rect.right(), rect.bottom() - 1.0)), Ring::Inside);
        }
    }
    given
}

/// What a screen reader hears for a row: its title, and its help after it.
pub(crate) fn spoken(title: &str, help: Option<&str>) -> String {
    match help {
        Some(help) => format!("{title}. {help}"),
        None => title.to_owned(),
    }
}

#[cfg(test)]
#[path = "settings_row_tests.rs"]
mod tests;
