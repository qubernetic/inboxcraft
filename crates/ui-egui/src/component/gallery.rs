// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Every component on one page for `ui_shot` (`"components": true`), in the theme's colours on flat panes:
//! cards open and closed, icon buttons, pills, badges, the sample's people as avatars, a row with its hover actions,
//! a tooltip, a popup menu, a toast, a board of controls that are coming soon, and the time picker as a float at
//! [`now`] with `thu 9` typed; under them the boards of the sets drawn as their sheets say, every state labelled in the
//! sheet's order, and under them the marks: avatar, badge, scroll bar, account bar and the icons. The data is
//! invented; the keys come from [`keymap`], a keymap with a key for each action shown.

use egui::{Id, Rect, Ui, UiBuilder, Vec2, pos2, vec2};
use inboxcraft_time::{CivilDateTime, Offset, Timestamp};

use crate::draw::paint::Panes;
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::sample;
use crate::theme::{TextScale, Theme};
use crate::tr;

use super::Shown;
use super::avatar::AvatarSize;
use super::badge::BadgeKind;
use super::button::{self, IconButton, LinkSize, TextButton, Tone};
use super::checkbox::{self, checkbox};
use super::hover::{self, Action};
use super::key::{ChipState, key_chip};
use super::scroll::{self, Bar};
use super::segmented::{self, segmented};
use super::select::Select;
use super::slider;
use super::soon::{soon, soon_tag};
use super::switch::{self, SwitchState, switch};
use super::tag::{TagKind, tag};
use super::time_picker::{self, Ask, Preset, TimePicker};
use super::toast::{self, Toast};
use super::{avatar, badge, card, cut, galley, icon, label, pill, popup, tooltip};

/// The page's size in points.
#[must_use]
pub fn size() -> Vec2 {
    vec2(WIDE + BLOCKS, TOP + BOARDS + CONTROLS + MARKS)
}

/// What draws a column of the page.
type Draw = fn(&mut Ui, &Theme, &Keymap);

/// The height of the page's upper part, the columns of components, in points.
const TOP: f32 = 660.0;

/// The height of the boards under the columns, in points: their taller column, and the gap the columns above leave
/// over the boards' first heading, so every band stands as far under the one above.
const BOARDS: f32 = 835.0;

/// The height of the boards of the toggles and choices, under the other boards, in points: their taller column and
/// the same gap.
const CONTROLS: f32 = 807.0;

/// The height of the band of marks under the boards (avatar, badge, icon, scroll bar, account bar), in points.
const MARKS: f32 = 480.0;

/// The width of the page's first part, every band but the shared rows and blocks, in points.
const WIDE: f32 = 1800.0;

/// The width of the shared rows and blocks (row, hover actions, banner, empty state, progress, settings row, check
/// step, facts) right of the rest, as high as the page, in points.
const BLOCKS: f32 = 1800.0;

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
    // The group stands at the line's right end, the row's text cut short of it.
    let line = rect.shrink2(vec2(s.s4, 0.0));
    let room = line.width() - hover::width(ui, theme, keymap, &actions) - s.s2;
    let text = cut(ui, "Odile Brannock  Allotment rota for November: the beds by the shed", theme.fonts.body, p.ink, room);
    let at = pos2(line.left(), rect.center().y - text.size().y / 2.0);
    ui.painter().galley(at, text, p.ink);
    hover::actions(ui, theme, keymap, Id::unique("gallery-row"), line, true, &actions);
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
    });
    // The boxes on a line of their own: beside the choices they ran past the column's right edge.
    ui.horizontal(|ui| {
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

/// How far under the top of an unavailable control the board puts its reason tooltip, in points.
const REASON_DROP: f32 = 36.0;

/// The room a board leaves under a row with a reason tooltip, so the next row stays clear of it and its shadow.
const REASON_ROOM: f32 = 48.0;

/// The reason tooltip of the unavailable control `name` at `control`, as the boards draw it: 36 under its top, its
/// left edge on the control's, over what follows (it takes no room of its own).
fn reason_under(ui: &mut Ui, theme: &Theme, keymap: &Keymap, control: Rect, name: &str, reason: &str) {
    let at = Rect::from_min_size(control.left_top() + vec2(0.0, REASON_DROP), vec2(320.0, 120.0));
    let mut child = ui.new_child(UiBuilder::new().max_rect(at).layout(egui::Layout::top_down(egui::Align::Min)));
    tooltip::bubble_noted(&mut child, theme, keymap, name, None, Some(reason));
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
            let reason = tr!("Not yet: choose a message first");
            let control = pin().unavailable(reason).show(ui, theme, keymap).rect;
            reason_under(ui, theme, keymap, control, tr!("Pin"), reason);
        });
    });
    ui.add_space(REASON_ROOM);
    // Not on the sheet: the pressed look of a selected button (see docs/gaps.md).
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("selected-pressed"), |ui| {
            pin().active(true).shown(Shown::Pressed).show(ui, theme, keymap);
        });
    });
}

