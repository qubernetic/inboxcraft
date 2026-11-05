// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake IMAP server over several connections in a row, as a client that reconnects sees it: the mailboxes and the
//! scripted points carry over from one connection to the next.

use std::time::Duration;

use inboxcraft_testkit::imap::{Action, Capability, FakeImap, PASSWORD, USER, When};
use inboxcraft_testkit::{Error, Fault, Listen, TestClient};

const WAIT: Duration = Duration::from_secs(5);

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn sign_in(client: &mut TestClient) -> Result {
    client.read_line()?;
    client.command(&format!("s1 LOGIN {USER} {PASSWORD}"))?;
    Ok(())
}

#[test]
fn changes_and_played_rules_carry_over_to_the_next_connection() -> Result {
    let fake = FakeImap::new().without(Capability::LoginDisabled).on(When::command("NOOP"), [Action::Fault(Fault::Drop)]);
    let server = Listen::plain().accept_timeout(Duration::from_millis(500)).start_imap_many(fake, 3)?;

    let mut first = TestClient::plain(server.addr(), WAIT)?;
    sign_in(&mut first)?;
    first.command("a1 CREATE Kept")?;
    first.send_line("a2 NOOP")?;
    assert!(first.read_line().is_err(), "the first connection is dropped");

    let mut second = TestClient::plain(server.addr(), WAIT)?;
    sign_in(&mut second)?;
    let listed = second.command("b1 LIST \"\" Kept")?;
    assert_eq!(listed, ["* LIST (\\HasNoChildren) \"/\" \"Kept\"", "b1 OK LIST completed"]);
    // The drop was played once: the second NOOP is answered.
    assert_eq!(second.command("b2 NOOP")?, ["b2 OK NOOP completed"]);
    second.command("b3 LOGOUT")?;
    drop(second);

    // No third client comes: the server ends quietly once it has served one.
    let lines = server.finish()?.received().to_vec();
    assert_eq!(lines.iter().filter(|line| line.starts_with("s1 LOGIN")).count(), 2);
    assert!(lines.contains(&"a1 CREATE Kept".to_owned()) && lines.contains(&"b3 LOGOUT".to_owned()), "{lines:?}");
    Ok(())
}

#[test]
fn without_any_client_the_server_reports_none() -> Result {
    let server = Listen::plain().accept_timeout(Duration::from_millis(50)).start_imap_many(FakeImap::new(), 2)?;
    assert!(matches!(server.finish(), Err(Error::NoClient)));
    Ok(())
}

#[test]
fn a_failed_connection_is_reported_at_the_end_and_the_next_is_served() -> Result {
    let server = Listen::plain().accept_timeout(Duration::from_millis(500)).start_imap_many(FakeImap::new(), 3)?;
    let mut client = TestClient::plain(server.addr(), WAIT)?;
    client.read_line()?;
    client.send_bytes(&vec![b'a'; 70_000])?;
    drop(client);
    let mut next = TestClient::plain(server.addr(), WAIT)?;
    next.read_line()?;
    assert_eq!(next.command("b1 LOGOUT")?, ["* BYE Logging out", "b1 OK LOGOUT completed"]);
    drop(next);
    let (transcript, ended) = server.finish_with_transcript();
    assert!(matches!(ended, Err(Error::LineTooLong)), "{ended:?}");
    assert_eq!(transcript.received().last().map(String::as_str), Some("b1 LOGOUT"));
    Ok(())
}

#[test]
fn dropping_the_server_stops_it_between_connections() -> Result {
    let server = Listen::plain().accept_timeout(Duration::from_secs(30)).start_imap_many(FakeImap::new(), 3)?;
    let mut client = TestClient::plain(server.addr(), WAIT)?;
    client.read_line()?;
    client.command("a1 LOGOUT")?;
    drop(client);
    // The server now waits for the next client; dropping it ends the wait at once.
    drop(server);
    Ok(())
}

#[test]
fn the_server_stops_after_the_last_connection_it_was_given() -> Result {
    let server = Listen::plain().start_imap_many(FakeImap::new(), 1)?;
    let mut client = TestClient::plain(server.addr(), WAIT)?;
    client.read_line()?;
    client.command("a1 LOGOUT")?;
    drop(client);
    assert_eq!(server.finish()?.received(), ["a1 LOGOUT"]);
    Ok(())
}
