// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendar commands: `calendar.list`, `event.list` with recurrence expanded and the event's own zone, on
//! invented calendars of `example.org`.

use inboxcraft_model::{AccountId, CalendarEvent, CalendarEventId, CalendarId};
use inboxcraft_store::{Batch, Change, Memory, Storage};
use inboxcraft_time::{FakeClock, Offset, OffsetTable};
use serde_json::{Value, json};

use crate::test_support::{NOW, at, mailbox};
use crate::{Caller, EngineError, Registry, Session};

/// The `VTIMEZONE` of Budapest: +01:00, +02:00 from the last Sunday of March to the last Sunday of October.
pub(crate) const BUDAPEST: &str = "BEGIN:VTIMEZONE\r\n\
    TZID:Europe/Budapest\r\n\
    BEGIN:DAYLIGHT\r\n\
    DTSTART:19700329T020000\r\n\
    TZOFFSETFROM:+0100\r\n\
    TZOFFSETTO:+0200\r\n\
    RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU\r\n\
    END:DAYLIGHT\r\n\
    BEGIN:STANDARD\r\n\
    DTSTART:19701025T030000\r\n\
    TZOFFSETFROM:+0200\r\n\
    TZOFFSETTO:+0100\r\n\
    RRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\r\n\
    END:STANDARD\r\n\
    END:VTIMEZONE\r\n";

/// The `VTIMEZONE` of New York: -05:00, -04:00 from the second Sunday of March to the first Sunday of November.
pub(crate) const NEW_YORK: &str = "BEGIN:VTIMEZONE\r\n\
    TZID:America/New_York\r\n\
    BEGIN:DAYLIGHT\r\n\
    DTSTART:19700308T020000\r\n\
    TZOFFSETFROM:-0500\r\n\
    TZOFFSETTO:-0400\r\n\
    RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=2SU\r\n\
    END:DAYLIGHT\r\n\
    BEGIN:STANDARD\r\n\
    DTSTART:19701101T020000\r\n\
    TZOFFSETFROM:-0400\r\n\
    TZOFFSETTO:-0500\r\n\
    RRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=1SU\r\n\
    END:STANDARD\r\n\
    END:VTIMEZONE\r\n";

/// A calendar object holding `inner` (zones and events).
pub(crate) fn object(inner: &str) -> String {
    format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//Example//EN\r\n{inner}END:VCALENDAR\r\n")
}

/// The stand-up: Mondays at 9:30 in Budapest for a quarter of an hour, three times from Monday 9 November 2026.
pub(crate) fn standup() -> String {
    object(&format!(
        "{BUDAPEST}BEGIN:VEVENT\r\nUID:standup@example.org\r\nDTSTART;TZID=Europe/Budapest:20261109T093000\r\nDURATION:PT15M\r\n\
         SUMMARY:Stand-up\r\nRRULE:FREQ=WEEKLY;COUNT=3\r\nEND:VEVENT\r\n"
    ))
}

/// Lunch on Friday 6 November 2026, 11:00 to 12:00 UTC: free time, private, with one guest.
pub(crate) fn lunch() -> String {
    object(
        "BEGIN:VEVENT\r\nUID:lunch@example.org\r\nDTSTART:20261106T110000Z\r\nDTEND:20261106T120000Z\r\nSUMMARY:Lunch\r\n\
         LOCATION:Corner café\r\nTRANSP:TRANSPARENT\r\nCLASS:PRIVATE\r\nORGANIZER:mailto:anna@example.org\r\n\
         ATTENDEE;CN=Dora:mailto:dora@example.com\r\nEND:VEVENT\r\n",
    )
}

/// Two whole days off, 10 and 11 November 2026.
pub(crate) fn days_off() -> String {
    object("BEGIN:VEVENT\r\nUID:off@example.org\r\nDTSTART;VALUE=DATE:20261110\r\nDTEND;VALUE=DATE:20261112\r\nSUMMARY:Days off\r\nEND:VEVENT\r\n")
}

