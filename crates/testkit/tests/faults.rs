// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The drop, delay and garbage faults, each seen by the client as its own error kind.

use std::time::Duration;

use inboxcraft_testkit::{ErrorKind, Listen, Script, TestClient};

const TIMEOUT: Duration = Duration::from_secs(5);

fn first_error(script: Script, timeout: Duration) -> Result<ErrorKind, Box<dyn std::error::Error>> {
    let server = Listen::plain().start(script)?;
    let mut client = TestClient::plain(server.addr(), timeout)?;
    assert_eq!(client.read_line()?, "* OK fake ready");
    let err = client.read_line().err().ok_or("the second line arrived")?;
    assert!(!err.to_string().is_empty());
    Ok(err.kind())
}

fn greeting() -> Script {
    Script::new().send("* OK fake ready")
}

#[test]
fn a_dropped_connection_is_dropped() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(first_error(greeting().drop_connection().send("never sent"), TIMEOUT)?, ErrorKind::Dropped);
    Ok(())
}

#[test]
fn a_delay_longer_than_the_timeout_is_a_timeout() -> Result<(), Box<dyn std::error::Error>> {
    let kind = first_error(greeting().delay(Duration::from_secs(2)).send("late"), Duration::from_millis(100))?;
    assert_eq!(kind, ErrorKind::Timeout);
    Ok(())
}

#[test]
fn a_short_delay_only_delays() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().start(greeting().delay(Duration::from_millis(20)).send("on time"))?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    assert_eq!(client.read_line()?, "* OK fake ready");
    assert_eq!(client.read_line()?, "on time");
    server.finish()?;
    Ok(())
}

#[test]
fn bytes_that_are_not_utf8_are_garbage() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(first_error(greeting().garbage(b"\xff\xfe\xc3\x28\r\n"), TIMEOUT)?, ErrorKind::Garbage);
    Ok(())
}

#[test]
fn control_characters_are_garbage() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(first_error(greeting().garbage(b"* OK \x1b[2J\x00\r\n"), TIMEOUT)?, ErrorKind::Garbage);
    Ok(())
}

#[test]
fn a_line_without_end_is_garbage() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(first_error(greeting().garbage(&[b'x'; 20_000]), TIMEOUT)?, ErrorKind::Garbage);
    Ok(())
}

#[test]
fn a_tab_is_printable() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().start(Script::new().garbage(b"a\tb\n"))?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    assert_eq!(client.read_line()?, "a\tb");
    server.finish()?;
    Ok(())
}

#[test]
fn a_refused_connection_is_an_io_error() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::net::TcpListener::bind("127.0.0.1:0")?.local_addr()?;
    let err = TestClient::plain(addr, TIMEOUT).err().ok_or("connected to a closed port")?;
    assert_eq!(err.kind(), ErrorKind::Io);
    Ok(())
}

#[test]
fn sending_to_a_closed_server_is_dropped() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().start(Script::new())?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    server.finish()?;
    let mut kinds = Vec::new();
    for _ in 0..50 {
        if let Err(err) = client.send_line(&"x".repeat(4096)) {
            kinds.push(err.kind());
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(kinds, [ErrorKind::Dropped]);
    Ok(())
}
