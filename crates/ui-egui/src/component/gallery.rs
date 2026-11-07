// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Every component on one page for `ui_shot` (`"components": true`), in the theme's colours over its colour fields:
//! cards open and closed, icon buttons, pills, badges, the sample's people as avatars, a row with its hover actions,
//! a tooltip, a popup menu, a toast, and the time picker as a float at [`now`] with `thu 9` typed. The data is
//! invented; the keys come from [`keymap`], a keymap with a key for each action shown.

use egui::{Id, Rect, Ui, UiBuilder, Vec2, pos2, vec2};
use inboxcraft_time::{CivilDateTime, Offset, Timestamp};

use crate::glass::{self, Layers, Under};
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::sample;
use crate::theme::Theme;
use crate::tr;

use super::button::IconButton;
use super::hover::{self, Action};
use super::time_picker::{self, Ask, Preset, TimePicker};
use super::toast::{self, Toast};
use super::{avatar, badge, card, cut, galley, line_height, pill, popup, tooltip};

/// The page's size in points.
#[must_use]
pub fn size() -> Vec2 {
    vec2(1400.0, 660.0)
}

/// The width of the time picker's column, in points.
const PICKER: f32 = 380.0;

/// The time the page is drawn at: Friday 6 November 2026, 14:30 UTC.
#[must_use]
pub fn now() -> Timestamp {
    at(6, 14, 30)
}

/// A time in November 2026, UTC.
fn at(day: u8, hour: u8, minute: u8) -> Timestamp {
    CivilDateTime::new(2026, 11, day, hour, minute, 0).and_then(|civil| civil.to_timestamp(Offset::UTC)).unwrap_or(Timestamp::UNIX_EPOCH)
}

/// The keys the page shows: the default keys and one for each action on it.
#[must_use]
pub fn keymap() -> Keymap {
    let mut keymap = Keymap::default();
    for (action, key) in [
        ("mail.done", egui::Key::D),
        ("mail.archive", egui::Key::A),
        ("mail.delete", egui::Key::Delete),
        ("mail.snooze", egui::Key::S),
        ("mail.pin", egui::Key::P),
        ("mail.move", egui::Key::M),
        ("mail.read", egui::Key::R),
    ] {
        keymap.bind(action, egui::KeyboardShortcut::new(egui::Modifiers::NONE, key));
    }
    keymap.bind("ui.undo", egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z));
    keymap
}

/// A section's caption.
fn caption(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.add_space(theme.spacing.s);
    ui.label(galley(ui, text, theme.fonts.caption, theme.palette.muted));
}

/// The left column: the cards and the avatars.
fn left(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let s = &theme.spacing;
    let p = &theme.palette;
    caption(ui, theme, tr!("Cards"));
    card::card(ui, theme, Id::unique("gallery-card-open"), tr!("Newsletters"), true, |ui| {
        for (sender, subject) in [("Studio Collective news", "The autumn programme is out"), ("Riverside Library", "Two loans are due on Friday")] {
            ui.horizontal(|ui| {
                ui.label(galley(ui, sender, theme.fonts.sender, p.text));
                ui.label(galley(ui, subject, theme.fonts.preview, p.muted));
            });
        }
    });
    ui.add_space(s.s);
    card::card(ui, theme, Id::unique("gallery-card-closed"), tr!("Screener: 3 new senders"), false, |_| ());
    caption(ui, theme, tr!("Avatars"));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(s.s, s.s);
        let side = line_height(ui, theme.fonts.body) * 2.0;
        for (name, address) in sample::people() {
            avatar::avatar(ui, theme, name, address, side);
        }
    });
    caption(ui, theme, tr!("Toast"));
    toast::panel(ui, theme, keymap, &Toast { text: tr!("Marked as done"), undo: Some("ui.undo") });
}

/// A row of the list with its hover actions shown and its text faded before them.
fn row(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let height = line_height(ui, theme.fonts.sender) + s.m * 2.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), egui::Sense::hover());
    ui.painter().rect_filled(rect, s.corner, p.surface);
    ui.painter().rect_filled(rect, s.corner, p.hover);
    let actions = [
        Action { icon: IconId::Done, name: tr!("Done"), action: Some("mail.done"), unavailable: None },
        Action { icon: IconId::MarkRead, name: tr!("Mark as read"), action: Some("mail.read"), unavailable: None },
        Action { icon: IconId::Delete, name: tr!("Delete"), action: Some("mail.delete"), unavailable: None },
    ];
    let shown = hover::actions(ui, theme, keymap, Id::unique("gallery-row"), rect, true, &actions);
    let text = cut(ui, "Odile Brannock  Allotment rota for November: the beds by the shed", theme.fonts.sender, p.text, f32::INFINITY);
    let at = pos2(rect.left() + s.m, rect.center().y - text.size().y / 2.0);
    glass::faded(ui.painter(), at, &text, p.text, shown.left - s.xs, s.xl);
}

