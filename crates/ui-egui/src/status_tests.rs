// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The status line's state: the eleven `status-line/*` frames as the facts and the message they show, the queue, the
//! worst state first, and hostile text made one clean line.

use super::{MOST, Message, QUEUE, Status, clean, kept_under};
use crate::component::status_item::Kind;
use crate::draw::status::{Facts, Part, Sync, parts};

/// The facts of the frames at rest: synced at 14:02, two accounts, one message in the outbox, the design review at
/// 15:00, 58 minutes away.
fn at_rest() -> Facts {
    Facts {
        sync: Sync::Synced(Some("14:02".to_owned())),
        accounts: 2,
        offline_since: None,
        failures: Vec::new(),
        problems: 0,
        outbox: 1,
        outbox_goes: None,
        next: Some(("15:00".to_owned(), "Design review".to_owned(), 58)),
    }
}

fn item(text: &str, kind: Kind) -> Part {
    Part::Item(text.to_owned(), kind)
}

#[test]
fn synced() {
    assert_eq!(
        parts(&at_rest(), false),
        [
            item("synced 14:02", Kind::Ok),
            item("2 accounts", Kind::Plain),
            item("outbox 1", Kind::Plain),
            item("next: Design review 15:00", Kind::Plain)
        ]
    );
}

#[test]
fn syncing() {
    let facts = Facts { sync: Sync::Syncing(0.54, "54 %".to_owned()), outbox: 0, ..at_rest() };
    assert_eq!(
        parts(&facts, false),
        [Part::Progress(0.54, "54 %".to_owned()), item("2 accounts", Kind::Plain), item("next: Design review 15:00", Kind::Plain)]
    );
}

#[test]
fn offline() {
    // `status-line/offline`: since when, what the window shows meanwhile, and the way to try again (coming soon: no
    // command signs the accounts in before their wait ends).
    let facts = Facts { sync: Sync::Offline(Some("14:10".to_owned())), ..at_rest() };
    assert_eq!(
        parts(&facts, false),
        [item("offline since 14:10", Kind::Offline), item("showing what is kept on this computer", Kind::Plain), Part::Link("try again", true)]
    );
    assert_eq!(parts(&facts, true), [item("offline since 14:10", Kind::Offline)], "beside a message the one fact");
}

#[test]
fn account_refused() {
    let facts = Facts { failures: vec!["Studio: sign-in refused".to_owned()], problems: 1, outbox: 0, ..at_rest() };
    assert_eq!(
        parts(&facts, false),
        [
            item("Studio: sign-in refused", Kind::Error),
            Part::Link("open account", false),
            item("synced 14:02", Kind::Ok),
            item("2 accounts", Kind::Plain),
            item("next: Design review 15:00", Kind::Plain)
        ]
    );
}

#[test]
fn everything_failing() {
    let failures = ["Studio: sign-in refused", "Personal: no route", "Club: timed out", "Work: certificate refused"].map(str::to_owned).to_vec();
    let facts = Facts { failures, problems: 4, ..at_rest() };
    assert_eq!(
        parts(&facts, false),
        [item("Studio: sign-in refused", Kind::Error), item("and 3 more problems", Kind::Plain), Part::Link("open log", true)]
    );
}

#[test]
fn next_event() {
    let facts = Facts { next: Some(("15:00".to_owned(), "Design review".to_owned(), 12)), ..at_rest() };
    assert_eq!(parts(&facts, false).last(), Some(&item("next: Design review 15:00 · in 12 min", Kind::Accent)));
}

#[test]
fn outbox_waiting() {
    let facts = Facts { outbox: 2, outbox_goes: Some("16:00".to_owned()), ..at_rest() };
    assert_eq!(parts(&facts, false).get(2), Some(&item("outbox 2 · one goes at 16:00", Kind::Waiting)));
}

/// The four frames with a message: the message, and after it the state's one fact.
fn with_message(message: Option<Message>, queued: Option<Message>) -> (Option<Message>, Vec<Part>) {
    let mut status = Status::default();
    if let Some(queued) = queued {
        status.push(queued);
    }
    (status.shown(10.0, None, message), parts(&at_rest(), true))
}

#[test]
fn undo() {
    let undo = Message::Undo { text: "Marked as done".to_owned(), until: 15.0 };
    assert_eq!(with_message(Some(undo.clone()), None), (Some(undo), vec![item("synced 14:02", Kind::Ok)]));
}

#[test]
fn send_countdown() {
    let mut status = Status::default();
    let countdown = Message::Countdown { text: "sending · Re: Thursday workshop".to_owned(), until: 14.0 };
    let undo = Message::Undo { text: "Marked as done".to_owned(), until: 15.0 };
    assert_eq!(status.shown(10.0, Some(countdown.clone()), Some(undo)), Some(countdown), "the countdown before an act's undo");
}

