// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_time::{FakeClock, Offset, Timestamp};
use serde_json::json;

use super::{Editor, REPEATS, guests_of, repeat_text, typed_day, typed_moment};
use crate::calendar::{Calendar, Day, EventRow, Moment};
use crate::mailbox::{Act, Mailbox};
use crate::sample;

/// 2026-11-05T09:00:00Z, a Thursday.
const NOW: i64 = 1_793_869_200;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

fn loaded() -> (Mailbox, Calendar) {
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    let mut calendar = Calendar::default();
    calendar.refresh(&mut mailbox);
    (mailbox, calendar)
}

fn day(month: u8, date: u8) -> Day {
    Day::new(2026, month, date).unwrap()
}

fn named<'a>(calendar: &'a Calendar, summary: &str) -> Option<&'a EventRow> {
    calendar.events().find(|event| event.summary == summary)
}

/// Picks the event `summary` and opens it in the editor.
fn editing(calendar: &mut Calendar, summary: &str) {
    let key = named(calendar, summary).map(EventRow::key);
    calendar.pick(key);
    calendar.edit_picked();
}

fn toast(mailbox: &Mailbox) -> Option<(Act, &str, bool)> {
    mailbox.undoable().map(|undo| (undo.act, undo.text.as_str(), undo.undoable))
}

#[test]
fn a_new_event_starts_at_nine_on_the_day_shown_in_the_first_calendar_shown() {
    let (_, mut calendar) = loaded();
    calendar.toggle(1);
    calendar.new_event();
    let editor = calendar.editor().unwrap();
    assert_eq!(editor.id(), None);
    assert_eq!(editor.title(), "New event");
    let form = editor.form();
    assert_eq!((form.calendar, form.summary.as_str()), (2, ""));
    assert_eq!((form.start.as_str(), form.end.as_str()), ("2026-11-05 09:00", "2026-11-05 10:00"));
    assert!(!form.all_day && form.busy && !form.private);
    assert_eq!(form.repeat, "none");
    assert_eq!(editor.choices().iter().map(|(id, _)| *id).collect::<Vec<_>>(), [1, 2, 3]);
    assert!(!editor.asks_notify(), "a new event asks nothing of its guests");
}

#[test]
fn saving_a_new_event_adds_it_with_a_toast_whose_undo_takes_it_away() {
    let (mut mailbox, mut calendar) = loaded();
    calendar.new_event();
    let form = calendar.editor_mut().unwrap().form_mut();
    form.summary = "Pottery class".to_owned();
    form.start = "2026-11-06 18:00".to_owned();
    form.end = "2026-11-06T19:30".to_owned();
    form.repeat = "weekly".to_owned();
    form.busy = false;
    form.private = true;
    form.guests = "kai@example.org; noor@example.org".to_owned();
    calendar.save(&mut mailbox);
    assert!(mailbox.error().is_none(), "{:?}", mailbox.error());
    assert!(calendar.editor().is_none(), "the editor closes");
    assert_eq!(toast(&mailbox), Some((Act::Event, "Event added", true)));
    let event = named(&calendar, "Pottery class").unwrap();
    assert_eq!((event.start, event.end.minute), (Moment { day: day(11, 6), minute: 18 * 60 }, 19 * 60 + 30));
    assert_eq!((event.busy, event.private, event.repeat.as_deref()), (false, true, Some("weekly")));
    assert_eq!(event.guests.iter().map(|guest| guest.address.as_str()).collect::<Vec<_>>(), ["kai@example.org", "noor@example.org"]);
    assert_eq!(calendar.picked().map(|event| event.summary.as_str()), None, "nothing is picked by saving");
    mailbox.undo();
    calendar.refresh(&mut mailbox);
    assert!(named(&calendar, "Pottery class").is_none(), "undo takes the new event away");
}

#[test]
fn an_all_day_event_takes_its_first_and_last_day() {
    let (mut mailbox, mut calendar) = loaded();
    calendar.new_event();
    let form = calendar.editor_mut().unwrap().form_mut();
    form.summary = "Harvest festival".to_owned();
    form.all_day = true;
    form.start = "2026-11-07".to_owned();
    form.end = "2026-11-08".to_owned();
    calendar.save(&mut mailbox);
    assert!(mailbox.error().is_none(), "{:?}", mailbox.error());
    let event = named(&calendar, "Harvest festival").unwrap();
    assert!(event.all_day);
    assert_eq!((event.first, event.last), (day(11, 7), day(11, 8)));
}

