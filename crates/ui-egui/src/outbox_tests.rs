// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use inboxcraft_engine::Session;
use inboxcraft_engine::notify::Notice;
use inboxcraft_model::{AccountId, Host, Role, Security, Server};
use inboxcraft_store::{Batch, Change, Storage as _};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use super::{Mailbox, Place, SendState};
use crate::compose::{Composer, Line};
use crate::sample;
use crate::workers::Scripted;

/// 2026-11-05T09:00:00Z, a Thursday.
const NOW: i64 = 1_793_869_200;

fn clock() -> FakeClock {
    FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap())
}

/// A mailbox over the sample mail whose first account can send, on `clock`.
fn sending(clock: &FakeClock) -> Mailbox {
    let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
    let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    let server = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
    let mut batch = Batch::new();
    batch.push(Change::PutAccount(account.with_submission(Some(server))));
    store.apply(&batch).unwrap();
    Mailbox::new(Session::new(Box::new(store), Box::new(clock.clone())).unwrap(), Box::new(clock.clone()))
}

/// A draft to Odile with `subject`, ready to go.
fn written(mailbox: &mut Mailbox, subject: &str) -> Composer {
    let mut composer = Composer::start(mailbox).unwrap();
    "Odile Brannock <odile@example.org>".clone_into(composer.typing_mut(Line::To).unwrap());
    composer.take(mailbox, Line::To).unwrap();
    subject.clone_into(composer.subject_mut());
    composer.edited();
    composer.save(mailbox).unwrap();
    composer
}

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_unix(NOW + seconds, 0).unwrap()
}

#[test]
fn the_outbox_is_empty_until_something_is_sent() {
    let clock = clock();
    let mailbox = sending(&clock);
    assert!(mailbox.outbox().is_empty());
    assert_eq!(mailbox.outbox_summary(), None);
}

#[test]
fn a_send_waits_in_the_outbox_counting_down_its_hold() {
    let clock = clock();
    let mut mailbox = sending(&clock);
    let mut composer = written(&mut mailbox, "Work party on Saturday");
    assert_eq!(composer.send(&mut mailbox).unwrap(), Some(at(5)));
    let row = mailbox.outbox().first().cloned().unwrap();
    assert_eq!(row.draft, composer.draft());
    assert_eq!(row.subject, "Work party on Saturday");
    assert_eq!(row.to, ["Odile Brannock"]);
    assert_eq!(row.state, SendState::Waiting(Some(at(5))));
    assert_eq!(mailbox.standing(&row), "Sending in 5 s");
    clock.advance(Duration::from_millis(2_500));
    assert_eq!(mailbox.standing(&row), "Sending in 3 s", "a second begun counts as one");
    assert_eq!(mailbox.seconds_left(composer.draft()), Some(3));
    assert_eq!(mailbox.outbox_summary().as_deref(), Some("1 waiting"));
    assert_eq!(mailbox.spoken_outbox(&row), "To Odile Brannock, Work party on Saturday, Sending in 3 s");
}

#[test]
fn a_send_later_says_when_it_goes() {
    let clock = clock();
    let mut mailbox = sending(&clock);
    let mut composer = written(&mut mailbox, "Re: Draft logo for the bakery");
    composer.set_later(Some(at(2 * 86_400 - 3_600)));
    assert_eq!(composer.send(&mut mailbox).unwrap(), Some(at(2 * 86_400 - 3_600)));
    let row = mailbox.outbox().first().cloned().unwrap();
    assert_eq!(mailbox.standing(&row), "Scheduled · Sat 08:00");
    assert_eq!(mailbox.seconds_left(row.draft), None, "not counted down: it is no hold");
}

#[test]
fn a_time_that_has_passed_is_not_scheduled() {
    let clock = clock();
    let mut mailbox = sending(&clock);
    let mut composer = written(&mut mailbox, "Late");
    composer.set_later(Some(at(60)));
    clock.advance(Duration::from_secs(120));
    assert!(composer.send(&mut mailbox).is_err(), "the engine refuses a time behind");
    assert!(mailbox.outbox().is_empty());
}

#[test]
fn unsend_takes_a_waiting_send_back_with_its_draft_as_it_was() {
    let clock = clock();
    let mut mailbox = sending(&clock);
    let mut composer = written(&mut mailbox, "Work party on Saturday");
    composer.send(&mut mailbox).unwrap();
    mailbox.unsend(composer.draft()).unwrap();
    assert!(mailbox.outbox().is_empty());
    let draft = mailbox.run("compose.show", &json!({ "id": composer.draft() })).unwrap();
    assert_eq!(draft["subject"], json!("Work party on Saturday"));
    assert!(mailbox.unsend(composer.draft()).is_err(), "nothing waits any more");
}

