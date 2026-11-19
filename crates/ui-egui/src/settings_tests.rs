// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Section, Settings};

#[test]
fn showing_and_leaving_the_place() {
    let mut settings = Settings::default();
    assert!(!settings.is_open());
    settings.open();
    assert!(settings.is_open());
    assert_eq!(settings.section(), Section::Appearance);
    settings.leave();
    assert!(!settings.is_open());
}

#[test]
fn leaving_ends_the_key_waited_for_and_the_notes_and_keeps_the_page_and_the_text() {
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
    settings.leave();
    assert!(!settings.is_open());
    assert_eq!((settings.capturing(), settings.find(), settings.keys_note()), (None, "cal", None));
    settings.open();
    assert_eq!((settings.section(), settings.find()), (Section::Shortcuts, "cal"), "the place comes back as it was left");
    settings.open_page(Some("accounts"));
    assert_eq!(settings.section(), Section::Accounts);
    for junk in [Some("Accounts"), Some(""), Some("accounts\u{0}"), Some("\u{202e}privacy"), None] {
        settings.open_page(junk);
        assert_eq!(settings.section(), Section::Accounts, "{junk:?}: the page last shown");
    }
}

#[test]
fn a_signature_typed_shows_until_it_is_saved() {
    let mut settings = Settings::default();
    settings.open_at(Section::Sending);
    assert_eq!(settings.signature(1), None);
    assert_eq!(settings.signature_mut(1, "Mira").as_str(), "Mira", "the one the account has");
    settings.signature_mut(1, "Mira").push_str("\nPlot 14");
    assert_eq!(settings.signature_mut(1, "ignored").as_str(), "Mira\nPlot 14", "typed text stays");
    settings.saved_signature(1);
    assert_eq!(settings.signature(1), None);
    settings.signature_mut(2, "").push_str("Studio");
    settings.leave();
    assert_eq!(settings.signature(2), Some("Studio"), "leaving keeps what was typed and not saved");
    // A stored signature longer than the field holds opens whole, never cut silently: the field lets it be edited
    // down and never grow, and a save the engine will not keep is refused with its reason.
    let long = "ő".repeat(16 * 1024);
    assert_eq!(settings.signature_mut(3, &long).as_str(), long, "whole");
}

#[test]
fn the_note_on_starting_with_the_computer_lasts_until_another_place_shows() {
    let mut settings = Settings::default();
    settings.open_at(Section::Notifications);
    assert_eq!(settings.login_note(), None);
    settings.set_login_note(Some("Not changed: the portal refused.".to_owned()));
    settings.show(Section::Triage);
    assert_eq!(settings.login_note(), Some("Not changed: the portal refused."), "kept while the settings show");
    settings.leave();
    assert_eq!(settings.login_note(), None);
}

#[test]
fn a_typed_signature_outlasts_every_way_to_another_place() {
    let mut settings = Settings::default();
    settings.open_at(Section::Sending);
    settings.signature_mut(1, "Mira").push_str("\nPlot 14");
    settings.show(Section::Appearance);
    assert_eq!(settings.signature(1), Some("Mira\nPlot 14"), "another page keeps it");
    settings.leave();
    settings.open();
    assert_eq!(settings.signature(1), Some("Mira\nPlot 14"), "leaving keeps it");
    settings.open_at(Section::Sending);
    assert_eq!(settings.signature_mut(1, "Mira"), "Mira\nPlot 14", "the field opens on what was typed, not on the kept one");
    settings.saved_signature(1);
    assert_eq!(settings.signature(1), None, "saved, the page shows the kept one");
}