#[test]
fn a_time_the_form_cannot_read_keeps_the_editor_open_and_says_why() {
    let (mut mailbox, mut calendar) = loaded();
    calendar.new_event();
    calendar.editor_mut().unwrap().form_mut().start = "next friday".to_owned();
    calendar.save(&mut mailbox);
    let editor = calendar.editor().unwrap();
    assert_eq!(editor.problem(), Some("Starts is no date and time like 2026-11-06 13:00"));
    assert!(mailbox.undoable().is_none());
    let form = calendar.editor_mut().unwrap().form_mut();
    form.start = "2026-11-06 13:00".to_owned();
    form.end = "2026-11-06".to_owned();
    calendar.save(&mut mailbox);
    assert_eq!(calendar.editor().unwrap().problem(), Some("Ends is no date and time like 2026-11-06 13:00"));
    let form = calendar.editor_mut().unwrap().form_mut();
    form.all_day = true;
    form.start = "2026-11-06 13:00".to_owned();
    calendar.save(&mut mailbox);
    assert_eq!(calendar.editor().unwrap().problem(), Some("Starts is no day like 2026-11-06"));
    let form = calendar.editor_mut().unwrap().form_mut();
    form.start = "2026-11-06".to_owned();
    form.end = "06/11/2026".to_owned();
    calendar.save(&mut mailbox);
    assert_eq!(calendar.editor().unwrap().problem(), Some("Ends is no day like 2026-11-06"));
}

#[test]
fn what_the_engine_refuses_keeps_the_editor_open_with_its_reason() {
    let (mut mailbox, mut calendar) = loaded();
    calendar.new_event();
    let form = calendar.editor_mut().unwrap().form_mut();
    form.start = "2026-11-06 13:00".to_owned();
    form.end = "2026-11-06 12:00".to_owned();
    calendar.save(&mut mailbox);
    let problem = calendar.editor().unwrap().problem().unwrap_or_default().to_owned();
    assert!(problem.contains("end"), "{problem}");
    assert!(mailbox.error().is_none(), "the refusal shows in the editor, not the banner");
    calendar.editor_mut().unwrap().form_mut().guests = "not an address".to_owned();
    calendar.editor_mut().unwrap().form_mut().end = "2026-11-06 14:00".to_owned();
    calendar.save(&mut mailbox);
    assert!(calendar.editor().unwrap().problem().unwrap_or_default().contains("guest"));
    calendar.close_editor();
    assert!(calendar.editor().is_none());
}

#[test]
fn editing_an_event_without_guests_never_asks_to_tell_anyone() {
    let (mut mailbox, mut calendar) = loaded();
    editing(&mut calendar, "Dentist");
    let editor = calendar.editor().unwrap();
    assert_eq!(editor.title(), "Edit event");
    assert!(editor.id().is_some());
    let form = editor.form();
    assert_eq!((form.summary.as_str(), form.start.as_str(), form.end.as_str()), ("Dentist", "2026-11-05 10:00", "2026-11-05 11:30"));
    assert!(form.private && form.busy && form.guests.is_empty());
    assert!(!editor.asks_notify());
    calendar.editor_mut().unwrap().form_mut().summary = "Dentist, check-up".to_owned();
    calendar.save(&mut mailbox);
    assert!(mailbox.error().is_none(), "{:?}", mailbox.error());
    assert_eq!(toast(&mailbox), Some((Act::Event, "Event saved", true)));
    assert!(named(&calendar, "Dentist, check-up").is_some());
    mailbox.undo();
    calendar.refresh(&mut mailbox);
    assert!(named(&calendar, "Dentist").is_some(), "undo writes the event back as it was");
}

#[test]
fn editing_an_event_with_guests_asks_to_tell_them_and_the_default_is_not_to() {
    let (_, mut calendar) = loaded();
    editing(&mut calendar, "Allotment work party");
    let editor = calendar.editor_mut().unwrap();
    assert_eq!(editor.form().guests, "mira@example.org");
    assert!(editor.asks_notify());
    assert_eq!(editor.notify_question(), "Tell the 1 guest about this change?");
    assert!(!editor.notify(), "the default is not to tell them");
    editor.form_mut().summary = "Allotment party".to_owned();
    let (command, params) = editor.changes(Offset::UTC).unwrap().unwrap();
    assert_eq!(command, "event.update");
    assert_eq!(params, json!({ "id": editor.id().unwrap(), "summary": "Allotment party" }));
    editor.set_notify(true);
    let (_, params) = editor.changes(Offset::UTC).unwrap().unwrap();
    assert_eq!(params.get("notify"), Some(&json!(true)));
    editor.form_mut().guests = "mira@example.org, kai@example.org".to_owned();
    assert_eq!(editor.notify_question(), "Tell the 2 guests about this change?");
    editor.form_mut().guests = " ".to_owned();
    assert!(!editor.asks_notify(), "without guests left there is nobody to tell");
    let (_, params) = editor.changes(Offset::UTC).unwrap().unwrap();
    assert_eq!(params.get("notify"), None);
    assert_eq!(params.get("guests"), Some(&json!([])));
}

