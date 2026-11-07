// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::Calendar;

#[test]
fn the_calendar_holds_no_calendar_before_sync() {
    let calendar = Calendar::default();
    assert!(calendar.is_empty());
    assert!(calendar.calendars().is_empty());
}
