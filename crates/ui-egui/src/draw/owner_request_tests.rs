// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An agent's request for an `owner` command, in the window: it waits until the owner answers, runs for the owner on
//! Allow only, as one step of the window's undo the agent cannot take back, and nothing runs on Deny, on a time out
//! or on a second request.

use std::time::Duration;

use inboxcraft_engine::EngineError;
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use super::InboxApp;
use crate::mailbox::Mailbox;
use crate::owner_request::Settled;
use crate::sample;

/// 2026-11-05T09:00:00Z.
const START: i64 = 1_793_869_200;

/// The sample calendar the imports go to.
const CALENDAR: u64 = 2;

/// The sample window on a clock the test moves.
fn window() -> (InboxApp, FakeClock) {
    let clock = FakeClock::new(Timestamp::from_unix(START, 0).unwrap());
    let app = InboxApp::new(Mailbox::new(sample::session(Box::new(clock.clone())).unwrap(), Box::new(clock.clone())));
    (app, clock)
}

/// Two invented events of `example.org` in an `.ics` file, for `calendar.import`.
fn import_params() -> Value {
    let event = |uid: &str, day: u8, summary: &str| {
        format!(
            "BEGIN:VEVENT\r\nUID:{uid}@example.org\r\nDTSTART:202611{day:02}T080000Z\r\nDTEND:202611{day:02}T090000Z\r\nSUMMARY:{summary}\r\nEND:VEVENT\r\n"
        )
    };
    let ics = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//Export//EN\r\n{}{}END:VCALENDAR\r\n",
        event("owner-request-1", 10, "Dentist"),
        event("owner-request-2", 12, "Piano lesson")
    );
    json!({ "calendar": CALENDAR, "ics": ics })
}

/// How many events the import's calendar holds.
fn events(app: &mut InboxApp) -> u64 {
    let listed = app.mailbox_mut().quietly(|mailbox| mailbox.run("calendar.list", &json!({}))).unwrap();
    listed["calendars"].as_array().unwrap().iter().find(|row| row["id"] == CALENDAR).unwrap()["events"].as_u64().unwrap()
}

fn asked(app: &mut InboxApp) -> crate::owner_request::Ticket {
    app.ask_owner(Some("inboxcraft-cli"), "calendar.import", &import_params()).unwrap()
}

#[test]
fn nothing_runs_until_the_owner_answers() {
    let (mut app, clock) = window();
    let held = events(&mut app);
    asked(&mut app);
    assert!(app.asks_owner());
    for _ in 0..12 {
        app.expire_owner_requests();
        clock.advance(Duration::from_secs(9));
    }
    app.expire_owner_requests();
    assert!(app.take_settled().is_empty());
    assert!(app.asks_owner());
    assert_eq!(events(&mut app), held);
    assert!(!app.mailbox().can_undo());
}

#[test]
fn allowing_runs_it_for_the_owner_and_answers_with_its_result() {
    let (mut app, _clock) = window();
    let held = events(&mut app);
    let ticket = asked(&mut app);
    app.answer_owner_request(true);
    let settled = app.take_settled();
    let [(got, Settled::Ran(Ok(answer)))] = settled.as_slice() else { panic!("not run: {settled:?}") };
    assert_eq!(*got, ticket);
    assert_eq!(answer["added"], json!(2));
    assert_eq!(events(&mut app), held + 2);
    assert!(!app.asks_owner());
}

#[test]
fn allowing_runs_it_for_the_owner_while_the_window_runs_for_an_agent() {
    let (mut app, _clock) = window();
    app.mailbox_mut().set_caller(inboxcraft_engine::Caller::Agent);
    asked(&mut app);
    app.answer_owner_request(true);
    assert!(matches!(app.take_settled().as_slice(), [(_, Settled::Ran(Ok(_)))]));
}

