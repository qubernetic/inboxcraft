// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The time picker, one for snooze, send later and reminders, as its sheet draws it (`time-picker`): in a float 320
//! wide, its title in the `label` style with the Esc key chip at its right end; under a hairline the presets still
//! ahead, menu rows with their time in `mono_s` at the right (a preset behind is hidden), `panel_line` between them;
//! an extra row when the caller asks for one ("Take the reminder off", "Send now instead"); "Or type a time" over the
//! [`typed_time`] field, whose text counts as soon as it parses, without Enter, the time it reads shown under it (and
//! while it is focused and empty, a line that says so); the alert row with its switch when the caller asks about one;
//! Cancel and the confirm button at the right of the last row, the confirm unavailable while no time is chosen; and a
//! note under all when the caller gives one.
//!
//! The time shown is always the time used: the line under the field and the confirm button read the same text at the
//! same now, every frame, so a typed `17:30` that falls behind while the picker is open shows, and is, tomorrow's.
//! The alert switch lives in the [`TimePicker`] the caller keeps, so it remembers its last value from one opening to
//! the next; [`TimePicker::open`] clears only the text. In a [`super::popup::float`] Esc closes the float; drawn
//! anywhere else Esc cancels.

use egui::{Align, Key, Layout, Modifiers, Response, Role, Sense, Ui, WidgetInfo, pos2, vec2};
use inboxcraft_time::{Offset, Timestamp};

use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::button::TextButton;
use super::key::{Chip, ChipState, key_words};
use super::popup::{self, Item, ItemState};
use super::switch::switch;
use super::typed_time::{self, short};
use super::{solid, soon};

pub use super::typed_time::MAX_TYPED;

/// A preset: its name and the time it stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset<'a> {
    /// What the row says ("Later today").
    pub name: &'a str,
    /// The time it picks.
    pub at: Timestamp,
}

/// What the picker asks: its words and its presets.
#[derive(Debug, Clone, Copy)]
pub struct Ask<'a> {
    /// The title over the presets ("Snooze until").
    pub title: &'a str,
    /// The confirm button ("Snooze", "Schedule", "Remind me").
    pub confirm: &'a str,
    /// The alert switch's name ("Notify me when it comes back"), or `None` for no switch.
    pub alert: Option<&'a str>,
    /// The presets, in the order shown; those not ahead of now are hidden.
    pub presets: &'a [Preset<'a>],
    /// One more row under the presets ("Take the reminder off", "Send now instead"), or `None`.
    pub extra: Option<&'a str>,
    /// A sentence under the buttons, or `None`.
    pub note: Option<&'a str>,
}

/// What the person did in this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Picked {
    /// Nothing yet: the picker stays open.
    Open,
    /// Cancel or Escape.
    Cancelled,
    /// The extra row.
    Extra,
    /// A time, from a preset or typed, and whether to alert (always `false` without an alert switch).
    At {
        /// The time picked, ahead of now.
        at: Timestamp,
        /// Whether the switch asked for an alert.
        alert: bool,
    },
}

/// The picker's state, kept by the caller between frames and between openings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TimePicker {
    typed: String,
    alert: bool,
}

impl TimePicker {
    /// A picker whose alert switch starts at `alert` (a value the caller saved).
    #[must_use]
    pub fn with_alert(alert: bool) -> Self {
        Self { typed: String::new(), alert }
    }

    /// A new opening: the typed text goes, the alert switch stays as it was left.
    pub fn open(&mut self) {
        self.typed.clear();
    }

    /// The text typed so far.
    #[must_use]
    pub fn typed(&self) -> &str {
        &self.typed
    }

    /// Puts `text` in the field, as if typed, cut to [`MAX_TYPED`] bytes at a character boundary.
    pub fn set_typed(&mut self, text: &str) {
        let mut end = text.len().min(MAX_TYPED);
        while !text.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        self.typed = text.get(..end).unwrap_or_default().to_owned();
    }

    /// Whether the alert switch is on.
    #[must_use]
    pub fn alert(&self) -> bool {
        self.alert
    }