/// The right column: buttons, pills, badges, a row with hover actions, a tooltip and a popup.
fn right(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let s = &theme.spacing;
    caption(ui, theme, tr!("Icon buttons"));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = s.xs;
        IconButton::new(IconId::Done, tr!("Done")).action("mail.done").show(ui, theme, keymap);
        IconButton::new(IconId::Archive, tr!("Archive")).action("mail.archive").show(ui, theme, keymap);
        IconButton::new(IconId::Delete, tr!("Delete")).action("mail.delete").show(ui, theme, keymap);
        IconButton::new(IconId::Snooze, tr!("Snooze")).action("mail.snooze").show(ui, theme, keymap);
        IconButton::new(IconId::Pin, tr!("Unpin")).action("mail.pin").active(true).show(ui, theme, keymap);
        IconButton::new(IconId::More, tr!("More")).show(ui, theme, keymap);
        IconButton::new(IconId::Reply, tr!("Reply (not yet)")).enabled(false).show(ui, theme, keymap);
    });
    caption(ui, theme, tr!("Pills"));
    ui.horizontal(|ui| {
        pill::pill(ui, theme, tr!("All"), false);
        pill::pill(ui, theme, tr!("Unread"), true);
        pill::pill(ui, theme, tr!("Flagged"), false);
    });
    caption(ui, theme, tr!("Badges"));
    ui.horizontal(|ui| {
        let side = line_height(ui, theme.fonts.heading) * 1.6;
        badge::group(ui, theme, IconId::People, tr!("People"), side);
        badge::group(ui, theme, IconId::Notify, tr!("Updates"), side);
        badge::group(ui, theme, IconId::Calendar, tr!("Events"), side);
        let (rect, _) = ui.allocate_exact_size(vec2(s.xl * 4.0, side), egui::Sense::hover());
        let left = badge::count(ui, theme, 128, rect.right(), rect.center().y);
        badge::count(ui, theme, 3, left - s.s, rect.center().y);
    });
    caption(ui, theme, tr!("Hover actions"));
    row(ui, theme, keymap);
    caption(ui, theme, tr!("Tooltip and popup"));
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| tooltip::bubble(ui, theme, keymap, tr!("Search"), Some(crate::keymap::FIND)));
        ui.add_space(s.l);
        popup::panel(ui, theme, Under::Sharp, 220.0, |ui| {
            popup::item(ui, theme, keymap, IconId::Snooze, tr!("Snooze"), Some("mail.snooze"));
            popup::item(ui, theme, keymap, IconId::Move, tr!("Move to a folder"), Some("mail.move"));
            popup::item(ui, theme, keymap, IconId::Flag, tr!("Flag"), None);
        });
    });
}

/// The time picker for snooze, with one preset behind (hidden) and `thu 9` typed; it keeps its state in `ui`'s memory.
fn picker(ui: &mut Ui, theme: &Theme, _: &Keymap) {
    let presets = [
        Preset { name: tr!("This morning"), at: at(6, 8, 0) },
        Preset { name: tr!("Later today"), at: at(6, 17, 30) },
        Preset { name: tr!("This evening"), at: at(6, 20, 0) },
        Preset { name: tr!("Tomorrow morning"), at: at(7, 8, 0) },
        Preset { name: tr!("Next week"), at: at(9, 8, 0) },
    ];
    let ask = Ask { title: tr!("Snooze until"), confirm: tr!("Snooze"), alert: Some(tr!("Notify me when it comes back")), presets: &presets };
    let id = Id::unique("gallery-time-picker");
    let mut state = ui.data_mut(|data| {
        data.get_temp_mut_or_insert_with(id, || {
            let mut state = TimePicker::with_alert(true);
            state.set_typed("thu 9");
            state
        })
        .clone()
    });
    caption(ui, theme, tr!("Time picker"));
    popup::panel(ui, theme, Under::Sharp, 320.0, |ui| time_picker::show(ui, theme, &mut state, &ask, now(), Offset::UTC));
    ui.data_mut(|data| data.insert_temp(id, state));
}

/// Draws the page into `ui` in `theme` with the keys of `keymap`.
pub fn page(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let s = theme.spacing;
    let whole = Rect::from_min_size(pos2(0.0, 0.0), size());
    glass::backdrop(ui.painter(), theme, whole);
    let mut layers = Layers::begin(ui.painter());
    let half = (whole.width() - PICKER - s.gap * 4.0) / 2.0;
    let mut x = s.gap;
    for (width, draw) in [(half, left as fn(&mut Ui, &Theme, &Keymap)), (half, right), (PICKER, picker)] {
        let column = Rect::from_min_size(pos2(x, s.gap), vec2(width, whole.height() - s.gap * 2.0));
        ui.scope_builder(UiBuilder::new().max_rect(column), |ui| {
            layers.card(ui, super::margin(s.l, s.m), |ui| {
                ui.set_min_size(ui.available_size());
                draw(ui, theme, keymap);
            });
        });
        x += width + s.gap;
    }
    layers.finish(ui.painter(), theme);
}

#[cfg(test)]
#[path = "gallery_tests.rs"]
mod tests;
