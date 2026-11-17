// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Every component on one page for `ui_shot` (`"components": true`), in the theme's colours on flat panes:
//! cards open and closed, icon buttons, pills, badges, the sample's people as avatars, a row with its hover actions,
//! a tooltip, a popup menu, a toast, a board of controls that are coming soon, and the time picker as a float at
//! [`now`] with `thu 9` typed; under them the boards of the sets drawn as their sheets say, every state labelled in the
//! sheet's order. The data is invented; the keys come from [`keymap`], a keymap with a key for each action shown.

use egui::{Id, Rect, Ui, UiBuilder, Vec2, pos2, vec2};
use inboxcraft_time::{CivilDateTime, Offset, Timestamp};

use crate::draw::paint::Panes;
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::sample;
use crate::theme::Theme;
use crate::tr;

use super::Shown;
use super::button::{self, IconButton, LinkSize, TextButton, Tone};
use super::checkbox::checkbox;
use super::hover::{self, Action};
use super::key::{ChipState, key_chip};
use super::segmented::segmented;
use super::select::Select;
use super::soon::{soon, soon_tag};
use super::switch::switch;
use super::tag::{TagKind, tag};
use super::time_picker::{self, Ask, Preset, TimePicker};
use super::toast::{self, Toast};
use super::{avatar, badge, card, cut, galley, pill, popup, tooltip};

/// The page's size in points.
#[must_use]
pub fn size() -> Vec2 {
    vec2(1400.0, TOP + BOARDS)
}

/// What draws a column of the page.
type Draw = fn(&mut Ui, &Theme, &Keymap);

/// The height of the page's upper part, the columns of components, in points.
const TOP: f32 = 660.0;

/// The height of the boards under the columns, in points.
const BOARDS: f32 = 560.0;

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
    keymap.bind("mail.send", egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Enter));
    keymap
}

/// A section's caption.
fn caption(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.add_space(theme.spacing.s2);
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
                ui.label(galley(ui, sender, theme.fonts.body, p.ink));
                ui.label(galley(ui, subject, theme.fonts.caption, p.muted));
            });
        }
    });
    ui.add_space(s.s2);
    card::card(ui, theme, Id::unique("gallery-card-closed"), tr!("Screener: 3 new senders"), false, |_| ());
    caption(ui, theme, tr!("Avatars"));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(s.s2, s.s2);
        let side = theme.fonts.body.line * 2.0;
        for (name, address) in sample::people() {
            avatar::avatar(ui, theme, name, address, side);
        }
    });
    caption(ui, theme, tr!("Toast"));
    toast::panel(ui, theme, keymap, &Toast { text: tr!("Marked as done"), undo: Some("ui.undo") });
    coming(ui, theme, keymap);
}

/// The board of what is coming soon: the tag alone on its head, then a button, a switch and a menu row drawn
/// unavailable.
fn coming(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let s = &theme.spacing;
    let p = &theme.palette;
    ui.add_space(s.s2);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = s.s2;
        ui.label(galley(ui, tr!("Coming soon"), theme.fonts.caption, p.muted));
        soon_tag(ui, theme);
    });
    ui.add_space(s.s1);
    soon(ui, theme, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = s.s3;
            TextButton::new(tr!("Export")).secondary().show(ui, theme, keymap);
            ui.label(galley(ui, tr!("Sync contacts"), theme.fonts.body, p.muted));
            switch(ui, theme, tr!("Sync contacts"), false);
        });
        ui.add_space(s.s1);
        ui.set_width(ui.available_width().min(s.s6 * 10.0));
        popup::item(ui, theme, keymap, IconId::Calendar, tr!("Scheduled messages"), None);
    });
}

/// A row of the list with its hover actions shown over its text.
fn row(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let height = theme.fonts.body.line + s.s3 * 2.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, p.surface);
    ui.painter().rect_filled(rect, 0.0, p.stage);
    let actions = [
        Action { icon: IconId::Done, name: tr!("Done"), action: Some("mail.done"), unavailable: None },
        Action { icon: IconId::MarkRead, name: tr!("Mark as read"), action: Some("mail.read"), unavailable: None },
        Action { icon: IconId::Delete, name: tr!("Delete"), action: Some("mail.delete"), unavailable: None },
    ];
    let text = cut(ui, "Odile Brannock  Allotment rota for November: the beds by the shed", theme.fonts.body, p.ink, f32::INFINITY);
    let at = pos2(rect.left() + s.s3, rect.center().y - text.size().y / 2.0);
    ui.painter().galley(at, text, p.ink);
    hover::actions(ui, theme, keymap, Id::unique("gallery-row"), rect, true, &actions);
}

