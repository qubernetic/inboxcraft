// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An agent's `history.undo` while the owner has an event open in the editor: refused with "an event is being
//! edited", and allowed again once the editor ends by Save, Cancel or Esc; the owner's own undo runs while it is open.

use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use super::InboxApp;
use super::calendar::Asked;
use crate::mailbox::Mailbox;
use crate::sample;

/// 2026-11-05T09:00:00Z.
const START: i64 = 1_793_869_200;

/// The sample window on the calendar, an agent's flag on message 1 to take back.
fn flagged_by_an_agent() -> InboxApp {
    let clock = FakeClock::new(Timestamp::from_unix(START, 0).unwrap());
    let mut app = InboxApp::new(Mailbox::new(sample::session(Box::new(clock.clone())).unwrap(), Box::new(clock)));
    app.show_calendar();
    let flagged = app.mailbox_mut().run_for_agent("mail.flag", &json!({ "id": 1, "flagged": true }));
    assert_eq!(flagged.unwrap()["changed"], json!(true));
    app
}

/// The editor opened on a new event, as New event's press asks.
fn open_editor(app: &mut InboxApp) {
    app.apply_calendar(vec![Asked::New]);
    assert!(app.calendar().editor().is_some(), "the editor is open");
}

/// An agent's `history.undo`, as the control channel runs it.
fn agent_undo(app: &mut InboxApp) -> Result<Value, inboxcraft_engine::EngineError> {
    app.mailbox_mut().run_for_agent("history.undo", &json!({}))
}

/// Whether message 1 is flagged, as the list holds it.
fn flagged(app: &InboxApp) -> Option<bool> {
    app.mailbox().messages().iter().find(|row| row.id == 1).map(|row| row.flagged)
}

#[test]
fn an_agent_s_undo_is_refused_while_the_editor_is_open_and_the_owner_s_runs() {
    let mut app = flagged_by_an_agent();
    open_editor(&mut app);
    let refused = agent_undo(&mut app).unwrap_err();
    assert_eq!(refused.code(), "editing");
    assert_eq!(refused.to_string(), "an event is being edited");
    assert_eq!(flagged(&app), Some(true), "nothing was taken back");
    // The owner's own undo, as the window's Undo runs it, takes the flag back under the open editor.
    let undone = app.mailbox_mut().run("history.undo", &json!({})).unwrap();
    assert_eq!(undone["command"], json!("mail.flag"));
    app.mailbox_mut().refresh();
    assert_eq!(flagged(&app), Some(false), "the owner's undo took it back");
    assert!(app.calendar().editor().is_some(), "and the editor stays open");
}

#[test]
fn an_agent_s_undo_runs_again_once_the_editor_is_cancelled() {
    let mut app = flagged_by_an_agent();
    open_editor(&mut app);
    app.apply_calendar(vec![Asked::Cancel]);
    assert!(app.calendar().editor().is_none());
    assert_eq!(agent_undo(&mut app).unwrap()["command"], json!("mail.flag"));
}

#[test]
fn an_agent_s_undo_runs_again_once_the_editor_is_left_by_escape() {
    let mut app = flagged_by_an_agent();
    open_editor(&mut app);
    app.apply_calendar(vec![Asked::Leave(crate::component::popup::Leave::Escape)]);
    assert!(app.calendar().editor().is_none(), "with nothing typed Esc closes it");
    assert_eq!(agent_undo(&mut app).unwrap()["command"], json!("mail.flag"));
}

#[test]
fn an_agent_s_undo_is_not_refused_for_the_editor_once_the_event_is_saved() {
    let mut app = flagged_by_an_agent();
    open_editor(&mut app);
    let (calendar, _) = app.calendar_mut();
    calendar.editor_mut().unwrap().form_mut().summary = "Lunch with Ada".to_owned();
    app.apply_calendar(vec![Asked::Save]);
    assert!(app.calendar().editor().is_none(), "saved");
    // The newest act is now the owner's event: whatever the engine says of it, it is no longer the editor.
    if let Err(error) = agent_undo(&mut app) {
        assert_ne!(error.code(), "editing", "{error}");
    }
}
