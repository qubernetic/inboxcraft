// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Someone else's event in the editor: shown, never changed or saved. The description: kept, sent when it changed,
//! and a press on a link in it with Ctrl held (any press in someone else's event) copies the link, as the reader does.

use inboxcraft_time::{FakeClock, Offset, Timestamp};
use serde_json::json;

use super::{Editor, MAX_LINK, link_at};
use crate::calendar::{Calendar, Day, EventRow, MAX_DESCRIPTION};
use crate::mailbox::Mailbox;
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

/// Picks the event `summary` and opens it in the editor.
fn editing(calendar: &mut Calendar, summary: &str) {
    let key = calendar.events().find(|event| event.summary == summary).map(EventRow::key);
    calendar.pick(key);
    calendar.edit_picked();
}

/// The row of a timed event with `description`, the owner's own unless `own` is false.
fn row(description: &str, own: bool) -> EventRow {
    let value = json!({
        "id": 7, "calendar": 1, "summary": "Kiln", "description": description,
        "local_start": "2026-11-06T14:00:00Z", "local_end": "2026-11-06T15:00:00Z",
        "start_date": "2026-11-06", "end_date": "2026-11-06",
        "organizer": { "address": "teodor@example.org", "name": "Teodor Lisk" }, "own": own,
    });
    EventRow::read(&value).unwrap()
}

#[test]
fn someone_elses_event_opens_read_only_and_saves_nothing() {
    let (mut mailbox, mut calendar) = loaded();
    editing(&mut calendar, "Allotment work party");
    let editor = calendar.editor_mut().unwrap();
    assert!(editor.read_only(), "Teodor Lisk organises the allotment work party, not the owner");
    assert_eq!(editor.title(), "Event");
    assert_eq!(editor.read_only_note().as_deref(), Some("Teodor Lisk's event. You can read it here; only the organiser can change it."));
    assert!(!editor.asks_notify(), "nothing is changed, so nobody is told");
    assert!(!editor.description_editable());
    editor.form_mut().summary = "Changed anyway".to_owned();
    assert_eq!(editor.changes(Offset::UTC), Ok(None), "nothing is ever sent for someone else's event");
    let before = mailbox.last_run();
    calendar.save(&mut mailbox);
    assert_eq!(mailbox.last_run(), before, "a save runs nothing");
    assert!(calendar.editor().is_none(), "and closes the editor");
}

#[test]
fn the_owners_own_event_opens_to_be_edited() {
    let (_, mut calendar) = loaded();
    editing(&mut calendar, "Call with Petra about the logo");
    let editor = calendar.editor().unwrap();
    assert!(!editor.read_only(), "the owner's studio address organises the call");
    assert_eq!(editor.title(), "Edit event");
    assert_eq!(editor.read_only_note(), None);
    editing(&mut calendar, "Dentist");
    assert!(!calendar.editor().unwrap().read_only(), "an event without an organiser is the owner's");
}

#[test]
fn an_unnamed_organiser_is_someone_else_all_the_same() {
    let mut value = json!({
        "id": 7, "calendar": 1, "summary": "Kiln", "local_start": "2026-11-06T14:00:00Z", "local_end": "2026-11-06T15:00:00Z",
        "start_date": "2026-11-06", "end_date": "2026-11-06", "own": false,
    });
    let editor = Editor::edit(&EventRow::read(&value).unwrap());
    assert_eq!(editor.read_only_note().as_deref(), Some("Someone else's event. You can read it here; only the organiser can change it."));
    value["own"] = json!(null);
    assert!(!Editor::edit(&EventRow::read(&value).unwrap()).read_only(), "a row that does not say is the owner's, as before");
}