/// A call at 9:00 in New York on Friday 6 November 2026 (14:00 UTC), for an hour.
pub(crate) fn call() -> String {
    object(&format!(
        "{NEW_YORK}BEGIN:VEVENT\r\nUID:call@example.org\r\nDTSTART;TZID=America/New_York:20261106T090000\r\n\
         DTEND;TZID=America/New_York:20261106T100000\r\nSUMMARY:Call with the New York office\r\nEND:VEVENT\r\n"
    ))
}

/// Where the work calendar of account 1 lives.
pub(crate) const WORK: &str = "https://dav.example.org/calendars/anna/work/";

/// The test mailbox with a CalDAV address on account 1, its calendars 1 (Work: the stand-up, lunch and the call)
/// and 2 (Home: the days off), and an empty calendar 3 of account 2.
pub(crate) fn store() -> Memory {
    let mut store = mailbox();
    let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    let account = account.with_caldav(Some("https://dav.example.org/")).unwrap();
    let calendar = |id: u64, owner: u64, href: &str, name: &str, colour: Option<&str>| {
        Change::PutCalendar(inboxcraft_model::Calendar {
            id: CalendarId::new(id),
            account: AccountId::new(owner),
            href: href.to_owned(),
            name: name.to_owned(),
            colour: colour.map(str::to_owned),
        })
    };
    let batch = Batch::new()
        .with(Change::PutAccount(account))
        .with(calendar(1, 1, WORK, "Work", Some("#3366ccff")))
        .with(calendar(2, 1, "https://dav.example.org/calendars/anna/home/", "Home", None))
        .with(calendar(3, 2, "https://dav.example.net/calendars/bob/default/", "Bob's", None))
        .with(Change::PutCalendarEvent(event(1, 1, "standup", Some("\"s1\""), &standup())))
        .with(Change::PutCalendarEvent(event(2, 1, "lunch", Some("\"l1\""), &lunch())))
        .with(Change::PutCalendarEvent(event(3, 1, "call", Some("\"c1\""), &call())))
        .with(Change::PutCalendarEvent(event(4, 2, "off", Some("\"o1\""), &days_off())));
    store.apply(&batch).unwrap();
    store
}

/// Event `id` of `calendar`, at `<calendar>/<name>.ics`.
pub(crate) fn event(id: u64, calendar: u64, name: &str, etag: Option<&str>, ical: &str) -> CalendarEvent {
    let base = match calendar {
        1 => WORK,
        2 => "https://dav.example.org/calendars/anna/home/",
        _ => "https://dav.example.net/calendars/bob/default/",
    };
    CalendarEvent {
        id: CalendarEventId::new(id),
        calendar: CalendarId::new(calendar),
        href: format!("{base}{name}.ics"),
        etag: etag.map(str::to_owned),
        ical: ical.to_owned(),
    }
}

/// A session on `clock` over [`store`], the computer at +01:00 (Budapest in winter).
pub(crate) fn session_at(clock: &FakeClock) -> Session {
    let mut session = Session::with_registry(Box::new(store()), Box::new(clock.clone()), Registry::builtin().unwrap()).unwrap();
    session.set_offsets(Box::new(Offset::from_minutes(60).unwrap()));
    session
}

pub(crate) fn session() -> Session {
    session_at(&FakeClock::new(at(NOW)))
}

/// The week from Thursday 5 to Thursday 12 November 2026, UTC.
fn week(session: &mut Session) -> Value {
    session.run("event.list", &json!({ "from": "2026-11-05", "to": "2026-11-12" })).unwrap()
}

fn summaries(list: &Value) -> Vec<&str> {
    list["events"].as_array().unwrap().iter().map(|row| row["summary"].as_str().unwrap()).collect()
}

