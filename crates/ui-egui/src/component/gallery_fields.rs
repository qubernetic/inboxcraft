// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The second page of the gallery, for `ui_shot` (`"components": true, "page": "fields"`): the boards of the
//! fields, pickers and menus (text, multiline and password fields, the recipient chip and line, the popup panel, the
//! menu item and choice, the suggestion row, the typed time and the time picker), every state of each sheet in its
//! order, labelled, with the design boards' words, on `bg`, in three columns. The data is invented; the keys come
//! from [`super::gallery::keymap`].

use egui::{Id, Rect, Ui, UiBuilder, Vec2, pos2, vec2};
use inboxcraft_time::{CivilDateTime, Offset, Timestamp};

use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::Shown;
use super::field::{self, Field, Secret};
use super::popup::{self, Item, ItemState};
use super::recipient::{self, Recipient, RecipientState, Recipients};
use super::soon::soon;
use super::time_picker::{self, Ask, Preset, TimePicker};
use super::{galley, typed_time};

/// The width of each of the page's three columns, in points.
const COLUMN: f32 = 760.0;

/// The page's height, in points: its tallest column.
const HEIGHT: f32 = 2400.0;

/// The page's size in points.
#[must_use]
pub fn size() -> Vec2 {
    vec2(COLUMN * 3.0, HEIGHT)
}

/// The time the page is drawn at: Friday 6 November 2026, 14:30 UTC, as the first page's.
fn now() -> Timestamp {
    super::gallery::now()
}

/// A time in November 2026, UTC.
fn at(day: u8, hour: u8, minute: u8) -> Timestamp {
    CivilDateTime::new(2026, 11, day, hour, minute, 0).and_then(|civil| civil.to_timestamp(Offset::UTC)).unwrap_or(Timestamp::UNIX_EPOCH)
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

/// A menu row of a board: its icon, name, caption, action and state.
type MenuRow<'a> = (Option<IconId>, &'a str, Option<&'a str>, Option<&'a str>, ItemState);

/// A cell of the menu item's board: its state's name, the row and what it is drawn in.
type MenuCell<'a> = (&'a str, MenuRow<'a>, Option<Shown>);

/// A cell of the multiline field's board: its state's name, the text, the field and what it is drawn in.
type NoteCell<'a> = (&'a str, String, Field<'a>, Option<Shown>);

/// A cell of the recipient field's board: its state's name, the chips, the text typed, the error, whether it can be
/// used and what it is drawn in.
type LineCell<'a> = (&'a str, Vec<Recipient>, &'a str, Option<&'a str>, bool, Option<Shown>);

/// A cell of the time picker's board: its state's name, the text typed, the alert, whether it is late, and what is
/// forced on it.
type PickerCell<'a> = (&'a str, &'a str, bool, bool, time_picker::Forced);

/// A cell of a field's board, `width` wide.
fn field_cell(ui: &mut Ui, theme: &Theme, state: &str, width: f32, add: impl FnOnce(&mut Ui)) {
    cell(ui, theme, state, |ui| {
        ui.push_id(state, |ui| {
            ui.set_width(width);
            add(ui);
        });
    });
}

/// The text field's board: empty, filled, hover, focus, error, disabled, read-only, long, with the sheet's words.
fn text_fields(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Text field"));
    let name = tr!("Name on sent mail");
    let one = |ui: &mut Ui, value: &str, field: Field<'_>, shown: Option<Shown>| {
        let mut text = value.to_owned();
        let id = ui.make_persistent_id("field");
        field::text_field_with(ui, theme, id, &mut text, &field, shown);
    };
    let field = Field::new(name, tr!("optional"));
    cells(ui, theme, |ui| {
        field_cell(ui, theme, tr!("empty"), 320.0, |ui| one(ui, "", field, None));
        field_cell(ui, theme, tr!("filled"), 320.0, |ui| one(ui, "Odile Brannock", field, None));
    });
    cells(ui, theme, |ui| {
        field_cell(ui, theme, tr!("hover"), 320.0, |ui| one(ui, "Odile Brannock", field, Some(Shown::Hover)));
        field_cell(ui, theme, tr!("focus"), 320.0, |ui| one(ui, "Odile Brannock", field, Some(Shown::Focus)));
    });
    cells(ui, theme, |ui| {
        let port = Field { error: Some(tr!("A port is a number from 1 to 65535.")), ..Field::new(tr!("Port"), "") };
        field_cell(ui, theme, tr!("error"), 320.0, |ui| one(ui, "99999", port, None));
        field_cell(ui, theme, tr!("disabled"), 320.0, |ui| one(ui, "Odile Brannock", Field { enabled: false, ..field }, None));
    });
    cells(ui, theme, |ui| {
        let address = Field { read_only: true, ..Field::new(tr!("Address"), "") };
        field_cell(ui, theme, tr!("read-only"), 320.0, |ui| one(ui, "odile@example.org", address, None));
        let caldav = Field::new(tr!("CalDAV address, for calendars"), "");
        field_cell(ui, theme, tr!("long"), 320.0, |ui| one(ui, "https://dav.example.org/calendars/odile/shared-with-the-studio/", caldav, None));
    });
}

