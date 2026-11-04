// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scripted line-protocol core over plain TCP.

use std::time::Duration;

use inboxcraft_testkit::{Error, Listen, Script, Step, TestClient};

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn a_client_follows_the_script() -> Result<(), Box<dyn std::error::Error>> {
    let script = Script::new().send("* OK fake ready").expect("a1 HELLO").send("a1 OK done");
    let server = Listen::plain().start(script)?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    assert_eq!(client.read_line()?, "* OK fake ready");
    client.send_line("a1 HELLO")?;
    assert_eq!(client.read_line()?, "a1 OK done");
    drop(client);
    let transcript = server.finish()?;
    assert_eq!(transcript.received(), ["a1 HELLO"]);
    assert_eq!(transcript.in_clear(), ["a1 HELLO"]);
    Ok(())
}

#[test]
fn a_line_the_script_did_not_expect_fails_the_script() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().start(Script::new().expect("a1 HELLO").send("never sent"))?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    client.send_line("a1 GOODBYE\r\n")?;
    let err = server.finish().err().ok_or("the script passed")?;
    assert!(matches!(&err, Error::Unexpected { expected, got } if expected == "a1 HELLO" && got == "a1 GOODBYE"), "{err}");
    assert_eq!(err.to_string(), "the script expected \"a1 HELLO\", the client sent \"a1 GOODBYE\"");
    Ok(())
}

#[test]
fn a_client_that_leaves_early_fails_the_script() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().start(Script::new().expect("a1 HELLO"))?;
    drop(TestClient::plain(server.addr(), TIMEOUT)?);
    let err = server.finish().err().ok_or("the script passed")?;
    assert!(matches!(err, Error::ClientGone), "{err}");
    Ok(())
}

#[test]
fn an_overlong_client_line_fails_the_script() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().start(Script::new().expect("a1 HELLO"))?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    client.send_line(&"x".repeat(20_000))?;
    let err = server.finish().err().ok_or("the script passed")?;
    assert!(matches!(err, Error::LineTooLong), "{err}");
    Ok(())
}

#[test]
fn a_long_line_that_ends_within_one_read_is_still_too_long() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().start(Script::new().expect("a1 HELLO"))?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    client.send_line(&"x".repeat(8193))?;
    let err = server.finish().err().ok_or("the script passed")?;
    assert!(matches!(err, Error::LineTooLong), "{err}");
    Ok(())
}

#[test]
fn a_server_without_a_client_gives_up() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().accept_timeout(Duration::from_millis(20)).start(Script::new())?;
    let err = server.finish().err().ok_or("the script passed")?;
    assert!(matches!(err, Error::NoClient), "{err}");
    Ok(())
}

#[test]
fn a_silent_client_times_the_server_out() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().io_timeout(Duration::from_millis(50)).start(Script::new().expect("a1 HELLO"))?;
    let _client = TestClient::plain(server.addr(), TIMEOUT)?;
    let err = server.finish().err().ok_or("the script passed")?;
    assert!(matches!(&err, Error::Io(_)), "{err}");
    assert!(err.to_string().starts_with("socket error: "), "{err}");
    assert!(std::error::Error::source(&err).is_some());
    Ok(())
}

#[test]
fn dropping_a_server_stops_its_script() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().start(Script::new().delay(Duration::from_secs(600)))?;
    let client = TestClient::plain(server.addr(), TIMEOUT)?;
    std::thread::sleep(Duration::from_millis(20));
    drop(server);
    drop(client);
    Ok(())
}

#[test]
fn dropping_a_server_without_a_client_stops_it() -> Result<(), Box<dyn std::error::Error>> {
    drop(Listen::plain().start(Script::new())?);
    Ok(())
}

#[test]
fn script_lines_cannot_smuggle_a_second_line() {
    let script = Script::new().send("a1 OK\r\na2 BAD\0");
    assert_eq!(script.steps(), [Step::Send("a1 OKa2 BAD".to_owned())]);
}

#[test]
fn errors_name_their_cause() {
    let messages = [Error::NoClient, Error::ClientGone, Error::LineTooLong, Error::Thread].map(|err| err.to_string());
    assert_eq!(
        messages,
        [
            "no client connected before the accept timeout",
            "the client closed the connection while the script expected a line",
            "the client sent a line longer than the server reads",
            "the server thread ended without a result",
        ]
    );
    assert!(std::error::Error::source(&Error::Thread).is_none());
}
