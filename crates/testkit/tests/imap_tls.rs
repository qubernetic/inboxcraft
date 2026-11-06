// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake IMAP server on a socket: STARTTLS, implicit TLS, the certificate faults and the faults of a script.

use std::collections::HashSet;
use std::time::Duration;

use inboxcraft_testkit::imap::{Action, Capability, FakeImap, PASSWORD, USER, When};
use inboxcraft_testkit::{CertFault, Clock, Error, ErrorKind, Fault, Listen, TestClient, TestPki};

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn starttls_comes_before_any_password() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let server = Listen::plain().start_imap(FakeImap::new().starttls(pki.server_config(None)?))?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    assert!(client.read_line()?.starts_with("* OK [CAPABILITY IMAP4rev1 STARTTLS LOGINDISABLED LITERAL+ ENABLE"));
    assert_eq!(client.command(&format!("a1 LOGIN {USER} {PASSWORD}"))?, ["a1 NO [PRIVACYREQUIRED] Sign in after TLS"]);
    assert_eq!(client.command("a2 AUTHENTICATE PLAIN")?, ["a2 NO [PRIVACYREQUIRED] Sign in after TLS"]);
    assert_eq!(client.command("a3 STARTTLS")?, ["a3 OK Begin TLS negotiation now"]);
    let mut client = client.start_tls(&pki.client(Clock::wall())?)?;
    let caps = client.command("a4 CAPABILITY")?;
    assert!(caps.first().is_some_and(|line| line.starts_with("* CAPABILITY IMAP4rev1 AUTH=PLAIN SASL-IR")), "{caps:?}");
    assert_eq!(client.command("a5 STARTTLS")?, ["a5 BAD STARTTLS is not available now"]);
    assert!(client.command(&format!("a6 LOGIN {USER} {PASSWORD}"))?.first().is_some_and(|line| line.starts_with("a6 OK")));
    assert_eq!(client.command("a7 LOGOUT")?, ["* BYE Logging out", "a7 OK LOGOUT completed"]);
    assert_eq!(client.read_line().map_err(|err| err.kind()), Err(ErrorKind::Dropped));
    let transcript = server.finish()?;
    assert_eq!(transcript.in_clear(), [format!("a1 LOGIN {USER} {PASSWORD}"), "a2 AUTHENTICATE PLAIN".to_owned(), "a3 STARTTLS".to_owned()]);
    assert_eq!(transcript.received().len(), 7);
    Ok(())
}

#[test]
fn a_server_without_logindisabled_takes_a_password_in_clear() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().start_imap(FakeImap::new().without(Capability::LoginDisabled))?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    assert!(client.read_line()?.starts_with("* OK [CAPABILITY IMAP4rev1 AUTH=PLAIN SASL-IR"));
    assert!(client.command(&format!("a1 LOGIN {USER} {PASSWORD}"))?.first().is_some_and(|line| line.starts_with("a1 OK")));
    drop(client);
    assert_eq!(server.finish()?.in_clear(), [format!("a1 LOGIN {USER} {PASSWORD}")]);
    Ok(())
}

#[test]
fn a_plain_server_without_tls_offers_no_starttls() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().start_imap(FakeImap::new())?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    assert!(client.read_line()?.starts_with("* OK [CAPABILITY IMAP4rev1 LOGINDISABLED LITERAL+"));
    assert_eq!(client.command("a1 STARTTLS")?, ["a1 BAD STARTTLS is not available now"]);
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn starttls_switched_off_is_not_offered() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let server = Listen::plain().start_imap(FakeImap::new().starttls(pki.server_config(None)?).without(Capability::StartTls))?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    assert!(client.read_line()?.starts_with("* OK [CAPABILITY IMAP4rev1 LOGINDISABLED LITERAL+"));
    assert_eq!(client.command("a1 STARTTLS")?, ["a1 BAD STARTTLS is not available now"]);
    drop(client);
    server.finish()?;
    Ok(())
}

#[test]
fn implicit_tls_greets_without_starttls() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let server = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
    let mut client = TestClient::tls(server.addr(), &pki.client(Clock::wall())?, TIMEOUT)?;
    assert!(client.read_line()?.starts_with("* OK [CAPABILITY IMAP4rev1 AUTH=PLAIN SASL-IR"));
    assert_eq!(client.command("a1 LOGOUT")?, ["* BYE Logging out", "a1 OK LOGOUT completed"]);
    let transcript = server.finish()?;
    assert!(transcript.in_clear().is_empty());
    Ok(())
}

