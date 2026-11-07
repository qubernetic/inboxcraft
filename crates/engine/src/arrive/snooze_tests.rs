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