/// The text button's board: each tone in default, hover, focus, pressed, disabled and unavailable; then with the
/// arrow, with its key, with an icon, and with a label cut at its widest.
fn text_buttons(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    title(ui, theme, tr!("Text button"));
    let tones = [
        (tr!("Send"), Tone::Primary, tr!("Not yet: add a recipient first")),
        (tr!("Reply all"), Tone::Secondary, tr!("Not yet: the message has one recipient")),
        (tr!("Delete"), Tone::Danger, tr!("Not yet: choose a message first")),
        (tr!("Discard"), Tone::Plain, tr!("Not yet: nothing is written")),
    ];
    let states = [(tr!("default"), Shown::Rest), (tr!("hover"), Shown::Hover), (tr!("focus"), Shown::Focus), (tr!("pressed"), Shown::Pressed)];
    for (label, tone, reason) in tones {
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
                let control = TextButton::new(label).tone(tone).unavailable(reason).show(ui, theme, keymap).rect;
                reason_under(ui, theme, keymap, control, label, reason);
            });
        });
        ui.add_space(REASON_ROOM);
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
        let long = tr!("Empfangsbestätigung für diese Nachricht anfordern");
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

/// The section label's board: default, active, hover (a label that folds its section).
fn section_labels(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Section label"));
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("default"), |ui| {
            label::section_label(ui, theme, "[ 01 ]", tr!("Mailboxes"), false);
        });
        cell(ui, theme, tr!("active"), |ui| {
            label::section_label(ui, theme, "[ 01 ]", tr!("Mailboxes"), true);
        });
        cell(ui, theme, tr!("hover"), |ui| {
            // The fill reaches 4 beside the label: room for it in the cell.
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                label::section_label_with(ui, theme, "[ 01 ]", tr!("Mailboxes"), false, true, Some(Shown::Hover));
                ui.add_space(4.0);
            });
        });
    });
    // Not on the sheet: the pressed look of a folding label (see docs/gaps.md).
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("pressed"), |ui| {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                label::section_label_with(ui, theme, "[ 01 ]", tr!("Mailboxes"), false, true, Some(Shown::Pressed));
                ui.add_space(4.0);
            });
        });
    });
}