fn starttls_error(pki: &TestPki, fault: CertFault) -> Result<ErrorKind, Box<dyn std::error::Error>> {
    let server = Listen::plain().start_imap(FakeImap::new().starttls(pki.server_config(Some(fault))?))?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    client.read_line()?;
    assert_eq!(client.command("a1 STARTTLS")?, ["a1 OK Begin TLS negotiation now"]);
    let err = client.start_tls(&pki.client(fault.clock())?).err().ok_or("the handshake passed")?;
    let (transcript, ended) = server.finish_with_transcript();
    let server_side = ended.err().ok_or("the server went on")?;
    assert!(matches!(server_side, Error::Io(_)), "{server_side}");
    assert_eq!(transcript.in_clear(), ["a1 STARTTLS"]);
    Ok(err.kind())
}

#[test]
fn each_certificate_fault_after_starttls_gives_its_own_error_kind() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let mut kinds = Vec::new();
    for fault in CertFault::ALL {
        kinds.push(starttls_error(&pki, fault)?);
    }
    assert_eq!(kinds, [ErrorKind::UnknownCa, ErrorKind::Expired, ErrorKind::WrongHost, ErrorKind::ClockFarOff]);
    assert_eq!(kinds.iter().collect::<HashSet<_>>().len(), 4);
    Ok(())
}

#[test]
fn starttls_on_a_client_already_encrypted_is_refused() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let server = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
    let client = TestClient::tls(server.addr(), &pki.client(Clock::wall())?, TIMEOUT)?;
    let err = client.start_tls(&pki.client(Clock::wall())?).err().ok_or("TLS twice")?;
    assert_eq!(err.kind(), ErrorKind::Tls);
    drop(server);
    Ok(())
}

#[test]
fn plaintext_after_the_starttls_answer_reaches_the_client_in_clear() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let injected = Action::Send("* CAPABILITY IMAP4rev1 AUTH=PLAIN".to_owned());
    let fake = FakeImap::new().starttls(pki.server_config(None)?).on(When::answered("starttls"), [injected]);
    let server = Listen::plain().start_imap(fake)?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    client.read_line()?;
    assert_eq!(client.command("a1 STARTTLS")?, ["a1 OK Begin TLS negotiation now"]);
    assert_eq!(client.read_line()?, "* CAPABILITY IMAP4rev1 AUTH=PLAIN");
    drop(client);
    drop(server);
    Ok(())
}

#[test]
fn a_password_pipelined_behind_starttls_is_recorded_in_clear_and_not_answered() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let server = Listen::plain().start_imap(FakeImap::new().starttls(pki.server_config(None)?))?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    client.read_line()?;
    client.send_bytes(format!("a1 STARTTLS\r\na2 LOGIN {USER} {PASSWORD}\r\na3 NO").as_bytes())?;
    assert_eq!(client.read_line()?, "a1 OK Begin TLS negotiation now");
    let mut client = client.start_tls(&pki.client(Clock::wall())?)?;
    assert_eq!(client.command("a4 LOGOUT")?, ["* BYE Logging out", "a4 OK LOGOUT completed"]);
    let transcript = server.finish()?;
    let in_clear = ["a1 STARTTLS".to_owned(), format!("a2 LOGIN {USER} {PASSWORD}"), "a3 NO".to_owned()];
    assert_eq!(transcript.in_clear(), in_clear);
    assert_eq!(transcript.received().get(3).map(String::as_str), Some("a4 LOGOUT"));
    Ok(())
}

#[test]
fn a_password_sent_after_the_starttls_answer_but_before_the_handshake_is_recorded_in_clear() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let server = Listen::plain().start_imap(FakeImap::new().starttls(pki.server_config(None)?))?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    client.read_line()?;
    assert_eq!(client.command("a1 STARTTLS")?, ["a1 OK Begin TLS negotiation now"]);
    std::thread::sleep(Duration::from_millis(100));
    client.send_line(&format!("a2 LOGIN {USER} {PASSWORD}"))?;
    let mut client = client.start_tls(&pki.client(Clock::wall())?)?;
    assert_eq!(client.command("a3 LOGOUT")?, ["* BYE Logging out", "a3 OK LOGOUT completed"]);
    let transcript = server.finish()?;
    assert_eq!(transcript.in_clear(), ["a1 STARTTLS".to_owned(), format!("a2 LOGIN {USER} {PASSWORD}")]);
    assert_eq!(transcript.received().len(), 3);
    Ok(())
}

