// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::Color32;
use inboxcraft_engine::appearance::{Accent, Appearance, CustomAccent};
use serde_json::json;

use super::{Section, Settings, colour};
use crate::theme::Preset;

#[test]
fn opening_fills_the_fields_with_the_colours_in_use() {
    let mut settings = Settings::default();
    assert!(!settings.is_open());
    settings.open(&Appearance { accent: Accent::Preset("citrus"), ..Appearance::default() });
    assert!(settings.is_open());
    assert_eq!(settings.colours(), &["#ff2e63", "#ff8f00", "#ffd000", "#3d5afe"].map(str::to_owned));
    let citrus = *Preset::named("citrus").unwrap();
    assert_eq!(settings.draft(), Some(Preset { id: "custom", ..citrus }));
    settings.close();
    assert!(!settings.is_open());
}

#[test]
fn only_six_hex_digits_are_a_colour() {
    assert_eq!(colour("#2F6bff"), Some(Color32::from_rgb(0x2f, 0x6b, 0xff)));
    assert_eq!(colour("  2f6bff \n"), Some(Color32::from_rgb(0x2f, 0x6b, 0xff)), "the # may be left out");
    for wrong in ["", "#", "#2f6bf", "#2f6bfff", "2f 6b ff", "#gg0000", "##2f6bff", "#2f6bff;", "ﬀﬀﬀ", "#ａｂｃｄｅｆ", "\u{0}"] {
        assert_eq!(colour(wrong), None, "{wrong:?}");
    }
}

#[test]
fn a_draft_with_one_wrong_field_is_no_draft() {
    let mut settings = Settings::default();
    settings.open(&Appearance::default());
    settings.colours_mut()[2] = "orange".to_owned();
    assert_eq!(settings.draft(), None);
    assert_eq!(settings.custom(), None);
    settings.colours_mut()[2] = "FF8800".to_owned();
    assert_eq!(settings.custom().unwrap()[2], "#ff8800", "written the way the command takes it");
}

#[test]
fn a_nudged_accent_leaves_a_note_and_an_untouched_one_none() {
    let mut settings = Settings::default();
    settings.open(&Appearance::default());
    let custom = CustomAccent::from_hex(&["#2f6bff", "#8a3ffc", "#ff2d95", "#3a1d8c"]).unwrap().readable();
    let after = Appearance { accent: Accent::Custom(custom), ..Appearance::default() };
    settings.took(&json!({ "nudged": true, "requested": "#3a1d8c" }), &after);
    let note = settings.note().unwrap();
    assert!(note.contains("#3a1d8c") && note.contains(&custom.accent.hex()), "{note}");
    assert_eq!(settings.colours()[3], custom.accent.hex());
    settings.took(&json!({ "nudged": false }), &after);
    assert_eq!(settings.note(), None);
    settings.took(&json!({ "nudged": true, "requested": 7 }), &after);
    assert_eq!(settings.note(), None, "an answer of another shape says nothing");
}

#[test]
fn following_a_preset_puts_its_colours_in_the_fields_and_drops_the_note() {
    let mut settings = Settings::default();
    let appearance = Appearance { accent: Accent::Preset("aurora"), ..Appearance::default() };
    settings.colours_mut()[0] = "#123456".to_owned();
    settings.follow(&appearance);
    let (draft, aurora) = (settings.draft().unwrap(), Preset::of(&appearance.accent));
    assert_eq!((draft.gradient, draft.accent), (aurora.gradient, aurora.accent));
    assert!(settings.note().is_none());
}

#[test]
fn closing_forgets_the_key_waited_for_the_text_and_the_notes_so_the_sheet_opens_clean() {
    let mut settings = Settings::default();
    settings.open_at(Section::Shortcuts, &Appearance::default());
    settings.find_mut().push_str("cal");
    settings.capture(crate::keymap::CALENDAR);
    assert_eq!(settings.capturing(), Some(crate::keymap::CALENDAR));
    settings.captured(Some("Ctrl+2 was the key of Show the calendar".to_owned()));
    assert_eq!(settings.capturing(), None);
    assert!(settings.keys_note().is_some());
    settings.capture(crate::keymap::MAIL);
    assert_eq!(settings.keys_note(), None, "waiting for a key drops the last note");
    settings.show(Section::Appearance);
    assert_eq!(settings.capturing(), None, "another page stops the wait");
    settings.show(Section::Shortcuts);
    settings.capture(crate::keymap::MAIL);
    settings.close();
    assert!(!settings.is_open());
    assert_eq!((settings.capturing(), settings.find(), settings.keys_note(), settings.note()), (None, "", None, None));
    settings.find_mut().push_str("left over");
    settings.open(&Appearance::default());
    assert_eq!((settings.section(), settings.find()), (Section::Appearance, ""), "opening starts clean too");
}

#[test]
fn a_signature_typed_shows_until_it_is_saved_or_the_sheet_closes() {
    let mut settings = Settings::default();
    settings.open_at(Section::Sending, &Appearance::default());
    assert_eq!(settings.signature(1), None);
    assert_eq!(settings.signature_mut(1, "Mira").as_str(), "Mira", "the one the account has");
    settings.signature_mut(1, "Mira").push_str("\nPlot 14");
    assert_eq!(settings.signature_mut(1, "ignored").as_str(), "Mira\nPlot 14", "typed text stays");
    settings.saved_signature(1);
    assert_eq!(settings.signature(1), None);
    settings.signature_mut(2, "").push_str("Studio");
    settings.close();
    assert_eq!(settings.signature(2), None, "closing forgets what was not saved");
    let long = "ő".repeat(super::MAX_SIGNATURE_BYTES);
    assert!(settings.signature_mut(3, &long).len() <= super::MAX_SIGNATURE_BYTES, "cut to what the engine keeps");
}

#[test]
fn the_note_on_starting_with_the_computer_lasts_until_the_sheet_opens_again() {
    let mut settings = Settings::default();
    settings.open_at(Section::Notifications, &Appearance::default());
    assert_eq!(settings.login_note(), None);
    settings.set_login_note(Some("Not changed: the portal refused.".to_owned()));
    settings.show(Section::Triage);
    assert_eq!(settings.login_note(), Some("Not changed: the portal refused."), "kept while the sheet is open");
    settings.close();
    assert_eq!(settings.login_note(), None);
    settings.set_login_note(Some("again".to_owned()));
    settings.open_at(Section::Notifications, &Appearance::default());
    assert_eq!(settings.login_note(), None);
}
