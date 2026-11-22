// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An agent's request for an `owner` command, in the window: it waits until the owner answers, runs for the owner on
//! Allow only, as one step of the window's undo the agent cannot take back, and nothing runs on Deny, on a time out
//! or on a second request.

use std::time::Duration;

use inboxcraft_engine::EngineError;
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use std::sync::Arc;

use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::InboxApp;
use crate::mailbox::Mailbox;
use crate::onboard::NoDetect;
use crate::onboard::tests::Kept;
use crate::owner_request::{ARM_SECONDS, Settled};
use crate::sample;
use crate::workers::NoWorkers;

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

/// The name of the import's calendar.
fn name_of(mailbox: &mut Mailbox) -> String {
    let listed = mailbox.quietly(|mailbox| mailbox.run("calendar.list", &json!({}))).unwrap();
    listed["calendars"].as_array().unwrap().iter().find(|row| row["id"] == CALENDAR).unwrap()["name"].as_str().unwrap().to_owned()
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
    let name = name_of(app.mailbox_mut());
    let undo = app.mailbox().undoable().unwrap();
    assert!(undo.undoable);
    assert_eq!(undo.text, format!("An agent added 2 events to “{name}”"), "the app's words");
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

// The question in the window.

/// The frames' step: small, so a few keys fall well inside [`ARM_SECONDS`].
const STEP: f32 = 1.0 / 60.0;

/// The sample window on the mail, at `size`, `set` done to it before its first frame, on a clock the test moves.
fn asking_window_at(size: egui::Vec2, set: impl FnOnce(&mut InboxApp)) -> (Harness<'static, InboxApp>, FakeClock) {
    let clock = FakeClock::new(Timestamp::from_unix(START, 0).unwrap());
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(Box::new(clock.clone())).unwrap(), Box::new(clock.clone()))).with_accounts(
        Arc::new(Kept::default()),
        Box::new(NoDetect::default()),
        Box::new(NoWorkers),
    );
    app.set_reduce_motion(true);
    set(&mut app);
    let mut harness = Harness::builder()
        .with_size(size)
        .with_max_steps(40)
        .with_step_dt(STEP)
        .with_theme(egui::Theme::Dark)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    (harness, clock)
}

/// The sample window on the mail.
fn shown_window() -> Harness<'static, InboxApp> {
    asking_window_at(vec2(1440.0, 900.0), |_| {}).0
}

/// The sample window with the keyboard in the composer's text, as the owner writing.
fn window_with_the_composer_focused() -> Harness<'static, InboxApp> {
    let (mut harness, _) = asking_window_at(vec2(1440.0, 900.0), InboxApp::compose);
    harness.ctx.memory_mut(|memory| memory.request_focus(super::super::compose::text_id()));
    harness.run();
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), Some(super::super::compose::text_id()), "the owner is writing");
    harness
}

/// An `.ics` file of `events`, each the inside of one VEVENT.
fn ics(events: &[String]) -> String {
    let events: String = events.iter().map(|event| format!("BEGIN:VEVENT\r\n{event}\r\nEND:VEVENT\r\n")).collect();
    format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//Export//EN\r\n{events}END:VCALENDAR\r\n")
}

/// One timed event of `example.org` on a day of November 2026.
fn event(uid: &str, day: u8, summary: &str) -> String {
    format!("UID:{uid}@example.org\r\nDTSTART:202611{day:02}T080000Z\r\nDTEND:202611{day:02}T090000Z\r\nSUMMARY:{summary}")
}

/// Seven invented events.
fn seven_events() -> String {
    let titles = ["Choir practice", "Dentist", "Piano lesson", "Book club", "Bike repair", "Allotment day", "Pottery class"];
    let events: Vec<String> = titles.iter().zip(1_u8..).map(|(title, n)| event(&format!("seven-{n}"), 10 + n, title)).collect();
    ics(&events)
}

/// Asks the owner, as the CLI does, to import `ics` into the sample's calendar.
fn ask(harness: &mut Harness<'static, InboxApp>, ics: &str) -> crate::owner_request::Ticket {
    ask_as(harness, Some("inboxcraft-cli"), ics)
}