#[test]
fn a_dropped_greeting_is_dropped() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().start_imap(FakeImap::new().on(When::Greeting, [Action::Fault(Fault::Drop)]))?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    assert_eq!(client.read_line().map_err(|err| err.kind()), Err(ErrorKind::Dropped));
    assert!(server.finish()?.received().is_empty());
    Ok(())
}

#[test]
fn faults_play_at_their_command_in_order() -> Result<(), Box<dyn std::error::Error>> {
    let fake = FakeImap::new()
        .on(When::command("NOOP"), [Action::Fault(Fault::Garbage(b"\x01junk\r\n".to_vec()))])
        .on(When::answered("NOOP"), [Action::Bye("Shutting down".to_owned())]);
    let server = Listen::plain().start_imap(fake)?;
    let mut client = TestClient::plain(server.addr(), Duration::from_millis(200))?;
    client.read_line()?;
    client.send_line("a1 NOOP")?;
    assert_eq!(client.read_line().map_err(|err| err.kind()), Err(ErrorKind::Garbage));
    assert_eq!(client.read_line()?, "a1 OK NOOP completed");
    assert_eq!(client.read_line()?, "* BYE Shutting down");
    assert_eq!(client.read_line().map_err(|err| err.kind()), Err(ErrorKind::Dropped));
    assert_eq!(server.finish()?.received(), ["a1 NOOP"]);
    let slow = FakeImap::new().on(When::command("NOOP"), [Action::Fault(Fault::Delay(Duration::from_millis(500)))]);
    let server = Listen::plain().start_imap(slow)?;
    let mut client = TestClient::plain(server.addr(), Duration::from_millis(200))?;
    client.read_line()?;
    assert_eq!(client.command("a1 NOOP").map_err(|err| err.kind()), Err(ErrorKind::Timeout));
    drop(client);
    drop(server);
    Ok(())
}

#[test]
fn a_server_dropped_by_the_test_stops_in_a_delay() -> Result<(), Box<dyn std::error::Error>> {
    let fake = FakeImap::new().on(When::Greeting, [Action::Fault(Fault::Delay(Duration::from_secs(30)))]);
    let server = Listen::plain().start_imap(fake)?;
    let _client = TestClient::plain(server.addr(), TIMEOUT)?;
    std::thread::sleep(Duration::from_millis(50));
    drop(server);
    Ok(())
}

#[test]
fn a_literal_sent_after_the_go_ahead_is_read() -> Result<(), Box<dyn std::error::Error>> {
    let fake =
        FakeImap::new().without(Capability::LoginDisabled).on(When::command("LOGIN"), [Action::Fault(Fault::Delay(Duration::from_millis(10)))]);
    let server = Listen::plain().start_imap(fake)?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    client.read_line()?;
    client.send_line(&format!("a1 LOGIN {{{}}}", USER.len()))?;
    assert_eq!(client.read_line()?, "+ Ready for the literal");
    client.send_line(&format!("{USER} {PASSWORD}"))?;
    assert!(client.read_line()?.starts_with("a1 OK [CAPABILITY"));
    drop(client);
    assert_eq!(server.finish()?.received(), [format!("a1 LOGIN {{{}}}", USER.len()), USER.to_owned(), format!(" {PASSWORD}")]);
    Ok(())
}

#[test]
fn a_refused_non_synchronising_literal_ends_the_session_with_an_error() -> Result<(), Box<dyn std::error::Error>> {
    let server = Listen::plain().start_imap(FakeImap::new().without(Capability::LiteralPlus))?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    client.read_line()?;
    client.send_line("a1 LOGIN {3+}")?;
    assert_eq!(client.read_line()?, "* BYE The literal is refused");
    let err = server.finish().err().ok_or("the server went on")?;
    assert!(matches!(err, Error::LineTooLong), "{err}");
    Ok(())
}
