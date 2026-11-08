// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_engine::{CommandSpec, Context, EngineError, Outcome, Registry, Run, Session};
use inboxcraft_store::{Memory, StoreConfig};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use super::{Calendar, Day, EventRow, Moment, Picked, View, colour};
use crate::mailbox::Mailbox;
use crate::sample;

/// 2026-11-05T09:00:00Z, a Thursday.
const NOW: i64 = 1_793_869_200;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

fn mailbox() -> Mailbox {
    Mailbox::new(sample::session(clock()).unwrap(), clock())
}

fn day(month: u8, date: u8) -> Day {
    Day::new(2026, month, date).unwrap()
}

fn loaded() -> (Mailbox, Calendar) {
    let mut mailbox = mailbox();
    let mut calendar = Calendar::default();
    calendar.refresh(&mut mailbox);
    (mailbox, calendar)
}

fn named<'a>(calendar: &'a Calendar, summary: &str) -> &'a EventRow {
    calendar.events().find(|event| event.summary == summary).unwrap()
}

fn summaries(calendar: &Calendar) -> Vec<&str> {
    calendar.events().map(|event| event.summary.as_str()).collect()
}

#[test]
fn the_calendar_holds_no_calendar_before_the_first_read() {
    let calendar = Calendar::default();
    assert!(calendar.is_empty());
    assert!(calendar.calendars().is_empty());
    assert_eq!(calendar.focus(), Day::EPOCH);
    assert_eq!(calendar.today(), Day::EPOCH);
    assert_eq!(calendar.now(), None);
    assert!(calendar.picked().is_none());
}

#[test]
fn the_first_read_shows_this_week_with_the_sample_calendars() {
    let (mailbox, calendar) = loaded();
    assert!(mailbox.error().is_none());
    let names: Vec<&str> = calendar.calendars().iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, ["Personal", "Studio", "Choir"]);
    assert_eq!(calendar.calendar(2).map(|row| row.colour), Some(Some([0x8b, 0x5c, 0xf6])));
    assert_eq!(calendar.index_of(3), 2);
    assert_eq!(calendar.index_of(99), 0);
    assert_eq!(calendar.view(), View::Week);
    assert_eq!(calendar.today(), day(11, 5));
    assert_eq!(calendar.now(), Some(Moment { day: day(11, 5), minute: 9 * 60 }));
    assert_eq!(calendar.focus(), day(11, 5));
    assert_eq!(calendar.month(), day(11, 1));
    assert_eq!(calendar.span(), (day(11, 2), day(11, 8)));
    assert_eq!(calendar.title(), "2 – 8 November 2026");
    assert_eq!(
        summaries(&calendar),
        [
            "Studio stand-up",
            "Print run at Lumen",
            "Choir practice",
            "Small press book fair",
            "Call with Petra about the logo",
            "Dentist",
            "Allotment work party",
            "Lunch with Odile",
            "Ferry to the island",
        ]
    );
    assert_eq!(calendar.left_out(), (false, 0));
}

#[test]
fn an_event_says_both_dates_and_its_own_zone_when_that_differs() {
    let (_, calendar) = loaded();
    let ferry = named(&calendar, "Ferry to the island");
    assert_eq!(ferry.when(), "Sunday 8 November, 08:00 – 11:00");
    assert_eq!(ferry.times(), "08:00 – 11:00");
    assert_eq!(ferry.zone_line().as_deref(), Some("Event zone Atlantic/Azores: 07:00 – 10:00"));
    assert_eq!(ferry.spoken(), "Ferry to the island, Sunday 8 November, 08:00 – 11:00");
    assert_eq!(ferry.description, "Grey Gull Ferries · Booking GG-2291");
    let petra = named(&calendar, "Call with Petra about the logo");
    assert_eq!(petra.zone_line(), None, "an event in UTC has no zone of its own");
    assert_eq!(petra.guests.first().map(|guest| (guest.name.as_str(), guest.status.as_str())), Some(("Petra Lindqvist", "accepted")));
    let fair = named(&calendar, "Small press book fair");
    assert!(fair.all_day && fair.is_block() && !fair.busy);
    assert_eq!(fair.when(), "Thursday 5 November – Friday 6 November");
    assert_eq!(fair.times(), "All day");
    assert!(named(&calendar, "Dentist").private);
    assert_eq!(named(&calendar, "Studio stand-up").repeat.as_deref(), Some("weekly"));
}