fn ask_as(harness: &mut Harness<'static, InboxApp>, client: Option<&str>, ics: &str) -> crate::owner_request::Ticket {
    harness.state_mut().ask_owner(client, "calendar.import", &json!({ "calendar": CALENDAR, "ics": ics })).unwrap()
}

fn calendar_name(harness: &mut Harness<'static, InboxApp>) -> String {
    name_of(harness.state_mut().mailbox_mut())
}

fn events_shown(harness: &mut Harness<'static, InboxApp>) -> u64 {
    events(harness.state_mut())
}

/// Moves egui's time on by `seconds`, a frame at a time.
fn wait(harness: &mut Harness<'static, InboxApp>, seconds: f64) {
    let from = harness.ctx.input(|input| input.time);
    for _ in 0..10_000 {
        if harness.ctx.input(|input| input.time) - from >= seconds {
            return;
        }
        harness.step();
    }
}

fn focused(harness: &Harness<'static, InboxApp>) -> Option<String> {
    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).map(|node| node.accesskit_node().label().unwrap_or_default())
}

fn press(harness: &mut Harness<'static, InboxApp>, label: &str) {
    harness.get_by_label(label).scroll_to_me();
    harness.run();
    harness.get_by_label(label).click();
    harness.run();
}

fn key(harness: &mut Harness<'static, InboxApp>, key: Key) {
    harness.key_press_modifiers(Modifiers::NONE, key);
    harness.run();
}

/// `key` pressed and held for `repeats` repeats, then let go; unless `fresh`, it was down before the keyboard came to
/// `then`.
fn hold(harness: &mut Harness<'static, InboxApp>, key: Key, repeats: usize, fresh: bool, then: &str) {
    let event = |pressed, repeat| egui::Event::Key { key, physical_key: None, pressed, repeat, modifiers: Modifiers::NONE };
    if !fresh && let Some(id) = harness.ctx.memory(|memory| memory.focused()) {
        harness.ctx.memory_mut(|memory| memory.surrender_focus(id));
    }
    harness.event(event(true, false));
    harness.run();
    if !fresh {
        harness.get_by_label(then).focus();
        harness.run();
    }
    for _ in 0..repeats {
        harness.event(event(true, true));
        harness.run();
    }
    harness.event(event(false, false));
    harness.run();
}

/// A press of the pointer at `at`, down in one frame and up in the next.
fn press_at(harness: &mut Harness<'static, InboxApp>, at: egui::Pos2) {
    harness.hover_at(at);
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        harness.run();
    }
}

/// Every label the window's tree holds.
fn labels(harness: &Harness<'static, InboxApp>) -> Vec<String> {
    // The data's no-break spaces (which keep a quoted run on one row) read as spaces.
    harness
        .root()
        .children_recursive()
        .filter_map(|node| node.accesskit_node().label().or_else(|| node.accesskit_node().value()))
        .map(|label| label.replace('\u{a0}', " "))
        .collect()
}

fn ran(settled: &[(crate::owner_request::Ticket, Settled)]) -> bool {
    settled.iter().any(|(_, settled)| matches!(settled, Settled::Ran(_)))
}

const LAST: &str = "Nothing already in the calendar is changed or removed. Undo takes the events back.";

#[test]
fn the_question_opens_on_deny_and_runs_only_on_the_owner_s_press_of_allow() {
    let mut harness = shown_window();
    ask(&mut harness, &seven_events());
    harness.run();
    assert_eq!(focused(&harness).as_deref(), Some("Deny"));
    let name = calendar_name(&mut harness);
    assert!(harness.query_by_label(&format!("Add 7 events to “{name}”?")).is_some());
    assert!(harness.state().surfaces(&harness.ctx).modal(), "the question is a modal surface");
    wait(&mut harness, ARM_SECONDS);
    press(&mut harness, "Allow once");
    let settled = harness.state_mut().take_settled();
    assert!(matches!(settled.as_slice(), [(_, Settled::Ran(Ok(answer)))] if answer["added"] == 7), "{settled:?}");
    assert!(harness.query_by_label("Allow once").is_none(), "the question is gone");
}