#[test]
fn calendar_list_shows_every_calendar_with_its_events_counted() {
    let mut session = session();
    let list = session.run_as(Caller::Agent, "calendar.list", &json!({})).unwrap();
    assert_eq!(
        list,
        json!({ "calendars": [
            { "id": 1, "account": 1, "name": "Work", "colour": "#3366ccff", "events": 3 },
            { "id": 2, "account": 1, "name": "Home", "colour": null, "events": 1 },
            { "id": 3, "account": 2, "name": "Bob's", "colour": null, "events": 0 },
        ] })
    );
    let one = session.run("calendar.list", &json!({ "account": 2 })).unwrap();
    assert_eq!(one["calendars"].as_array().unwrap().len(), 1);
    assert_eq!(session.run("calendar.list", &json!({ "account": 9 })).unwrap_err(), EngineError::NotFound { what: "account", id: 9 });
}

#[test]
fn event_list_expands_the_week_in_time_order() {
    let mut session = session();
    let list = week(&mut session);
    assert_eq!(summaries(&list), ["Lunch", "Call with the New York office", "Stand-up", "Days off"]);
    assert_eq!(list["truncated"], json!(false));
    let longer = session.run("event.list", &json!({ "from": "2026-11-05", "to": "2026-12-01" })).unwrap();
    assert_eq!(summaries(&longer), ["Lunch", "Call with the New York office", "Stand-up", "Days off", "Stand-up", "Stand-up"]);
}

#[test]
fn an_occurrence_carries_its_moments_its_dates_and_the_wall_clock() {
    let mut session = session();
    let list = week(&mut session);
    let standup = &list["events"][2];
    assert_eq!(standup["id"], json!(1));
    assert_eq!(standup["calendar"], json!(1));
    assert_eq!(standup["uid"], json!("standup@example.org"));
    assert_eq!(standup["start"], json!("2026-11-09T08:30:00Z"));
    assert_eq!(standup["end"], json!("2026-11-09T08:45:00Z"));
    assert_eq!(standup["local_start"], json!("2026-11-09T09:30:00+01:00"));
    assert_eq!(standup["local_end"], json!("2026-11-09T09:45:00+01:00"));
    assert_eq!(standup["start_date"], json!("2026-11-09"));
    assert_eq!(standup["end_date"], json!("2026-11-09"));
    assert_eq!(standup["all_day"], json!(false));
    assert_eq!(standup["repeat"], json!("weekly"));
    assert_eq!(standup["recurrence_id"], json!("2026-11-09T09:30:00"));
    assert_eq!(standup["busy"], json!(true));
    assert_eq!(standup["private"], json!(false));
    assert_eq!(standup["guests"], json!([]));
}

#[test]
fn an_all_day_event_names_its_first_and_last_day() {
    let mut session = session();
    let list = week(&mut session);
    let off = &list["events"][3];
    assert_eq!(off["all_day"], json!(true));
    assert_eq!(off["start_date"], json!("2026-11-10"));
    assert_eq!(off["end_date"], json!("2026-11-11"));
    assert_eq!(off["local_start"], json!("2026-11-10T00:00:00+01:00"));
    assert_eq!(off["start"], json!("2026-11-09T23:00:00Z"));
    assert_eq!(off["repeat"], Value::Null);
    assert_eq!(off["recurrence_id"], Value::Null);
}

#[test]
fn free_time_private_events_and_guests_are_said() {
    let mut session = session();
    let list = week(&mut session);
    let lunch = &list["events"][0];
    assert_eq!(lunch["busy"], json!(false));
    assert_eq!(lunch["private"], json!(true));
    assert_eq!(lunch["location"], json!("Corner café"));
    assert_eq!(lunch["guests"], json!([{ "address": "dora@example.com", "name": "Dora", "status": "needs_action" }]));
}