#[test]
fn telling_the_guests_is_said_in_the_toast() {
    let (mut mailbox, mut calendar) = loaded();
    editing(&mut calendar, "Allotment work party");
    let editor = calendar.editor_mut().unwrap();
    editor.form_mut().start = "2026-11-06 14:00".to_owned();
    editor.form_mut().end = "2026-11-06 16:00".to_owned();
    editor.set_notify(true);
    calendar.save(&mut mailbox);
    assert!(mailbox.error().is_none(), "{:?}", mailbox.error());
    assert_eq!(toast(&mailbox), Some((Act::Event, "Event saved; the guests are told", true)));
    let event = named(&calendar, "Allotment work party").unwrap();
    assert_eq!((event.start.minute, event.end.minute), (14 * 60, 16 * 60));
}

#[test]
fn an_edit_sends_only_what_changed_and_nothing_changed_runs_nothing() {
    let (mut mailbox, mut calendar) = loaded();
    editing(&mut calendar, "Allotment work party");
    let editor = calendar.editor_mut().unwrap();
    assert_eq!(editor.changes(Offset::UTC), Ok(None));
    let before = mailbox.last_run();
    calendar.save(&mut mailbox);
    assert!(calendar.editor().is_none(), "nothing to save closes the editor");
    assert_eq!(mailbox.last_run(), before, "and runs nothing");
    assert!(mailbox.undoable().is_none());
    editing(&mut calendar, "Allotment work party");
    let editor = calendar.editor_mut().unwrap();
    let id = editor.id().unwrap();
    editor.form_mut().all_day = true;
    editor.form_mut().start = "2026-11-06".to_owned();
    editor.form_mut().end = "2026-11-06".to_owned();
    editor.form_mut().busy = false;
    editor.form_mut().private = true;
    editor.form_mut().repeat = "monthly".to_owned();
    let (_, params) = editor.changes(Offset::UTC).unwrap().unwrap();
    assert_eq!(
        params,
        json!({ "id": id, "all_day": true, "start": "2026-11-06", "end": "2026-11-06", "busy": false, "private": true, "repeat": "monthly" })
    );
}

#[test]
fn a_timed_edit_sends_both_times_in_utc_at_the_computer_s_offset() {
    let (_, mut calendar) = loaded();
    editing(&mut calendar, "Dentist");
    let editor = calendar.editor_mut().unwrap();
    editor.form_mut().end = "2026-11-05 12:00".to_owned();
    let plus_one = Offset::from_minutes(60).unwrap();
    let (_, params) = editor.changes(plus_one).unwrap().unwrap();
    assert_eq!(params.get("start"), Some(&json!("2026-11-05T09:00:00Z")));
    assert_eq!(params.get("end"), Some(&json!("2026-11-05T11:00:00Z")));
}

#[test]
fn a_repeat_the_presets_do_not_name_stays_as_it_is() {
    let mut row = EventRow::read(&json!({
        "id": 9, "calendar": 1, "summary": "Odd rule", "local_start": "2026-11-05T09:00:00Z", "local_end": "2026-11-05T10:00:00Z",
        "repeat": "custom", "guests": [{ "address": "a@example.org", "name": "" }, { "name": "No address" }, 7]
    }))
    .unwrap();
    let editor = Editor::edit(&row);
    assert_eq!(editor.form().repeat, "custom");
    assert_eq!(editor.form().guests, "a@example.org", "only guests with an address are listed");
    assert_eq!(editor.changes(Offset::UTC), Ok(None));
    row.repeat = None;
    assert_eq!(Editor::edit(&row).form().repeat, "none");
    assert_eq!(repeat_text("custom"), "Custom");
    assert_eq!(repeat_text("anything"), "Custom");
    assert_eq!(REPEATS.map(repeat_text), ["Never", "Daily", "Weekdays", "Weekly", "Monthly", "Yearly"]);
}