#[test]
fn a_zone_on_other_days_than_the_computer_s_says_its_dates() {
    let (_, calendar) = loaded();
    let mut late = named(&calendar, "Ferry to the island").clone();
    late.zone = Some(super::Zone {
        name: "Pacific/Kiritimati".to_owned(),
        start: Moment { day: day(11, 8), minute: 22 * 60 },
        end: Moment { day: day(11, 9), minute: 0 },
    });
    assert_eq!(late.zone_line().as_deref(), Some("Event zone Pacific/Kiritimati: Sunday 8 November, 22:00 – 00:00"));
    late.zone = Some(super::Zone {
        name: "Pacific/Kiritimati".to_owned(),
        start: Moment { day: day(11, 8), minute: 22 * 60 },
        end: Moment { day: day(11, 9), minute: 60 },
    });
    assert_eq!(late.zone_line().as_deref(), Some("Event zone Pacific/Kiritimati: Sunday 8 November, 22:00 – Monday 9 November, 01:00"));
    late.all_day = false;
    late.first = day(11, 8);
    late.last = day(11, 9);
    late.end = Moment { day: day(11, 9), minute: 60 };
    assert_eq!(late.when(), "Sunday 8 November, 08:00 – Monday 9 November, 01:00");
}

#[test]
fn the_views_show_their_days_and_step_by_them() {
    let (mut mailbox, mut calendar) = loaded();
    calendar.set_view(&mut mailbox, View::Month);
    assert_eq!(calendar.title(), "November 2026");
    assert_eq!(calendar.span(), (day(10, 26), day(12, 6)));
    assert!(summaries(&calendar).contains(&"Coast trip"));
    assert!(summaries(&calendar).contains(&"Studio open day"));
    let coast = named(&calendar, "Coast trip");
    assert_eq!((coast.first, coast.last), (day(11, 16), day(11, 18)));
    assert_eq!(coast.when(), "Monday 16 November – Wednesday 18 November");
    assert_eq!(calendar.events().filter(|event| event.summary == "Choir practice").count(), 5, "repeating events expanded");
    calendar.step(&mut mailbox, true);
    assert_eq!(calendar.title(), "December 2026");
    calendar.step(&mut mailbox, false);
    calendar.step(&mut mailbox, false);
    assert_eq!(calendar.title(), "October 2026");

    calendar.go_today(&mut mailbox);
    calendar.set_view(&mut mailbox, View::Day);
    assert_eq!(calendar.title(), "Thursday 5 November 2026");
    assert_eq!(summaries(&calendar), ["Small press book fair", "Call with Petra about the logo", "Dentist", "Allotment work party"]);
    calendar.step(&mut mailbox, true);
    assert_eq!(calendar.title(), "Friday 6 November 2026");

    calendar.set_view(&mut mailbox, View::Week);
    calendar.step(&mut mailbox, true);
    assert_eq!(calendar.title(), "9 – 15 November 2026");
    calendar.step(&mut mailbox, false);

    calendar.set_view(&mut mailbox, View::Agenda);
    assert_eq!(calendar.title(), "From Friday 6 November");
    calendar.go_today(&mut mailbox);
    assert_eq!(calendar.title(), "From today");
    assert_eq!(calendar.span(), (day(11, 5), day(12, 4)));
    calendar.step(&mut mailbox, true);
    assert_eq!(calendar.focus(), day(12, 5));
    assert_eq!(calendar.month(), day(12, 1));

    calendar.open_day(&mut mailbox, day(11, 8));
    assert_eq!(calendar.view(), View::Day);
    assert_eq!(summaries(&calendar), ["Ferry to the island"]);
    assert!(mailbox.error().is_none());
}