/// The right column: buttons, pills, badges, a row with hover actions, a tooltip and a popup.
fn right(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let s = &theme.spacing;
    caption(ui, theme, tr!("Icon buttons"));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = s.s1;
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
    caption(ui, theme, tr!("Choices"));
    ui.horizontal(|ui| {
        let font = theme.fonts.button;
        segmented(ui, theme, &[(0, tr!("Day"), font), (1, tr!("Week"), font), (2, tr!("Month"), font)], 1);
        let names = [(0, tr!("Never")), (1, tr!("Daily"))];
        Select { name: tr!("Repeat"), shown: tr!("Never"), options: &names, current: Some(0), icon: IconId::Changed, width: s.s6 * 7.0 }
            .show(ui, theme, &mut false);
        checkbox(ui, theme, tr!("Personal"), true);
        checkbox(ui, theme, tr!("Private"), false);
    });
    caption(ui, theme, tr!("Badges"));
    ui.horizontal(|ui| {
        let side = theme.fonts.heading.line * 1.6;
        badge::group(ui, theme, IconId::People, tr!("People"), side);
        badge::group(ui, theme, IconId::Notify, tr!("Updates"), side);
        badge::group(ui, theme, IconId::Calendar, tr!("Events"), side);
        let (rect, _) = ui.allocate_exact_size(vec2(s.s6 * 4.0, side), egui::Sense::hover());
        let left = badge::count(ui, theme, 128, rect.right(), rect.center().y);
        badge::count(ui, theme, 3, left - s.s2, rect.center().y);
    });
    caption(ui, theme, tr!("Hover actions"));
    row(ui, theme, keymap);
    caption(ui, theme, tr!("Tooltip and popup"));
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| tooltip::bubble(ui, theme, keymap, tr!("Search"), Some(crate::keymap::FIND)));
        ui.add_space(s.s4);
        popup::panel(ui, theme, 220.0, |ui| {
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
    popup::panel(ui, theme, 320.0, |ui| time_picker::show(ui, theme, &mut state, &ask, now(), Offset::UTC));
    ui.data_mut(|data| data.insert_temp(id, state));
}

/// A board's title: the set's name.
fn title(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.add_space(theme.spacing.s2);
    ui.label(galley(ui, text, theme.fonts.heading, theme.palette.ink));
    ui.add_space(theme.spacing.s1);
}

/// One state of a board: its name in `mono_s` and `muted` over the control `add` draws.
fn cell(ui: &mut Ui, theme: &Theme, state: &str, add: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = theme.spacing.s1;
        ui.label(galley(ui, state, theme.fonts.mono_s, theme.palette.muted));
        add(ui);
    });
}

/// A row of a board's cells, apart enough that a focus ring never touches the next control.
fn cells(ui: &mut Ui, theme: &Theme, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.s4;
        add(ui);
    });
    ui.add_space(theme.spacing.s2);
}

/// The icon button's board: default, hover, focus, pressed, selected, disabled, unavailable.
fn icon_buttons(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    title(ui, theme, tr!("Icon button"));
    let pin = || IconButton::new(IconId::Pin, tr!("Pin"));
    cells(ui, theme, |ui| {
        for (state, shown) in
            [(tr!("default"), Shown::Rest), (tr!("hover"), Shown::Hover), (tr!("focus"), Shown::Focus), (tr!("pressed"), Shown::Pressed)]
        {
            cell(ui, theme, state, |ui| {
                pin().shown(shown).show(ui, theme, keymap);
            });
        }
        cell(ui, theme, tr!("selected"), |ui| {
            pin().active(true).shown(Shown::Rest).show(ui, theme, keymap);
        });
        cell(ui, theme, tr!("disabled"), |ui| {
            pin().enabled(false).show(ui, theme, keymap);
        });
        cell(ui, theme, tr!("unavailable"), |ui| {
            pin().unavailable(tr!("Not yet: choose a message first")).show(ui, theme, keymap);
        });
    });
}

/// The text button's board: each tone in default, hover, focus, pressed, disabled and unavailable; then with the
/// arrow, with its key, with an icon, and with a label cut at its widest.
fn text_buttons(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    title(ui, theme, tr!("Text button"));
    let tones = [(tr!("Send"), Tone::Primary), (tr!("Reply all"), Tone::Secondary), (tr!("Delete"), Tone::Danger), (tr!("Discard"), Tone::Plain)];
    let states = [(tr!("default"), Shown::Rest), (tr!("hover"), Shown::Hover), (tr!("focus"), Shown::Focus), (tr!("pressed"), Shown::Pressed)];
    for (label, tone) in tones {
        cells(ui, theme, |ui| {
            for (state, shown) in states {
                cell(ui, theme, state, |ui| {
                    TextButton::new(label).tone(tone).shown(shown).show(ui, theme, keymap);
                });
            }
            cell(ui, theme, tr!("disabled"), |ui| {
                TextButton::new(label).tone(tone).enabled(false).show(ui, theme, keymap);
            });
            cell(ui, theme, tr!("unavailable"), |ui| {
                TextButton::new(label).tone(tone).unavailable(tr!("Not yet: add a recipient first")).show(ui, theme, keymap);
            });
        });
    }
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("with-arrow"), |ui| {
            TextButton::new(tr!("Continue")).primary().arrow().show(ui, theme, keymap);
        });
        cell(ui, theme, tr!("with-key"), |ui| {
            TextButton::new(tr!("Send")).primary().action("mail.send").key(true).show(ui, theme, keymap);
        });
        cell(ui, theme, tr!("with-icon"), |ui| {
            TextButton::new(tr!("Attach")).secondary().icon(IconId::Attach).show(ui, theme, keymap);
        });
    });
    cells(ui, theme, |ui| {
        let long = tr!("Ask for a receipt when this message is read by every recipient");
        cell(ui, theme, tr!("long-label"), |ui| {
            TextButton::new(long).primary().show(ui, theme, keymap);
        });
        cell(ui, theme, tr!("long-label"), |ui| {
            TextButton::new(long).secondary().show(ui, theme, keymap);
        });
    });
}