/// The notes of the multiline field's board, line by line.
const NOTES: [&str; 16] = [
    "Bring the printed proofs.",
    "The side door is open from 14:00.",
    "Parking is behind the mill.",
    "As the hall is cold, bring a jumper.",
    "Tea and coffee are on the long table.",
    "The proofs go back with Teodor.",
    "Leave the wet sketches on the rack.",
    "The lift is out; the stairs are left.",
    "We close the shutters at 18:00.",
    "Odile has the key to the back room.",
    "The plotter is on the first floor.",
    "Lunch is at the bakery on the corner.",
    "Bring your own pens for the marking.",
    "The new paper samples came on Monday.",
    "Ask Mirela about the frames.",
    "Thank you, all of you.",
];

/// The multiline field's board: empty, filled, focus, error, disabled, read-only, grown, at-limit, scrolling,
/// scrolling-focus.
fn multiline_fields(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Multiline field"));
    let notes = Field::new(tr!("Notes for the guests"), tr!("Anything they should know"));
    let first = |count: usize| NOTES.iter().take(count).copied().collect::<Vec<_>>().join("\n");
    let one = |ui: &mut Ui, value: String, field: Field<'_>, shown: Option<Shown>| {
        let id = ui.make_persistent_id("field");
        let mut text = value;
        field::multiline_with(ui, theme, id, &mut text, &field, shown);
    };
    let error = Field { error: Some(tr!("Too long for a note: 200 characters at most, this has 236.")), ..notes };
    let rows: [[NoteCell<'_>; 2]; 5] = [
        [(tr!("empty"), String::new(), notes, None), (tr!("filled"), first(2), notes, None)],
        [(tr!("focus"), first(2) + "\n", notes, Some(Shown::Focus)), (tr!("error"), first(5), error, None)],
        [
            (tr!("disabled"), first(2), Field { enabled: false, ..notes }, None),
            (tr!("read-only"), first(2), Field { read_only: true, ..notes }, None),
        ],
        [(tr!("grown"), first(6), notes, None), (tr!("at-limit"), first(10), notes, None)],
        [(tr!("scrolling"), first(16), notes, None), (tr!("scrolling-focus"), first(16), notes, Some(Shown::Focus))],
    ];
    for row in rows {
        cells(ui, theme, |ui| {
            for (state, value, field, shown) in row {
                field_cell(ui, theme, state, 320.0, |ui| one(ui, value, field, shown));
            }
        });
    }
}

/// The fields' left column of boards.
fn fields_left(ui: &mut Ui, theme: &Theme, _: &Keymap) {
    ui.push_id("text-field", |ui| text_fields(ui, theme));
    ui.push_id("multiline-field", |ui| multiline_fields(ui, theme));
    ui.push_id("recipient-chip", |ui| recipient_chips(ui, theme));
    ui.push_id("recipient-field", |ui| recipient_fields(ui, theme));
}

