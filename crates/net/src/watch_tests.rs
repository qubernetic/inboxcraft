// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::num::NonZeroU16;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use inboxcraft_model::{AccountId, Host, Job, JobId, JobKind, Security, Server, Uid, UidValidity};
use inboxcraft_proto::imap::{Event, FetchItem};
use inboxcraft_testkit::imap::{Action, FakeImap, When};
use inboxcraft_testkit::{FakeServer, HOST, Listen, TestPki};
use inboxcraft_time::SystemClock;

use super::{WatchConfig, idle, is_change, poll};
use crate::{Connector, ImapConnection, Timeouts, TlsSettings, Trust};

fn quick() -> WatchConfig {
    WatchConfig { poll: Duration::from_millis(10), ..WatchConfig::default() }
}

#[test]
fn the_defaults_renew_idle_before_servers_end_it() {
    let config = WatchConfig::default();
    assert_eq!(config.tick, Duration::from_millis(250));
    assert!(config.renew < Duration::from_secs(29 * 60));
    assert_eq!(config.poll, Duration::from_secs(300));
}

#[test]
fn polling_ends_with_a_job_a_resync_or_the_end() {
    let (jobs, queue) = mpsc::channel();
    let job = Job::new(JobId::new(1), AccountId::new(1), JobKind::SyncFolderList);
    jobs.send(job.clone()).unwrap();
    let wake = poll(&quick(), &queue);
    assert_eq!(wake.job, Some(job));
    assert!(!wake.changed && !wake.closed);
    let wake = poll(&quick(), &queue);
    assert!(wake.changed && wake.job.is_none() && !wake.closed);
    drop(jobs);
    let wake = poll(&quick(), &queue);
    assert!(wake.closed && !wake.changed);
}

#[test]
fn what_tells_of_a_change() {
    let uid = Uid::new(1).unwrap();
    assert!(is_change(&Event::Exists(4)));
    assert!(is_change(&Event::Expunged(1)));
    assert!(is_change(&Event::Vanished { uids: std::iter::once((uid, uid)).collect(), earlier: false }));
    assert!(is_change(&Event::Fetched(FetchItem::default())));
    assert!(is_change(&Event::UidValidityChanged { old: UidValidity::new(1).unwrap(), new: None }));
    assert!(!is_change(&Event::Idling));
    assert!(!is_change(&Event::Alert("hello".to_owned())));
}

/// A connection with INBOX open on the fake server `fake`, signed in by PREAUTH.
fn inbox_open(pki: &TestPki, fake: FakeImap) -> (FakeServer, ImapConnection) {
    let server = Listen::tls(pki.server_config(None).unwrap()).start_imap(fake.preauth()).unwrap();
    let tls = TlsSettings::new(Trust::Roots(vec![pki.ca()]), Arc::new(SystemClock::new()));
    let timeouts = Timeouts { connect: Duration::from_secs(5), io: Duration::from_secs(10) };
    let connector = Connector::new(tls).with_timeouts(timeouts).with_addresses(vec![server.addr()]);
    let account_server = Server::new(Host::new(HOST).unwrap(), NonZeroU16::new(993).unwrap(), Security::Tls);
    let mut connection = ImapConnection::open(&account_server, &connector).unwrap();
    connection.run(|client| client.examine("INBOX"), &mut |_| {}).unwrap().unwrap();
    (server, connection)
}

/// The tag the client gives its IDLE: the EXAMINE before it is the first command.
const IDLE_TAG: &str = "A0002";

#[test]
fn an_idle_the_server_ends_itself_asks_for_a_resync() {
    let pki = TestPki::new().unwrap();
    let ended = Action::Send(format!("{IDLE_TAG} OK IDLE ended by the server"));
    let (server, mut connection) = inbox_open(&pki, FakeImap::sample().on(When::Idle, [ended]));
    let (_jobs, queue) = mpsc::channel::<Job>();
    let wake = idle(&mut connection, &WatchConfig::default(), &queue).unwrap();
    assert!(wake.changed && !wake.refused && !wake.closed && wake.job.is_none(), "{wake:?}");
    drop(connection);
    let (transcript, _ended) = server.finish_with_transcript();
    assert_eq!(transcript.received().last().map(String::as_str), Some(format!("{IDLE_TAG} IDLE").as_str()), "no DONE after the server's end");
}

#[test]
fn an_idle_the_server_refuses_turns_to_polling() {
    let pki = TestPki::new().unwrap();
    let refused = Action::Send(format!("{IDLE_TAG} NO IDLE is not allowed here"));
    let (server, mut connection) = inbox_open(&pki, FakeImap::sample().on(When::command("IDLE"), [refused]));
    let (_jobs, queue) = mpsc::channel::<Job>();
    let wake = idle(&mut connection, &WatchConfig::default(), &queue).unwrap();
    assert!(wake.refused && !wake.changed && !wake.closed && wake.job.is_none(), "{wake:?}");
    drop(connection);
    let (transcript, _ended) = server.finish_with_transcript();
    assert_eq!(transcript.received().last().map(String::as_str), Some(format!("{IDLE_TAG} IDLE").as_str()));
}