    /// Sets the alert switch.
    pub fn set_alert(&mut self, on: bool) {
        self.alert = on;
    }

    /// The time the typed text names at `now` in `offset`: the one shown and the one the confirm button uses.
    #[must_use]
    pub fn chosen(&self, now: Timestamp, offset: Offset) -> Option<Timestamp> {
        typed_time::parse(&self.typed, now, offset)
    }
}

/// The width of the picker's float, in points.
pub const WIDTH: f32 = 320.0;

/// A hairline across the picker in `colour`.
fn rule(ui: &mut Ui, colour: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, 0.0, colour);
    }
}

/// The title row: 36 high, the title in the `label` style and the Esc chip at the right end.
fn title(ui: &mut Ui, theme: &Theme, text: &str) {
    let p = &theme.palette;
    let pad = theme.spacing.s3;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), theme.boxes().field), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, text));
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = solid(ui);
    let chip = Chip::new(ui, theme, &key_words(&egui::KeyboardShortcut::new(Modifiers::NONE, Key::Escape)), ChipState::Default);
    let size = chip.size();
    let right = rect.right() - pad - size.x;
    chip.paint(&painter, pos2(right, (rect.center().y - size.y / 2.0).round()));
    let words = super::cut(ui, text, theme.fonts.label, p.muted, right - theme.spacing.s2 - rect.left() - pad);
    painter.galley(pos2(rect.left() + pad, rect.center().y - words.size().y / 2.0), words, p.muted);
}

/// A preset row: the name, and its time in `mono_s` at the right end; clicked when chosen. A screen reader hears both.
fn preset_row(ui: &mut Ui, theme: &Theme, name: &str, time: &str, shown: Option<super::Shown>) -> Response {
    let item = Item { icon: None, name, caption: Some(time), action: None, state: ItemState::Default };
    let response = popup::menu_item_with(ui, theme, &Keymap::empty(), &item, false, shown);
    let spoken = tr!("{name}, {time}", name = name, time = time);
    ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(spoken.as_str()));
    response
}

/// Draws the picker into `ui` (a float's panel 320 wide, see [`super::popup`]) for `state` and `ask`, at `now` on the
/// wall clock of `offset`; what the person did in this frame.
pub fn show(ui: &mut Ui, theme: &Theme, state: &mut TimePicker, ask: &Ask<'_>, now: Timestamp, offset: Offset) -> Picked {
    show_with(ui, theme, state, ask, now, offset, Forced::default())
}

/// What a board of the gallery forces on the picker: a hovered preset, a focused typed time.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Forced {
    /// The preset (counted among those shown) drawn hovered.
    pub hover: Option<usize>,
    /// The typed time drawn focused, its caret still.
    pub typing: bool,
}