/// The recipient field's board: empty, focus, typing, with-chips, invalid, disabled, wrapped, each 560 wide.
fn recipient_fields(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Recipient field"));
    let people = |names: &[&str]| -> Vec<Recipient> {
        names
            .iter()
            .map(|name| Recipient {
                shown: (*name).to_owned(),
                spoken: (*name).to_owned(),
                state: if name.ends_with('@') { RecipientState::Invalid } else { RecipientState::Valid },
            })
            .collect()
    };
    let wrapped = ["Teodor Vasquez", "Mirela Anton", "Pavel Hruska", "Bakery on the corner", "Ines Okafor", "Studio, everyone"];
    let states: [LineCell<'_>; 7] = [
        (tr!("empty"), Vec::new(), "", None, true, None),
        (tr!("focus"), Vec::new(), "", None, true, Some(Shown::Focus)),
        (tr!("typing"), Vec::new(), "teo", None, true, Some(Shown::Focus)),
        (tr!("with-chips"), people(&["Teodor Vasquez", "Mirela Anton"]), "", None, true, None),
        (tr!("invalid"), people(&["Mirela Anton", "teodor@"]), "", Some(tr!("teodor@ is not an address")), true, None),
        (tr!("disabled"), people(&["Teodor Vasquez"]), "", None, false, None),
        (tr!("wrapped"), people(&wrapped), "", None, true, Some(Shown::Focus)),
    ];
    for (state, chips, typed, error, enabled, shown) in states {
        cells(ui, theme, |ui| {
            field_cell(ui, theme, state, 560.0, |ui| {
                let line = Recipients { name: tr!("To"), chips: &chips, hint: tr!("Name or address"), copies: true, error, enabled };
                let mut typing = typed.to_owned();
                let id = ui.make_persistent_id("to");
                recipient::recipient_field_with(ui, theme, id, &mut typing, &line, shown);
            });
        });
    }
}

/// The recipient chip's board: valid, invalid, hover, focus, long, disabled; and pressed, which the sheet does not
/// draw.
fn recipient_chips(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Recipient chip"));
    let teodor = "Teodor Vasquez";
    let one = |ui: &mut Ui, name: &str, state: RecipientState, shown: Option<Shown>| {
        ui.push_id(ui.next_auto_id(), |ui| recipient::chip_with(ui, theme, name, name, state, shown));
    };
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("valid"), |ui| one(ui, teodor, RecipientState::Valid, None));
        cell(ui, theme, tr!("invalid"), |ui| one(ui, "teodor@", RecipientState::Invalid, None));
        cell(ui, theme, tr!("hover"), |ui| one(ui, teodor, RecipientState::Valid, Some(Shown::Hover)));
        cell(ui, theme, tr!("focus"), |ui| one(ui, teodor, RecipientState::Valid, Some(Shown::Focus)));
    });
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("long"), |ui| one(ui, "Bakery on the corner, orders and deliveries", RecipientState::Valid, None));
        cell(ui, theme, tr!("disabled"), |ui| {
            ui.add_enabled_ui(false, |ui| one(ui, teodor, RecipientState::Valid, None));
        });
        cell(ui, theme, tr!("pressed"), |ui| one(ui, teodor, RecipientState::Valid, Some(Shown::Pressed)));
    });
}

/// The password field's board: empty, filled, focus, error, disabled, stored. The dots stand for an invented password.
fn password_fields(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Password field"));
    let typed = "kettle-on-12";
    let password = Field::new(tr!("Password"), "");
    let one = |ui: &mut Ui, value: &str, field: Field<'_>, secret: Secret, shown: Option<Shown>| {
        let id = ui.make_persistent_id("field");
        let mut text = value.to_owned();
        field::password_field_with(ui, theme, id, &mut text, &field, secret, shown);
    };
    cells(ui, theme, |ui| {
        field_cell(ui, theme, tr!("empty"), 320.0, |ui| one(ui, "", password, Secret::Typed, None));
        field_cell(ui, theme, tr!("filled"), 320.0, |ui| one(ui, typed, password, Secret::Typed, None));
    });
    cells(ui, theme, |ui| {
        field_cell(ui, theme, tr!("focus"), 320.0, |ui| one(ui, typed, password, Secret::Typed, Some(Shown::Focus)));
        let refused = Field { error: Some(tr!("The server refused this password.")), ..password };
        field_cell(ui, theme, tr!("error"), 320.0, |ui| one(ui, typed, refused, Secret::Typed, None));
    });
    cells(ui, theme, |ui| {
        field_cell(ui, theme, tr!("disabled"), 320.0, |ui| one(ui, typed, Field { enabled: false, ..password }, Secret::Typed, None));
        let stored = Field::new(tr!("New password"), tr!("empty keeps the stored one"));
        field_cell(ui, theme, tr!("stored"), 320.0, |ui| one(ui, "", stored, Secret::Stored, None));
    });
}

