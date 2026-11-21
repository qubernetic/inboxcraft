// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The status line's facts as [`super::read`] reads them from the mailbox: offline since the window first read it so,
//! and the problems counted, not the accounts: a send the server refused is one too.

use std::time::Duration;

use inboxcraft_engine::Session;
use inboxcraft_model::{AccountId, Host, Security, Server};
use inboxcraft_store::{Batch, Change, Storage as _};
use inboxcraft_time::{FakeClock, Timestamp};

use super::{Sync, read};
use crate::compose::{Composer, Line};
use crate::mailbox::Mailbox;
use crate::sample;
use crate::workers::Scripted;

/// 2026-11-05T09:00:00Z, a Thursday.
const NOW: i64 = 1_793_869_200;

fn clock() -> FakeClock {
    FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap())
}

/// The sample mailbox on `clock`, its first account able to send. Its accounts sign in nowhere: it reads offline.
fn mailbox(clock: &FakeClock) -> Mailbox {
    let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
    let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    let server = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
    let mut batch = Batch::new();
    batch.push(Change::PutAccount(account.with_submission(Some(server))));
    store.apply(&batch).unwrap();
    Mailbox::new(Session::new(Box::new(store), Box::new(clock.clone())).unwrap(), Box::new(clock.clone()))
}

#[test]
fn offline_says_since_when_the_window_first_read_it_so_until_it_is_online() {
    let clock = clock();
    let mut mailbox = mailbox(&clock);
    let first = read(&mut mailbox, None);
    let at = mailbox.short_date(Timestamp::from_unix(NOW, 0).unwrap());
    assert_eq!(first.sync, Sync::Offline(Some(at.clone())));
    clock.advance(Duration::from_secs(25 * 60));
    let later = read(&mut mailbox, first.offline_since);
    assert_eq!(later.sync, Sync::Offline(Some(at)), "still since the first read, not since now");
    assert_eq!(later.offline_since, first.offline_since);
    // Online, the time is let go: the next offline counts from its own first read.
    let accounts: Vec<u64> = mailbox.accounts().iter().map(|account| account.id).collect();
    for account in accounts {
        let online = inboxcraft_model::EventKind::Connection(inboxcraft_model::ConnectionState::Online);
        mailbox.tell(&inboxcraft_model::Event::new(AccountId::new(account), None, online)).unwrap();
    }
    let online = read(&mut mailbox, later.offline_since);
    assert!(!matches!(online.sync, Sync::Offline(_)), "{:?}", online.sync);
    assert_eq!(online.offline_since, None);
}

#[test]
fn a_send_the_server_refused_is_a_problem_counted_with_the_accounts() {
    let clock = clock();
    let mut mailbox = mailbox(&clock);
    assert_eq!(read(&mut mailbox, None).problems, 0);
    let mut composer = Composer::start(&mut mailbox).unwrap();
    "Odile Brannock <odile@example.org>".clone_into(composer.typing_mut(Line::To).unwrap());
    composer.take(&mut mailbox, Line::To).unwrap();
    "Work party on Saturday".clone_into(composer.subject_mut());
    composer.edited();
    composer.save(&mut mailbox).unwrap();
    composer.send(&mut mailbox).unwrap();
    clock.advance(Duration::from_secs(6));
    assert!(mailbox.pump(&mut Scripted::default().refusing("550 5.1.1 No such user here")));
    let facts = read(&mut mailbox, None);
    assert_eq!(facts.problems, 1, "the refused send");
    assert!(facts.failures.is_empty(), "no account fails: {:?}", facts.failures);
}
