// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_time::{CivilDateTime, Offset, Offsets, Timestamp};
use proptest::prelude::*;

use super::{Calendar, Expansion, MAX_OCCURRENCES, Occurrence};
use crate::zone::tests::BUDAPEST;
use crate::{Event, IcalError, When};

fn civil(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> CivilDateTime {
    CivilDateTime::new(year, month, day, hour, minute, 0).unwrap()
}

fn utc(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Timestamp {
    civil(year, month, day, hour, minute).to_timestamp(Offset::UTC).unwrap()
}

/// A calendar with the Budapest zone and `events` (VEVENT bodies).
fn calendar(events: &[&str]) -> Calendar {
    let mut text = format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//Test//EN\r\n{BUDAPEST}");
    for event in events {
        text.push_str(&format!("BEGIN:VEVENT\r\n{event}END:VEVENT\r\n"));
    }
    text.push_str("END:VCALENDAR\r\n");
    Calendar::parse(text.as_bytes()).unwrap()
}

/// The person's own zone in the tests: three hours west of UTC, no summer time.
fn local() -> Offset {
    Offset::from_minutes(-180).unwrap()
}

fn starts(calendar: &Calendar, from: Timestamp, to: Timestamp) -> Vec<Timestamp> {
    calendar.occurrences(from, to, &local()).occurrences.iter().map(|occurrence| occurrence.start).collect()
}

#[test]
fn reads_and_writes_a_calendar_and_keeps_what_it_cannot_read() {
    let text = format!(
        "BEGIN:VCALENDAR\r\n\
        VERSION:2.0\r\n\
        PRODID:-//Example//Test//EN\r\n\
        METHOD:REQUEST\r\n\
        X-WR-CALNAME:Home\r\n\
        {BUDAPEST}{BUDAPEST}\
        BEGIN:VTIMEZONE\r\nEND:VTIMEZONE\r\n\
        BEGIN:VEVENT\r\nUID:1\r\nDTSTART:20261109T090000Z\r\nEND:VEVENT\r\n\
        BEGIN:VEVENT\r\nUID:2\r\nEND:VEVENT\r\n\
        BEGIN:VTODO\r\nUID:3\r\nEND:VTODO\r\n\
        END:VCALENDAR\r\n"
    );
    let calendar = Calendar::parse(text.as_bytes()).unwrap();
    assert_eq!(calendar.product, "-//Example//Test//EN");
    assert_eq!(calendar.method.as_deref(), Some("REQUEST"));
    assert_eq!(calendar.other.len(), 1);
    assert_eq!(calendar.zones.all().len(), 1);
    assert_eq!(calendar.events.len(), 1);
    let kept: Vec<_> = calendar.components.iter().map(|component| component.name()).collect();
    assert_eq!(kept, ["VTIMEZONE", "VTIMEZONE", "VEVENT", "VTODO"]);
    let written = calendar.write().unwrap();
    assert!(written.starts_with("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//Test//EN\r\nMETHOD:REQUEST\r\nX-WR-CALNAME:Home\r\n"));
    assert_eq!(Calendar::parse(written.as_bytes()).unwrap(), calendar);
}

#[test]
fn a_calendar_needs_a_vcalendar_at_the_top() {
    assert_eq!(Calendar::parse(b"BEGIN:VEVENT\r\nEND:VEVENT\r\n"), Err(IcalError::NotACalendar));
    assert_eq!(Calendar::parse(b"x"), Err(IcalError::BadLine { line: 1 }));
    let empty = Calendar::parse(b"BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n").unwrap();
    assert_eq!(empty, Calendar::new(""));
    let mut built = Calendar::new("-//Qubernetic//InboxCraft//EN");
    built.events.push(Event::new("1@example.org", When::Utc(utc(2026, 1, 1, 0, 0))));
    assert_eq!(Calendar::parse(built.write().unwrap().as_bytes()).unwrap(), built);
}

#[test]
fn a_weekly_event_keeps_its_wall_clock_time_across_summer_time() {
    let calendar =
        calendar(&["UID:1\r\nDTSTART;TZID=Europe/Budapest:20260316T090000\r\nDTEND;TZID=Europe/Budapest:20260316T100000\r\nRRULE:FREQ=WEEKLY\r\n"]);
    let expansion = calendar.occurrences(utc(2026, 3, 16, 0, 0), utc(2026, 4, 1, 0, 0), &local());
    let found: Vec<_> = expansion.occurrences.iter().map(|occurrence| (occurrence.start, occurrence.end)).collect();
    assert_eq!(
        found,
        [
            (utc(2026, 3, 16, 8, 0), utc(2026, 3, 16, 9, 0)),
            (utc(2026, 3, 23, 8, 0), utc(2026, 3, 23, 9, 0)),
            (utc(2026, 3, 30, 7, 0), utc(2026, 3, 30, 8, 0)),
        ]
    );
    assert!(!expansion.truncated);
    assert_eq!(expansion.occurrences[2].recurrence_id, Some(When::Zoned(civil(2026, 3, 30, 9, 0), "Europe/Budapest".into())));
}

#[test]
fn a_single_event_occurs_once_and_only_in_its_span() {
    let calendar = calendar(&["UID:1\r\nDTSTART;TZID=Europe/Budapest:20261109T090000\r\nDURATION:PT30M\r\n"]);
    let expansion = calendar.occurrences(utc(2026, 11, 9, 0, 0), utc(2026, 11, 10, 0, 0), &local());
    assert_eq!(expansion.occurrences, [Occurrence { event: 0, start: utc(2026, 11, 9, 8, 0), end: utc(2026, 11, 9, 8, 30), recurrence_id: None }]);
    assert!(starts(&calendar, utc(2026, 11, 9, 8, 30), utc(2026, 11, 10, 0, 0)).is_empty());
    assert!(starts(&calendar, utc(2026, 11, 8, 0, 0), utc(2026, 11, 9, 8, 0)).is_empty());
    // An occurrence that began before the span and runs into it counts.
    assert_eq!(starts(&calendar, utc(2026, 11, 9, 8, 15), utc(2026, 11, 9, 8, 20)), [utc(2026, 11, 9, 8, 0)]);
}

#[test]
fn an_occurrence_without_length_counts_where_it_starts() {
    let point = calendar(&["UID:1\r\nDTSTART:20261109T090000Z\r\n"]);
    assert_eq!(starts(&point, utc(2026, 11, 9, 9, 0), utc(2026, 11, 9, 10, 0)), [utc(2026, 11, 9, 9, 0)]);
    assert!(starts(&point, utc(2026, 11, 9, 8, 0), utc(2026, 11, 9, 9, 0)).is_empty());
    // An end before the start or a negative duration is no length.
    for body in ["UID:1\r\nDTSTART:20261109T090000Z\r\nDTEND:20261109T080000Z\r\n", "UID:1\r\nDTSTART:20261109T090000Z\r\nDURATION:-PT1H\r\n"] {
        let found = calendar(&[body]).occurrences(utc(2026, 11, 9, 0, 0), utc(2026, 11, 10, 0, 0), &local()).occurrences;
        assert_eq!((found[0].start, found[0].end), (utc(2026, 11, 9, 9, 0), utc(2026, 11, 9, 9, 0)));
    }
}

#[test]
fn whole_days_are_on_the_persons_wall_clock() {
    let calendar = calendar(&[
        "UID:1\r\nDTSTART;VALUE=DATE:20261224\r\n",
        "UID:2\r\nDTSTART;VALUE=DATE:20261224\r\nDTEND;VALUE=DATE:20261227\r\n",
        "UID:3\r\nDTSTART;VALUE=DATE:20261224\r\nDTEND;VALUE=DATE:20261220\r\n",
    ]);
    let found = calendar.occurrences(utc(2026, 12, 1, 0, 0), utc(2027, 1, 1, 0, 0), &local()).occurrences;
    let spans: Vec<_> = found.iter().map(|occurrence| (occurrence.event, occurrence.start, occurrence.end)).collect();
    assert_eq!(
        spans,
        [
            (0, utc(2026, 12, 24, 3, 0), utc(2026, 12, 25, 3, 0)),
            (1, utc(2026, 12, 24, 3, 0), utc(2026, 12, 27, 3, 0)),
            (2, utc(2026, 12, 24, 3, 0), utc(2026, 12, 24, 3, 0)),
        ]
    );
    assert!(calendar.events[0].all_day());
}

#[test]
fn a_duration_in_days_follows_the_wall_clock() {
    let calendar = calendar(&["UID:1\r\nDTSTART;TZID=Europe/Budapest:20260328T120000\r\nDURATION:P1DT1H\r\n"]);
    let found = calendar.occurrences(utc(2026, 3, 28, 0, 0), utc(2026, 3, 30, 0, 0), &local()).occurrences;
    // 12:00 the next day in summer time is 10:00 UTC; the hour on top is exact.
    assert_eq!((found[0].start, found[0].end), (utc(2026, 3, 28, 11, 0), utc(2026, 3, 29, 11, 0)));
}

#[test]
fn exceptions_dates_and_changed_instances_shape_the_set() {
    let calendar = calendar(&[
        "UID:r\r\nDTSTART;TZID=Europe/Budapest:20261102T090000\r\nDURATION:PT1H\r\nRRULE:FREQ=DAILY;COUNT=7\r\n\
         EXDATE;TZID=Europe/Budapest:20261103T090000\r\nEXDATE;VALUE=DATE:20261104\r\n\
         RDATE;TZID=Europe/Budapest:20261102T150000\r\nRDATE;VALUE=DATE:20261110\r\nRDATE:20261111T080000Z\r\n",
        "UID:r\r\nRECURRENCE-ID;TZID=Europe/Budapest:20261105T090000\r\nDTSTART;TZID=Europe/Budapest:20261105T130000\r\nDURATION:PT1H\r\n",
        "UID:r\r\nRECURRENCE-ID:20261106T080000Z\r\nDTSTART;TZID=Europe/Budapest:20261120T090000\r\n",
        "UID:other\r\nRECURRENCE-ID;TZID=Europe/Budapest:20261107T090000\r\nDTSTART;TZID=Europe/Budapest:20261107T100000\r\n",
    ]);
    let found = calendar.occurrences(utc(2026, 11, 1, 0, 0), utc(2026, 11, 15, 0, 0), &local()).occurrences;
    let seen: Vec<_> = found.iter().map(|occurrence| (occurrence.event, occurrence.start)).collect();
    assert_eq!(
        seen,
        [
            (0, utc(2026, 11, 2, 8, 0)),
            (0, utc(2026, 11, 2, 14, 0)),
            (1, utc(2026, 11, 5, 12, 0)),
            (0, utc(2026, 11, 7, 8, 0)),
            (3, utc(2026, 11, 7, 9, 0)),
            (0, utc(2026, 11, 8, 8, 0)),
            (0, utc(2026, 11, 9, 23, 0)),
            (0, utc(2026, 11, 11, 8, 0)),
        ]
    );
    // The date added to a timed event keeps the time of day; the changed instance knows which one it replaced.
    assert_eq!(found[2].recurrence_id, Some(When::Zoned(civil(2026, 11, 5, 9, 0), "Europe/Budapest".into())));
    assert_eq!(found[6].recurrence_id, Some(When::Zoned(civil(2026, 11, 10, 0, 0), "Europe/Budapest".into())));
    // The instance moved out of the span shows where it went.
    assert_eq!(starts(&calendar, utc(2026, 11, 20, 0, 0), utc(2026, 11, 21, 0, 0)), [utc(2026, 11, 20, 8, 0)]);
}

#[test]
fn an_until_ends_the_rule_on_the_events_wall_clock() {
    let cases = [
        ("DTSTART;TZID=Europe/Budapest:20261101T090000\r\nRRULE:FREQ=DAILY;UNTIL=20261103T080000Z\r\n", 3),
        ("DTSTART;TZID=Europe/Budapest:20261101T090000\r\nRRULE:FREQ=DAILY;UNTIL=20261103T075959Z\r\n", 2),
        ("DTSTART;TZID=Europe/Budapest:20261101T090000\r\nRRULE:FREQ=DAILY;UNTIL=20261103\r\n", 3),
        ("DTSTART;VALUE=DATE:20261101\r\nRRULE:FREQ=DAILY;UNTIL=20261103\r\n", 3),
        ("DTSTART:20261101T090000\r\nRRULE:FREQ=DAILY;UNTIL=20261103T085959\r\n", 2),
        ("DTSTART:20261101T090000Z\r\nRRULE:FREQ=DAILY;COUNT=4\r\n", 4),
        ("DTSTART:20261101T090000Z\r\nRRULE:FREQ=DAILY;COUNT=2\r\nRRULE:FREQ=DAILY;INTERVAL=5;COUNT=2\r\n", 3),
    ];
    for (body, count) in cases {
        let calendar = calendar(&[&format!("UID:1\r\n{body}")]);
        assert_eq!(starts(&calendar, utc(2026, 10, 1, 0, 0), utc(2027, 1, 1, 0, 0)).len(), count, "{body}");
    }
}

#[test]
fn a_rule_from_long_ago_is_found_quickly_in_a_late_span() {
    let calendar = calendar(&["UID:1\r\nDTSTART:19000101T090000\r\nRRULE:FREQ=DAILY\r\n"]);
    assert_eq!(starts(&calendar, utc(2026, 11, 9, 0, 0), utc(2026, 11, 10, 0, 0)), [utc(2026, 11, 9, 12, 0)]);
    // A span that reaches the end of time still ends.
    let found = calendar.occurrences(utc(9999, 12, 30, 0, 0), Timestamp::MAX, &Offset::from_minutes(600).unwrap());
    assert_eq!(found.occurrences.first().map(|occurrence| occurrence.start), Some(utc(9999, 12, 30, 23, 0)));
    assert!(!found.truncated);
}

#[test]
fn limits_end_an_expansion_and_say_so() {
    let many = calendar(&["UID:1\r\nDTSTART:20260101T000000Z\r\nRRULE:FREQ=SECONDLY\r\n"]);
    let expansion = many.occurrences(utc(2026, 1, 1, 0, 0), utc(2026, 1, 2, 0, 0), &local());
    assert_eq!(expansion.occurrences.len(), MAX_OCCURRENCES);
    assert!(expansion.truncated);
    let walked = calendar(&["UID:1\r\nDTSTART:20260101T000000Z\r\nRRULE:FREQ=SECONDLY;COUNT=4000000000\r\n"]);
    let expansion = walked.occurrences(utc(2026, 3, 1, 0, 0), utc(2026, 3, 2, 0, 0), &local());
    assert!(expansion.occurrences.is_empty());
    assert!(expansion.truncated);
    let never = calendar(&["UID:1\r\nDTSTART:20260101T000000Z\r\nRRULE:FREQ=DAILY;BYMONTH=2;BYMONTHDAY=30\r\n"]);
    // A rule that never matches stops at the end of the span, so nothing is left out.
    let expansion = never.occurrences(utc(2026, 3, 1, 0, 0), utc(2026, 3, 2, 0, 0), &local());
    assert_eq!(expansion, Expansion { occurrences: Vec::new(), truncated: false });
}

#[test]
fn floating_and_unknown_zones_are_on_the_persons_wall_clock() {
    let calendar = calendar(&[
        "UID:1\r\nDTSTART:20261109T090000\r\n",
        "UID:2\r\nDTSTART;TZID=Mars/Olympus:20261109T090000\r\n",
        "UID:3\r\nDTSTART;TZID=UTC:20261109T090000\r\n",
    ]);
    assert_eq!(
        starts(&calendar, utc(2026, 11, 9, 0, 0), utc(2026, 11, 10, 0, 0)),
        [utc(2026, 11, 9, 9, 0), utc(2026, 11, 9, 12, 0), utc(2026, 11, 9, 12, 0)]
    );
}

fn rule() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![
        "FREQ=DAILY",
        "FREQ=DAILY;INTERVAL=3",
        "FREQ=WEEKLY;BYDAY=MO,TH",
        "FREQ=MONTHLY;BYDAY=-1FR",
        "FREQ=MONTHLY;BYMONTHDAY=31",
        "FREQ=YEARLY;BYMONTH=3,10;BYDAY=-1SU",
        "FREQ=HOURLY;INTERVAL=5",
        "FREQ=DAILY;COUNT=30",
    ])
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// What a span shows is what a wider span shows, cut to the span.
    #[test]
    fn a_span_shows_what_a_wider_span_shows_cut_to_it(
        rule in rule(),
        zoned in any::<bool>(),
        days_in in 0i64..400,
        length_minutes in 0i64..3000,
        span_days in 1i64..40,
    ) {
        let start = if zoned { "DTSTART;TZID=Europe/Budapest:20260115T093000" } else { "DTSTART:20260115T093000" };
        let body = format!("UID:p\r\n{start}\r\nDURATION:PT{length_minutes}M\r\nRRULE:{rule}\r\n");
        let calendar = calendar(&[&body]);
        let day = |days: i64| Timestamp::from_unix(utc(2026, 1, 1, 0, 0).unix_seconds() + days * 86_400, 0).unwrap();
        let (from, to) = (day(days_in), day(days_in + span_days));
        let narrow = calendar.occurrences(from, to, &local());
        let wide = calendar.occurrences(day(0), day(days_in + span_days + 40), &local());
        let cut: Vec<_> = wide.occurrences.into_iter().filter(|occurrence| super::overlaps(occurrence.start, occurrence.end, (from, to))).collect();
        prop_assert_eq!(narrow.occurrences, cut);
    }

    /// A weekly event in a zone with summer time always starts at the same wall-clock time there.
    #[test]
    fn a_zoned_rule_keeps_its_wall_clock(weeks in 0i64..520) {
        let calendar = calendar(&["UID:1\r\nDTSTART;TZID=Europe/Budapest:20200106T093000\r\nRRULE:FREQ=WEEKLY\r\n"]);
        let from = Timestamp::from_unix(utc(2020, 1, 6, 0, 0).unix_seconds() + weeks * 7 * 86_400, 0).unwrap();
        let to = Timestamp::from_unix(from.unix_seconds() + 7 * 86_400, 0).unwrap();
        let found = calendar.occurrences(from, to, &local()).occurrences;
        prop_assert_eq!(found.len(), 1);
        let zone = calendar.zones.get("Europe/Budapest").unwrap();
        let wall = zone.civil(found[0].start).unwrap();
        prop_assert_eq!((wall.hour(), wall.minute()), (9, 30));
    }
}

#[test]
fn a_rule_without_a_dtstart_match_still_counts_dtstart() {
    // The DTSTART is the first instance even where the rule would not make it.
    let calendar = calendar(&["UID:1\r\nDTSTART:20261103T090000Z\r\nRRULE:FREQ=WEEKLY;BYDAY=MO;COUNT=2\r\n"]);
    assert_eq!(
        starts(&calendar, utc(2026, 11, 1, 0, 0), utc(2026, 12, 1, 0, 0)),
        [utc(2026, 11, 3, 9, 0), utc(2026, 11, 9, 9, 0), utc(2026, 11, 16, 9, 0)]
    );
}
