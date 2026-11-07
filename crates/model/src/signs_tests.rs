// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Precedence, Signs};
use crate::Envelope;

fn read(fields: &[(&str, &str)]) -> Signs {
    Signs::read(fields.iter().copied())
}

#[test]
fn a_plain_header_has_no_signs() {
    let signs = read(&[("From", "anna@example.org"), ("Subject", "Lunch"), ("Content-Type", "text/plain")]);
    assert_eq!(signs, Signs::default());
    assert!(signs.is_empty());
}

#[test]
fn any_list_field_marks_list_mail() {
    for name in ["List-Id", "list-unsubscribe", " LIST-POST", "List-Help", "List-Subscribe", "List-Owner", "List-Archive"] {
        assert!(read(&[(name, "<x.example.org>")]).list(), "{name}");
    }
    assert!(!read(&[("X-List", "yes"), ("Listen", "now")]).list());
}

#[test]
fn precedence_is_read_from_its_first_word_without_case() {
    for (value, precedence) in [
        ("  BULK ; whatever", Some(Precedence::Bulk)),
        ("list", Some(Precedence::List)),
        ("Junk (an answer)", Some(Precedence::Junk)),
        ("first-class", Some(Precedence::Other)),
        (" ; nothing", None),
        ("", None),
    ] {
        assert_eq!(read(&[("Precedence", value)]).precedence(), precedence, "{value:?}");
    }
}

#[test]
fn auto_submitted_is_automatic_unless_it_says_no() {
    for (value, automatic) in [
        ("auto-generated", true),
        ("Auto-Replied; owner-email=\"x@example.org\"", true),
        ("auto-notified (by a robot)", true),
        ("No", false),
        ("no (a person)", false),
        ("(only a comment)", false),
    ] {
        assert_eq!(read(&[("Auto-Submitted", value)]).automatic(), automatic, "{value:?}");
    }
}

#[test]
fn a_calendar_message_is_told_by_its_media_type() {
    assert!(read(&[("Content-Type", "Text/Calendar; method=REQUEST; charset=utf-8")]).calendar_type());
    assert!(!read(&[("Content-Type", "multipart/alternative; boundary=\"b\"")]).calendar_type());
    assert!(!read(&[("Content-Type", "text/calendars")]).calendar_type());
}

#[test]
fn the_first_of_a_repeated_field_counts() {
    let signs = read(&[
        ("Precedence", "list"),
        ("Precedence", "junk"),
        ("Auto-Submitted", "no"),
        ("Auto-Submitted", "auto-generated"),
        ("Content-Type", "text/plain"),
        ("Content-Type", "text/calendar"),
    ]);
    assert_eq!(signs.precedence(), Some(Precedence::List));
    assert!(!signs.automatic());
    assert!(!signs.calendar_type());
}

#[test]
fn hostile_values_read_as_any_other_word() {
    let long = "x".repeat(300_000);
    let signs = read(&[("Precedence", &long), ("Auto-Submitted", "\u{0}\u{202e}"), ("Content-Type", "ÁRVÍZ/tűrő")]);
    assert_eq!(signs.precedence(), Some(Precedence::Other));
    assert!(signs.automatic());
    assert!(!signs.calendar_type());
}

#[test]
fn the_calendar_part_is_told_apart_from_the_header() {
    let signs = Signs::default().with_calendar(true);
    assert!(signs.calendar());
    assert!(!signs.calendar_type());
    assert!(!signs.is_empty());
    assert!(!signs.with_calendar(false).calendar());
}

#[test]
fn an_envelope_keeps_its_signs_and_a_stored_one_without_them_reads() {
    let signs = read(&[("List-Id", "<news.example.org>"), ("Precedence", "bulk"), ("Auto-Submitted", "auto-generated")]).with_calendar(true);
    let envelope = Envelope::new("News").with_signs(signs);
    assert_eq!(envelope.signs(), &signs);
    let stored = serde_json::to_string(&envelope).unwrap();
    let back: Envelope = serde_json::from_str(&stored).unwrap();
    assert_eq!(back, envelope);
    let mut old = serde_json::to_value(Envelope::new("Old")).unwrap();
    old.as_object_mut().unwrap().remove("signs");
    let old: Envelope = serde_json::from_value(old).unwrap();
    assert_eq!(old.signs(), &Signs::default());
    let partial: Signs = serde_json::from_value(serde_json::json!({ "list": true })).unwrap();
    assert_eq!(partial, read(&[("List-Id", "x")]));
}

#[test]
fn a_summary_takes_its_header_read_again() {
    let summary = crate::Summary::new(crate::Uid::new(4).unwrap(), Envelope::new("Invite"));
    let envelope = summary.envelope().clone().with_signs(Signs::default().with_calendar(true));
    let again = summary.with_envelope(envelope.clone());
    assert_eq!(again.envelope(), &envelope);
    assert_eq!(again.uid().get(), 4);
}
