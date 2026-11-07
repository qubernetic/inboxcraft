// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The snooze field written on top of a message. The commands, the return and the stage are tested through the
//! session in `cmd/snooze_tests.rs`.

use inboxcraft_time::Timestamp;

use super::with_field;

#[test]
fn the_field_goes_on_top_in_utc() {
    let until = Timestamp::from_unix(1_793_901_600, 0).unwrap();
    let copy = with_field(b"Subject: x\r\n\r\nBody", until).unwrap();
    assert_eq!(copy, b"InboxCraft-Snoozed-Until: Thu, 5 Nov 2026 18:00:00 +0000\r\nSubject: x\r\n\r\nBody");
}

#[test]
fn a_time_no_mail_date_can_say_writes_no_field() {
    assert_eq!(with_field(b"Subject: x\r\n\r\n", Timestamp::MAX), None);
}

#[test]
fn the_field_replaces_an_earlier_one_on_top() {
    let until = Timestamp::from_unix(1_793_901_600, 0).unwrap();
    let snoozed_twice = b"InboxCraft-Snoozed-Until: Wed, 4 Nov 2026 08:00:00 +0000\r\n\
        inboxcraft-snoozed-until: Tue, 3 Nov 2026\r\n 08:00:00 +0000\r\n\
        Subject: x\r\n\r\nBody";
    let copy = with_field(snoozed_twice, until).unwrap();
    assert_eq!(copy, b"InboxCraft-Snoozed-Until: Thu, 5 Nov 2026 18:00:00 +0000\r\nSubject: x\r\n\r\nBody");
}

#[test]
fn only_the_fields_on_top_go() {
    let until = Timestamp::from_unix(1_793_901_600, 0).unwrap();
    let below = b"Subject: x\r\nInboxCraft-Snoozed-Until: Wed, 4 Nov 2026 08:00:00 +0000\r\n\r\nInboxCraft-Snoozed-Until: in the body";
    let copy = with_field(below, until).unwrap();
    assert_eq!(copy, [b"InboxCraft-Snoozed-Until: Thu, 5 Nov 2026 18:00:00 +0000\r\n".as_slice(), below].concat());
    let bare = with_field(b"InboxCraft-Snoozed-Until: Wed, 4 Nov 2026 08:00:00 +0000\nSubject: x\n\nBody", until).unwrap();
    assert_eq!(bare, b"InboxCraft-Snoozed-Until: Thu, 5 Nov 2026 18:00:00 +0000\r\nSubject: x\n\nBody", "a bare LF ends a line too");
    let only = with_field(b"InboxCraft-Snoozed-Until: Wed, 4 Nov 2026 08:00:00 +0000", until).unwrap();
    assert_eq!(only, b"InboxCraft-Snoozed-Until: Thu, 5 Nov 2026 18:00:00 +0000\r\n", "a source that is nothing but the field");
}
