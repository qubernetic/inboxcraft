// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Section, Settings};

#[test]
fn opening_and_closing_the_sheet() {
    let mut settings = Settings::default();
    assert!(!settings.is_open());
    settings.open();
    assert!(settings.is_open());
    assert_eq!(settings.section(), Section::Appearance);
    settings.close();
    assert!(!settings.is_open());
}

#[test]
fn closing_forgets_the_key_waited_for_the_text_and_the_notes_so_the_sheet_opens_clean() {
    let mut settings = Settings::default();
    settings.open_at(Section::Shortcuts);
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
    assert_eq!((settings.capturing(), settings.find(), settings.keys_note()), (None, "", None));
    settings.find_mut().push_str("left over");
    settings.open();
    assert_eq!((settings.section(), settings.find()), (Section::Appearance, ""), "opening starts clean too");
}

#[test]
fn a_signature_typed_shows_until_it_is_saved_or_the_sheet_closes() {
    let mut settings = Settings::default();
    settings.open_at(Section::Sending);
    assert_eq!(settings.signature(1), None);
    assert_eq!(settings.signature_mut(1, "Mira").as_str(), "Mira", "the one the account has");
    settings.signature_mut(1, "Mira").push_str("\nPlot 14");
    assert_eq!(settings.signature_mut(1, "ignored").as_str(), "Mira\nPlot 14", "typed text stays");
    settings.saved_signature(1);
    assert_eq!(settings.signature(1), None);
    settings.signature_mut(2, "").push_str("Studio");
    settings.close();
    assert_eq!(settings.signature(2), None, "closing forgets what was not saved");
    // A stored signature longer than the field holds opens whole, never cut silently: the field lets it be edited
    // down and never grow, and a save the engine will not keep is refused with its reason.
    let long = "ő".repeat(16 * 1024);
    assert_eq!(settings.signature_mut(3, &long).as_str(), long, "whole");
}

#[test]
fn the_note_on_starting_with_the_computer_lasts_until_the_sheet_opens_again() {
    let mut settings = Settings::default();
    settings.open_at(Section::Notifications);
    assert_eq!(settings.login_note(), None);
    settings.set_login_note(Some("Not changed: the portal refused.".to_owned()));
    settings.show(Section::Triage);
    assert_eq!(settings.login_note(), Some("Not changed: the portal refused."), "kept while the sheet is open");
    settings.close();
    assert_eq!(settings.login_note(), None);
    settings.set_login_note(Some("again".to_owned()));
    settings.open_at(Section::Notifications);
    assert_eq!(settings.login_note(), None);
}
