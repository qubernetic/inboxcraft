// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Alert field's state: what `event.list` says is read into the row and the form, a change marks the editor
//! edited and goes out with the save, and what the field cannot name is never sent.

use inboxcraft_time::{FakeClock, Offset, Timestamp};
use serde_json::{Value, json};

use super::{ALERTS, Alert, Editor, alert_text};
use crate::calendar::{Calendar, EventRow};
use crate::mailbox::Mailbox;
use crate::sample;
use inboxcraft_engine::all_day_alert::AllDayAlert;

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

fn named<'a>(calendar: &'a Calendar, summary: &str) -> Option<&'a EventRow> {
    calendar.events().find(|event| event.summary == summary)
}

/// A row as `event.list` gives it, with `alert` (left out when `None`).
fn row(alert: Option<Value>) -> EventRow {
    let mut value = json!({ "id": 9, "calendar": 1, "summary": "Kiln", "local_start": "2026-11-06T14:00:00Z", "local_end": "2026-11-06T15:00:00Z" });
    if let (Value::Object(fields), Some(alert)) = (&mut value, alert) {
        fields.insert("alert".to_owned(), alert);
    }
    EventRow::read(&value).unwrap()
}

#[test]
fn the_row_reads_the_alert_and_takes_anything_else_as_custom() {
    assert_eq!(row(None).alert, Alert::Default);
    assert_eq!(row(Some(Value::Null)).alert, Alert::Default);
    assert_eq!(row(Some(json!("none"))).alert, Alert::None);
    assert_eq!(row(Some(json!(30))).alert, Alert::Before(30));
    assert_eq!(row(Some(json!(0))).alert, Alert::Before(0));
    for odd in [json!("custom"), json!("None"), json!(-5), json!(1.5), json!(5_000_000_000_u64), json!(true), json!([10]), json!({ "minutes": 10 })] {
        assert_eq!(row(Some(odd.clone())).alert, Alert::Custom, "{odd}");
    }
}

#[test]
fn the_field_names_its_choices_and_the_default_as_the_lead_it_rings_with() {
    let names: Vec<String> = ALERTS.iter().map(|alert| alert_text(*alert, None)).collect();
    assert_eq!(
        names,
        [
            "None",
            "At the start",
            "5 min before",
            "10 min before",
            "15 min before",
            "30 min before",
            "1 hour before",
            "2 hours before",
            "1 day before",
            "2 days before",
            "1 week before"
        ]
    );
    assert_eq!(alert_text(Alert::Default, None), "10 min before");
    assert_eq!(alert_text(Alert::Custom, None), "Custom");
    assert_eq!(alert_text(Alert::Before(90), None), "90 min before");
    assert_eq!(alert_text(Alert::Before(20_160), None), "2 weeks before");
    assert_eq!(alert_text(Alert::Before(u32::MAX), None), "4294967295 min before");
}

#[test]
fn an_all_day_events_default_alert_says_the_hour_the_setting_chose() {
    let texts: Vec<String> = AllDayAlert::ALL.iter().map(|choice| alert_text(Alert::Default, Some(*choice))).collect();
    assert_eq!(texts, ["09:00 the day before", "12:00 the day before", "18:00 the day before", "09:00 on the day"]);
    // What a saved all-day event reads back: its lead before midnight as the hour the day before.
    assert_eq!(alert_text(Alert::Before(900), Some(AllDayAlert::DayBefore18)), "09:00 the day before");
    assert_eq!(alert_text(Alert::Before(360), Some(AllDayAlert::default())), "18:00 the day before");
    assert_eq!(alert_text(Alert::Before(60), Some(AllDayAlert::default())), "23:00 the day before");
    // Whole days, minutes that are no whole hour, and a timed event keep their words.
    assert_eq!(alert_text(Alert::Before(1440), Some(AllDayAlert::default())), "1 day before");
    assert_eq!(alert_text(Alert::Before(90), Some(AllDayAlert::default())), "90 min before");
    assert_eq!(alert_text(Alert::Before(0), Some(AllDayAlert::default())), "At the start");
    assert_eq!(alert_text(Alert::Before(900), None), "15 hours before");
}

