// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An event's own alert: the `alert` parameter of `event.create` and `event.update`, written as one `DISPLAY` alarm
//! before the start, read back by `event.list`, and what the local alerts make of it. Events from the server keep the
//! alarms the field does not show.

use inboxcraft_ical::{Calendar, Span, Trigger};
use inboxcraft_model::{AccountId, CalendarEventId, CalendarId, CalendarReport, Event, EventKind, FetchedEvent};
use inboxcraft_time::{CivilDateTime, FakeClock, Offset, Timestamp};
use serde_json::{Value, json};

use super::calendar_tests::{WORK, object, session_at};
use crate::test_support::{NOW, at};
use crate::{EngineError, Session};

fn session() -> Session {
    session_at(&FakeClock::new(at(NOW)))
}

fn utc(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Timestamp {
    CivilDateTime::new(year, month, day, hour, minute, 0).unwrap().to_timestamp(Offset::UTC).unwrap()
}

/// The calendar object event `id` holds now.
fn held(session: &Session, id: u64) -> Calendar {
    let held = session.store().calendar_event(CalendarEventId::new(id)).unwrap().unwrap();
    Calendar::parse(held.ical.as_bytes()).unwrap()
}

fn text(session: &Session, id: u64) -> String {
    session.store().calendar_event(CalendarEventId::new(id)).unwrap().unwrap().ical
}

/// When event `id`'s local alerts go off, as the store holds them.
fn alerts(session: &Session, id: u64) -> Vec<Timestamp> {
    session.store().alerts(CalendarEventId::new(id)).unwrap().into_iter().map(|alert| alert.at).collect()
}

/// The `alert` event `id`'s occurrences show in the week from Thursday 5 November 2026.
fn shown(session: &mut Session, id: u64) -> Value {
    let list = session.run("event.list", &json!({ "from": "2026-11-05", "to": "2026-11-12" })).unwrap();
    let rows = list["events"].as_array().unwrap();
    let row = rows.iter().find(|row| row["id"] == json!(id)).unwrap();
    row["alert"].clone()
}

/// A dentist's appointment on Friday 6 November 2026 at 14:00 UTC, with `extra` parameters.
fn create(session: &mut Session, extra: Value) -> Result<u64, EngineError> {
    let mut params = json!({ "calendar": 1, "summary": "Dentist", "start": "2026-11-06T14:00:00Z" });
    if let (Value::Object(params), Value::Object(extra)) = (&mut params, extra) {
        params.extend(extra);
    }
    Ok(session.run("event.create", &params)?["id"].as_u64().unwrap())
}

fn synced(session: &mut Session, name: &str, ical: String) {
    let fetched = FetchedEvent { url: format!("{WORK}{name}.ics"), etag: Some("\"k1\"".to_owned()), ical };
    let synced = CalendarReport::Synced { calendar: CalendarId::new(1), changed: vec![fetched], removed: vec![], token: None, full: false };
    session.apply(&Event::new(AccountId::new(1), None, EventKind::Calendar(synced))).unwrap();
}

/// The kiln firing on Friday 6 November 2026, 14:00 to 15:00 UTC, as another program wrote it: a mail alarm an hour
/// before, a display alarm a quarter of an hour before with a property of its own, a sound five minutes before, and
/// a display alarm whose trigger cannot be read.
fn kiln() -> String {
    object(
        "BEGIN:VEVENT\r\nUID:kiln@example.org\r\nDTSTART:20261106T140000Z\r\nDTEND:20261106T150000Z\r\nSUMMARY:Kiln\r\n\
         BEGIN:VALARM\r\nACTION:EMAIL\r\nTRIGGER:-PT1H\r\nDESCRIPTION:Mail me\r\nSUMMARY:Kiln\r\nATTENDEE:mailto:anna@example.org\r\nEND:VALARM\r\n\
         BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nDESCRIPTION:Kiln soon\r\nX-EXAMPLE-KEEP:yes\r\nEND:VALARM\r\n\
         BEGIN:VALARM\r\nACTION:AUDIO\r\nTRIGGER:-PT5M\r\nEND:VALARM\r\n\
         BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:soon\r\nDESCRIPTION:Junk\r\nEND:VALARM\r\n\
         END:VEVENT\r\n",
    )
}

/// The kiln firing without its sound alarm: one display alarm the field can show.
fn kiln_quiet() -> String {
    kiln().replace("BEGIN:VALARM\r\nACTION:AUDIO\r\nTRIGGER:-PT5M\r\nEND:VALARM\r\n", "")
}

#[test]
fn a_new_event_with_an_alert_carries_one_display_alarm_before_its_start() {
    let mut session = session();
    let id = create(&mut session, json!({ "alert": 30 })).unwrap();
    let calendar = held(&session, id);
    let alarms = &calendar.events[0].alarms;
    assert_eq!(alarms.len(), 1, "{alarms:?}");
    assert_eq!(alarms[0].action, "DISPLAY");
    assert_eq!(alarms[0].trigger, Trigger::Start(Span::new(0, -1800).unwrap()));
    assert!(text(&session, id).contains("TRIGGER:-PT30M\r\n"), "{}", text(&session, id));
    assert_eq!(shown(&mut session, id), json!(30));
    assert_eq!(alerts(&session, id), [utc(2026, 11, 6, 13, 30)], "the local alert follows the event's own");
}

#[test]
fn whole_days_are_written_as_days_and_read_back_in_minutes() {
    let mut session = session();
    let id = create(&mut session, json!({ "alert": 1440 })).unwrap();
    assert!(text(&session, id).contains("TRIGGER:-P1D\r\n"), "{}", text(&session, id));
    assert_eq!(shown(&mut session, id), json!(1440));
    let at_start = create(&mut session, json!({ "alert": 0 })).unwrap();
    assert_eq!(shown(&mut session, at_start), json!(0));
    assert_eq!(alerts(&session, at_start), [utc(2026, 11, 6, 14, 0)]);
}

#[test]
fn a_new_event_without_an_alert_has_no_alarm_and_the_default_lead() {
    let mut session = session();
    let id = create(&mut session, json!({})).unwrap();
    assert!(held(&session, id).events[0].alarms.is_empty());
    assert_eq!(shown(&mut session, id), Value::Null);
    assert_eq!(alerts(&session, id), [utc(2026, 11, 6, 13, 50)], "ten minutes before, the default");
}

#[test]
fn no_alert_is_written_so_the_default_does_not_ring() {
    let mut session = session();
    let id = create(&mut session, json!({ "alert": "none" })).unwrap();
    let calendar = held(&session, id);
    let alarms = &calendar.events[0].alarms;
    assert_eq!(alarms.len(), 1, "{alarms:?}");
    assert_eq!(alarms[0].action, "NONE");
    assert_eq!(shown(&mut session, id), json!("none"));
    assert!(alerts(&session, id).is_empty(), "{:?}", alerts(&session, id));
    // Picking a lead again takes the marker away.
    session.run("event.update", &json!({ "id": id, "alert": 5 })).unwrap();
    let calendar = held(&session, id);
    let actions: Vec<&str> = calendar.events[0].alarms.iter().map(|alarm| alarm.action.as_str()).collect();
    assert_eq!(actions, ["DISPLAY"]);
    assert_eq!(alerts(&session, id), [utc(2026, 11, 6, 13, 55)]);
}

#[test]
fn an_event_from_the_server_shows_its_display_or_sound_alarm() {
    let mut session = session();
    synced(&mut session, "kiln", kiln_quiet());
    assert_eq!(shown(&mut session, 5), json!(15), "the mail alarm is the server's, the junk one is left alone");
    assert_eq!(alerts(&session, 5), [utc(2026, 11, 6, 13, 45)]);
}

#[test]
fn an_update_moves_the_shown_alarm_and_keeps_every_other_one() {
    let mut session = session();
    synced(&mut session, "kiln", kiln());
    let before = held(&session, 5).events[0].alarms.clone();
    session.run("event.update", &json!({ "id": 5, "alert": 60 })).unwrap();
    let calendar = held(&session, 5);
    let alarms = &calendar.events[0].alarms;
    assert_eq!(alarms.len(), 3, "{alarms:?}");
    assert_eq!(alarms[0], before[0], "the mail alarm untouched");
    assert_eq!(alarms[1].trigger, Trigger::Start(Span::new(0, -3600).unwrap()));
    assert_eq!((&alarms[1].action, &alarms[1].description, &alarms[1].other), (&before[1].action, &before[1].description, &before[1].other));
    assert_eq!(alarms[2], before[2], "the sound alarm untouched");
    let written = text(&session, 5);
    assert!(written.contains("TRIGGER:soon\r\n") && written.contains("X-EXAMPLE-KEEP:yes\r\n"), "{written}");
    assert_eq!(shown(&mut session, 5), json!("custom"), "the sound alarm still rings beside the moved one");
    assert_eq!(alerts(&session, 5), [utc(2026, 11, 6, 13, 0), utc(2026, 11, 6, 13, 55)]);
}

#[test]
fn no_alert_on_an_event_from_the_server_takes_its_display_and_sound_alarms_only() {
    let mut session = session();
    synced(&mut session, "kiln", kiln());
    let mail = held(&session, 5).events[0].alarms[0].clone();
    session.run("event.update", &json!({ "id": 5, "alert": "none" })).unwrap();
    let calendar = held(&session, 5);
    assert_eq!(calendar.events[0].alarms, [mail], "the mail alarm stays, no marker beside it");
    assert!(text(&session, 5).contains("TRIGGER:soon\r\n"));
    assert_eq!(shown(&mut session, 5), json!("none"));
    assert!(alerts(&session, 5).is_empty());
}

#[test]
fn an_alarm_the_field_cannot_name_is_custom_until_a_lead_is_picked() {
    let mut session = session();
    let odd = kiln_quiet().replace("TRIGGER:-PT15M", "TRIGGER:-PT90S");
    synced(&mut session, "kiln", odd);
    assert_eq!(shown(&mut session, 5), json!("custom"), "not whole minutes");
    let after = kiln_quiet().replace("TRIGGER:-PT15M", "TRIGGER:PT15M");
    synced(&mut session, "kiln", after);
    assert_eq!(shown(&mut session, 5), json!("custom"), "after the start");
    let after_end = kiln_quiet().replace("TRIGGER:-PT15M", "TRIGGER;RELATED=END:-PT15M");
    synced(&mut session, "kiln", after_end);
    assert_eq!(shown(&mut session, 5), json!("custom"), "from the end");
    assert_eq!(alerts(&session, 5), [utc(2026, 11, 6, 14, 45)]);
    session.run("event.update", &json!({ "id": 5, "alert": 30 })).unwrap();
    let written = text(&session, 5);
    assert!(written.contains("TRIGGER:-PT30M\r\n") && !written.contains("RELATED=END"), "{written}");
    assert_eq!(shown(&mut session, 5), json!(30));
    assert_eq!(alerts(&session, 5), [utc(2026, 11, 6, 13, 30)]);
}

#[test]
fn the_alert_alone_is_a_change() {
    let mut session = session();
    let id = create(&mut session, json!({})).unwrap();
    session.run("event.update", &json!({ "id": id, "alert": 15 })).unwrap();
    assert_eq!(shown(&mut session, id), json!(15));
}

#[test]
fn an_alert_outside_the_choices_is_refused() {
    let mut session = session();
    for alert in [json!(10_081), json!(-5), json!(1.5), json!("soon"), json!("NONE "), json!(true), json!([10]), json!({ "minutes": 10 })] {
        let error = create(&mut session, json!({ "alert": alert })).unwrap_err();
        assert!(matches!(error, EngineError::BadParams { .. }), "{alert}: {error:?}");
        let error = session.run("event.update", &json!({ "id": 1, "alert": alert })).unwrap_err();
        assert!(matches!(error, EngineError::BadParams { .. }), "{alert}: {error:?}");
    }
    assert!(create(&mut session, json!({ "alert": 10_080 })).is_ok(), "a week is the most");
}

/// A daily check-in from Friday 6 November 2026 at 9:00 UTC, three times, as another program wrote it: a display
/// alarm a quarter of an hour before, Saturday's moved to 10:00 with an alarm of its own, Sunday's changed without one.
fn checkin() -> String {
    object(
        "BEGIN:VEVENT\r\nUID:checkin@example.org\r\nDTSTAMP:20261101T080000Z\r\nDTSTART:20261106T090000Z\r\nDURATION:PT15M\r\n\
         SUMMARY:Check-in\r\nRRULE:FREQ=DAILY;COUNT=3\r\nSEQUENCE:2\r\n\
         ORGANIZER;CN=Dora:mailto:dora@example.com\r\nATTENDEE;PARTSTAT=ACCEPTED:mailto:anna@example.org\r\n\
         ATTENDEE;PARTSTAT=NEEDS-ACTION:mailto:dora@example.com\r\n\
         BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nDESCRIPTION:Check-in\r\nEND:VALARM\r\nEND:VEVENT\r\n\
         BEGIN:VEVENT\r\nUID:checkin@example.org\r\nRECURRENCE-ID:20261107T090000Z\r\nDTSTAMP:20261101T080000Z\r\n\
         DTSTART:20261107T100000Z\r\nDURATION:PT15M\r\nSUMMARY:Check-in\r\nSEQUENCE:2\r\n\
         BEGIN:VALARM\r\nACTION:DISPLAY\r\nTRIGGER:-PT15M\r\nDESCRIPTION:Check-in\r\nEND:VALARM\r\nEND:VEVENT\r\n\
         BEGIN:VEVENT\r\nUID:checkin@example.org\r\nRECURRENCE-ID:20261108T090000Z\r\nDTSTAMP:20261101T080000Z\r\n\
         DTSTART:20261108T093000Z\r\nDURATION:PT15M\r\nSUMMARY:Check-in\r\nSEQUENCE:2\r\nEND:VEVENT\r\n",
    )
}

/// The `alert` of every row event `id` shows in the week from Thursday 5 November 2026.
fn every_shown(session: &mut Session, id: u64) -> Vec<Value> {
    let list = session.run("event.list", &json!({ "from": "2026-11-05", "to": "2026-11-12" })).unwrap();
    list["events"].as_array().unwrap().iter().filter(|row| row["id"] == json!(id)).map(|row| row["alert"].clone()).collect()
}

/// `text` without its `VALARM` blocks.
fn outside_alarms(text: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for line in text.split_inclusive("\r\n") {
        if line == "BEGIN:VALARM\r\n" {
            inside = true;
        } else if line == "END:VALARM\r\n" {
            inside = false;
        } else if !inside {
            out.push_str(line);
        }
    }
    out
}

#[test]
fn no_alert_reaches_the_changed_instances_of_a_series_too() {
    let mut session = session();
    synced(&mut session, "checkin", checkin());
    assert_eq!(every_shown(&mut session, 5), [json!(15), json!(15), Value::Null]);
    session.run("event.update", &json!({ "id": 5, "alert": "none" })).unwrap();
    let calendar = held(&session, 5);
    for event in &calendar.events {
        let actions: Vec<&str> = event.alarms.iter().map(|alarm| alarm.action.as_str()).collect();
        assert_eq!(actions, ["NONE"], "{:?}", event.recurrence_id);
    }
    assert_eq!(every_shown(&mut session, 5), [json!("none"), json!("none"), json!("none")]);
    assert!(alerts(&session, 5).is_empty(), "{:?}", alerts(&session, 5));
}

#[test]
fn a_lead_reaches_the_changed_instances_of_a_series_too() {
    let mut session = session();
    synced(&mut session, "checkin", checkin());
    session.run("event.update", &json!({ "id": 5, "alert": 30 })).unwrap();
    let calendar = held(&session, 5);
    for event in &calendar.events {
        let triggers: Vec<Trigger> = event.alarms.iter().map(|alarm| alarm.trigger).collect();
        assert_eq!(triggers, [Trigger::Start(Span::new(0, -1800).unwrap())], "{:?}", event.recurrence_id);
    }
    assert_eq!(every_shown(&mut session, 5), [json!(30), json!(30), json!(30)]);
    assert_eq!(alerts(&session, 5), [utc(2026, 11, 6, 8, 30), utc(2026, 11, 7, 9, 30), utc(2026, 11, 8, 9, 0)]);
}

#[test]
fn a_lead_with_other_changes_reaches_the_changed_instances_too() {
    let mut session = session();
    synced(&mut session, "checkin", checkin());
    session.run("event.update", &json!({ "id": 5, "alert": "none", "location": "Room 2" })).unwrap();
    assert_eq!(every_shown(&mut session, 5), [json!("none"), json!("none"), json!("none")]);
    assert!(alerts(&session, 5).is_empty(), "{:?}", alerts(&session, 5));
}

#[test]
fn an_alert_alone_changes_nothing_outside_the_alarms() {
    let mut session = session();
    let stored = checkin();
    synced(&mut session, "checkin", stored.clone());
    for alert in [json!(30), json!("none"), json!(1440)] {
        session.run("event.update", &json!({ "id": 5, "alert": alert })).unwrap();
        let written = text(&session, 5);
        assert_eq!(outside_alarms(&written), outside_alarms(&stored), "{alert}: {written}");
        assert!(written.contains("SEQUENCE:2\r\n") && written.contains("DTSTAMP:20261101T080000Z\r\n"), "{written}");
        assert!(!written.contains("SCHEDULE-AGENT"), "{written}");
    }
}

#[test]
fn more_than_one_display_or_sound_alarm_is_custom() {
    let mut session = session();
    let two = kiln().replace("BEGIN:VALARM\r\nACTION:AUDIO\r\nTRIGGER:-PT5M\r\nEND:VALARM\r\n", "");
    synced(&mut session, "kiln", two);
    assert_eq!(shown(&mut session, 5), json!(15), "one display alarm, a mail one and a junk one");
    synced(&mut session, "kiln", kiln());
    assert_eq!(shown(&mut session, 5), json!("custom"), "a display and a sound alarm: the field cannot show both");
}

#[test]
fn a_moved_alarm_drops_the_default_marker_and_the_acknowledgement() {
    let mut session = session();
    let marked = kiln().replace(
        "X-EXAMPLE-KEEP:yes\r\n",
        "X-EXAMPLE-KEEP:yes\r\nX-APPLE-DEFAULT-ALARM:TRUE\r\nACKNOWLEDGED:20261105T080000Z\r\nUID:alarm-1@example.org\r\n",
    );
    synced(&mut session, "kiln", marked);
    session.run("event.update", &json!({ "id": 5, "alert": 60 })).unwrap();
    let written = text(&session, 5);
    assert!(!written.contains("X-APPLE-DEFAULT-ALARM") && !written.contains("ACKNOWLEDGED"), "{written}");
    assert!(written.contains("X-EXAMPLE-KEEP:yes\r\n") && written.contains("UID:alarm-1@example.org\r\n"), "{written}");
}

#[test]
fn a_new_alarm_has_a_uid_and_the_event_summary() {
    let mut session = session();
    let id = create(&mut session, json!({ "alert": 30, "summary": "Dentist\r\nat\0 noon" })).unwrap();
    let calendar = held(&session, id);
    let alarm = &calendar.events[0].alarms[0];
    assert_eq!(alarm.description.as_deref(), Some("Dentist at noon"));
    assert!(alarm.other.iter().any(|property| property.name() == "UID" && !property.value().is_empty()), "{alarm:?}");
    let long = "x".repeat(500);
    let id = create(&mut session, json!({ "alert": 30, "summary": long })).unwrap();
    let description = held(&session, id).events[0].alarms[0].description.clone().unwrap();
    assert!(description.chars().count() <= 200 && description.starts_with("xxx"), "{description}");
    let id = create(&mut session, json!({ "alert": 30, "summary": " \r\n " })).unwrap();
    assert_eq!(held(&session, id).events[0].alarms[0].description.as_deref(), Some("Reminder"));
}

#[test]
fn no_alert_twice_writes_one_marker() {
    let mut session = session();
    let id = create(&mut session, json!({ "alert": "none" })).unwrap();
    session.run("event.update", &json!({ "id": id, "alert": "none" })).unwrap();
    let actions: Vec<String> = held(&session, id).events[0].alarms.iter().map(|alarm| alarm.action.clone()).collect();
    assert_eq!(actions, ["NONE"]);
    assert!(alerts(&session, id).is_empty());
}

#[test]
fn no_alert_on_an_event_with_only_a_mail_alarm_adds_no_marker() {
    let mut session = session();
    let mail_only = object(
        "BEGIN:VEVENT\r\nUID:post@example.org\r\nDTSTART:20261106T140000Z\r\nDTEND:20261106T150000Z\r\nSUMMARY:Post\r\n\
         BEGIN:VALARM\r\nACTION:EMAIL\r\nTRIGGER:-PT1H\r\nDESCRIPTION:Mail me\r\nSUMMARY:Post\r\nATTENDEE:mailto:anna@example.org\r\nEND:VALARM\r\n\
         END:VEVENT\r\n",
    );
    synced(&mut session, "post", mail_only);
    session.run("event.update", &json!({ "id": 5, "alert": "none" })).unwrap();
    let actions: Vec<String> = held(&session, 5).events[0].alarms.iter().map(|alarm| alarm.action.clone()).collect();
    assert_eq!(actions, ["EMAIL"]);
    assert_eq!(shown(&mut session, 5), json!("none"));
    assert!(alerts(&session, 5).is_empty());
}