#[test]
fn the_mini_month_steps_on_its_own() {
    let (_, mut calendar) = loaded();
    calendar.step_month(true);
    assert_eq!(calendar.month(), day(12, 1));
    calendar.step_month(false);
    calendar.step_month(false);
    assert_eq!(calendar.month(), day(10, 1));
    assert_eq!(calendar.focus(), day(11, 5), "the view stays");
}

#[test]
fn a_hidden_calendar_s_events_leave_and_a_picked_one_is_let_go() {
    let (mut mailbox, mut calendar) = loaded();
    let petra = named(&calendar, "Call with Petra about the logo").key();
    calendar.pick(Some(petra));
    assert_eq!(calendar.picked().map(|event| event.summary.as_str()), Some("Call with Petra about the logo"));
    calendar.toggle(1);
    assert!(!calendar.is_shown(1));
    assert!(!summaries(&calendar).contains(&"Dentist"));
    assert!(calendar.picked().is_some(), "a Studio event stays picked");
    calendar.toggle(2);
    assert!(calendar.picked().is_none());
    assert_eq!(summaries(&calendar), ["Choir practice"]);
    calendar.toggle(2);
    assert!(calendar.picked().is_none(), "showing it again does not pick it again");
    calendar.toggle(1);
    calendar.refresh(&mut mailbox);
    assert_eq!(calendar.events().count(), 9);
    calendar.pick(Some(Picked { id: 999, start: petra.start }));
    assert!(calendar.picked().is_none());
}

#[test]
fn a_session_without_calendars_lists_no_events() {
    let store = Memory::new(StoreConfig::default());
    let mut mailbox = Mailbox::new(Session::new(Box::new(store), clock()).unwrap(), clock());
    let mut calendar = Calendar::default();
    calendar.refresh(&mut mailbox);
    assert!(calendar.is_empty());
    assert_eq!(mailbox.last_run(), Some("calendar.list"));
    assert_eq!(calendar.focus(), day(11, 5));
}

fn refused(_: &Context<'_>, _: &Value) -> Result<Outcome, EngineError> {
    Err(EngineError::NotFound { what: "calendar", id: 1 })
}

fn shapeless(_: &Context<'_>, _: &Value) -> Result<Outcome, EngineError> {
    Ok(Outcome::new(json!({ "events": [{ "id": 1 }, "text", { "id": 2, "calendar": 1, "local_start": "2026-11-05T09:00:00Z", "local_end": "x" }] })))
}

fn hostile(_: &Context<'_>, _: &Value) -> Result<Outcome, EngineError> {
    Ok(Outcome::new(json!({ "events": [{
        "id": 7, "calendar": 1, "summary": " \u{202e}\n", "local_start": "2026-11-05T10:00:00Z", "local_end": "2026-11-05T09:00:00Z",
        "start_date": "2026-11-05", "end_date": "2026-11-01", "zone": { "name": "X\nY", "start": 5, "end": "bad" },
        "guests": [{ "address": "dora@example.org", "name": "  " }, { "status": 7 }, 3],
        "repeat": "custom", "busy": "yes"
    }], "truncated": true, "unreadable": 2 })))
}

/// A mailbox over the sample whose `event.list` runs `run` instead.
fn listing(run: Run) -> Mailbox {
    let mut registry = Registry::new();
    for spec in Registry::builtin().unwrap().specs() {
        let spec = if spec.id == "event.list" { CommandSpec { run, ..*spec } } else { *spec };
        registry.register(spec).unwrap();
    }
    let store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
    Mailbox::new(Session::with_registry(Box::new(store), clock(), registry).unwrap(), clock())
}

#[test]
fn a_refused_or_odd_event_list_shows_nothing_it_cannot_read() {
    let mut mailbox = listing(refused);
    let mut calendar = Calendar::default();
    calendar.refresh(&mut mailbox);
    assert!(mailbox.error().is_some(), "the refusal is shown");
    assert_eq!(calendar.calendars().len(), 3);
    assert_eq!(calendar.events().count(), 0);

    let mut mailbox = listing(shapeless);
    calendar.refresh(&mut mailbox);
    assert_eq!(calendar.events().count(), 0);

    let mut mailbox = listing(hostile);
    calendar.refresh(&mut mailbox);
    let event = calendar.events().next().unwrap();
    assert_eq!(event.summary, "(No title)");
    assert_eq!((event.first, event.last), (day(11, 5), day(11, 5)), "a last day before the first is the first");
    assert_eq!(event.zone, None);
    assert_eq!(event.guests.iter().map(|guest| guest.name.as_str()).collect::<Vec<_>>(), ["dora@example.org", ""]);
    assert!(event.busy);
    assert_eq!(calendar.left_out(), (true, 2));
}

