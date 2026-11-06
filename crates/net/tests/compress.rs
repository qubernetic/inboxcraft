// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What a signed-in connection switches on where the server offers it: the COMPRESS=DEFLATE stream and QRESYNC.

mod support;

use inboxcraft_model::{ConnectionState, ErrorKind, EventKind, JobKind, Security};
use inboxcraft_net::{ImapConnection, Password};
use inboxcraft_testkit::imap::{Action, Capability, FakeImap, PASSWORD, USER, When};
use inboxcraft_testkit::{Fault, Listen, TestPki};
use support::run::{Options, Run, sync_inbox};
use support::{Result, connector, server, wall};

/// The command names of `lines`, without tags and arguments past the first word or two.
fn verbs(lines: &[String]) -> Vec<String> {
    lines.iter().filter_map(|line| line.split_once(' ')).map(|(_, rest)| rest.split(' ').take(2).collect::<Vec<_>>().join(" ")).collect()
}

/// Signs in to `fake` and runs `then` on the connection; the lines the server got.
fn connection(fake: FakeImap, then: impl FnOnce(&mut ImapConnection) -> Result) -> Result<Vec<String>> {
    let pki = TestPki::new()?;
    let server_side = Listen::tls(pki.server_config(None)?).start_imap(fake)?;
    let mut open = ImapConnection::open(&server(Security::Tls)?, &connector(&pki, wall(), server_side.addr()))?;
    then(&mut open)?;
    open.logout();
    Ok(server_side.finish()?.received().to_vec())
}

#[test]
fn the_worker_compresses_and_switches_qresync_on_where_offered() -> Result {
    let mut run = Run::sample()?;
    run.job(JobKind::SyncFolderList)?;
    run.job(sync_inbox(None)?)?;
    run.synced()?;
    let lines = run.finish()?;
    assert_eq!(
        verbs(&lines),
        ["AUTHENTICATE PLAIN", "ENABLE QRESYNC", "COMPRESS DEFLATE", "LIST \"\"", "EXAMINE \"INBOX\"", "UID FETCH", "UID FETCH", "LOGOUT"]
    );
    Ok(())
}

#[test]
fn without_the_capabilities_or_the_setting_neither_is_asked_for() -> Result {
    let fake = FakeImap::sample().without(Capability::CompressDeflate).without(Capability::QResync);
    let mut run = Run::start(fake, Some(PASSWORD), inboxcraft_net::SyncConfig::default())?;
    run.job(JobKind::SyncFolderList)?;
    run.until(|kind| matches!(kind, EventKind::Folders { .. }))?;
    assert_eq!(verbs(&run.finish()?), ["AUTHENTICATE PLAIN", "LIST \"\"", "LOGOUT"]);

    let mut run = Run::with(FakeImap::sample(), Options { compress: false, ..Options::default() })?;
    run.job(JobKind::SyncFolderList)?;
    run.until(|kind| matches!(kind, EventKind::Folders { .. }))?;
    assert_eq!(verbs(&run.finish()?), ["AUTHENTICATE PLAIN", "ENABLE QRESYNC", "LIST \"\"", "LOGOUT"]);
    Ok(())
}

#[test]
fn compressing_and_enabling_twice_asks_once() -> Result {
    let lines = connection(FakeImap::new(), |open| {
        assert!(!open.compress()?, "not before the sign-in");
        assert!(!open.enable_qresync()?, "not before the sign-in");
        open.login(USER, &Password::new(PASSWORD.to_owned()))?;
        assert!(open.enable_qresync()? && open.enable_qresync()? && open.qresync());
        assert!(open.compress()? && open.compress()? && open.is_compressed());
        assert!(format!("{open:?}").contains("compressed: true"));
        Ok(())
    })?;
    assert_eq!(verbs(&lines), ["AUTHENTICATE PLAIN", "ENABLE QRESYNC", "COMPRESS DEFLATE", "LOGOUT"]);
    Ok(())
}

#[test]
fn a_broken_compressed_stream_ends_the_connection() -> Result {
    let garbage = Action::Fault(Fault::Garbage(vec![0xff; 32]));
    let fake = FakeImap::sample().on(When::answered("COMPRESS"), [garbage]);
    let mut run = Run::with(fake, Options::default())?;
    run.job(JobKind::SyncFolderList)?;
    // The garbage comes with the OK (the sign-in fails) or after it (the job fails), as the reads fall.
    let events = run.until(|kind| matches!(kind, EventKind::Connection(ConnectionState::BackingOff { .. } | ConnectionState::Offline)))?;
    assert!(events.contains(&EventKind::Failed { kind: ErrorKind::Protocol, folder: None }), "{events:?}");
    assert!(!events.iter().any(|kind| matches!(kind, EventKind::Folders { .. })), "{events:?}");
    run.worker.stop();
    Ok(())
}
