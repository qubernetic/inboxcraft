// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{format, text};

#[test]
fn english_text_is_its_own_message_id() {
    assert_eq!(text("Inbox"), "Inbox");
    assert_eq!(crate::tr!("Search mail"), "Search mail");
}

#[test]
fn named_placeholders_are_filled_in() {
    assert_eq!(crate::tr!("{count} unread", count = 3), "3 unread");
    assert_eq!(crate::tr!("{name} <{address}>", name = "Mira", address = "mira@example.org",), "Mira <mira@example.org>");
    assert_eq!(format("{a}{a}", &[("a", &1)]), "11", "every occurrence");
}

#[test]
fn unknown_and_broken_placeholders_stay_as_written() {
    assert_eq!(format("{missing} and {", &[("count", &1)]), "{missing} and {");
    assert_eq!(format("}{count", &[("count", &1)]), "}{count");
    assert_eq!(format("{}", &[]), "{}");
}

#[test]
fn a_value_is_never_read_as_a_placeholder() {
    // Message text (a subject, a name) is data: a brace in it is shown, never expanded.
    assert_eq!(format("{a} {b}", &[("a", &"{b}"), ("b", &"x")]), "{b} x");
}

#[test]
fn text_outside_ascii_survives() {
    assert_eq!(format("Ünnep {név} ✓", &[("név", &"Éva")]), "Ünnep Éva ✓");
}