#[test]
fn moments_and_colours_read_only_what_they_should() {
    assert_eq!(Moment::parse("2026-11-05T09:30:00+01:00"), Some(Moment { day: day(11, 5), minute: 570 }));
    assert_eq!(Moment::parse("2026-11-05"), Some(Moment { day: day(11, 5), minute: 0 }));
    for bad in ["2026-11-05T24:00", "2026-11-05T09:60", "2026-11-05 09:30", "2026-11-05T9:30", "2026-11-05T09-30", "2026-11-05T", "x"] {
        assert_eq!(Moment::parse(bad), None, "{bad:?}");
    }
    assert_eq!(Moment { day: day(11, 5), minute: 75 }.clock(), "01:15");
    assert_eq!(colour("#3366cc"), Some([0x33, 0x66, 0xcc]));
    assert_eq!(colour("#3366ccff"), Some([0x33, 0x66, 0xcc]));
    for bad in ["3366cc", "#36c", "#3366cg", "#3366cc0", "#éé66cc", ""] {
        assert_eq!(colour(bad), None, "{bad:?}");
    }
}

#[test]
fn the_views_have_names_and_ids() {
    for view in View::ALL {
        assert_eq!(View::named(view.id()), Some(view));
    }
    assert_eq!(View::ALL.map(View::text), ["Day", "Week", "Month", "Agenda"]);
    assert_eq!(View::named("year"), None);
}

#[test]
fn an_alert_opens_its_occurrence_on_its_day() {
    let (mut mailbox, mut calendar) = loaded();
    let id = named(&calendar, "Studio stand-up").id;
    // Next Monday's stand-up, 09:00 UTC.
    calendar.open_event(&mut mailbox, id, Timestamp::from_unix(NOW + 4 * 86_400, 0).unwrap());
    assert_eq!(calendar.focus(), day(11, 9));
    let picked = calendar.picked().unwrap();
    assert_eq!((picked.id, picked.start), (id, Moment { day: day(11, 9), minute: 9 * 60 }));
}

#[test]
fn an_alert_off_its_minute_picks_the_event_on_that_day_and_one_gone_picks_nothing() {
    let (mut mailbox, mut calendar) = loaded();
    let id = named(&calendar, "Studio stand-up").id;
    calendar.open_event(&mut mailbox, id, Timestamp::from_unix(NOW + 4 * 86_400 + 3600, 0).unwrap());
    assert_eq!(calendar.picked().map(|picked| (picked.id, picked.start.day)), Some((id, day(11, 9))));
    calendar.open_event(&mut mailbox, 999, Timestamp::from_unix(NOW + 11 * 86_400, 0).unwrap());
    assert_eq!(calendar.focus(), day(11, 16));
    assert!(calendar.picked().is_none());
}

#[test]
fn an_alert_says_when_its_occurrence_is() {
    let at = |seconds: i64| Timestamp::from_unix(NOW + seconds, 0).unwrap();
    let utc = inboxcraft_time::Offset::UTC;
    assert_eq!(super::alert_when(false, at(4 * 86_400), at(4 * 86_400 + 3600), utc), "Monday 9 November, 09:00 – 10:00");
    // Two whole days: from midnight of the 5th to midnight of the 7th.
    assert_eq!(super::alert_when(true, at(-9 * 3600), at(2 * 86_400 - 9 * 3600), utc), "Thursday 5 November – Friday 6 November");
    assert_eq!(super::alert_when(false, at(3600), at(0), utc), "Thursday 5 November, 10:00 – 10:00", "an end before the start");
}