/// [`show`] with `forced` states: a board of the gallery.
pub(crate) fn show_with(ui: &mut Ui, theme: &Theme, state: &mut TimePicker, ask: &Ask<'_>, now: Timestamp, offset: Offset, forced: Forced) -> Picked {
    let p = &theme.palette;
    let s = &theme.spacing;
    let keys = Keymap::empty();
    let mut picked = Picked::Open;
    ui.spacing_mut().item_spacing = vec2(s.s2, 0.0);
    title(ui, theme, ask.title);
    rule(ui, p.line);
    for (index, preset) in ask.presets.iter().filter(|preset| preset.at > now).enumerate() {
        if index > 0 {
            rule(ui, p.panel_line);
        }
        let shown = (forced.hover == Some(index)).then_some(super::Shown::Hover);
        if preset_row(ui, theme, preset.name, &short(preset.at, now, offset), shown).clicked() {
            picked = Picked::At { at: preset.at, alert: false };
        }
    }
    if let Some(extra) = ask.extra {
        rule(ui, p.line);
        let item = Item { icon: None, name: extra, caption: None, action: None, state: ItemState::Default };
        if popup::menu_item(ui, theme, &keys, &item).clicked() {
            picked = Picked::Extra;
        }
    }
    rule(ui, p.line);
    // The typed time: 12 around it, its label, the field and what it reads as.
    let id = ui.make_persistent_id("time-picker-typed");
    let mut entered = false;
    let chosen = egui::Frame::NONE.inner_margin(super::margin(s.s3, s.s3)).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = s.s2;
        let caption = super::label(ui, theme.fonts.label, tr!("Or type a time"), p.muted);
        let typing = forced.typing.then_some(super::Shown::Focus);
        let field = typed_time::typed_time_with(ui, theme, id, tr!("Or type a time"), &mut state.typed, now, offset, typing).labelled_by(caption.id);
        // Read after the edit, so the line and the button see this frame's text.
        entered = !soon::is_soon(ui) && field.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));
        if (field.has_focus() || forced.typing) && state.typed.trim().is_empty() {
            let mut job = egui::text::LayoutJob::single_section(
                tr!("It counts as soon as it reads as a time: no Enter needed.").to_owned(),
                theme.fonts.caption.format(p.muted),
            );
            job.wrap.max_width = ui.available_width().max(1.0);
            let text = ui.painter().layout_job(job);
            let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), text.size().y), Sense::hover());
            response.widget_info(|| WidgetInfo::labeled(Role::Label, true, text.text()));
            solid(ui).galley(rect.min, text, p.muted);
        }
        state.chosen(now, offset)
    });
    let chosen = chosen.inner;
    if let Some(name) = ask.alert {
        rule(ui, p.panel_line);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), theme.boxes().field), Sense::hover());
        let mut row = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(s.s3, 0.0))).layout(Layout::right_to_left(Align::Center)));
        if switch(&mut row, theme, name, state.alert).clicked() {
            state.alert = !state.alert;
        }
        let words = super::cut(ui, name, theme.fonts.body, p.ink, row.min_rect().left() - s.s2 - rect.left() - s.s3);
        solid(ui).galley(pos2(rect.left() + s.s3, rect.center().y - words.size().y / 2.0), words, p.ink);
    }
    rule(ui, p.line);
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), theme.boxes().button + s.s2 * 2.0), Sense::hover());
    let mut buttons = ui.new_child(egui::UiBuilder::new().max_rect(row.shrink2(vec2(s.s3, s.s2))).layout(Layout::right_to_left(Align::Center)));
    buttons.spacing_mut().item_spacing.x = s.s2;
    let confirm = match chosen {
        Some(_) => TextButton::new(ask.confirm).primary(),
        None => TextButton::new(ask.confirm).primary().unavailable(tr!("Pick a time above or type one first")),
    };
    if (confirm.show(&mut buttons, theme, &keys).clicked() || entered)
        && let Some(at) = chosen
    {
        picked = Picked::At { at, alert: false };
    }
    if TextButton::new(tr!("Cancel")).plain().show(&mut buttons, theme, &keys).clicked() {
        picked = Picked::Cancelled;
    }
    if let Some(note) = ask.note {
        rule(ui, p.line);
        egui::Frame::NONE.inner_margin(egui::Margin { left: 12, right: 12, top: 8, bottom: 10 }).show(ui, |ui| {
            let mut job = egui::text::LayoutJob::single_section(note.to_owned(), theme.fonts.caption.format(p.muted));
            job.wrap.max_width = ui.available_width().max(1.0);
            let text = ui.painter().layout_job(job);
            let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), text.size().y), Sense::hover());
            response.widget_info(|| WidgetInfo::labeled(Role::Label, true, note));
            solid(ui).galley(rect.min, text, p.muted);
        });
    }
    // Esc cancels where no float takes it.
    if !soon::is_soon(ui) && popup::escape(ui.ctx()) {
        picked = Picked::Cancelled;
    }
    match picked {
        Picked::At { at, .. } => Picked::At { at, alert: ask.alert.is_some() && state.alert },
        other => other,
    }
}

#[cfg(test)]
#[path = "time_picker_tests.rs"]
mod tests;