#[test]
fn the_description_is_sent_only_when_it_changed() {
    let mut editor = Editor::edit(&row("Bring the glaze.\nAnd the gloves.", true));
    assert_eq!(editor.form().description, "Bring the glaze.\nAnd the gloves.", "its lines kept");
    editor.form_mut().summary = "Kiln firing".to_owned();
    let (_, params) = editor.changes(Offset::UTC).unwrap().unwrap();
    assert_eq!(params.get("description"), None, "an untouched description is not sent");
    editor.form_mut().description.push_str("\nAnd tea.");
    let (_, params) = editor.changes(Offset::UTC).unwrap().unwrap();
    assert_eq!(params["description"], json!("Bring the glaze.\nAnd the gloves.\nAnd tea."));
    let mut new = Editor::new_event(1, Day::new(2026, 11, 6).unwrap(), vec![(1, "Personal".to_owned())]);
    let (_, params) = new.changes(Offset::UTC).unwrap().unwrap();
    assert_eq!(params.get("description"), None, "a new event without one sends none");
    new.form_mut().description = "Bring the glaze.".to_owned();
    let (_, params) = new.changes(Offset::UTC).unwrap().unwrap();
    assert_eq!(params["description"], json!("Bring the glaze."));
}

#[test]
fn a_description_read_cut_short_is_shown_and_never_sent_back() {
    let long = "a".repeat(MAX_DESCRIPTION + 50);
    let mut editor = Editor::edit(&row(&long, true));
    assert_eq!(editor.form().description.chars().count(), MAX_DESCRIPTION, "the row holds it cut");
    assert!(!editor.description_editable(), "editing it would write the cut text over the whole one");
    editor.form_mut().description.push('b');
    editor.form_mut().summary = "Kiln firing".to_owned();
    let (_, params) = editor.changes(Offset::UTC).unwrap().unwrap();
    assert_eq!(params.get("description"), None);
}

#[test]
fn the_link_under_a_press_is_the_word_that_starts_with_its_scheme() {
    let text = "The boards: (https://files.example.org/bakery/boards). Mail mailto:teodor@example.org, or HTTP://Example.ORG.";
    let at = |word: &str| text.find(word).map(|byte| text.get(..byte).unwrap().chars().count()).unwrap();
    assert_eq!(link_at(text, at("files")).as_deref(), Some("https://files.example.org/bakery/boards"), "brackets and the full stop left off");
    assert_eq!(link_at(text, at("https")).as_deref(), Some("https://files.example.org/bakery/boards"), "its first character");
    assert_eq!(link_at(text, at("boards)")).as_deref(), Some("https://files.example.org/bakery/boards"));
    assert_eq!(link_at(text, at("(https")), None, "the bracket before it is no link");
    assert_eq!(link_at(text, at("teodor")).as_deref(), Some("mailto:teodor@example.org"));
    assert_eq!(link_at(text, at("Example.ORG")).as_deref(), Some("HTTP://Example.ORG"), "a scheme in capitals");
    assert_eq!(link_at(text, at("boards:")), None, "a plain word");
    assert_eq!(link_at(text, 10_000), None, "past the end");
    assert_eq!(link_at("https:// x", 2), None, "a web link without a host");
    assert_eq!(link_at("javascript:alert(1)", 3), None, "another scheme");
    assert_eq!(link_at("ünï https://example.org/ä", 6).as_deref(), Some("https://example.org/ä"), "counted in characters");
    let long = format!("https://example.org/{}", "a".repeat(MAX_LINK));
    assert_eq!(link_at(&long, 3), None, "too long to copy");
}

#[test]
fn a_press_copies_a_link_while_ctrl_is_held_and_in_someone_elses_event_always() {
    let text = "Boards: https://files.example.org/boards";
    let mut editor = Editor::edit(&row(text, true));
    assert_eq!(editor.press_description(12, false), None, "a plain press while editing puts the caret there");
    assert_eq!(editor.take_copied(), None);
    assert_eq!(editor.press_description(12, true).as_deref(), Some("https://files.example.org/boards"));
    assert_eq!(editor.take_copied().as_deref(), Some("https://files.example.org/boards"), "the status line says it once");
    assert_eq!(editor.take_copied(), None);
    assert_eq!(editor.press_description(2, true), None, "no link under the press");
    let mut theirs = Editor::edit(&row(text, false));
    assert_eq!(theirs.press_description(12, false).as_deref(), Some("https://files.example.org/boards"), "reading, as the reader does");
}