#[test]
fn denying_runs_nothing() {
    let (mut app, _clock) = window();
    let held = events(&mut app);
    let ticket = asked(&mut app);
    app.answer_owner_request(false);
    assert!(matches!(app.take_settled().as_slice(), [(got, Settled::Denied)] if *got == ticket));
    assert_eq!(events(&mut app), held);
    assert!(!app.mailbox().can_undo());
    assert!(app.mailbox().undoable().is_none());
}

#[test]
fn a_request_nobody_answers_is_timed_out_after_two_minutes_and_runs_nothing() {
    let (mut app, clock) = window();
    let held = events(&mut app);
    let ticket = asked(&mut app);
    app.expire_owner_requests();
    assert_eq!(app.owner_request_due_in(), Some(Duration::from_secs(120)));
    clock.advance(Duration::from_secs(119));
    app.expire_owner_requests();
    assert!(app.take_settled().is_empty());
    assert_eq!(app.owner_request_due_in(), Some(Duration::from_secs(1)));
    clock.advance(Duration::from_secs(1));
    app.expire_owner_requests();
    assert!(matches!(app.take_settled().as_slice(), [(got, Settled::TimedOut)] if *got == ticket));
    assert_eq!(events(&mut app), held);
    assert_eq!(app.owner_request_due_in(), None);
}

#[test]
fn a_late_allow_answers_no_other_request() {
    let (mut app, clock) = window();
    let held = events(&mut app);
    let first = asked(&mut app);
    let second = asked(&mut app);
    app.expire_owner_requests();
    clock.advance(Duration::from_secs(120));
    // The owner presses Allow on the first's question after its time is over, before the window looked again.
    app.answer_owner_request(true);
    assert!(matches!(app.take_settled().as_slice(), [(got, Settled::TimedOut)] if *got == first));
    assert_eq!(events(&mut app), held);
    assert!(app.asks_owner(), "the second still waits for its own answer");
    app.answer_owner_request(false);
    assert!(matches!(app.take_settled().as_slice(), [(got, Settled::Denied)] if *got == second));
}

#[test]
fn an_allowed_request_is_one_step_of_the_window_s_undo() {
    let (mut app, _clock) = window();
    let held = events(&mut app);
    asked(&mut app);
    app.answer_owner_request(true);
    assert!(app.mailbox().can_undo());
    let undo = app.mailbox().undoable().unwrap();
    assert!(undo.undoable);
    assert_eq!(undo.text, "An agent ran calendar.import");
    assert_eq!(undo.act, crate::mailbox::Act::Asked);
    app.mailbox_mut().undo();
    assert_eq!(events(&mut app), held, "one undo takes both events back");
    assert!(!app.mailbox().can_undo());
}

#[test]
fn an_allowed_request_is_one_step_of_the_window_s_undo_when_the_history_is_full() {
    let (mut app, _clock) = window();
    // An agent's acts fill the engine's history and end the window's undo for the acts before them.
    for turn in 0..inboxcraft_engine::MAX_HISTORY {
        app.mailbox_mut().run_for_agent("mail.flag", &json!({ "id": 1, "flagged": turn % 2 == 0 })).unwrap();
    }
    assert!(!app.mailbox().can_undo());
    let held = events(&mut app);
    asked(&mut app);
    app.answer_owner_request(true);
    assert_eq!(events(&mut app), held + 2);
    assert!(app.mailbox().can_undo());
    assert!(app.mailbox().undoable().unwrap().undoable);
    app.mailbox_mut().undo();
    assert_eq!(events(&mut app), held, "Undo in the window takes the import back");
}

#[test]
fn a_withdrawn_request_leaves_the_queue_and_a_later_allow_runs_nothing() {
    let (mut app, _clock) = window();
    let held = events(&mut app);
    let ticket = asked(&mut app);
    app.expire_owner_requests();
    app.withdraw_owner_request(ticket);
    assert!(!app.asks_owner());
    app.answer_owner_request(true);
    assert_eq!(events(&mut app), held);
    assert!(app.take_settled().is_empty(), "nobody is answered");
}