/// The tooltip's board: default, with its key, with the reason a control cannot be used, and long words that wrap.
fn tooltips(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    title(ui, theme, tr!("Tooltip"));
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("default"), |ui| tooltip::bubble(ui, theme, keymap, tr!("Archive"), None));
        cell(ui, theme, tr!("with-key"), |ui| tooltip::bubble(ui, theme, keymap, tr!("Archive"), Some("mail.archive")));
        cell(ui, theme, tr!("unavailable-reason"), |ui| {
            tooltip::bubble_noted(ui, theme, keymap, tr!("Snooze"), None, Some(tr!("Not yet: choose a message first")));
        });
    });
    cells(ui, theme, |ui| {
        let long = tr!("Snooze is not there for a message that is still being sent. Wait until it has left the outbox.");
        cell(ui, theme, tr!("long"), |ui| tooltip::bubble(ui, theme, keymap, long, None));
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
    section_labels(ui, theme);
    tooltips(ui, theme, keymap);
}

/// The check box's board: off, on, off-hover, on-hover, focus, disabled.
fn checkboxes(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Checkbox"));
    let name = tr!("Keep a copy");
    let states = [
        (tr!("off"), false, Shown::Rest),
        (tr!("on"), true, Shown::Rest),
        (tr!("off-hover"), false, Shown::Hover),
        (tr!("on-hover"), true, Shown::Hover),
        (tr!("focus"), true, Shown::Focus),
    ];
    // Two rows: one is wider than the column.
    let (first, second) = states.split_at(4);
    cells(ui, theme, |ui| {
        for (state, on, shown) in first {
            cell(ui, theme, state, |ui| {
                checkbox::checkbox_with(ui, theme, name, *on, Some(*shown));
            });
        }
    });
    cells(ui, theme, |ui| {
        for (state, on, shown) in second {
            cell(ui, theme, state, |ui| {
                checkbox::checkbox_with(ui, theme, name, *on, Some(*shown));
            });
        }
        cell(ui, theme, tr!("disabled"), |ui| {
            ui.add_enabled_ui(false, |ui| checkbox::checkbox(ui, theme, name, false));
        });
    });
}

/// The radio button's board: off, on, hover, on-hover (which the sheet does not draw), focus, disabled.
fn radios(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Radio"));
    let name = tr!("Tell the guests");
    let states = [
        (tr!("off"), false, Shown::Rest),
        (tr!("on"), true, Shown::Rest),
        (tr!("hover"), false, Shown::Hover),
        (tr!("on-hover"), true, Shown::Hover),
        (tr!("focus"), true, Shown::Focus),
    ];
    // Two rows: one is wider than the column.
    let (first, second) = states.split_at(3);
    cells(ui, theme, |ui| {
        for (state, on, shown) in first {
            cell(ui, theme, state, |ui| {
                checkbox::radio_with(ui, theme, name, *on, Some(*shown));
            });
        }
    });
    cells(ui, theme, |ui| {
        for (state, on, shown) in second {
            cell(ui, theme, state, |ui| {
                checkbox::radio_with(ui, theme, name, *on, Some(*shown));
            });
        }
        cell(ui, theme, tr!("disabled"), |ui| {
            ui.add_enabled_ui(false, |ui| checkbox::radio(ui, theme, name, false));
        });
    });
}

/// The switch's board: off, on, hover, on-hover (which the sheet does not draw), focus, disabled, disabled-on, waiting,
/// and unavailable with its reason.
fn switches(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    title(ui, theme, tr!("Switch"));
    let name = tr!("Start with the computer");
    cells(ui, theme, |ui| {
        let states = [
            (tr!("off"), SwitchState::Off, Shown::Rest),
            (tr!("on"), SwitchState::On, Shown::Rest),
            (tr!("hover"), SwitchState::Off, Shown::Hover),
            (tr!("on-hover"), SwitchState::On, Shown::Hover),
            (tr!("focus"), SwitchState::On, Shown::Focus),
        ];
        for (state, stands, shown) in states {
            cell(ui, theme, state, |ui| {
                switch::switch_with(ui, theme, name, stands, Some(shown));
            });
        }
        for (state, stands) in [(tr!("disabled"), SwitchState::Off), (tr!("disabled-on"), SwitchState::On)] {
            cell(ui, theme, state, |ui| {
                ui.add_enabled_ui(false, |ui| switch::switch_in(ui, theme, name, stands));
            });
        }
        cell(ui, theme, tr!("waiting"), |ui| {
            switch::switch_in(ui, theme, name, SwitchState::Waiting);
        });
    });
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("unavailable"), |ui| {
            switch::switch_noted(ui, theme, name, SwitchState::Unavailable, Some(tr!("Not there in the Flatpak yet")));
            // The tooltip the switch shows under the pointer, drawn in place as the sheet draws it.
            tooltip::bubble_noted(ui, theme, keymap, name, None, Some(tr!("Not there in the Flatpak yet")));
        });
    });
}

/// The slider's board: default (on 110 %), hover, focus, dragging (from 110 % towards 120 %), disabled, and at rest on
/// 90 %, 100 % and 130 %.
fn sliders(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Slider"));
    let at = |index: usize| TextScale::ALL.get(index).copied().unwrap_or_default();
    let states = [
        (tr!("default"), at(2), Some(Shown::Rest), None),
        (tr!("hover"), at(2), Some(Shown::Hover), None),
        (tr!("focus"), at(2), Some(Shown::Focus), None),
        (tr!("dragging"), at(2), Some(Shown::Pressed), Some(at(3))),
        (tr!("disabled"), at(2), None, None),
        (tr!("at-90"), at(0), Some(Shown::Rest), None),
        (tr!("at-100"), at(1), Some(Shown::Rest), None),
        (tr!("at-130"), at(4), Some(Shown::Rest), None),
    ];
    // Two to a row: three are wider than the column.
    for pair in states.chunks(2) {
        cells(ui, theme, |ui| {
            for (state, current, shown, towards) in pair {
                cell(ui, theme, state, |ui| {
                    ui.add_enabled_ui(shown.is_some(), |ui| slider::text_size_with(ui, theme, *current, *shown, *towards));
                });
            }
        });
    }
}