#[test]
fn keys_on_their_way_when_the_question_opens_never_allow() {
    let mut harness = window_with_the_composer_focused();
    let before = events_shown(&mut harness);
    ask(&mut harness, &seven_events());
    for pressed in [Key::Tab, Key::Enter, Key::Space, Key::Tab, Key::Enter] {
        key(&mut harness, pressed); // within ARM_SECONDS of the question showing
    }
    let settled = harness.state_mut().take_settled();
    assert!(!ran(&settled), "{settled:?}");
    assert_eq!(events_shown(&mut harness), before);
}

#[test]
fn an_allow_sooner_than_the_arming_second_is_no_answer() {
    let mut harness = shown_window();
    ask(&mut harness, &seven_events());
    harness.run();
    press(&mut harness, "Allow once");
    assert!(harness.state_mut().take_settled().is_empty(), "too soon: nothing");
    assert!(harness.state().asks_owner(), "the question still waits");
    wait(&mut harness, ARM_SECONDS);
    press(&mut harness, "Allow once");
    assert!(ran(&harness.state_mut().take_settled()), "armed now");
}

#[test]
fn a_press_counts_only_for_the_question_it_was_drawn_for() {
    let mut harness = shown_window();
    let name = calendar_name(&mut harness);
    ask(&mut harness, &seven_events());
    ask(&mut harness, &ics(&[event("next-1", 20, "Tyre change"), event("next-2", 21, "Haircut")]));
    harness.run();
    wait(&mut harness, ARM_SECONDS);
    // The first question, armed, is denied; the next one comes to the front unarmed.
    press(&mut harness, "Deny");
    assert!(matches!(harness.state_mut().take_settled().as_slice(), [(_, Settled::Denied)]));
    assert!(harness.query_by_label(&format!("Add 2 events to “{name}”?")).is_some(), "the next question shows");
    assert_eq!(focused(&harness).as_deref(), Some("Deny"), "and opens on Deny");
    press(&mut harness, "Allow once");
    assert!(harness.state_mut().take_settled().is_empty(), "a press meant for the question before lands on no other");
    // The same when the front was withdrawn by its asker.
    let third = ask(&mut harness, &ics(&[event("third-1", 22, "Window cleaner")]));
    wait(&mut harness, ARM_SECONDS);
    let second = harness.state().owner_requests.front().map(|front| front.ticket).unwrap();
    harness.state_mut().withdraw_owner_request(second);
    harness.run();
    assert_eq!(harness.state().owner_requests.front().map(|front| front.ticket), Some(third));
    press(&mut harness, "Allow once");
    assert!(harness.state_mut().take_settled().is_empty(), "the withdrawn one's arming is not the next one's");
    wait(&mut harness, ARM_SECONDS);
    press(&mut harness, "Allow once");
    assert!(matches!(harness.state_mut().take_settled().as_slice(), [(ticket, Settled::Ran(Ok(_)))] if *ticket == third));
}

#[test]
fn esc_and_a_press_on_the_dimmed_window_deny() {
    let mut harness = window_with_the_composer_focused();
    let writing = harness.ctx.memory(|memory| memory.focused());
    ask(&mut harness, &seven_events());
    harness.run();
    assert_eq!(focused(&harness).as_deref(), Some("Deny"));
    assert!(harness.query_by_label("Esc deny").is_some(), "the status line says Esc denies");
    key(&mut harness, Key::Escape);
    assert!(matches!(harness.state_mut().take_settled().as_slice(), [(_, Settled::Denied)]));
    harness.run();
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), writing, "the keyboard goes back where it was");
    ask(&mut harness, &seven_events());
    harness.run();
    press_at(&mut harness, egui::pos2(40.0, 60.0));
    assert!(matches!(harness.state_mut().take_settled().as_slice(), [(_, Settled::Denied)]));
    harness.run();
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), writing, "the keyboard goes back where it was");
}

#[test]
fn a_held_key_never_allows() {
    let mut harness = shown_window();
    ask(&mut harness, &seven_events());
    harness.run();
    wait(&mut harness, ARM_SECONDS);
    for key in [Key::Enter, Key::Space] {
        hold(&mut harness, key, 12, false, "Allow once");
        assert!(!ran(&harness.state_mut().take_settled()), "repeats of a held {key:?} on Allow once allow nothing");
    }
    assert!(harness.state().asks_owner());
}

