// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake SMTP server on a socket: submission over implicit TLS and STARTTLS, no password before TLS, the
//! certificate faults, and a client that sends again on a second connection.

use std::time::Duration;

use inboxcraft_testkit::smtp::{Extension, FakeSmtp, Reply, When};
use inboxcraft_testkit::{CertFault, Clock, Error, ErrorKind, Listen, TestClient, TestPki};

const TIMEOUT: Duration = Duration::from_secs(5);
/// AUTH PLAIN with the fake's user and password.
const SIGN_IN: &str = "AUTH PLAIN AGFkYUBleGFtcGxlLm9yZwBub3QtYS1yZWFsLXBhc3N3b3Jk";

/// Sends `line` and reads the reply, every line of it.
fn talk(client: &mut TestClient, line: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    client.send_line(line)?;
    read_reply(client)
}

/// Reads one reply: lines up to the one with a space, or nothing, after its code.
fn read_reply(client: &mut TestClient) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut lines = Vec::new();
    loop {
        let line = client.read_line()?;
        let last = line.get(3..4).is_none_or(|mark| mark == " ");
        lines.push(line);
        if last {
            return Ok(lines);
        }
    }
}

/// Sends one message from Ada to Bea and returns the last reply.
fn send_message(client: &mut TestClient) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    talk(client, "MAIL FROM:<ada@example.org>")?;
    talk(client, "RCPT TO:<bea@example.org>")?;
    talk(client, "DATA")?;
    client.send_line("Subject: Hi")?;
    client.send_line("")?;
    talk(client, ".")
}

#[test]
fn starttls_comes_before_any_password() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let fake = FakeSmtp::new().starttls(pki.server_config(None)?);
    let outbox = fake.outbox();
    let server = Listen::plain().start_smtp(fake)?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    assert_eq!(read_reply(&mut client)?, ["220 smtp.example.org ESMTP InboxCraft fake SMTP ready"]);
    let ehlo = talk(&mut client, "EHLO c.example.org")?;
    assert_eq!(ehlo.get(1).map(String::as_str), Some("250-STARTTLS"));
    assert!(!ehlo.iter().any(|line| line.contains("AUTH")), "{ehlo:?}");
    assert_eq!(talk(&mut client, SIGN_IN)?, ["530 5.7.0 Must issue a STARTTLS command first"]);
    assert_eq!(talk(&mut client, "MAIL FROM:<ada@example.org>")?, ["530 5.7.0 Authentication required"]);
    assert_eq!(talk(&mut client, "STARTTLS")?, ["220 2.0.0 Ready to start TLS"]);
    let mut client = client.start_tls(&pki.client(Clock::wall())?)?;
    assert_eq!(talk(&mut client, "MAIL FROM:<ada@example.org>")?, ["503 5.5.1 Say EHLO first"]);
    let ehlo = talk(&mut client, "EHLO c.example.org")?;
    assert!(!ehlo.iter().any(|line| line.contains("STARTTLS")), "{ehlo:?}");
    assert_eq!(ehlo.last().map(String::as_str), Some("250 AUTH PLAIN LOGIN"));
    assert_eq!(talk(&mut client, "STARTTLS")?, ["503 5.5.1 TLS is already active"]);
    assert_eq!(talk(&mut client, SIGN_IN)?, ["235 2.7.0 Authentication successful"]);
    assert_eq!(send_message(&mut client)?, ["250 2.0.0 Queued as 1"]);
    assert_eq!(talk(&mut client, "QUIT")?, ["221 2.0.0 Bye"]);
    assert_eq!(client.read_line().map_err(|err| err.kind()), Err(ErrorKind::Dropped));
    let transcript = server.finish()?;
    assert_eq!(transcript.in_clear(), ["EHLO c.example.org", SIGN_IN, "MAIL FROM:<ada@example.org>", "STARTTLS"]);
    assert!(outbox.messages().first().is_some_and(|sent| sent.tls() && sent.data() == b"Subject: Hi\r\n\r\n"));
    Ok(())
}

#[test]
fn implicit_tls_offers_auth_at_once() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let server = Listen::tls(pki.server_config(None)?).start_smtp(FakeSmtp::new())?;
    let mut client = TestClient::tls(server.addr(), &pki.client(Clock::wall())?, TIMEOUT)?;
    read_reply(&mut client)?;
    let ehlo = talk(&mut client, "EHLO c.example.org")?;
    assert_eq!(ehlo.last().map(String::as_str), Some("250 AUTH PLAIN LOGIN"));
    assert_eq!(talk(&mut client, "STARTTLS")?, ["503 5.5.1 TLS is already active"]);
    assert_eq!(talk(&mut client, SIGN_IN)?, ["235 2.7.0 Authentication successful"]);
    assert_eq!(talk(&mut client, "QUIT")?, ["221 2.0.0 Bye"]);
    let transcript = server.finish()?;
    assert!(transcript.in_clear().is_empty());
    assert_eq!(transcript.received().len(), 4);
    Ok(())
}