#[test]
fn a_new_all_day_event_in_the_calendar_says_the_hour_the_session_holds() {
    let (mut mailbox, mut calendar) = loaded();
    mailbox.set_all_day_alert(AllDayAlert::DayBefore18);
    calendar.follow_week(&mut mailbox);
    calendar.new_event();
    let editor = calendar.editor().unwrap();
    assert_eq!(editor.all_day_alert(), AllDayAlert::DayBefore18, "taken when it opens");
    mailbox.set_all_day_alert(AllDayAlert::OnTheDay9);
    calendar.follow_week(&mut mailbox);
    assert_eq!(calendar.editor().unwrap().all_day_alert(), AllDayAlert::OnTheDay9, "and followed while it is open");
}

#[test]
fn changing_the_alert_marks_the_edit_and_sends_it_alone() {
    let editor = Editor::edit(&row(Some(json!(15))));
    assert_eq!(editor.form().alert, Alert::Before(15));
    assert_eq!(editor.changes(Offset::UTC), Ok(None));
    let mut editor = editor;
    editor.form_mut().alert = Alert::Before(60);
    assert!(editor.edited());
    assert_eq!(editor.changes(Offset::UTC), Ok(Some(("event.update", json!({ "id": 9, "alert": 60 })))));
    editor.form_mut().alert = Alert::None;
    assert_eq!(editor.changes(Offset::UTC), Ok(Some(("event.update", json!({ "id": 9, "alert": "none" })))));
    editor.form_mut().alert = Alert::Before(15);
    assert!(!editor.edited(), "back as it was");
    assert_eq!(editor.changes(Offset::UTC), Ok(None));
}

#[test]
fn a_default_or_custom_alert_is_never_sent() {
    let mut editor = Editor::edit(&row(Some(json!("custom"))));
    assert_eq!(editor.form().alert, Alert::Custom);
    editor.form_mut().summary = "Kiln firing".to_owned();
    assert_eq!(editor.changes(Offset::UTC), Ok(Some(("event.update", json!({ "id": 9, "summary": "Kiln firing" })))));
    let mut editor = Editor::edit(&row(Some(json!(30))));
    editor.form_mut().alert = Alert::Default;
    assert_eq!(editor.changes(Offset::UTC), Ok(None), "the default is no choice of the field");
    editor.form_mut().alert = Alert::Custom;
    assert_eq!(editor.changes(Offset::UTC), Ok(None), "nor is custom");
}

#[test]
fn a_new_event_sends_the_alert_only_when_one_is_picked() {
    let (_, mut calendar) = loaded();
    calendar.new_event();
    let editor = calendar.editor_mut().unwrap();
    assert_eq!(editor.form().alert, Alert::Default);
    let (_, params) = editor.changes(Offset::UTC).unwrap().unwrap();
    assert_eq!(params.get("alert"), None, "the small editor has no Alert field: the default lead rings");
    editor.form_mut().alert = Alert::Before(30);
    let (_, params) = editor.changes(Offset::UTC).unwrap().unwrap();
    assert_eq!(params.get("alert"), Some(&json!(30)));
    editor.form_mut().alert = Alert::None;
    let (_, params) = editor.changes(Offset::UTC).unwrap().unwrap();
    assert_eq!(params.get("alert"), Some(&json!("none")));
}

#[test]
fn a_saved_alert_comes_back_on_the_event_and_into_its_editor() {
    let (mut mailbox, mut calendar) = loaded();
    calendar.new_event();
    let form = calendar.editor_mut().unwrap().form_mut();
    form.summary = "Glaze test".to_owned();
    form.start = "2026-11-06 18:00".to_owned();
    form.end = "2026-11-06 19:00".to_owned();
    form.alert = Alert::Before(30);
    calendar.save(&mut mailbox);
    assert!(mailbox.error().is_none(), "{:?}", mailbox.error());
    let event = named(&calendar, "Glaze test").unwrap();
    assert_eq!(event.alert, Alert::Before(30));
    let key = event.key();
    calendar.pick(Some(key));
    calendar.edit_picked();
    let editor = calendar.editor_mut().unwrap();
    assert_eq!(editor.form().alert, Alert::Before(30));
    editor.form_mut().alert = Alert::None;
    calendar.save(&mut mailbox);
    assert!(mailbox.error().is_none(), "{:?}", mailbox.error());
    assert_eq!(named(&calendar, "Glaze test").unwrap().alert, Alert::None);
}