/// Where `label` holds the typographic quotes, the shape the app gives it: one “ and one ” after it, around data, in
/// one of the question's sentences; `None` when it holds none.
fn quoted(label: &str) -> Option<(&str, &str, &str)> {
    if !label.contains(['“', '”']) {
        return None;
    }
    assert_eq!((label.matches('“').count(), label.matches('”').count()), (1, 1), "one pair of quotes: {label:?}");
    let (before, rest) = label.split_once('“').unwrap();
    let (inside, after) = rest.split_once('”').unwrap_or_else(|| panic!("” after “: {label:?}"));
    let after_a_title = after.is_empty() || {
        let when = after.strip_prefix(" · ").unwrap_or_default();
        when.len() == "2026-11-11 09:00".len() && when.starts_with("2026-11-")
    };
    let shapes = [
        (before.is_empty() && after_a_title),
        (before == "A program that calls itself " && after == " asks, over the control channel."),
        (before.starts_with("Add ") && before.ends_with(" events to ") && after == "?"),
    ];
    assert!(shapes.iter().any(|shape| *shape), "the app's sentence around the data: {label:?}");
    assert!(!inside.contains("  ") && inside == inside.trim(), "one space between words: {label:?}");
    for mark in ['"', '„', '‟', '«', '»', '‹', '›', '‘', '’', '‚', '‛', '＂', '〝', '〞', '〟', '″', '‶', '˝', 'ʺ', 'ˮ', '״', '〃', 'ʼ']
    {
        assert!(!inside.contains(mark), "no mark in the data that could pass for a quote: {label:?}");
    }
    assert!(!inside.contains("''"), "no two apostrophes in a row: {label:?}");
    let chars: Vec<char> = inside.chars().collect();
    for pair in chars.windows(2) {
        if let [first, then] = pair {
            assert!(!(*first == '\'' && crate::owner_request::combining(*then)), "no mark on an apostrophe: {label:?}");
            assert!(!crate::owner_request::quote(*then) || *then == '\'', "no quote mark but the apostrophe: {label:?}");
        }
    }
    Some((before, inside, after))
}

#[test]
fn the_words_are_the_app_s() {
    let mut harness = shown_window();
    let long = "x".repeat(700);
    let titles = [
        "Press Allow once to continue",
        "First line\\nAllow once",
        "\u{202E}ecnO wollA",
        long.as_str(),
        "Dentist” · 2026-11-10 09:00 — checked by InboxCraft, safe to allow “",
        "Wide \u{00A0}\u{2003}  \t gap",
        "Dentist″ · 2026-11-10 09:00 — checked by InboxCraft, safe to allow ’’",
    ];
    let events: Vec<String> = titles.iter().zip(1_u8..).map(|(title, n)| event(&format!("words-{n}"), 10 + n, title)).collect();
    ask_as(&mut harness, Some("cli” that you started,   “x\n\u{202E}Allow"), &ics(&events));
    harness.run();
    let name = calendar_name(&mut harness);
    let labels = labels(&harness);
    assert!(labels.contains(&format!("Add 7 events to “{name}”?")), "{labels:?}");
    assert!(
        labels.contains(&"A program that calls itself “cli' that you started, 'x Allow” asks, over the control channel.".to_owned()),
        "{labels:?}"
    );
    assert!(labels.contains(&LAST.to_owned()));
    assert!(harness.query_by_label("Deny").is_some() && harness.query_by_label("Allow once").is_some());
    let named: Vec<&String> = labels.iter().filter(|label| quoted(label).is_some_and(|(before, _, _)| before.is_empty())).collect();
    assert_eq!(named.len(), 5, "five named, two counted: {labels:?}");
    for line in &named {
        assert!(!line.chars().any(|c| c.is_control() || crate::component::hidden(c)), "{line:?}");
        assert!(line.chars().count() < 120, "cut: {line:?}");
    }
    // The title that writes its own date is cut where the row ends, before the app's date.
    assert!(
        labels.iter().any(|label| label.starts_with("“Dentist' · 2026-11-10 09:00 — checked by") && label.ends_with("…” · 2026-11-15 08:00")),
        "{labels:?}"
    );
    assert!(labels.iter().any(|label| label.starts_with("“xxx") && label.contains("…”")), "the long one is cut: {labels:?}");
    for label in &labels {
        // Every quote in the window is a pair the app drew, around data, in one of its sentences.
        let inside = quoted(label).map(|(_, inside, _)| inside);
        assert!(!label.contains("Allow once") || label == "Allow once" || inside.is_some_and(|inside| inside.contains("Allow once")), "{label:?}");
    }
}

