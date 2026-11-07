// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_time::{CivilDateTime, Offset, Timestamp};
use proptest::prelude::*;

use super::{Alarm, Attendee, Class, Event, PartStat, Status, Trigger};
use crate::{Component, Date, Frequency, IcalError, Property, Rule, Span, When, parse};

fn civil(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> CivilDateTime {
    CivilDateTime::new(year, month, day, hour, minute, 0).unwrap()
}

fn utc(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Timestamp {
    civil(year, month, day, hour, minute).to_timestamp(Offset::UTC).unwrap()
}

fn read(text: &str) -> Result<Event, IcalError> {
    Event::from_component(&parse(text.as_bytes()).unwrap())
}

const FULL: &str = "BEGIN:VEVENT\r\n\
    UID:42@example.org\r\n\
    SEQUENCE:3\r\n\
    DTSTAMP:20261101T120000Z\r\n\
    DTSTART;TZID=Europe/Budapest:20261109T090000\r\n\
    DTEND;TZID=Europe/Budapest:20261109T100000\r\n\
    SUMMARY:Stand-up\\, daily\r\n\
    DESCRIPTION:Line one\\nLine two\r\n\
    LOCATION:Room 1\\; east\r\n\
    STATUS:confirmed\r\n\
    TRANSP:TRANSPARENT\r\n\
    CLASS:PRIVATE\r\n\
    RRULE:FREQ=WEEKLY;BYDAY=MO,WE\r\n\
    RDATE;TZID=Europe/Budapest:20261121T090000\r\n\
    EXDATE;TZID=Europe/Budapest:20261111T090000,20261116T090000\r\n\
    EXDATE;VALUE=DATE:20261118\r\n\
    ORGANIZER;CN=Anna Kovacs:mailto:anna@example.org\r\n\
    ATTENDEE;CN=Bela;ROLE=REQ-PARTICIPANT;PARTSTAT=ACCEPTED;RSVP=TRUE:mailto:bela@example.org\r\n\
    ATTENDEE;PARTSTAT=X-PONDERING:mailto:cili@example.org\r\n\
    ATTENDEE:mailto:dora@example.org\r\n\
    X-COLOUR:green\r\n\
    BEGIN:VALARM\r\n\
    ACTION:DISPLAY\r\n\
    TRIGGER:-PT10M\r\n\
    DESCRIPTION:Soon\r\n\
    X-KEEP:1\r\n\
    END:VALARM\r\n\
    BEGIN:VALARM\r\n\
    ACTION:AUDIO\r\n\
    TRIGGER;RELATED=END:PT5M\r\n\
    END:VALARM\r\n\
    BEGIN:VALARM\r\n\
    ACTION:EMAIL\r\n\
    TRIGGER;VALUE=DATE-TIME:20261109T070000Z\r\n\
    END:VALARM\r\n\
    BEGIN:VALARM\r\n\
    ACTION:DISPLAY\r\n\
    END:VALARM\r\n\
    BEGIN:X-THING\r\n\
    END:X-THING\r\n\
    END:VEVENT\r\n";

#[test]
fn reads_every_field_it_knows_and_keeps_the_rest() {
    let event = read(FULL).unwrap();
    let budapest = |civil| When::Zoned(civil, "Europe/Budapest".into());
    assert_eq!(event.uid, "42@example.org");
    assert_eq!(event.sequence, 3);
    assert_eq!(event.stamp, Some(utc(2026, 11, 1, 12, 0)));
    assert_eq!(event.start, budapest(civil(2026, 11, 9, 9, 0)));
    assert_eq!(event.end, Some(budapest(civil(2026, 11, 9, 10, 0))));
    assert_eq!(event.duration, None);
    assert_eq!(event.summary.as_deref(), Some("Stand-up, daily"));
    assert_eq!(event.description.as_deref(), Some("Line one\nLine two"));
    assert_eq!(event.location.as_deref(), Some("Room 1; east"));
    assert_eq!(event.status, Some(Status::Confirmed));
    assert!(event.free);
    assert_eq!(event.class, Some(Class::Private));
    assert_eq!(event.rules, [Rule::parse("FREQ=WEEKLY;BYDAY=MO,WE").unwrap()]);
    assert_eq!(event.dates, [budapest(civil(2026, 11, 21, 9, 0))]);
    assert_eq!(
        event.exceptions,
        [budapest(civil(2026, 11, 11, 9, 0)), budapest(civil(2026, 11, 16, 9, 0)), When::Date(Date::new(2026, 11, 18).unwrap())]
    );
    assert_eq!(event.recurrence_id, None);
    let organizer = event.organizer.as_ref().unwrap();
    assert_eq!((organizer.address.as_str(), organizer.name.as_deref()), ("mailto:anna@example.org", Some("Anna Kovacs")));
    assert_eq!(
        event.attendees[0],
        Attendee {
            address: "mailto:bela@example.org".into(),
            name: Some("Bela".into()),
            role: Some("REQ-PARTICIPANT".into()),
            status: PartStat::Accepted,
            rsvp: true,
        }
    );
    assert_eq!(event.attendees[1].status, PartStat::Other("X-PONDERING".into()));
    assert_eq!(event.attendees[2], Attendee::new("mailto:dora@example.org"));
    assert_eq!(event.other, [Property::new("X-COLOUR", "green")]);
    assert_eq!(event.alarms.len(), 3);
    assert_eq!(event.alarms[0].trigger, Trigger::Start(Span::new(0, -600).unwrap()));
    assert_eq!(event.alarms[0].description.as_deref(), Some("Soon"));
    assert_eq!(event.alarms[0].other, [Property::new("X-KEEP", "1")]);
    assert_eq!(event.alarms[1].trigger, Trigger::End(Span::new(0, 300).unwrap()));
    assert_eq!(event.alarms[2].trigger, Trigger::At(utc(2026, 11, 9, 7, 0)));
    // An alarm without a trigger, and a component it does not know, are kept as they came.
    assert_eq!(event.components.iter().map(Component::name).collect::<Vec<_>>(), ["VALARM", "X-THING"]);
    assert!(!event.all_day());
}

#[test]
fn reads_a_short_all_day_event() {
    let event = read("BEGIN:VEVENT\r\nUID:1\r\nDTSTART;VALUE=DATE:20261224\r\nDURATION:P2D\r\nSTATUS:x-maybe\r\nCLASS:x-family\r\nTRANSP:OPAQUE\r\nRECURRENCE-ID;VALUE=DATE:20261224\r\nEND:VEVENT\r\n").unwrap();
    assert!(event.all_day());
    assert_eq!(event.duration, Some(Span::new(2, 0).unwrap()));
    assert_eq!(event.status, Some(Status::Other("x-maybe".into())));
    assert_eq!(event.class, Some(Class::Other("x-family".into())));
    assert!(!event.free);
    assert_eq!(event.recurrence_id, Some(When::Date(Date::new(2026, 12, 24).unwrap())));
    assert_eq!(event.sequence, 0);
    assert_eq!(event.stamp, None);
}

#[test]
fn refuses_events_it_cannot_read() {
    let event = |body: &str| read(&format!("BEGIN:VEVENT\r\n{body}END:VEVENT\r\n"));
    let start = "UID:1\r\nDTSTART:20261109T090000Z\r\n";
    assert_eq!(event("DTSTART:20261109T090000Z\r\n"), Err(IcalError::Missing { component: "VEVENT", property: "UID" }));
    assert_eq!(event("UID:1\r\n"), Err(IcalError::Missing { component: "VEVENT", property: "DTSTART" }));
    assert_eq!(event("UID:1\r\nDTSTART:x\r\n"), Err(IcalError::bad_value("DTSTART")));
    for (line, name) in [
        ("SEQUENCE:-1", "SEQUENCE"),
        ("DTSTAMP:20261109T090000", "DTSTAMP"),
        ("DTSTAMP:x", "DTSTAMP"),
        ("DTEND:x", "DTEND"),
        ("DURATION:x", "DURATION"),
        ("RRULE:FREQ=x", "RRULE"),
        ("RDATE:x", "RDATE"),
        ("EXDATE:x", "EXDATE"),
        ("RECURRENCE-ID:x", "RECURRENCE-ID"),
    ] {
        assert_eq!(event(&format!("{start}{line}\r\n")), Err(IcalError::bad_value(name)), "{line}");
    }
}

#[test]
fn refuses_alarms_it_cannot_read() {
    let alarm = |body: &str| Alarm::from_component(&parse(format!("BEGIN:VALARM\r\n{body}END:VALARM\r\n").as_bytes()).unwrap());
    assert_eq!(alarm("TRIGGER:-PT5M\r\n"), Err(IcalError::Missing { component: "VALARM", property: "ACTION" }));
    assert_eq!(alarm("ACTION:DISPLAY\r\n"), Err(IcalError::Missing { component: "VALARM", property: "TRIGGER" }));
    assert_eq!(alarm("ACTION:DISPLAY\r\nTRIGGER:soon\r\n"), Err(IcalError::bad_value("TRIGGER")));
    assert_eq!(alarm("ACTION:DISPLAY\r\nTRIGGER;VALUE=DATE-TIME:20261109T070000\r\n"), Err(IcalError::bad_value("TRIGGER")));
    assert_eq!(alarm("ACTION:DISPLAY\r\nTRIGGER;VALUE=DATE-TIME:x\r\n"), Err(IcalError::bad_value("TRIGGER")));
}

#[test]
fn writes_an_event() {
    let mut event = Event::new("7@example.org", When::Date(Date::new(2026, 12, 24).unwrap()));
    event.duration = Some(Span::new(1, 0).unwrap());
    event.sequence = 2;
    event.stamp = Some(utc(2026, 11, 7, 12, 0));
    event.summary = Some("Eve; dinner, family".into());
    event.status = Some(Status::Tentative);
    event.class = Some(Class::Confidential);
    event.free = true;
    event.rules.push(Rule::new(Frequency::Yearly));
    event.organizer = Some(Attendee { name: Some("Anna".into()), ..Attendee::new("mailto:anna@example.org") });
    event.attendees.push(Attendee { status: PartStat::Declined, rsvp: true, role: Some("CHAIR".into()), ..Attendee::new("mailto:bela@example.org") });
    event.alarms.push(Alarm::display(Trigger::Start(Span::new(-1, 0).unwrap()), "Tomorrow"));
    let text = event.to_component().write().unwrap();
    for line in [
        "UID:7@example.org",
        "DTSTAMP:20261107T120000Z",
        "DTSTART;VALUE=DATE:20261224",
        "DURATION:P1D",
        "SEQUENCE:2",
        "SUMMARY:Eve\\; dinner\\, family",
        "STATUS:TENTATIVE",
        "TRANSP:TRANSPARENT",
        "CLASS:CONFIDENTIAL",
        "RRULE:FREQ=YEARLY",
        "ORGANIZER;CN=Anna:mailto:anna@example.org",
        "ATTENDEE;ROLE=CHAIR;PARTSTAT=DECLINED;RSVP=TRUE:mailto:bela@example.org",
        "BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-P1D\r\nDESCRIPTION:Tomorrow\r\nEND:VALARM",
    ] {
        assert!(text.contains(line), "{line} not in {text}");
    }
    // With an end the duration is left out.
    event.end = Some(When::Date(Date::new(2026, 12, 25).unwrap()));
    let text = event.to_component().write().unwrap();
    assert!(text.contains("DTEND;VALUE=DATE:20261225") && !text.contains("DURATION"));
}

fn when() -> impl Strategy<Value = When> {
    let civil = (1990i32..2040, 1u8..=12, 1u8..=28, 0u8..24, 0u8..60, 0u8..60)
        .prop_map(|(y, mo, d, h, mi, s)| CivilDateTime::new(y, mo, d, h, mi, s).unwrap());
    prop_oneof![
        civil.clone().prop_map(|civil| When::Date(Date::of(civil))),
        civil.clone().prop_map(When::Floating),
        civil.clone().prop_map(|civil| When::Utc(civil.to_timestamp(Offset::UTC).unwrap())),
        (civil, "[A-Za-z/_;:\"^ ]{1,20}").prop_map(|(civil, tzid)| When::Zoned(civil, tzid)),
    ]
}

fn text() -> impl Strategy<Value = Option<String>> {
    prop::option::of("[ -~éő\n\t]{0,40}")
}

fn attendee() -> impl Strategy<Value = Attendee> {
    let status = prop::sample::select(vec![
        PartStat::NeedsAction,
        PartStat::Accepted,
        PartStat::Declined,
        PartStat::Tentative,
        PartStat::Delegated,
        PartStat::Other("X-LATER".into()),
    ]);
    ("mailto:[a-z]{1,8}@example\\.org", prop::option::of("[ -~é]{0,16}"), prop::option::of("[A-Z-]{1,12}"), status, any::<bool>())
        .prop_map(|(address, name, role, status, rsvp)| Attendee { address, name, role, status, rsvp })
}

fn alarm() -> impl Strategy<Value = Alarm> {
    let span = (-30i64..30, 0i64..86_400).prop_map(|(days, seconds)| Span::new(days, if days < 0 { -seconds } else { seconds }).unwrap());
    let trigger = prop_oneof![
        span.clone().prop_map(Trigger::Start),
        span.prop_map(Trigger::End),
        (0i64..2_000_000_000).prop_map(|secs| Trigger::At(Timestamp::from_unix(secs, 0).unwrap()))
    ];
    (prop::sample::select(vec!["DISPLAY", "AUDIO", "EMAIL"]), trigger, text()).prop_map(|(action, trigger, description)| Alarm {
        action: action.into(),
        trigger,
        description,
        other: Vec::new(),
    })
}

fn event() -> impl Strategy<Value = Event> {
    let texts = (text(), text(), text());
    let kinds = (
        prop::option::of(prop::sample::select(vec![Status::Tentative, Status::Confirmed, Status::Cancelled, Status::Other("X-ON-HOLD".into())])),
        prop::option::of(prop::sample::select(vec![Class::Public, Class::Private, Class::Confidential, Class::Other("X-TEAM".into())])),
        any::<bool>(),
    );
    let times =
        (when(), prop::option::of(when()), prop::option::of(when()), prop::collection::vec(when(), 0..3), prop::collection::vec(when(), 0..3));
    let people = (prop::option::of(attendee()), prop::collection::vec(attendee(), 0..3), prop::collection::vec(alarm(), 0..2));
    let rule =
        prop::option::of(prop::sample::select(vec!["FREQ=DAILY;COUNT=3", "FREQ=WEEKLY;BYDAY=MO,FR", "FREQ=MONTHLY;BYMONTHDAY=-1;UNTIL=20300101"]));
    ("[ -~]{1,30}", any::<u32>(), texts, kinds, times, people, rule).prop_map(
        |(
            uid,
            sequence,
            (summary, description, location),
            (status, class, free),
            (start, end, recurrence_id, dates, exceptions),
            (organizer, attendees, alarms),
            rule,
        )| {
            Event {
                sequence,
                summary,
                description,
                location,
                status,
                class,
                free,
                end,
                recurrence_id,
                dates,
                exceptions,
                organizer,
                attendees,
                alarms,
                rules: rule.map(|rule| Rule::parse(rule).unwrap()).into_iter().collect(),
                other: vec![Property::new("X-SOURCE", "test")],
                ..Event::new(&uid, start)
            }
        },
    )
}

proptest! {
    #[test]
    fn written_events_read_back(event in event()) {
        let text = event.to_component().write().unwrap();
        let back = Event::from_component(&parse(text.as_bytes()).unwrap()).unwrap();
        prop_assert_eq!(back, event);
    }
}