/// A menu row of the boards: `icon`, `name` and the key of `action`, in `state`, drawn in `shown`.
fn menu_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, row: MenuRow<'_>, shown: Option<Shown>) {
    let (icon, name, caption, action, state) = row;
    popup::menu_item_with(ui, theme, keymap, &Item { icon, name, caption, action, state }, false, shown);
}

/// The popup panel's board: default (a hovered row, a hairline, a danger row), scrolling (a title with the count and
/// a thumb), empty.
fn popup_panels(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    title(ui, theme, tr!("Popup panel"));
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("default"), |ui| {
            popup::panel(ui, theme, 240.0, |ui| {
                menu_row(ui, theme, keymap, (Some(IconId::Mail), tr!("Mark as read"), None, None, ItemState::Default), None);
                menu_row(
                    ui,
                    theme,
                    keymap,
                    (Some(IconId::Archive), tr!("Archive"), None, Some("mail.archive"), ItemState::Default),
                    Some(Shown::Hover),
                );
                menu_row(ui, theme, keymap, (Some(IconId::Snooze), tr!("Snooze…"), None, None, ItemState::Default), None);
                menu_row(ui, theme, keymap, (Some(IconId::Move), tr!("Move to…"), None, None, ItemState::Default), None);
                popup::hairline(ui, theme);
                menu_row(ui, theme, keymap, (Some(IconId::Delete), tr!("Delete"), None, None, ItemState::Danger), None);
            });
        });
        cell(ui, theme, tr!("scrolling"), |ui| {
            popup::panel(ui, theme, 240.0, |ui| {
                popup::title(ui, theme, tr!("Move to"), Some("40"));
                popup::scrolled(ui, theme, Id::unique("gallery-move-to"), 147.0, |ui| {
                    let names = [
                        tr!("Archive"),
                        tr!("Clients"),
                        tr!("Clients / Bakery"),
                        tr!("Invoices 2026"),
                        tr!("Newsletters"),
                        tr!("Receipts"),
                        tr!("Studio"),
                    ];
                    for (index, name) in names.into_iter().enumerate() {
                        let shown = (index == 2).then_some(Shown::Hover);
                        ui.push_id(index, |ui| menu_row(ui, theme, keymap, (None, name, None, None, ItemState::Default), shown));
                    }
                });
            });
        });
    });
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("empty"), |ui| {
            popup::panel(ui, theme, 240.0, |ui| popup::empty(ui, theme, tr!("No other mailbox to move this to.")));
        });
    });
}