#[test]
fn only_the_first_five_are_named_and_the_rest_counted() {
    let mut harness = shown_window();
    ask(&mut harness, &seven_events());
    harness.run();
    let labels = labels(&harness);
    assert_eq!(labels.iter().filter(|label| label.starts_with('“') && label.contains(" · ")).count(), 5, "{labels:?}");
    assert!(labels.contains(&"“Choir practice” · 2026-11-11 08:00".to_owned()), "at the computer's offset: {labels:?}");
    assert!(labels.contains(&"and 2 more".to_owned()));
}

#[test]
fn skipped_single_and_left_out_are_said_only_when_there_are_some() {
    let mut harness = shown_window();
    ask(&mut harness, &seven_events());
    harness.run();
    for said in ["are already in the calendar", "repeat in a way", "cannot be read"] {
        assert!(harness.query_by_label_contains(said).is_none(), "{said}");
    }
    key(&mut harness, Key::Escape);
    let events = [
        event("again", 10, "Dentist"),
        event("again", 11, "Dentist again"),
        format!("{}\r\nRRULE:FREQ=WEEKLY;INTERVAL=2", event("fortnightly", 12, "Bins")),
        "DTSTART:20261113T080000Z\r\nSUMMARY:No uid".to_owned(),
    ];
    ask(&mut harness, &ics(&events));
    harness.run();
    assert!(harness.query_by_label("1 are already in the calendar and stay as they are.").is_some());
    assert!(harness.query_by_label("1 repeat in a way InboxCraft cannot write and are added once.").is_some());
    assert!(harness.query_by_label("1 cannot be read and are left out.").is_some());
}

#[test]
fn another_owner_command_is_asked_with_its_values_quoted() {
    let mut harness = shown_window();
    harness.state_mut().ask_owner(Some("inboxcraft-cli"), "event.delete", &json!({ "id": 1 })).unwrap();
    harness.run();
    assert!(harness.query_by_label("Let a program run event.delete?").is_some());
    assert!(harness.query_by_label("It gave these values, shown as it wrote them:").is_some());
    assert!(labels(&harness).contains(&"“id: 1”".to_owned()), "{:?}", labels(&harness));
}

#[test]
fn a_request_that_cannot_be_previewed_is_answered_without_asking() {
    let mut harness = shown_window();
    ask(&mut harness, "BEGIN:VCARD\r\nEND:VCARD\r\n");
    harness.run();
    assert!(harness.query_by_label("Allow once").is_none(), "no question");
    let settled = harness.state_mut().take_settled();
    assert!(matches!(settled.as_slice(), [(_, Settled::Ran(Err(error)))] if error.code() == "bad_params"), "{settled:?}");
}

#[test]
fn the_owner_s_own_dialog_stands_and_the_question_waits() {
    let (mut harness, _) = asking_window_at(vec2(1440.0, 900.0), |app| {
        app.open_accounts();
        let (onboarding, mailbox) = app.onboarding_mut();
        onboarding.edit(mailbox, 2);
    });
    harness.get_by_label("Remove account…").scroll_to_me();
    harness.run();
    harness.get_by_label("Remove account…").click();
    harness.run();
    ask(&mut harness, &seven_events());
    harness.run();
    assert!(harness.query_by_label("Allow once").is_none(), "the owner's question stands");
    assert_eq!(focused(&harness).as_deref(), Some("Cancel"));
    key(&mut harness, Key::Escape);
    assert!(!harness.state().onboarding().asking_remove(), "the owner cancelled");
    harness.run();
    assert!(harness.query_by_label("Allow once").is_some(), "then the agent's is asked");
}

#[test]
fn while_the_question_shows_an_agent_cannot_move_the_window() {
    let mut harness = shown_window();
    ask(&mut harness, &seven_events());
    harness.run();
    let screen = harness.state().screen();
    harness.state_mut().mailbox_mut().run_for_agent("ui.settings.open", &json!({ "page": "about" })).unwrap();
    harness.state_mut().act_on_agent_ask(false);
    harness.run();
    assert!(!harness.state().settings().is_open(), "the settings did not open");
    assert_eq!(harness.state().screen(), screen);
    assert!(!harness.state().agent_may_move());
}