/// The toggles' column of boards.
fn controls_left(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    checkboxes(ui, theme);
    radios(ui, theme);
    switches(ui, theme, keymap);
    sliders(ui, theme);
}

/// The segmented control's board: default, hover (the second option), focus, pressed (the second option), disabled,
/// and a long option cut at its widest.
fn segments(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Segmented"));
    let font = theme.fonts.button;
    let views = [(0, tr!("Grouped"), font), (1, tr!("Smart"), font), (2, tr!("List"), font)];
    cells(ui, theme, |ui| {
        let states = [(tr!("default"), None), (tr!("hover"), Some((Shown::Hover, 1))), (tr!("focus"), Some((Shown::Focus, 0)))];
        for (state, shown) in states {
            cell(ui, theme, state, |ui| {
                segmented::segmented_with(ui, theme, &views, 0, None, shown);
            });
        }
    });
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("pressed"), |ui| {
            segmented::segmented_with(ui, theme, &views, 0, None, Some((Shown::Pressed, 1)));
        });
        cell(ui, theme, tr!("disabled"), |ui| {
            ui.add_enabled_ui(false, |ui| segmented(ui, theme, &views, 0));
        });
    });
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("long-label"), |ui| {
            let long = [(0, tr!("Grouped"), font), (1, tr!("A summary of everything in the inbox"), font), (2, tr!("List"), font)];
            segmented(ui, theme, &long, 0);
        });
    });
}

/// The pill's board: default, hover, focus, selected, disabled; then pressed, off and selected, which the sheet does
/// not draw.
fn pills(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Pill"));
    let name = tr!("New senders");
    cells(ui, theme, |ui| {
        let states = [
            (tr!("default"), false, Shown::Rest),
            (tr!("hover"), false, Shown::Hover),
            (tr!("focus"), false, Shown::Focus),
            (tr!("selected"), true, Shown::Rest),
        ];
        for (state, selected, shown) in states {
            cell(ui, theme, state, |ui| {
                pill::pill_with(ui, theme, name, selected, Some(shown));
            });
        }
        cell(ui, theme, tr!("disabled"), |ui| {
            ui.add_enabled_ui(false, |ui| pill::pill(ui, theme, name, false));
        });
    });
    cells(ui, theme, |ui| {
        for (state, selected) in [(tr!("pressed"), false), (tr!("selected-pressed"), true)] {
            cell(ui, theme, state, |ui| {
                pill::pill_with(ui, theme, name, selected, Some(Shown::Pressed));
            });
        }
    });
}

/// The select's board: default, hover, focus, open (the field alone, its list being a popup's), disabled, unavailable
/// with its reason; then pressed, which the sheet does not draw.
fn selects(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    title(ui, theme, tr!("Select"));
    let names = [(0, tr!("Every week")), (1, tr!("Every month"))];
    // One name on every cell: each in its own id scope, as a select's id is made from its name.
    let select = Select { name: tr!("Repeat"), shown: tr!("Every week"), options: &names, current: Some(0), icon: IconId::Changed, width: 200.0 };
    cells(ui, theme, |ui| {
        for (state, shown) in [(tr!("default"), Shown::Rest), (tr!("hover"), Shown::Hover), (tr!("focus"), Shown::Focus)] {
            cell(ui, theme, state, |ui| {
                ui.push_id(state, |ui| select.show_with(ui, theme, &mut false, None, Some(shown)));
            });
        }
    });
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("open"), |ui| {
            ui.push_id("open", |ui| select.field_open(ui, theme));
        });
        cell(ui, theme, tr!("disabled"), |ui| {
            ui.push_id("disabled", |ui| ui.add_enabled_ui(false, |ui| select.show(ui, theme, &mut false)));
        });
        cell(ui, theme, tr!("pressed"), |ui| {
            ui.push_id("pressed", |ui| select.show_with(ui, theme, &mut false, None, Some(Shown::Pressed)));
        });
    });
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("unavailable"), |ui| {
            let calendar = Select { name: tr!("Calendar"), ..select };
            calendar.show_noted(ui, theme, &mut false, Some(tr!("Not yet: add a calendar account first")));
            tooltip::bubble_noted(ui, theme, keymap, tr!("Calendar"), None, Some(tr!("Not yet: add a calendar account first")));
        });
    });
}

/// The choices' column of boards.
fn controls_right(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    segments(ui, theme);
    pills(ui, theme);
    selects(ui, theme, keymap);
}

