// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_store::Change;
use inboxcraft_time::Timestamp;

use super::changes;

#[test]
fn the_sample_has_three_calendars_and_eleven_events_on_example_org() {
    let changes = changes(Timestamp::from_unix(1_793_869_200, 0).unwrap()).unwrap();
    let calendars = changes.iter().filter(|change| matches!(change, Change::PutCalendar(_))).count();
    assert_eq!(calendars, 3);
    for change in &changes {
        if let Change::PutCalendarEvent(event) = change {
            assert!(event.href.starts_with("https://dav.example.org/calendars/mira/"));
            assert!(event.ical.contains("@example.org\r\n"));
            assert!(!event.ical.contains("{start}") && !event.ical.contains("{end}"));
        }
    }
    assert_eq!(changes.len(), 14);
}

#[test]
fn a_time_without_a_day_builds_no_sample_calendar() {
    assert!(changes(Timestamp::MAX).is_ok(), "the last day of the calendar still has a week");
    assert!(changes(Timestamp::MIN).is_ok());
}