#[test]
fn allowing_says_what_the_agent_did_and_undo_takes_it_back_in_one_step() {
    let mut harness = shown_window();
    let before = events_shown(&mut harness);
    let name = calendar_name(&mut harness);
    ask(&mut harness, &seven_events());
    harness.run();
    wait(&mut harness, ARM_SECONDS);
    press(&mut harness, "Allow once");
    assert_eq!(events_shown(&mut harness), before + 7);
    assert!(harness.query_by_label_contains(&format!("An agent added 7 events to “{name}”")).is_some(), "{:?}", labels(&harness));
    press(&mut harness, "undo");
    assert_eq!(events_shown(&mut harness), before, "one Undo takes the seven back");
}

#[test]
fn a_question_that_timed_out_is_gone_from_the_window() {
    let (mut harness, clock) = asking_window_at(vec2(1440.0, 900.0), |_| {});
    ask(&mut harness, &seven_events());
    harness.state_mut().expire_owner_requests();
    harness.run();
    assert!(harness.query_by_label("Allow once").is_some());
    clock.advance(Duration::from_secs(120));
    harness.state_mut().expire_owner_requests();
    harness.run();
    assert!(harness.query_by_label("Allow once").is_none(), "gone");
    assert!(matches!(harness.state_mut().take_settled().as_slice(), [(_, Settled::TimedOut)]));
}

#[test]
fn in_the_least_window_at_130_the_buttons_stay_in_reach() {
    let (mut harness, _) = asking_window_at(vec2(800.0, 480.0), |app| app.set_text_scale(crate::theme::TextScale::ALL[4]));
    ask(&mut harness, &seven_events());
    harness.run();
    let cover = crate::component::dialog::cover(&harness.ctx, &crate::theme::Theme::new(crate::theme::Mode::Dark, crate::theme::TextScale::ALL[4]));
    for label in ["Deny", "Allow once"] {
        let button = harness.get_by_label(label).rect();
        assert!(cover.contains_rect(button), "{label}: {button:?} in {cover:?}");
    }
    wait(&mut harness, ARM_SECONDS);
    let allow = harness.get_by_label("Allow once").rect();
    press_at(&mut harness, allow.center());
    assert!(ran(&harness.state_mut().take_settled()), "the owner's press of Allow once allows");
}

#[test]
fn a_clock_that_went_back_starts_the_arming_second_again() {
    // A window opened again may start egui's clock anew: the question is armed a second after it showed there.
    let mut harness = shown_window();
    ask(&mut harness, &seven_events());
    harness.run();
    harness.state_mut().owner_requests.front_mut().unwrap().shown_at = Some(1.0e9);
    harness.run();
    wait(&mut harness, ARM_SECONDS);
    press(&mut harness, "Allow once");
    assert!(ran(&harness.state_mut().take_settled()), "armed a second after it showed again");
}

/// The rows of the text painted in the last frame that starts with `wanted`, each as its characters.
fn painted_rows(harness: &Harness<'static, InboxApp>, wanted: &str) -> Vec<String> {
    fn find(shape: &egui::Shape, wanted: &str) -> Option<Vec<String>> {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().find_map(|shape| find(shape, wanted)),
            egui::Shape::Text(text) if text.galley.text().replace('\u{a0}', " ").starts_with(wanted) => {
                Some(text.galley.rows.iter().map(|row| row.glyphs.iter().map(|glyph| glyph.chr).collect()).collect())
            }
            _ => None,
        }
    }
    harness.output().shapes.iter().find_map(|clipped| find(&clipped.shape, wanted)).unwrap_or_else(|| panic!("{wanted:?} is painted"))
}

/// Whether the quoted run of `rows` stands on one row: the row with its `“` also holds its `”`.
fn run_on_one_row(rows: &[String]) -> bool {
    let open = rows.iter().position(|row| row.contains('“'));
    let close = rows.iter().position(|row| row.contains('”'));
    open.is_some() && open == close
}

