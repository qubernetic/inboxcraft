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
fn an_event_from_the_server_shows_its_first_display_or_sound_alarm() {
    let mut session = session();
    synced(&mut session, "kiln", kiln());
    assert_eq!(shown(&mut session, 5), json!(15), "the mail alarm is the server's, the junk one is left alone");
    assert_eq!(alerts(&session, 5), [utc(2026, 11, 6, 13, 45), utc(2026, 11, 6, 13, 55)]);
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
    assert_eq!(shown(&mut session, 5), json!(60));
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
    let after_end = kiln().replace("TRIGGER:-PT15M", "TRIGGER;RELATED=END:-PT15M");
    synced(&mut session, "kiln", after_end);
    assert_eq!(shown(&mut session, 5), json!("custom"));
    let odd = kiln().replace("TRIGGER:-PT15M", "TRIGGER:-PT90S");
    synced(&mut session, "kiln", odd);
    assert_eq!(shown(&mut session, 5), json!("custom"), "not whole minutes");
    let after = kiln().replace("TRIGGER:-PT15M", "TRIGGER:PT15M");
    synced(&mut session, "kiln", after);
    assert_eq!(shown(&mut session, 5), json!("custom"), "after the start");
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