/// The avatar's board: default, small, large, unknown.
fn avatars(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Avatar"));
    cells(ui, theme, |ui| {
        let people = [
            (tr!("default"), "Tamsin Vale", "tamsin@example.org", AvatarSize::M),
            (tr!("size-s"), "Odile Brannock", "odile@example.org", AvatarSize::S),
            (tr!("size-l"), "Mira Solberg", "mira@example.org", AvatarSize::L),
            (tr!("unknown"), "", "", AvatarSize::M),
        ];
        for (state, name, address, size) in people {
            cell(ui, theme, state, |ui| {
                avatar::avatar(ui, theme, name, address, size.side(theme));
            });
        }
    });
}

/// The badge's board: count, overflow, muted, accent.
fn badges(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Badge"));
    cells(ui, theme, |ui| {
        let counts = [
            (tr!("count"), 7, BadgeKind::Count),
            (tr!("overflow"), 12_480, BadgeKind::Count),
            (tr!("muted"), 12, BadgeKind::Muted),
            (tr!("accent"), 3, BadgeKind::Accent),
        ];
        for (state, n, kind) in counts {
            cell(ui, theme, state, |ui| {
                badge::badge(ui, theme, n, kind);
            });
        }
    });
}

/// The scroll bar's board: rest, hover, dragging, each a strip 12 wide and 96 high with the sheet's thumb.
fn scroll_bars(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Scroll bar"));
    cells(ui, theme, |ui| {
        let bars = [(tr!("rest"), 16.0, Bar::Rest), (tr!("hover"), 16.0, Bar::Hover), (tr!("dragging"), 40.0, Bar::Dragging)];
        for (state, top, bar) in bars {
            cell(ui, theme, state, |ui| {
                let (strip, _) = ui.allocate_exact_size(vec2(theme.sizes.scrollbar, 96.0), egui::Sense::hover());
                let thumb = egui::Rangef::new(strip.top() + top, strip.top() + top + 32.0);
                scroll::paint(ui.painter(), theme, strip, thumb, bar);
            });
        }
    });
}

/// The account bar's board: the six accounts' colours.
fn account_bars(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Account bar"));
    cells(ui, theme, |ui| {
        let accounts = [tr!("acct1"), tr!("acct2"), tr!("acct3"), tr!("acct4"), tr!("acct5"), tr!("acct6")];
        for (account, state) in (0_u64..).zip(accounts) {
            cell(ui, theme, state, |ui| {
                super::account_bar(ui, theme, account);
            });
        }
    });
}

/// The marks' left column.
fn marks_left(ui: &mut Ui, theme: &Theme, _: &Keymap) {
    avatars(ui, theme);
    badges(ui, theme);
    scroll_bars(ui, theme);
    account_bars(ui, theme);
}

/// The icon's board: the 45 icons at 16 px in `muted`, in the sheet's order, eight to a row.
fn icons(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Icon"));
    for row in IconId::ALL.chunks(8) {
        cells(ui, theme, |ui| {
            for id in row {
                let name = icon::name(*id);
                cell(ui, theme, name, |ui| {
                    icon::icon(ui, theme, *id, name);
                });
            }
        });
    }
}

/// The marks' right column.
fn marks_right(ui: &mut Ui, theme: &Theme, _: &Keymap) {
    icons(ui, theme);
}

/// The row's board: the mail row's read, hover, selected and focus states, a row on the panel under the pointer, and
/// the last row of a list, without its line.
fn rows(ui: &mut Ui, theme: &Theme) {
    use super::row::{RowGround, row_with};
    title(ui, theme, tr!("Row"));
    let p = &theme.palette;
    let states = [
        (tr!("read"), RowGround::Surface, false, false, Shown::Rest),
        (tr!("hover"), RowGround::Surface, false, false, Shown::Hover),
        (tr!("selected"), RowGround::Surface, true, false, Shown::Rest),
        (tr!("focus"), RowGround::Surface, false, false, Shown::Focus),
        (tr!("panel-hover"), RowGround::Panel, false, false, Shown::Hover),
        (tr!("last"), RowGround::Surface, false, true, Shown::Rest),
    ];
    for pair in states.chunks(1) {
        cells(ui, theme, |ui| {
            for (state, ground, current, last, shown) in pair {
                cell(ui, theme, state, |ui| {
                    ui.allocate_ui(vec2(440.0, theme.boxes().mail_row), |ui| {
                        ui.set_width(440.0);
                        let fill = if *ground == RowGround::Panel { p.panel } else { p.surface };
                        ui.painter().rect_filled(Rect::from_min_size(ui.cursor().min, vec2(440.0, theme.boxes().mail_row)), 0.0, fill);
                        let id = Id::unique("gallery-row-board").with(*state);
                        let drawn = row_with(ui, theme, id, 56.0, *ground, *current, *last, Some(*shown));
                        let name = format!("{} {state}", tr!("Row"));
                        drawn.response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
                        let text = cut(ui, "Teodor Vasquez", theme.fonts.body, p.ink, 300.0);
                        let at = drawn.rect.left_top() + vec2(34.0, 7.0);
                        ui.painter().galley(at, text, p.ink);
                        drawn.finish(ui, theme);
                    });
                });
            }
        });
    }
}