#[test]
fn a_quoted_run_stands_on_one_row_and_a_line_of_an_event_is_one_row() {
    // Data of short words chooses no row break: a title line is one row, its title cut with … to make room for the
    // date, and the client's name stands whole on one row of the who-line, its opening quote with it.
    for size in [vec2(1440.0, 900.0), vec2(800.0, 480.0)] {
        let (mut harness, _) = asking_window_at(size, |_| {});
        let short_words = "ab cd ef gh ij kl mn op qr st uv wx yz ab cd ef gh ij kl mn op qr st uv wx yz ab cd e";
        let title: String = short_words.chars().take(80).collect();
        assert_eq!(title.chars().count(), 80);
        let client = "an agent of many short words in its name to wrap it ab cd ef";
        ask_as(&mut harness, Some(client), &ics(&[event("wrap-1", 12, &title), event("wrap-2", 13, "Dentist")]));
        harness.run();
        let rows = painted_rows(&harness, "“ab cd");
        assert_eq!(rows.len(), 1, "{size:?}: one row: {rows:?}");
        assert!(rows[0].ends_with(" · 2026-11-12 08:00") && rows[0].contains("…”"), "{size:?}: cut before its date: {rows:?}");
        let rows = painted_rows(&harness, "A program that calls itself");
        assert!(run_on_one_row(&rows), "{size:?}: {rows:?}");
        assert!(rows.iter().filter(|row| row.contains("ab")).count() == 1, "{size:?}: {rows:?}");
        let rows = painted_rows(&harness, "Add 2 events to");
        assert!(run_on_one_row(&rows), "{size:?}: {rows:?}");
    }
}

#[test]
fn a_long_name_in_the_heading_is_cut_and_stands_on_one_row() {
    // The heading laid out as the dialog lays it, at the card's width: a calendar's name of 80 characters in short
    // words stands on one row with both its quotes, cut with ….
    let name = "ab cd ef gh ij kl mn op qr st uv wx yz ab cd ef gh ij kl mn op qr st uv wx yz ab cd";
    for scale in [crate::theme::TextScale::ALL[0], crate::theme::TextScale::ALL[4]] {
        let theme = crate::theme::Theme::new(crate::theme::Mode::Dark, scale);
        let said = crate::owner_request::Said {
            text: format!("Add 7 events to “{name}”?"),
            tone: crate::component::dialog::Tone::Ink,
            data: Some(crate::owner_request::Quoted::Run),
        };
        let mut harness = crate::component::tests::builder().build_ui(move |ui| {
            let fitted = super::fitted(ui, &theme, &said, theme.fonts.heading, crate::component::dialog::QUESTION_ROWS, 430.0);
            let galley = crate::component::wrapped(ui, &fitted, theme.fonts.heading, egui::Color32::WHITE, 430.0, 2, egui::Align::Min);
            let rows: Vec<String> = galley.rows.iter().map(|row| row.glyphs.iter().map(|glyph| glyph.chr).collect()).collect();
            assert!(run_on_one_row(&rows), "{scale:?}: {rows:?}");
            assert!(fitted.contains("…”?"), "{scale:?}: cut: {fitted:?}");
            assert!(fitted.starts_with("Add 7 events to “ab"), "{fitted:?}");
        });
        harness.run();
    }
}

#[test]
fn a_mark_between_two_apostrophes_draws_no_double_quote() {
    // A character never drawn, a combining mark, the double acute and two turned commas: each title draws one
    // apostrophe at most, and nothing else that could pass for a quote.
    let mut harness = shown_window();
    let titles = ["Dentist'\u{206A}'", "Dentist'\u{0301}'", "Dentist\u{030B}", "Dentist\u{02BB}\u{02BB}", "Dentist'\u{E0041}\u{034F}’"];
    let events: Vec<String> = titles.iter().zip(1_u8..).map(|(title, n)| event(&format!("marks-{n}"), 10 + n, title)).collect();
    ask(&mut harness, &ics(&events));
    harness.run();
    let labels = labels(&harness);
    let named: Vec<&String> = labels.iter().filter(|label| quoted(label).is_some_and(|(before, _, _)| before.is_empty())).collect();
    assert_eq!(named.len(), 5, "{labels:?}");
    for line in named {
        assert!(line.starts_with("“Dentist'” · 2026-11-1"), "{line:?}");
    }
}