/// The menu item's board: default, hover, focus, chosen, unavailable, with-key, with-caption, danger, long,
/// from-plugin (a plugin's row is coming soon), and pressed, which the sheet does not draw.
fn menu_items(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    title(ui, theme, tr!("Menu item"));
    let archive = (Some(IconId::Archive), tr!("Archive"), None, None, ItemState::Default);
    let rows: [[MenuCell<'_>; 2]; 5] = [
        [(tr!("default"), archive, None), (tr!("hover"), archive, Some(Shown::Hover))],
        [(tr!("focus"), archive, Some(Shown::Focus)), (tr!("chosen"), (archive.0, archive.1, None, None, ItemState::Chosen), None)],
        [
            (tr!("unavailable"), (Some(IconId::Move), tr!("Move to…"), Some(tr!("no other mailbox")), None, ItemState::Unavailable), None),
            (tr!("with-key"), (Some(IconId::Snooze), tr!("Snooze…"), None, Some("mail.snooze"), ItemState::Default), None),
        ],
        [
            (tr!("with-caption"), (Some(IconId::Snooze), tr!("Tomorrow morning"), Some("Tue 08:00"), None, ItemState::Default), None),
            (tr!("danger"), (Some(IconId::Delete), tr!("Delete"), None, Some("mail.delete"), ItemState::Danger), None),
        ],
        [
            (
                tr!("long"),
                (Some(IconId::Move), tr!("Move to Studio / Clients / Bakery on the corner / 2026"), None, Some("mail.move"), ItemState::Default),
                None,
            ),
            (tr!("pressed"), archive, Some(Shown::Pressed)),
        ],
    ];
    for row in rows {
        cells(ui, theme, |ui| {
            for (state, item, shown) in row {
                field_cell(ui, theme, state, 240.0, |ui| menu_row(ui, theme, keymap, item, shown));
            }
        });
    }
    cells(ui, theme, |ui| {
        field_cell(ui, theme, tr!("from-plugin"), 240.0, |ui| {
            soon(ui, theme, |ui| {
                menu_row(
                    ui,
                    theme,
                    keymap,
                    (Some(IconId::File), tr!("Track this parcel"), Some("parcels.track · no key"), None, ItemState::Default),
                    None,
                )
            });
        });
        field_cell(ui, theme, tr!("chosen-pressed"), 240.0, |ui| {
            menu_row(ui, theme, keymap, (archive.0, archive.1, None, None, ItemState::Chosen), Some(Shown::Pressed))
        });
    });
}

/// The fields' middle column of boards.
fn fields_middle(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    ui.push_id("password-field", |ui| password_fields(ui, theme));
    ui.push_id("popup-panel", |ui| popup_panels(ui, theme, keymap));
    ui.push_id("menu-item", |ui| menu_items(ui, theme, keymap));
    ui.push_id("menu-choice", |ui| menu_choices(ui, theme));
    ui.push_id("suggestion-row", |ui| suggestion_rows(ui, theme));
}

/// The suggestion row's board: default, hover, focus, selected, long; and pressed, which the sheet does not draw.
fn suggestion_rows(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Suggestion row"));
    let (name, address) = ("Teodor Vasquez", "teodor@example.org");
    let rows: [[(&str, bool, Option<Shown>); 2]; 2] = [
        [(tr!("default"), false, None), (tr!("hover"), false, Some(Shown::Hover))],
        [(tr!("focus"), false, Some(Shown::Focus)), (tr!("selected"), true, None)],
    ];
    for row in rows {
        cells(ui, theme, |ui| {
            for (state, selected, shown) in row {
                field_cell(ui, theme, state, 320.0, |ui| {
                    popup::suggestion_row_with(ui, theme, name, address, selected, shown);
                });
            }
        });
    }
    cells(ui, theme, |ui| {
        field_cell(ui, theme, tr!("long"), 320.0, |ui| {
            let long = ("Bakery on the corner, orders and deliveries", "orders-and-deliveries@bakery-on-the-corner.example.org");
            popup::suggestion_row_with(ui, theme, long.0, long.1, false, None);
        });
        field_cell(ui, theme, tr!("pressed"), 320.0, |ui| {
            popup::suggestion_row_with(ui, theme, name, address, false, Some(Shown::Pressed));
        });
    });
}

/// The menu choice's board: off, on, hover, unavailable with its reason; and pressed off and on, which the sheet does
/// not draw.
fn menu_choices(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Menu choice"));
    let week = tr!("Every week");
    cells(ui, theme, |ui| {
        field_cell(ui, theme, tr!("off"), 240.0, |ui| {
            popup::menu_choice_with(ui, theme, week, false, None, None);
        });
        field_cell(ui, theme, tr!("on"), 240.0, |ui| {
            popup::menu_choice_with(ui, theme, week, true, None, None);
        });
    });
    cells(ui, theme, |ui| {
        field_cell(ui, theme, tr!("hover"), 240.0, |ui| {
            popup::menu_choice_with(ui, theme, week, false, None, Some(Shown::Hover));
        });
        field_cell(ui, theme, tr!("unavailable"), 240.0, |ui| {
            popup::menu_choice_with(ui, theme, tr!("Every month"), false, Some(tr!("not synced")), None);
        });
    });
    cells(ui, theme, |ui| {
        field_cell(ui, theme, tr!("pressed"), 240.0, |ui| {
            popup::menu_choice_with(ui, theme, week, false, None, Some(Shown::Pressed));
        });
        field_cell(ui, theme, tr!("on-pressed"), 240.0, |ui| {
            popup::menu_choice_with(ui, theme, week, true, None, Some(Shown::Pressed));
        });
    });
}

/// The typed time's board: empty, focus, valid, invalid; and past, which the time picker's board shows.
fn typed_times(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Typed time"));
    let one = |ui: &mut Ui, value: &str, shown: Option<Shown>| {
        let id = ui.make_persistent_id("field");
        let mut text = value.to_owned();
        typed_time::typed_time_with(ui, theme, id, tr!("Or type a time"), &mut text, now(), Offset::UTC, shown);
    };
    cells(ui, theme, |ui| {
        field_cell(ui, theme, tr!("empty"), 296.0, |ui| one(ui, "", None));
        field_cell(ui, theme, tr!("focus"), 296.0, |ui| one(ui, "", Some(Shown::Focus)));
    });
    cells(ui, theme, |ui| {
        field_cell(ui, theme, tr!("valid"), 296.0, |ui| one(ui, "thu 9", Some(Shown::Focus)));
        field_cell(ui, theme, tr!("invalid"), 296.0, |ui| one(ui, "thursdy", Some(Shown::Focus)));
    });
}

/// The time picker's board: presets, hover, typed-valid, typed-invalid, with-alert, past, empty-hint, from Monday
/// 16 November 2026 at 10:00 (18:00 for past, when Later today has gone), with the sheet's presets.
fn time_pickers(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Time picker"));
    let presets = [
        Preset { name: tr!("Later today"), at: at(16, 17, 0) },
        Preset { name: tr!("Tomorrow morning"), at: at(17, 8, 0) },
        Preset { name: tr!("This weekend"), at: at(21, 9, 0) },
        Preset { name: tr!("Next week"), at: at(23, 8, 0) },
    ];
    let one = |ui: &mut Ui, typed: &str, alert: bool, late: bool, forced: time_picker::Forced| {
        let ask = Ask {
            title: tr!("Snooze until"),
            confirm: tr!("Snooze"),
            alert: alert.then_some(tr!("Notify me when it comes back")),
            presets: &presets,
            extra: None,
            note: None,
        };
        let mut state = TimePicker::with_alert(true);
        state.set_typed(typed);
        let now = at(16, if late { 18 } else { 10 }, 0);
        popup::panel(ui, theme, time_picker::WIDTH, |ui| time_picker::show_with(ui, theme, &mut state, &ask, now, Offset::UTC, forced));
    };
    let typing = time_picker::Forced { hover: None, typing: true };
    let rows: [[PickerCell<'_>; 2]; 3] = [
        [
            (tr!("presets"), "", false, false, time_picker::Forced::default()),
            (tr!("hover"), "", false, false, time_picker::Forced { hover: Some(1), typing: false }),
        ],
        [(tr!("typed-valid"), "thu 9", false, false, typing), (tr!("typed-invalid"), "thursdy", false, false, typing)],
        [(tr!("with-alert"), "thu 9", true, false, typing), (tr!("past"), "2026-11-02 09:00", false, true, typing)],
    ];
    for row in rows {
        cells(ui, theme, |ui| {
            for (state, typed, alert, late, forced) in row {
                cell(ui, theme, state, |ui| {
                    ui.push_id(state, |ui| one(ui, typed, alert, late, forced));
                });
            }
        });
    }
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("empty-hint"), |ui| {
            ui.push_id("empty-hint", |ui| one(ui, "", false, false, typing));
        });
    });
}

