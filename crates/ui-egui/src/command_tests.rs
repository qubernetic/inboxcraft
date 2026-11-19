// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command line's state: the mode from the first character only, the cap, the pick.

use super::{Command, MOST, Mode};

#[test]
fn the_first_character_alone_sets_the_mode_and_becomes_the_mark() {
    for (typed, mode, left) in [(">snooze", Mode::Command, "snooze"), ("@teo", Mode::Person, "teo"), ("#inv", Mode::Mailbox, "inv")] {
        let mut command = Command::default();
        command.set_text(typed);
        assert_eq!(command.mode(), mode, "{typed}");
        assert_eq!(command.text(), left, "the prefix becomes the mark: {typed}");
    }
    // A prefix later in the text is text; one after the mark too.
    let mut command = Command::default();
    command.set_text("bak>@#");
    assert_eq!((command.mode(), command.text()), (Mode::Search, "bak>@#"));
    let mut command = Command::default();
    command.set_text(">@x");
    assert_eq!((command.mode(), command.text()), (Mode::Command, "@x"));
}

#[test]
fn backspace_in_an_empty_field_drops_the_mark() {
    let mut command = Command::default();
    command.set_text("#");
    assert_eq!((command.mode(), command.text()), (Mode::Mailbox, ""));
    command.drop_scope();
    assert_eq!(command.mode(), Mode::Search);
}

#[test]
fn a_long_text_is_cut_at_the_most_characters() {
    let mut command = Command::default();
    command.set_text(&"é".repeat(10_000));
    assert_eq!(command.text().chars().count(), MOST);
    command.set_text(&format!(">{}", "x".repeat(10_000)));
    assert_eq!(command.mode(), Mode::Command);
    assert_eq!(command.text().chars().count(), MOST - 1, "cut first, then the mark leaves the text");
}

#[test]
fn the_pick_goes_round_and_starts_again_on_a_change() {
    let mut command = Command::default();
    command.step(true, 0);
    assert_eq!(command.picked(), 0);
    command.step(false, 3);
    assert_eq!(command.picked(), 2, "up from the first is the last");
    command.step(true, 3);
    assert_eq!(command.picked(), 0, "down from the last is the first");
    command.step(true, 3);
    command.set_text("x");
    assert_eq!(command.picked(), 0);
}

#[test]
fn people_are_kept_for_the_text_they_were_found_for() {
    let mut command = Command::default();
    command.set_text("@teo");
    assert!(command.people().is_none());
    command.set_people(Vec::new());
    assert_eq!(command.people().map(<[_]>::len), Some(0));
    command.set_text("teod");
    assert!(command.people().is_none(), "another text looks again");
}