/// The banner's board: remote pictures, bounce, error, failing, notice and careful, 640 wide, with the design
/// board's words.
fn banners(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use super::banner::{BannerKind, BannerLink, banner};
    title(ui, theme, tr!("Banner"));
    type State<'a> = (&'a str, BannerKind, &'a str, [(&'a str, &'a str); 2], usize);
    let states: [State<'_>; 6] = [
        (
            tr!("remote-pictures"),
            BannerKind::RemotePictures,
            "3 remote pictures are not shown. Loading them tells the sender that you opened this message.",
            [(tr!("load once"), "pictures.once"), ("always for news@brightshelf.example.org", "pictures.always")],
            2,
        ),
        (
            tr!("bounce"),
            BannerKind::Bounce,
            "This message did not arrive: 550 5.1.1 The address ansel@example.org does not exist.",
            [(tr!("open the report"), "bounce.report"), (tr!("edit and send again"), "bounce.again")],
            2,
        ),
        (
            tr!("error"),
            BannerKind::Error,
            "The message could not be moved: the server refused (NO [CANNOT]).",
            [(tr!("try again"), "error.again"), (tr!("dismiss"), "error.dismiss")],
            2,
        ),
        (
            tr!("failing"),
            BannerKind::Failing,
            "Studio refused the sign-in, and one message was not sent.",
            [(tr!("open account"), "failing.account"), (tr!("open outbox"), "failing.outbox")],
            2,
        ),
        (tr!("notice"), BannerKind::Notice, "The text is cut here: the rest is longer than the reader shows.", [("", ""); 2], 0),
        (tr!("careful"), BannerKind::Careful, "The text says “example.org”, but the link goes to example.net.", [("", ""); 2], 0),
    ];
    for (state, kind, text, links, count) in states {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(640.0);
                let links: Vec<BannerLink<'_>> = links.iter().take(count).map(|(name, id)| BannerLink { name, id }).collect();
                banner(ui, theme, keymap, kind, text, &links);
            });
        });
    }
}

/// The settings row's board: default, hover, focus, unavailable (coming soon) and with help, with the motion choice.
fn settings_rows(ui: &mut Ui, theme: &Theme) {
    use super::settings_row::{settings_row_soon, settings_row_with};
    title(ui, theme, tr!("Settings row"));
    let f = theme.fonts.button;
    let choice = |ui: &mut Ui| {
        segmented(ui, theme, &[(0, tr!("full"), f), (1, tr!("reduced"), f), (2, tr!("off"), f)], 1);
    };
    for (state, help, shown) in [
        (tr!("default"), None, Shown::Rest),
        (tr!("hover"), None, Shown::Hover),
        (tr!("focus"), None, Shown::Focus),
        (tr!("with-help"), Some(tr!("Reduced keeps what explains a change and drops the text effects. Off: everything at once.")), Shown::Rest),
    ] {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(640.0);
                settings_row_with(ui, theme, tr!("Motion"), help, false, Some(shown), choice);
            });
        });
    }
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("unavailable"), |ui| {
            ui.set_width(640.0);
            settings_row_soon(ui, theme, tr!("Calendar alerts"), tr!("Comes with the Calendar settings page."));
        });
    });
}