/// The link's board: a link of a line in default, hover, focus, pressed and disabled; a row's small link in default,
/// hover, focus and disabled.
fn links(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    title(ui, theme, tr!("Link"));
    let states = [(tr!("default"), Shown::Rest), (tr!("hover"), Shown::Hover), (tr!("focus"), Shown::Focus), (tr!("pressed"), Shown::Pressed)];
    for (size, name) in [(LinkSize::Line, tr!("Show details")), (LinkSize::Small, tr!("open log"))] {
        cells(ui, theme, |ui| {
            for (state, shown) in states.into_iter().filter(|(_, shown)| size == LinkSize::Line || *shown != Shown::Pressed) {
                cell(ui, theme, state, |ui| {
                    button::link_with(ui, theme, keymap, name, size, true, Some(shown));
                });
            }
            cell(ui, theme, tr!("disabled"), |ui| {
                button::link_with(ui, theme, keymap, name, size, false, None);
            });
        });
    }
}

/// The tag's board: default, accent, warn, minus, plus, coming soon.
fn tags(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Tag"));
    cells(ui, theme, |ui| {
        let tags = [
            (tr!("default"), tr!("Draft"), TagKind::Default),
            (tr!("accent"), tr!("New sender"), TagKind::Accent),
            (tr!("warn"), tr!("Returned"), TagKind::Warn),
            (tr!("minus"), tr!("Not delivered"), TagKind::Minus),
            (tr!("plus"), tr!("Let in"), TagKind::Plus),
        ];
        for (state, text, kind) in tags {
            cell(ui, theme, state, |ui| {
                tag(ui, theme, text, kind);
            });
        }
        cell(ui, theme, tr!("coming-soon"), |ui| {
            soon_tag(ui, theme);
        });
    });
}

/// The key chip's board: default, on cyan, capture, none, disabled.
fn key_chips(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Key chip"));
    cells(ui, theme, |ui| {
        let chips = [
            (tr!("default"), "A", ChipState::Default),
            (tr!("on-cyan"), "Ctrl Enter", ChipState::OnCyan),
            (tr!("capture"), tr!("press the keys"), ChipState::Capture),
            (tr!("none"), tr!("none"), ChipState::Disabled),
            (tr!("disabled"), "H", ChipState::Disabled),
        ];
        for (state, key, chip) in chips {
            cell(ui, theme, state, |ui| {
                key_chip(ui, theme, key, chip);
            });
        }
    });
}

/// The boards' left column.
fn boards_left(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    icon_buttons(ui, theme, keymap);
    text_buttons(ui, theme, keymap);
}

/// The boards' right column.
fn boards_right(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    links(ui, theme, keymap);
    key_chips(ui, theme);
    tags(ui, theme);
}

/// Draws the page into `ui` in `theme` with the keys of `keymap`.
pub fn page(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let s = theme.spacing;
    let whole = Rect::from_min_size(pos2(0.0, 0.0), size());
    ui.painter().rect_filled(whole, 0.0, theme.palette.bg);
    let mut layers = Panes::begin(ui.painter());
    let half = (whole.width() - PICKER - s.s2 * 4.0) / 2.0;
    let board = (whole.width() - s.s2 * 3.0) / 2.0;
    let top = TOP - s.s2 * 2.0;
    let columns: [(f32, f32, f32, Draw); 5] = [
        (s.s2, s.s2, half, left),
        (s.s2 * 2.0 + half, s.s2, half, right),
        (s.s2 * 3.0 + half * 2.0, s.s2, PICKER, picker),
        (s.s2, TOP, board, boards_left),
        (s.s2 * 2.0 + board, TOP, board, boards_right),
    ];
    for (x, y, width, draw) in columns {
        let height = if y < TOP { top } else { BOARDS - s.s2 };
        let column = Rect::from_min_size(pos2(x.round(), y), vec2(width.floor(), height));
        ui.scope_builder(UiBuilder::new().max_rect(column), |ui| {
            layers.card(ui, super::margin(s.s4, s.s3), |ui| {
                ui.set_min_size(ui.available_size());
                draw(ui, theme, keymap);
            });
        });
    }
    layers.finish(ui.painter(), theme);
}

#[cfg(test)]
#[path = "gallery_tests.rs"]
mod tests;