#[test]
fn a_request_another_side_took_back_first_runs_nothing_on_allow() {
    let (mut app, _clock) = window();
    let held = events(&mut app);
    let ticket = app.ask_owner_if(Some("inboxcraft-cli"), "calendar.import", &import_params(), Box::new(|| false)).unwrap();
    app.answer_owner_request(true);
    assert_eq!(events(&mut app), held);
    assert!(!app.asks_owner());
    assert!(app.take_settled().is_empty(), "nobody is answered for {ticket:?}");
}

#[test]
fn withdrawing_the_front_starts_the_two_minutes_of_the_one_behind_it() {
    let (mut app, clock) = window();
    let first = asked(&mut app);
    clock.advance(Duration::from_secs(60));
    let second = asked(&mut app);
    app.expire_owner_requests();
    clock.advance(Duration::from_secs(100));
    app.withdraw_owner_request(first);
    app.expire_owner_requests();
    clock.advance(Duration::from_secs(119));
    app.expire_owner_requests();
    assert!(app.take_settled().is_empty(), "the second waits its own two minutes");
    clock.advance(Duration::from_secs(1));
    app.expire_owner_requests();
    assert!(matches!(app.take_settled().as_slice(), [(got, Settled::TimedOut)] if *got == second));
}

#[test]
fn the_agent_cannot_take_the_owner_s_step_back() {
    let (mut app, _clock) = window();
    let held = events(&mut app);
    asked(&mut app);
    app.answer_owner_request(true);
    let undone = app.mailbox_mut().run_for_agent("history.undo", &json!({}));
    assert!(matches!(undone, Err(EngineError::NotAllowed { .. })), "{undone:?}");
    assert_eq!(events(&mut app), held + 2);
    assert!(app.mailbox().can_undo());
}

#[test]
fn a_command_that_is_not_the_owner_s_is_not_queued() {
    let (mut app, _clock) = window();
    // The registry holds no `never` command yet: a `safe` one, an unknown one and no id stand for what is not the
    // owner's.
    for command in ["mail.list", "calendar.import.preview", "history.undo", "no.such", ""] {
        assert_eq!(app.ask_owner(None, command, &json!({})), None, "{command}");
    }
    assert!(!app.asks_owner());
    assert_eq!(app.owner_request_due_in(), None);
}

#[test]
fn a_ninth_request_is_turned_away() {
    let (mut app, _clock) = window();
    for _ in 0..8 {
        asked(&mut app);
    }
    assert_eq!(app.ask_owner(None, "calendar.import", &import_params()), None);
}

#[test]
fn a_second_request_waits_and_is_not_an_approval() {
    let (mut app, _clock) = window();
    let held = events(&mut app);
    let first = asked(&mut app);
    let second = asked(&mut app);
    app.answer_owner_request(true);
    let settled = app.take_settled();
    assert!(matches!(settled.as_slice(), [(got, Settled::Ran(Ok(_)))] if *got == first), "{settled:?}");
    assert!(app.asks_owner());
    assert_eq!(app.owner_requests.front().map(|front| front.ticket), Some(second));
    assert!(app.take_settled().is_empty());
    assert_eq!(events(&mut app), held + 2, "the second import ran not");
}

#[test]
fn a_refused_command_is_answered_with_its_error() {
    let (mut app, _clock) = window();
    let params = json!({ "calendar": 999, "summary": "Call", "start": "2026-11-10T08:00:00Z" });
    app.ask_owner(None, "event.create", &params).unwrap();
    app.answer_owner_request(true);
    let settled = app.take_settled();
    assert!(matches!(settled.as_slice(), [(_, Settled::Ran(Err(EngineError::NotFound { .. })))]), "{settled:?}");
    assert!(app.mailbox().undoable().is_none(), "no toast");
    assert!(!app.mailbox().can_undo(), "the history as before");
}

#[test]
fn an_answer_with_nothing_waiting_does_nothing() {
    let (mut app, _clock) = window();
    app.answer_owner_request(true);
    app.answer_owner_request(false);
    app.expire_owner_requests();
    assert!(app.take_settled().is_empty());
    assert!(app.mailbox().undoable().is_none());
}