/// The facts' board: default, details open and long text, with the design board's words.
fn facts_board(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use super::facts::{Facts, facts_of};
    title(ui, theme, tr!("Facts"));
    let (from, to, date) = ("Teodor Vasquez  <teodor@example.org>", "studio@example.org", "Mon 16 Nov 2026, 14:02");
    let open = Facts {
        from,
        to,
        date,
        cc: "Marit Solheim  <marit@example.org>",
        reply_to: "teodor@example.org",
        checked: "example.org · SPF pass · DKIM pass",
        id: "<20261116.1402.7f3a@example.org>",
        recipients: None,
    };
    let long = Facts {
        from: "Annelise Vandenberghe-Okonkwo-Halvorssen  <annelise.vandenberghe-okonkwo-halvorssen@studio.example.org>",
        to: "studio@example.org, Marit Solheim, Ines Okafor, Pavel Hruska, Greta Lindqvist, Tobias Wren, Ada Brannock",
        date,
        recipients: Some(12),
        ..Facts::default()
    };
    for (state, shown, opened) in
        [(tr!("default"), Facts { from, to, date, ..Facts::default() }, false), (tr!("details-open"), open, true), (tr!("long-text"), long, false)]
    {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(560.0);
                facts_of(ui, theme, keymap, &shown, opened);
            });
        });
    }
}

/// The left column of the shared rows and blocks.
fn blocks_left(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    rows(ui, theme);
    banners(ui, theme, keymap);
    settings_rows(ui, theme);
    facts_board(ui, theme, keymap);
}

/// The hover actions' board: default, action-hover, focus and unavailable, Done D, Snooze H, Read R and Delete Del.
fn hover_actions(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use crate::keymap::SNOOZE;
    title(ui, theme, tr!("Hover actions"));
    // The page's keymap gives D, R and Del to its own actions; Snooze keeps the app's H, as the sheet shows.
    let (done, read, delete) = ("mail.done", "mail.read", "mail.delete");
    let group = |snooze: Option<&'static str>| {
        [
            Action { icon: IconId::Done, name: tr!("Done"), action: Some(done), unavailable: None },
            Action { icon: IconId::Snooze, name: tr!("Snooze"), action: Some(SNOOZE), unavailable: snooze },
            Action { icon: IconId::MarkRead, name: tr!("Mark as read"), action: Some(read), unavailable: None },
            Action { icon: IconId::Delete, name: tr!("Delete"), action: Some(delete), unavailable: None },
        ]
    };
    let states = [
        (tr!("default"), None, None),
        (tr!("action-hover"), None, Some((0, Shown::Hover))),
        (tr!("focus"), None, Some((0, Shown::Focus))),
        (tr!("unavailable"), Some(tr!("Not yet: the time picker comes with the pickers")), None),
    ];
    for pair in states.chunks(2) {
        cells(ui, theme, |ui| {
            for (state, snooze, forced) in pair {
                cell(ui, theme, state, |ui| {
                    let width = hover::width(ui, theme, keymap, &group(*snooze));
                    let (_, line) = ui.allocate_space(vec2(width, theme.boxes().strip));
                    let id = Id::unique("gallery-hover-actions").with(*state);
                    hover::actions_with(ui, theme, keymap, id, line, true, &group(*snooze), *forced);
                });
            }
        });
    }
}

/// The empty state's board: the bare one, its lines centred, and the one with an action, its lines on the left.
fn empty_states(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use super::empty::{empty_state, empty_state_with};
    title(ui, theme, tr!("Empty state"));
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("default"), |ui| {
            ui.allocate_ui(vec2(440.0, 216.0), |ui| {
                empty_state(
                    ui,
                    theme,
                    tr!("Inbox · nothing to read"),
                    tr!("All read.\nNew mail lands here."),
                    tr!("This folder holds no unread messages. “Show done” brings back what you finished."),
                );
            });
        });
    });
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("with-action"), |ui| {
            ui.allocate_ui(vec2(440.0, 250.0), |ui| {
                let action = TextButton::new(tr!("Add account")).primary().arrow();
                empty_state_with(
                    ui,
                    theme,
                    tr!("No accounts yet"),
                    tr!("Add an account to read its mail here."),
                    tr!("Nothing leaves this computer before you do."),
                    action,
                    keymap,
                );
            });
        });
    });
}

/// The progress line's board: running (the sheet's still frame: the page rests), done and failed.
fn progresses(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use super::progress::{ProgressState, progress_with};
    title(ui, theme, tr!("Progress"));
    cells(ui, theme, |ui| {
        let states = [
            (tr!("running"), tr!("syncing"), ProgressState::Running(0.54), "54 %"),
            (tr!("done"), tr!("synced"), ProgressState::Done, "14:02"),
            (tr!("failed"), tr!("sync failed"), ProgressState::Failed(0.54), tr!("open log")),
        ];
        for (state, word, stands, note) in states {
            cell(ui, theme, state, |ui| {
                progress_with(ui, theme, keymap, word, stands, note, true);
            });
        }
    });
}

