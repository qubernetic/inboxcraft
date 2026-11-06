// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::Color32;
use inboxcraft_engine::appearance::{Accent, Appearance, CustomAccent};
use serde_json::json;

use super::{Settings, colour};
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