#[test]
fn deleting_the_picked_event_shows_a_toast_whose_undo_brings_it_back() {
    let (mut mailbox, mut calendar) = loaded();
    let key = named(&calendar, "Dentist").map(EventRow::key);
    calendar.pick(key);
    calendar.delete_picked(&mut mailbox);
    assert!(mailbox.error().is_none(), "{:?}", mailbox.error());
    assert!(named(&calendar, "Dentist").is_none());
    assert!(calendar.picked().is_none());
    assert_eq!(toast(&mailbox), Some((Act::Event, "Event deleted", true)));
    mailbox.undo();
    calendar.refresh(&mut mailbox);
    assert!(named(&calendar, "Dentist").is_some(), "undo brings the event back");
}

#[test]
fn without_a_picked_event_or_a_calendar_nothing_opens_or_goes() {
    let (mut mailbox, mut calendar) = loaded();
    calendar.edit_picked();
    assert!(calendar.editor().is_none());
    let before = mailbox.last_run();
    calendar.delete_picked(&mut mailbox);
    assert_eq!(mailbox.last_run(), before);
    calendar.save(&mut mailbox);
    assert_eq!(mailbox.last_run(), before);
    let mut empty = Calendar::default();
    empty.new_event();
    assert!(empty.editor().is_none(), "no calendar, no new event");
    for id in [1, 2, 3] {
        calendar.toggle(id);
    }
    calendar.new_event();
    assert_eq!(calendar.editor().map(|editor| editor.form().calendar), Some(1), "all hidden: the first calendar");
}

#[test]
fn a_refused_delete_shows_in_the_banner() {
    let (mut mailbox, mut calendar) = loaded();
    let key = named(&calendar, "Dentist").map(EventRow::key);
    calendar.pick(key);
    // Gone from elsewhere while the screen still shows it: the engine refuses.
    let id = key.unwrap().id;
    mailbox.run("event.delete", &json!({ "id": id })).unwrap();
    calendar.delete_picked(&mut mailbox);
    assert!(mailbox.error().is_some());
    assert!(calendar.picked().is_some(), "the screen is not read again after a refusal");
}

#[test]
fn typed_times_and_guests_read_only_what_they_should() {
    let at = |text: &str| typed_moment(text, Offset::UTC);
    assert_eq!(at("2026-11-06 13:00").as_deref(), Some("2026-11-06T13:00:00Z"));
    assert_eq!(at(" 2026-11-06T13:05 ").as_deref(), Some("2026-11-06T13:05:00Z"));
    assert_eq!(at("2026-11-06 9:00"), None);
    assert_eq!(at("2026-11-06 24:00"), None);
    assert_eq!(at("2026-11-06"), None);
    assert_eq!(at("2026-11-06 13:00:00"), None);
    assert_eq!(at("2026-02-30 13:00"), None);
    assert_eq!(at("é026-11-06 13:00"), None);
    assert_eq!(at("2026-11-06 1é:00"), None);
    assert_eq!(typed_moment("2026-11-06 00:30", Offset::from_minutes(60).unwrap()).as_deref(), Some("2026-11-05T23:30:00Z"));
    assert_eq!(typed_day(" 2026-11-06 ").as_deref(), Some("2026-11-06"));
    assert_eq!(typed_day("2026-11-6"), None);
    assert_eq!(typed_day("2026-11-06 13:00"), None);
    assert_eq!(guests_of(" a@example.org,b@example.org ;\n, "), ["a@example.org", "b@example.org"]);
    assert!(guests_of("").is_empty());
}

#[test]
fn switching_all_day_turns_the_typed_times_into_days_and_back() {
    let (_, mut calendar) = loaded();
    calendar.new_event();
    let editor = calendar.editor_mut().unwrap();
    editor.set_all_day(true);
    assert!(editor.form().all_day);
    assert_eq!((editor.form().start.as_str(), editor.form().end.as_str()), ("2026-11-05", "2026-11-05"));
    editor.set_all_day(true);
    assert_eq!(editor.form().start, "2026-11-05");
    editor.form_mut().end = "soon".to_owned();
    editor.set_all_day(false);
    assert_eq!((editor.form().start.as_str(), editor.form().end.as_str()), ("2026-11-05 09:00", "soon"), "what is no day stays as typed");
}