/// The fields' right column of boards.
fn fields_right(ui: &mut Ui, theme: &Theme, _: &Keymap) {
    ui.push_id("typed-time", |ui| typed_times(ui, theme));
    ui.push_id("time-picker", |ui| time_pickers(ui, theme));
}

/// What draws a column of the page.
type Draw = fn(&mut Ui, &Theme, &Keymap);

/// Draws the page into `ui` in `theme` with the keys of `keymap`: three columns of boards on `bg`.
pub fn page(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let s = theme.spacing;
    let whole = Rect::from_min_size(pos2(0.0, 0.0), size());
    ui.painter().rect_filled(whole, 0.0, theme.palette.bg);
    let columns: [Draw; 3] = [fields_left, fields_middle, fields_right];
    for (index, draw) in columns.into_iter().enumerate() {
        let column = Rect::from_min_size(pos2(COLUMN * index as f32, 0.0), vec2(COLUMN, HEIGHT));
        ui.scope_builder(UiBuilder::new().max_rect(column), |ui| {
            egui::Frame::NONE.inner_margin(super::margin(s.s4, s.s3)).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                ui.push_id(index, |ui| draw(ui, theme, keymap));
            });
        });
    }
}

#[cfg(test)]
#[path = "gallery_fields_tests.rs"]
mod tests;