#[test]
fn the_events_own_zone_is_shown_when_it_differs_from_the_computers() {
    let mut session = session();
    let list = week(&mut session);
    // Budapest is the computer's +01:00 in November: nothing to show.
    assert_eq!(list["events"][2]["zone"], Value::Null);
    let call = &list["events"][1];
    assert_eq!(call["start"], json!("2026-11-06T14:00:00Z"));
    assert_eq!(call["local_start"], json!("2026-11-06T15:00:00+01:00"));
    assert_eq!(call["zone"], json!({ "name": "America/New_York", "start": "2026-11-06T09:00:00-05:00", "end": "2026-11-06T10:00:00-05:00" }));
    // A UTC moment and a whole day have no zone of their own.
    assert_eq!(list["events"][0]["zone"], Value::Null);
    assert_eq!(list["events"][3]["zone"], Value::Null);
}

#[test]
fn on_a_computer_in_utc_the_budapest_stand_up_shows_its_zone() {
    let mut session = session();
    session.set_offsets(Box::new(Offset::UTC));
    let list = week(&mut session);
    let standup = list["events"].as_array().unwrap().iter().find(|row| row["summary"] == "Stand-up").unwrap();
    assert_eq!(standup["local_start"], json!("2026-11-09T08:30:00+00:00"));
    assert_eq!(standup["zone"], json!({ "name": "Europe/Budapest", "start": "2026-11-09T09:30:00+01:00", "end": "2026-11-09T09:45:00+01:00" }));
}

#[test]
fn the_zone_shows_only_on_the_side_of_a_summer_time_change_where_it_differs() {
    // The computer changes from +01:00 to +02:00 on 22 November; Budapest stays at +01:00.
    let mut session = session();
    let change = at(NOW + 17 * 86_400);
    session.set_offsets(Box::new(OffsetTable::new(Offset::from_minutes(60).unwrap()).with_change(change, Offset::from_minutes(120).unwrap())));
    let list = session.run("event.list", &json!({ "from": "2026-11-09", "to": "2026-12-01" })).unwrap();
    let zones: Vec<bool> =
        list["events"].as_array().unwrap().iter().filter(|row| row["summary"] == "Stand-up").map(|row| row["zone"].is_object()).collect();
    assert_eq!(zones, [false, false, true]);
}

#[test]
fn event_list_narrows_to_one_calendar_or_account() {
    let mut session = session();
    let home = session.run("event.list", &json!({ "from": "2026-11-05", "to": "2026-11-12", "calendar": 2 })).unwrap();
    assert_eq!(summaries(&home), ["Days off"]);
    let bob = session.run("event.list", &json!({ "from": "2026-11-05", "to": "2026-11-12", "account": 2 })).unwrap();
    assert_eq!(summaries(&bob), Vec::<&str>::new());
    let missing = session.run("event.list", &json!({ "from": "2026-11-05", "to": "2026-11-12", "calendar": 7 })).unwrap_err();
    assert_eq!(missing, EngineError::NotFound { what: "calendar", id: 7 });
}

#[test]
fn event_list_refuses_a_span_it_cannot_read() {
    let mut session = session();
    for params in [
        json!({ "from": "2026-11-12", "to": "2026-11-05" }),
        json!({ "from": "yesterday", "to": "2026-11-05" }),
        json!({ "from": "2026-11-05", "to": "2026-11-05" }),
        json!({ "from": "2026-01-01", "to": "2028-01-01" }),
        json!({ "from": "2026-11-05" }),
    ] {
        assert_eq!(session.run("event.list", &params).unwrap_err().code(), "bad_params", "{params}");
    }
}

#[test]
fn an_event_that_cannot_be_read_is_counted_not_shown() {
    let mut session = session();
    crate::test_support::change(&mut session, Change::PutCalendarEvent(event(5, 2, "broken", None, "BEGIN:VCALENDAR\r\nnot a calendar")));
    let list = week(&mut session);
    assert_eq!(summaries(&list), ["Lunch", "Call with the New York office", "Stand-up", "Days off"]);
    assert_eq!(list["unreadable"], json!(1));
}