#[test]
fn a_careless_server_takes_a_password_in_clear() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().start_smtp(FakeSmtp::new().auth_in_clear())?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    read_reply(&mut client)?;
    assert_eq!(talk(&mut client, "EHLO c.example.org")?.last().map(String::as_str), Some("250 AUTH PLAIN LOGIN"));
    assert_eq!(talk(&mut client, SIGN_IN)?, ["235 2.7.0 Authentication successful"]);
    drop(client);
    assert_eq!(server.finish()?.in_clear(), ["EHLO c.example.org", SIGN_IN]);
    Ok(())
}

#[test]
fn starttls_is_not_offered_without_a_configuration_or_switched_off() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    for fake in [FakeSmtp::new(), FakeSmtp::new().starttls(pki.server_config(None)?).without(Extension::StartTls)] {
        let server = Listen::plain().start_smtp(fake)?;
        let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
        read_reply(&mut client)?;
        assert!(!talk(&mut client, "EHLO c.example.org")?.iter().any(|line| line.contains("STARTTLS")));
        assert_eq!(talk(&mut client, "STARTTLS")?, ["502 5.5.1 STARTTLS is not available"]);
        drop(client);
        server.finish()?;
    }
    Ok(())
}

#[test]
fn lines_pipelined_behind_starttls_are_heard_in_clear_and_not_answered() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let server = Listen::plain().start_smtp(FakeSmtp::new().starttls(pki.server_config(None)?))?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    read_reply(&mut client)?;
    talk(&mut client, "EHLO c.example.org")?;
    client.send_bytes(format!("STARTTLS\r\n{SIGN_IN}\r\n").as_bytes())?;
    assert_eq!(read_reply(&mut client)?, ["220 2.0.0 Ready to start TLS"]);
    let mut client = client.start_tls(&pki.client(Clock::wall())?)?;
    assert_eq!(talk(&mut client, "NOOP")?, ["250 2.0.0 OK"]);
    talk(&mut client, "QUIT")?;
    let transcript = server.finish()?;
    assert_eq!(transcript.in_clear(), ["EHLO c.example.org", "STARTTLS", SIGN_IN]);
    Ok(())
}

#[test]
fn each_certificate_fault_fails_the_handshake_with_its_own_kind() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let mut kinds = Vec::new();
    for fault in CertFault::ALL {
        let server = Listen::tls(pki.server_config(Some(fault))?).accept_timeout(TIMEOUT).start_smtp(FakeSmtp::new())?;
        let err = TestClient::tls(server.addr(), &pki.client(fault.clock())?, TIMEOUT).err().map(|err| err.kind());
        assert!(server.finish().is_err(), "{fault:?}");
        let kind = err.ok_or("the handshake did not fail")?;
        kinds.push(kind);

        let server = Listen::plain().start_smtp(FakeSmtp::new().starttls(pki.server_config(Some(fault))?))?;
        let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
        read_reply(&mut client)?;
        talk(&mut client, "EHLO c.example.org")?;
        talk(&mut client, "STARTTLS")?;
        let err = client.start_tls(&pki.client(fault.clock())?).err().map(|err| err.kind());
        assert_eq!(err, Some(kind), "{fault:?} after STARTTLS");
        let (transcript, ended) = server.finish_with_transcript();
        assert!(ended.is_err(), "{fault:?}");
        assert_eq!(transcript.in_clear(), ["EHLO c.example.org", "STARTTLS"]);
    }
    assert_eq!(kinds, [ErrorKind::UnknownCa, ErrorKind::Expired, ErrorKind::WrongHost, ErrorKind::ClockFarOff]);
    Ok(())
}

#[test]
fn a_client_that_sends_again_meets_the_same_server() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let fake = FakeSmtp::new().refuse(When::command("DATA"), Reply::new(421, "4.4.2", "Timeout, closing"));
    let outbox = fake.outbox();
    let server = Listen::tls(pki.server_config(None)?).accept_timeout(Duration::from_millis(500)).start_smtp_many(fake, 3)?;
    for expected in ["421 4.4.2 Timeout, closing", "250 2.0.0 Queued as 1"] {
        let mut client = TestClient::tls(server.addr(), &pki.client(Clock::wall())?, TIMEOUT)?;
        read_reply(&mut client)?;
        talk(&mut client, "EHLO c.example.org")?;
        talk(&mut client, SIGN_IN)?;
        talk(&mut client, "MAIL FROM:<ada@example.org>")?;
        talk(&mut client, "RCPT TO:<bea@example.org>")?;
        let reply = talk(&mut client, "DATA")?;
        let reply = if reply == ["354 End data with <CR><LF>.<CR><LF>"] { talk(&mut client, ".")? } else { reply };
        assert_eq!(reply, [expected]);
        talk(&mut client, "QUIT")?;
    }
    let transcript = server.finish()?;
    assert_eq!(transcript.received().first().map(String::as_str), Some("EHLO c.example.org"));
    assert_eq!(outbox.messages().len(), 1);
    Ok(())
}

#[test]
fn a_line_too_long_on_a_socket_ends_the_session_with_an_error() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().start_smtp(FakeSmtp::new())?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    read_reply(&mut client)?;
    assert_eq!(talk(&mut client, &format!("EHLO {}", "x".repeat(9000)))?, ["421 4.7.0 Line too long"]);
    assert!(matches!(server.finish(), Err(Error::LineTooLong)));
    Ok(())
}
