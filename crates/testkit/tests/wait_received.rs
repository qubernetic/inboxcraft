// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Waiting for the lines a client has sent so far, while the server still runs.

use std::time::Duration;

use inboxcraft_testkit::imap::{Action, Capability, FakeImap, PASSWORD, USER, When};
use inboxcraft_testkit::{Error, Fault, Listen, Script, TestClient};

const WAIT: Duration = Duration::from_secs(5);
/// How long a wait that has to fail is given.
const SHORT: Duration = Duration::from_millis(50);

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
fn the_wait_returns_once_the_lines_are_there() -> Result {
    let server = Listen::plain().start(Script::new().expect("a1 HELLO").expect("a2 HELLO").send("a2 OK done"))?;
    let mut client = TestClient::plain(server.addr(), WAIT)?;
    client.send_line("a1 HELLO")?;
    server.wait_received(|lines| lines == ["a1 HELLO"], WAIT)?;
    client.send_line("a2 HELLO")?;
    server.wait_received(|lines| lines.len() == 2, WAIT)?;
    assert_eq!(client.read_line()?, "a2 OK done");
    drop(client);
    // The transcript is the same as without the wait.
    assert_eq!(server.finish()?.received(), ["a1 HELLO", "a2 HELLO"]);
    Ok(())
}

#[test]
fn a_wait_for_lines_that_do_not_come_times_out() -> Result {
    let server = Listen::plain().start(Script::new().expect("a1 HELLO").expect("a2 HELLO"))?;
    let mut client = TestClient::plain(server.addr(), WAIT)?;
    client.send_line("a1 HELLO")?;
    let err = server.wait_received(|lines| lines.len() == 2, SHORT).err().ok_or("the wait passed")?;
    assert!(matches!(err, Error::Timeout), "{err}");
    assert_eq!(err.to_string(), "the client did not send the awaited lines before the timeout");
    client.send_line("a2 HELLO")?;
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn a_wait_without_any_client_times_out() -> Result {
    let server = Listen::plain().start(Script::new().expect("a1 HELLO"))?;
    assert!(matches!(server.wait_received(|lines| !lines.is_empty(), SHORT), Err(Error::Timeout)));
    Ok(())
}

#[test]
fn the_wait_sees_the_lines_of_every_connection_in_order() -> Result {
    let fake = FakeImap::new().without(Capability::LoginDisabled).on(When::command("NOOP"), [Action::Fault(Fault::Drop)]);
    let server = Listen::plain().accept_timeout(Duration::from_millis(500)).start_imap_many(fake, 2)?;
    let login = format!("s1 LOGIN {USER} {PASSWORD}");

    let mut first = TestClient::plain(server.addr(), WAIT)?;
    first.read_line()?;
    first.command(&login)?;
    first.send_line("a1 NOOP")?;
    assert!(first.read_line().is_err(), "the first connection is dropped");
    server.wait_received(|lines| lines.last().map(String::as_str) == Some("a1 NOOP"), WAIT)?;

    let mut second = TestClient::plain(server.addr(), WAIT)?;
    second.read_line()?;
    second.command(&login)?;
    server.wait_received(|lines| lines == [login.as_str(), "a1 NOOP", login.as_str()], WAIT)?;
    second.command("b1 LOGOUT")?;
    drop(second);
    assert_eq!(server.finish()?.received(), [login.as_str(), "a1 NOOP", login.as_str(), "b1 LOGOUT"]);
    Ok(())
}
