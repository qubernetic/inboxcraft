// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The steps of the live round trip against the fake server, one connection after the other: the admin client of
//! the live test creates the per-run folder and appends an invented message, a worker finds both, and the admin
//! client removes the folder again. The fake serves one connection at a time, so the IDLE step, which needs the
//! admin client and the worker at once, runs in the live test and in `watch.rs` only.

mod support;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use inboxcraft_model::{EventKind, FolderId, FolderPath, JobKind};
use inboxcraft_net::{TlsSettings, Trust};
use inboxcraft_testkit::imap::{FakeImap, PASSWORD, USER};
use inboxcraft_testkit::live::{Folders, Live, LiveConfig, RunFolder};
use inboxcraft_testkit::{HOST, Listen, TestPki};
use support::admin::Admin;
use support::run::{Options, Run, added};
use support::{Result, wall};

const RUN: FolderId = FolderId::new(50);

/// A live configuration of invented values, as the owner's run would have.
fn invented() -> Result<LiveConfig> {
    let vars = HashMap::from([
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "first@example.org"),
        ("INBOXCRAFT_LIVE_IMAP_HOST", "imap.example.org"),
        ("INBOXCRAFT_LIVE_IMAP_USER", "first-login"),
        ("INBOXCRAFT_LIVE_IMAP_SECRET", "imap-secret-invented"),
        ("INBOXCRAFT_LIVE_IMAP2_ADDRESS", "second@example.org"),
        ("INBOXCRAFT_LIVE_IMAP2_HOST", "mail.example.org"),
        ("INBOXCRAFT_LIVE_IMAP2_USER", "second-login"),
        ("INBOXCRAFT_LIVE_IMAP2_SECRET", "imap2-secret-invented"),
        ("INBOXCRAFT_LIVE_RUN_ID", "20261105T120000Z-7"),
    ]);
    match Live::from_vars(|name| vars.get(name).map(|value| (*value).to_owned()))? {
        Live::Run(config) => Ok(*config),
        Live::Skip(reason) => Err(format!("skipped: {reason}").into()),
    }
}

fn admin(pki: &TestPki, addr: std::net::SocketAddr, password: &str) -> Result<Admin> {
    let config = TlsSettings::new(Trust::Roots(vec![pki.ca()]), wall()).client_config()?;
    Ok(Admin::connect(addr, HOST, Arc::clone(&config), USER, password)?)
}

#[test]
fn the_live_steps_run_against_the_fake_one_connection_after_the_other() -> Result {
    let config = invented()?;
    let pki = TestPki::new()?;
    let fake = Listen::tls(pki.server_config(None)?).accept_timeout(Duration::from_secs(2)).start_imap_many(FakeImap::new(), 3)?;
    let name = config.run_folder('/')?;

    // The admin client makes the run folder and appends an invented message.
    let mut first = admin(&pki, fake.addr(), PASSWORD)?;
    first.create(&name)?;
    first.append(&name, &config.invented_message(1))?;
    first.logout();

    // A worker finds the folder and the message: the server's state, not ours.
    let mut run = Run::on(fake, &pki, Options::default())?;
    run.job(JobKind::SyncFolderList)?;
    let events = run.until(|kind| matches!(kind, EventKind::Folders { .. }))?;
    let Some(EventKind::Folders { listings }) = events.last() else { return Err("no folders".into()) };
    assert!(listings.iter().any(|listing| listing.path().name() == name), "{listings:?}");
    let path = FolderPath::new(&name, Some('/'))?;
    run.job(JobKind::SyncFolder { folder: RUN, path: path.clone(), known: None })?;
    let events = run.synced()?;
    let subjects: Vec<String> = added(&events).iter().map(|summary| summary.envelope().subject().to_owned()).collect();
    assert_eq!(subjects, [format!("InboxCraft live test {} message 1", config.run_id())]);
    run.worker.stop();
    let fake = run.fake;

    // The admin client removes the folder again.
    let mut last = admin(&pki, fake.addr(), PASSWORD)?;
    assert!(RunFolder::create(&mut last, &config, '/').is_err(), "the folder exists already");
    last.delete(&name)?;
    assert!(last.delete(&name).is_err(), "deleted twice");
    last.logout();
    let lines = fake.finish()?.received().to_vec();
    assert!(lines.iter().any(|line| line.ends_with(&format!("DELETE \"{name}\""))), "{lines:?}");
    Ok(())
}

#[test]
fn a_refused_login_is_an_error_that_does_not_repeat_the_password() -> Result {
    let pki = TestPki::new()?;
    let fake = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
    let refused = admin(&pki, fake.addr(), "an-invented-wrong-password").err().ok_or("signed in")?;
    assert_eq!(refused.to_string(), "LOGIN refused");
    drop(fake);
    Ok(())
}

#[test]
fn values_that_cannot_be_quoted_are_refused_before_they_are_sent() -> Result {
    let pki = TestPki::new()?;
    let fake = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
    let mut open = admin(&pki, fake.addr(), PASSWORD)?;
    assert!(open.create("Elküldött").is_err());
    assert!(open.append("INBOX\r\nx", "Subject: x\r\n\r\n").is_err());
    assert!(open.append("Nowhere", "Subject: x\r\n\r\n").is_err());
    open.create("Quote \"and\" back\\slash")?;
    open.logout();
    let lines = fake.finish()?.received().to_vec();
    assert!(lines.iter().any(|line| line.ends_with("CREATE \"Quote \\\"and\\\" back\\\\slash\"")), "{lines:?}");
    assert!(!lines.iter().any(|line| line.contains("Elk")), "{lines:?}");
    Ok(())
}