#[test]
fn the_reading_commands_are_safe_for_an_agent() {
    let mut session = session();
    for id in ["calendar.list", "event.list", "event.repeats"] {
        assert_eq!(session.registry().lookup(id).map(|(_, class)| class), Some(crate::AgentClass::Safe), "{id}");
    }
    assert!(session.run_as(Caller::Agent, "event.list", &json!({ "from": "2026-11-05", "to": "2026-11-12" })).is_ok());
}

#[test]
fn event_repeats_lists_the_presets() {
    let mut session = session();
    let presets = session.run("event.repeats", &json!({})).unwrap();
    assert_eq!(presets, json!({ "repeats": ["none", "daily", "weekdays", "weekly", "monthly", "yearly"] }));
}

#[test]
fn every_answer_of_a_guest_is_named() {
    let mut session = session();
    let answers = object(
        "BEGIN:VEVENT\r\nUID:a@example.org\r\nDTSTART:20261106T150000Z\r\nSUMMARY:Answers\r\n\
         ATTENDEE;PARTSTAT=ACCEPTED:mailto:a@example.org\r\nATTENDEE;PARTSTAT=DECLINED:MAILTO:b@example.org\r\n\
         ATTENDEE;PARTSTAT=TENTATIVE:mailto:c@example.org\r\nATTENDEE;PARTSTAT=DELEGATED:mailto:d@example.org\r\n\
         ATTENDEE;PARTSTAT=X-LATER:urn:uuid:e\r\nEND:VEVENT\r\n",
    );
    crate::test_support::change(&mut session, Change::PutCalendarEvent(event(5, 2, "answers", None, &answers)));
    let list = session.run("event.list", &json!({ "from": "2026-11-06", "to": "2026-11-07", "calendar": 2 })).unwrap();
    let guests: Vec<(&str, &str)> = list["events"][0]["guests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|guest| (guest["address"].as_str().unwrap(), guest["status"].as_str().unwrap()))
        .collect();
    assert_eq!(
        guests,
        [
            ("a@example.org", "accepted"),
            ("b@example.org", "declined"),
            ("c@example.org", "tentative"),
            ("d@example.org", "delegated"),
            ("urn:uuid:e", "other")
        ]
    );
}

#[test]
fn a_repeat_no_preset_names_is_custom_and_each_instance_is_named_in_the_events_terms() {
    let mut session = session();
    let fortnightly = object(
        "BEGIN:VEVENT\r\nUID:f@example.org\r\nDTSTART;VALUE=DATE:20261106\r\nSUMMARY:Bins\r\nRRULE:FREQ=WEEKLY;INTERVAL=2;COUNT=2\r\nEND:VEVENT\r\n",
    );
    let daily =
        object("BEGIN:VEVENT\r\nUID:u@example.org\r\nDTSTART:20261106T070000Z\r\nSUMMARY:Pills\r\nRRULE:FREQ=DAILY;COUNT=1\r\nEND:VEVENT\r\n");
    crate::test_support::change(&mut session, Change::PutCalendarEvent(event(5, 2, "bins", None, &fortnightly)));
    crate::test_support::change(&mut session, Change::PutCalendarEvent(event(6, 2, "pills", None, &daily)));
    let list = session.run("event.list", &json!({ "from": "2026-11-06", "to": "2026-11-30", "calendar": 2 })).unwrap();
    let rows: Vec<(&str, &str, &str)> = list["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["summary"] != "Days off")
        .map(|row| (row["summary"].as_str().unwrap(), row["repeat"].as_str().unwrap(), row["recurrence_id"].as_str().unwrap()))
        .collect();
    assert_eq!(rows, [("Bins", "custom", "2026-11-06"), ("Pills", "daily", "2026-11-06T07:00:00Z"), ("Bins", "custom", "2026-11-20")]);
}
