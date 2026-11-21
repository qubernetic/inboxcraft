// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The script's failing accounts and notice: what each value tells the window, and what a wrong one gets.

use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use super::{NoSave, SHOT_TIME, Shot, fail, notice, parse_script, shoot, sync};
use crate::mailbox::Mailbox;
use crate::{InboxApp, sample};

fn shot(json: &str) -> Shot {
    parse_script(json).unwrap().remove(0)
}

fn app() -> InboxApp {
    let clock = FakeClock::new(Timestamp::from_unix(SHOT_TIME, 0).unwrap());
    let session = sample::session(Box::new(clock.clone())).unwrap();
    InboxApp::with_outside(Mailbox::new(session, Box::new(clock)), Box::new(sample::Pictures::default()), Box::new(NoSave))
}

/// Each account's label and the kind of its failure (`None` while it does not fail).
fn failures(app: &mut InboxApp) -> Vec<(String, Option<String>)> {
    let answer = app.mailbox_mut().run("account.list", &json!({})).unwrap();
    let accounts = answer.get("accounts").and_then(Value::as_array).cloned().unwrap_or_default();
    accounts
        .iter()
        .map(|account| {
            let label = account.get("label").and_then(Value::as_str).unwrap_or_default().to_owned();
            let kind = account.get("failure").and_then(|failure| failure.get("kind")).and_then(Value::as_str).map(str::to_owned);
            (label, kind)
        })
        .collect()
}

fn failed(json: &str) -> Vec<(String, Option<String>)> {
    let shot = shot(json);
    let mut app = app();
    sync(&mut app, &shot).unwrap();
    fail(&mut app, &shot).unwrap();
    failures(&mut app)
}

#[test]
fn refused_fails_the_last_account_s_sign_in_and_leaves_the_others_synced() {
    let refused = failed(r#"{"out": "x.png", "mode": "dark", "sync": "synced", "fail": "refused"}"#);
    assert_eq!(refused, [("Personal".to_owned(), None), ("Studio".to_owned(), Some("login_refused".to_owned()))]);
}

#[test]
fn everything_fails_every_account() {
    let everything = failed(r#"{"out": "x.png", "mode": "dark", "sync": "synced", "fail": "everything"}"#);
    assert_eq!(everything, [("Personal".to_owned(), Some("connection_refused".to_owned())), ("Studio".to_owned(), Some("login_refused".to_owned()))]);
}

#[test]
fn without_fail_no_account_fails() {
    let plain = failed(r#"{"out": "x.png", "mode": "dark", "sync": "synced"}"#);
    assert!(plain.iter().all(|(_, kind)| kind.is_none()), "{plain:?}");
}

#[test]
fn the_failing_status_line_is_its_own_picture() {
    let synced = shoot(&shot(r#"{"out": "x.png", "mode": "dark", "size": [900, 600], "sync": "synced"}"#)).unwrap();
    let refused = shoot(&shot(r#"{"out": "x.png", "mode": "dark", "size": [900, 600], "sync": "synced", "fail": "refused"}"#)).unwrap();
    assert_ne!(synced, refused);
}

#[test]
fn a_fail_the_script_does_not_know_is_refused_by_name() {
    let error = shoot(&shot(r#"{"out": "x.png", "mode": "dark", "fail": "sometimes"}"#)).unwrap_err().to_string();
    assert!(error.contains("x.png") && error.contains("sometimes"), "{error}");
    let page = shoot(&shot(r#"{"out": "x.png", "mode": "dark", "icons": true, "fail": "refused"}"#)).unwrap_err().to_string();
    assert!(page.contains("x.png"), "a page with no window takes no failing account: {page}");
}

#[test]
fn a_notice_is_said_in_the_status_line() {
    let shot = shot(r#"{"out": "x.png", "mode": "dark", "notice": "Link copied: https://example.org/open-day"}"#);
    let mut app = app();
    assert_eq!(app.notice(), None);
    notice(&mut app, &shot);
    assert_eq!(app.notice(), Some("Link copied: https://example.org/open-day"));
    let plain = shoot(&self::shot(r#"{"out": "x.png", "mode": "dark", "size": [900, 600]}"#)).unwrap();
    let said = shoot(&self::shot(r#"{"out": "x.png", "mode": "dark", "size": [900, 600], "notice": "Link copied"}"#)).unwrap();
    assert_ne!(plain, said, "the notice shows in the picture");
}

#[test]
fn a_notice_with_line_breaks_is_said_on_one_line() {
    let shot = shot(r#"{"out": "x.png", "mode": "dark", "notice": "Link\r\ncopied\u0000"}"#);
    let mut app = app();
    notice(&mut app, &shot);
    let said = app.notice().unwrap_or_default().to_owned();
    assert!(!said.contains(['\r', '\n', '\0']), "{said:?}");
}

#[test]
fn the_status_line_says_a_failing_account_s_reason_in_the_frames_few_words() {
    // `status-line/account-refused`: "Studio: sign-in refused", not the engine's sentence for logs and agents.
    let shot = shot(r#"{"out": "x.png", "mode": "dark", "sync": "synced", "fail": "everything"}"#);
    let mut app = app();
    sync(&mut app, &shot).unwrap();
    fail(&mut app, &shot).unwrap();
    let facts = crate::draw::status::read(app.mailbox_mut(), None);
    // `status-line/everything-failing`: the worst first, what needs the owner before what mends itself.
    assert_eq!(facts.failures, ["Studio: sign-in refused", "Personal: no connection"]);
    assert_eq!(facts.problems, 2);
}