#[test]
fn a_refused_send_shows_the_servers_reply_and_goes_again_on_retry() {
    let clock = clock();
    let mut mailbox = sending(&clock);
    let mut composer = written(&mut mailbox, "Work party on Saturday");
    composer.send(&mut mailbox).unwrap();
    clock.advance(Duration::from_secs(6));
    let mut workers = Scripted::default().refusing("550 5.1.1 No such user here");
    assert!(mailbox.pump(&mut workers));
    let row = mailbox.outbox().first().cloned().unwrap();
    assert_eq!(row.state, SendState::Refused { reply: "550 5.1.1 No such user here".to_owned(), permanent: true });
    assert_eq!(mailbox.standing(&row), "Not sent");
    assert_eq!(mailbox.outbox_summary().as_deref(), Some("1 not sent"));
    assert_eq!(mailbox.spoken_outbox(&row), "To Odile Brannock, Work party on Saturday, Not sent: 550 5.1.1 No such user here");
    assert_eq!(mailbox.retry(composer.draft()).unwrap(), Some(at(11)), "held again as every send is");
    assert_eq!(mailbox.outbox().first().map(|row| row.state.clone()), Some(SendState::Waiting(Some(at(11)))));
}

#[test]
fn a_send_on_its_way_says_so() {
    let clock = clock();
    let mut mailbox = sending(&clock);
    let mut composer = written(&mut mailbox, "Work party on Saturday");
    composer.send(&mut mailbox).unwrap();
    clock.advance(Duration::from_secs(6));
    mailbox.pump(&mut Scripted::default());
    let row = mailbox.outbox().first().cloned().unwrap();
    assert_eq!(row.state, SendState::Sending);
    assert_eq!(mailbox.standing(&row), "Sending…");
    assert_eq!(mailbox.outbox_summary().as_deref(), Some("1 waiting"));
}

#[test]
fn the_outbox_place_shows_while_anything_waits_and_gives_way_to_the_inbox_after() {
    let clock = clock();
    let mut mailbox = sending(&clock);
    let mut composer = written(&mut mailbox, "Work party on Saturday");
    composer.send(&mut mailbox).unwrap();
    mailbox.open(Place::Outbox);
    assert_eq!(mailbox.place(), Place::Outbox);
    assert_eq!(mailbox.title(), "Outbox");
    assert!(mailbox.messages().is_empty(), "the outbox lists sends, not messages");
    mailbox.unsend(composer.draft()).unwrap();
    assert_eq!(mailbox.place(), Place::View(Role::Inbox));
}

#[test]
fn the_outbox_place_is_not_opened_while_nothing_waits() {
    let clock = clock();
    let mut mailbox = sending(&clock);
    mailbox.open(Place::Outbox);
    assert_eq!(mailbox.place(), Place::View(Role::Inbox));
}

#[test]
fn the_hold_is_five_seconds_until_the_owner_chooses_and_stays_in_range() {
    let clock = clock();
    let mut mailbox = sending(&clock);
    assert_eq!(mailbox.hold_seconds(), 5);
    mailbox.set_hold(30);
    assert_eq!(mailbox.hold_seconds(), 30);
    assert_eq!(mailbox.last_run(), Some("send.set"));
    mailbox.set_hold(90);
    assert_eq!(mailbox.hold_seconds(), 30, "out of range: refused");
    assert!(mailbox.error().is_some());
    mailbox.restore_hold(10);
    assert_eq!(mailbox.hold_seconds(), 10);
    mailbox.restore_hold(200);
    assert_eq!(mailbox.hold_seconds(), 10, "a kept value out of range is left out");
}

#[test]
fn a_send_whose_time_passed_while_inboxcraft_was_off_is_told_once() {
    let clock = clock();
    let mut mailbox = sending(&clock);
    let mut composer = written(&mut mailbox, "Work party on Saturday");
    composer.set_later(Some(at(3_600)));
    composer.send(&mut mailbox).unwrap();
    clock.advance(Duration::from_secs(7_200));
    mailbox.pump(&mut Scripted::default());
    assert_eq!(mailbox.take_late(), ["Work party on Saturday"]);
    assert!(mailbox.take_late().is_empty(), "told once");
    let notices = mailbox.take_notices();
    assert!(matches!(notices.as_slice(), [Notice::SentLate { .. }]), "the desktop still hears of it: {notices:?}");
}

#[test]
fn a_bounced_message_says_why_in_its_row() {
    let clock = clock();
    let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
    sample::bounce(&mut store).unwrap();
    let mut mailbox = Mailbox::new(Session::sample(Box::new(store), Box::new(clock.clone())).unwrap(), Box::new(clock));
    mailbox.open(Place::View(Role::Sent));
    let row = mailbox.messages().iter().find(|row| row.bounced.is_some()).cloned().unwrap();
    assert!(row.bounced.as_deref().unwrap().contains("550 5.1.1"));
    assert!(mailbox.spoken(&row).ends_with("not delivered"));
}