/// The check step's board: its eight states in the sheet's order, with the design board's words.
fn check_steps(ui: &mut Ui, theme: &Theme) {
    use super::check_step::{StepState, check_step};
    title(ui, theme, tr!("Check step"));
    let smtp = "smtp.example.org:465 · TLS";
    let states = [
        (tr!("waiting"), StepState::Waiting, "SMTP", smtp, tr!("waiting")),
        (tr!("running"), StepState::Running, "SMTP", smtp, "0.2 s …"),
        (tr!("passed"), StepState::Passed, "IMAP", tr!("sign-in, 6 folders, IDLE CONDSTORE QRESYNC MOVE"), "0.6 s"),
        (tr!("failed"), StepState::Failed, "IMAP", tr!("sign-in"), "1.2 s"),
        (tr!("skipped"), StepState::Skipped, "SMTP", smtp, tr!("skipped")),
        (tr!("not-run"), StepState::NotRun, tr!("Clock"), tr!("compared with the server's"), tr!("not run")),
        (tr!("no-server"), StepState::NoServer, "CalDAV", tr!("calendars"), tr!("no server set")),
        (tr!("not-built"), StepState::NotBuilt, tr!("Push"), tr!("new mail without asking"), tr!("not built yet")),
    ];
    for (state, stands, step, detail, note) in states {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(560.0);
                let message = (stands == StepState::Failed).then_some("refused: [AUTHENTICATIONFAILED] Invalid credentials.");
                check_step(ui, theme, step, detail, note, stands, message);
            });
        });
    }
}

/// The right column of the shared rows and blocks.
fn blocks_right(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    hover_actions(ui, theme, keymap);
    empty_states(ui, theme, keymap);
    progresses(ui, theme, keymap);
    check_steps(ui, theme);
}

/// Draws the page into `ui` in `theme` with the keys of `keymap`.
pub fn page(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let s = theme.spacing;
    let whole = Rect::from_min_size(pos2(0.0, 0.0), size());
    ui.painter().rect_filled(whole, 0.0, theme.palette.bg);
    let mut layers = Panes::begin(ui.painter());
    let half = (WIDE - PICKER - s.s2 * 4.0) / 2.0;
    let board = (WIDE - s.s2 * 3.0) / 2.0;
    let top = TOP - s.s2 * 2.0;
    let columns: [(f32, f32, f32, Draw); 11] = [
        (s.s2, s.s2, half, left),
        (s.s2 * 2.0 + half, s.s2, half, right),
        (s.s2 * 3.0 + half * 2.0, s.s2, PICKER, picker),
        (s.s2, TOP, board, boards_left),
        (s.s2 * 2.0 + board, TOP, board, boards_right),
        (s.s2, TOP + BOARDS, board, controls_left),
        (s.s2 * 2.0 + board, TOP + BOARDS, board, controls_right),
        (s.s2, TOP + BOARDS + CONTROLS, board, marks_left),
        (s.s2 * 2.0 + board, TOP + BOARDS + CONTROLS, board, marks_right),
        (WIDE + s.s2, 0.0, board, blocks_left),
        (WIDE + s.s2 * 2.0 + board, 0.0, board, blocks_right),
    ];
    for (x, y, width, draw) in columns {
        let height = if x >= WIDE {
            whole.height() - s.s2
        } else if y < TOP {
            top
        } else if y < TOP + BOARDS {
            BOARDS - s.s2
        } else if y < TOP + BOARDS + CONTROLS {
            CONTROLS - s.s2
        } else {
            MARKS - s.s2
        };
        let column = Rect::from_min_size(pos2(x.round(), y), vec2(width.floor(), height));
        ui.scope_builder(UiBuilder::new().max_rect(column), |ui| {
            if y < TOP && x < WIDE {
                layers.card(ui, super::margin(s.s4, s.s3), |ui| {
                    ui.set_min_size(ui.available_size());
                    draw(ui, theme, keymap);
                });
            } else {
                // The boards stand on `bg`, as the design's boards do.
                egui::Frame::NONE.inner_margin(super::margin(s.s4, s.s3)).show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    draw(ui, theme, keymap);
                });
            }
        });
    }
    layers.finish(ui.painter(), theme);
}

#[cfg(test)]
#[path = "gallery_tests.rs"]
mod tests;