#[test]
fn notice() {
    let notice = Message::Notice { text: "Link copied: https://example.org/open-day".to_owned(), until: 4.0 };
    let shown = Message::Notice { text: "Link copied: https://example.org/open-day".to_owned(), until: 14.0 };
    assert_eq!(with_message(None, Some(notice)), (Some(shown), vec![item("synced 14:02", Kind::Ok)]));
}

#[test]
fn error() {
    let error = Message::Error { text: "Not moved: the message is still in Inbox.".to_owned() };
    assert_eq!(with_message(None, Some(error.clone())), (Some(error), vec![item("synced 14:02", Kind::Ok)]));
}

#[test]
fn the_worst_state_and_a_count() {
    // Offline, one account refused and one more failing: the failure wins the first fact, with the count of the rest;
    // offline is not said over it.
    let facts = Facts {
        sync: Sync::Offline(None),
        failures: vec!["Studio: sign-in refused".to_owned(), "Personal: no route".to_owned()],
        problems: 2,
        ..at_rest()
    };
    let shown = parts(&facts, false);
    assert_eq!(shown.first(), Some(&item("Studio: sign-in refused", Kind::Error)));
    assert_eq!(shown.get(1), Some(&item("and 1 more problem", Kind::Plain)), "one more problem, not problems");
    assert!(!shown.iter().any(|part| matches!(part, Part::Item(_, Kind::Offline))), "{shown:?}");
    // With a message shown, the failure is still the one fact.
    assert_eq!(parts(&facts, true), [item("Studio: sign-in refused", Kind::Error)]);
}

#[test]
fn an_error_stays_while_an_undo_comes_and_goes() {
    let mut status = Status::default();
    let error = Message::Error { text: "Not sent: the server refused it".to_owned() };
    status.push(error.clone());
    let undo = Message::Undo { text: "Marked as done".to_owned(), until: 5.0 };
    assert_eq!(status.shown(0.0, None, Some(undo.clone())), Some(undo), "the undo shows for its time");
    assert_eq!(status.shown(6.0, None, None), Some(error.clone()), "the error is back once it went");
    assert_eq!(status.shown(10_000.0, None, None), Some(error), "an error has no time");
    assert_eq!(status.dismiss().as_deref(), Some("Not sent: the server refused it"));
    assert_eq!(status.shown(10_001.0, None, None), None);
}

#[test]
fn a_notice_waits_its_turn_and_its_time_starts_when_it_shows() {
    let mut status = Status::default();
    status.push(Message::Notice { text: "Draft saved".to_owned(), until: 4.0 });
    status.push(Message::Notice { text: "Link copied".to_owned(), until: 4.0 });
    let shown = |status: &mut Status, now: f64| status.shown(now, None, None).map(|message| message.text().to_owned());
    assert_eq!(shown(&mut status, 100.0).as_deref(), Some("Draft saved"));
    assert_eq!(shown(&mut status, 103.9).as_deref(), Some("Draft saved"));
    assert_eq!(shown(&mut status, 104.0).as_deref(), Some("Link copied"));
    assert_eq!(shown(&mut status, 108.0), None);
}

#[test]
fn the_queue_holds_at_most_eight_and_a_notice_makes_room_before_an_error() {
    let mut status = Status::default();
    status.push(Message::Notice { text: "first".to_owned(), until: 4.0 });
    for n in 0..20 {
        status.push(Message::Error { text: format!("error {n}") });
    }
    assert_eq!(status.waiting(), QUEUE);
    assert_eq!(status.notice(), None, "the notice made room first");
    assert_eq!(status.shown(0.0, None, None).map(|message| message.text().to_owned()).as_deref(), Some("error 12"), "then the oldest errors");
}

#[test]
fn a_hostile_failure_text_is_one_clean_line() {
    let hostile = format!("refused\r\nby\0 the \u{202E}server\u{2066} \u{200F}{}", "x".repeat(10_000));
    let cleaned = clean(&hostile);
    assert!(cleaned.starts_with("refused by the server x"), "{}", cleaned.get(..40).unwrap_or_default());
    assert_eq!(cleaned.chars().count(), MOST);
    assert!(cleaned.ends_with('…'));
    assert!(!cleaned.chars().any(|c| c.is_control() || matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}')));
    // What the queue keeps is the clean line.
    let mut status = Status::default();
    status.push(Message::Error { text: hostile });
    assert_eq!(status.shown(0.0, None, None), Some(Message::Error { text: cleaned }));
    // A short text is kept whole, a combining run with it; nothing is cut inside a character.
    assert_eq!(clean("  café \u{301}\u{301} \n"), "café \u{301}\u{301}");
    assert_eq!(clean(&"é".repeat(400)).chars().count(), MOST);
}

#[test]
fn under_1280_three_facts_and_three_keys_under_980_two_and_two() {
    assert_eq!(
        [kept_under(1440.0), kept_under(1280.0), kept_under(1279.0), kept_under(980.0), kept_under(979.0)],
        [None, None, Some((3, 3)), Some((3, 3)), Some((2, 2))]
    );
}
