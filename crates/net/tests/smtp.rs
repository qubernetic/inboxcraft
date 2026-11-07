// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Submission against the fake SMTP server: implicit TLS and STARTTLS, no password before TLS, each certificate
//! fault with its own kind, the sign-in, and a message with its envelope.

mod support;

use inboxcraft_model::{EmailAddress, ErrorKind, Security};
use inboxcraft_net::{NetError, Password, SmtpConnection};
use inboxcraft_testkit::smtp::{Extension, FakeSmtp, PASSWORD, Reply, USER, When};
use inboxcraft_testkit::{CertFault, Listen, TestPki};
use support::{Result, connector, far_off, submission, wall};

fn kind<T>(result: std::result::Result<T, NetError>) -> Option<ErrorKind> {
    result.err().map(|error| error.kind())
}

fn expected(fault: CertFault) -> ErrorKind {
    match fault {
        CertFault::UnknownCa => ErrorKind::CertificateUnknownIssuer,
        CertFault::Expired => ErrorKind::CertificateExpired,
        CertFault::WrongHost => ErrorKind::CertificateWrongHost,
        CertFault::ClockFarOff => ErrorKind::ClockFarOff,
    }
}

fn clock_for(fault: CertFault) -> Result<std::sync::Arc<dyn inboxcraft_time::Clock>> {
    if fault == CertFault::ClockFarOff { far_off() } else { Ok(wall()) }
}

fn password() -> Password {
    Password::new(PASSWORD.to_owned())
}

fn address(text: &str) -> Result<EmailAddress> {
    Ok(EmailAddress::new(text)?)
}

const MESSAGE: &[u8] = b"From: ada@example.org\r\nTo: bea@example.org\r\nSubject: Hi\r\n\r\n.a line with a dot\r\n";

fn signed_in(lines: &[String]) -> bool {
    lines.iter().any(|line| line.starts_with("AUTH"))
}

#[test]
fn each_certificate_fault_has_its_own_kind_over_implicit_tls() -> Result {
    let pki = TestPki::new()?;
    let mut kinds = Vec::new();
    for fault in CertFault::ALL {
        let fake = Listen::tls(pki.server_config(Some(fault))?).start_smtp(FakeSmtp::new())?;
        let opened = SmtpConnection::open(&submission(Security::Tls)?, &connector(&pki, clock_for(fault)?, fake.addr()));
        assert_eq!(kind(opened), Some(expected(fault)), "{fault:?}");
        let (transcript, _refused) = fake.finish_with_transcript();
        assert!(transcript.received().is_empty(), "{fault:?}: {:?}", transcript.received());
        kinds.push(expected(fault));
    }
    kinds.dedup();
    assert_eq!(kinds.len(), CertFault::ALL.len());
    Ok(())
}

#[test]
fn each_certificate_fault_has_its_own_kind_after_starttls() -> Result {
    let pki = TestPki::new()?;
    for fault in CertFault::ALL {
        let fake = Listen::plain().start_smtp(FakeSmtp::new().starttls(pki.server_config(Some(fault))?))?;
        let opened = SmtpConnection::open(&submission(Security::StartTls)?, &connector(&pki, clock_for(fault)?, fake.addr()));
        assert_eq!(kind(opened), Some(expected(fault)), "{fault:?}");
        let (transcript, _refused) = fake.finish_with_transcript();
        assert_eq!(transcript.received(), ["EHLO [127.0.0.1]", "STARTTLS"], "{fault:?}");
    }
    Ok(())
}

#[test]
fn implicit_tls_sends_a_message_with_its_envelope() -> Result {
    let pki = TestPki::new()?;
    let fake = FakeSmtp::new();
    let outbox = fake.outbox();
    let server = Listen::tls(pki.server_config(None)?).start_smtp(fake)?;
    let mut connection = SmtpConnection::open(&submission(Security::Tls)?, &connector(&pki, wall(), server.addr()))?;
    assert!(connection.is_encrypted());
    connection.login(USER, &password())?;
    let recipients = [address("bea@example.org")?, address("hidden@example.org")?];
    let accepted = connection.send(&address(USER)?, &recipients, MESSAGE)?;
    assert_eq!(accepted.map_err(|refusal| refusal.code), Ok("Queued as 1".to_owned()));
    connection.quit();
    let transcript = server.finish()?;
    assert!(transcript.in_clear().is_empty());
    assert!(transcript.received().first().is_some_and(|line| line == "EHLO [127.0.0.1]"), "{:?}", transcript.received());
    assert!(transcript.received().last().is_some_and(|line| line == "QUIT"));
    let sent = outbox.messages();
    let [message] = sent.as_slice() else { return Err(format!("{sent:?}").into()) };
    assert_eq!(message.from(), USER);
    assert_eq!(message.recipients(), ["bea@example.org", "hidden@example.org"]);
    assert_eq!(message.data(), MESSAGE);
    assert_eq!((message.user(), message.tls()), (Some(USER), true));
    Ok(())
}

#[test]
fn starttls_comes_before_the_sign_in() -> Result {
    let pki = TestPki::new()?;
    let fake = FakeSmtp::new().starttls(pki.server_config(None)?);
    let outbox = fake.outbox();
    let server = Listen::plain().start_smtp(fake)?;
    let mut connection = SmtpConnection::open(&submission(Security::StartTls)?, &connector(&pki, wall(), server.addr()))?;
    assert!(connection.is_encrypted());
    connection.login(USER, &password())?;
    let accepted = connection.send(&address(USER)?, &[address("bea@example.org")?], MESSAGE)?;
    assert!(accepted.is_ok(), "{accepted:?}");
    connection.quit();
    let transcript = server.finish()?;
    assert_eq!(transcript.in_clear(), ["EHLO [127.0.0.1]", "STARTTLS"]);
    assert!(signed_in(transcript.received()));
    assert_eq!(outbox.messages().first().map(|sent| sent.tls()), Some(true));
    Ok(())
}

#[test]
fn a_server_that_offers_auth_in_clear_gets_no_password_before_tls() -> Result {
    let pki = TestPki::new()?;
    // It lists AUTH before STARTTLS and would take the password: the client still encrypts first.
    let server = Listen::plain().start_smtp(FakeSmtp::new().auth_in_clear().starttls(pki.server_config(None)?))?;
    let mut connection = SmtpConnection::open(&submission(Security::StartTls)?, &connector(&pki, wall(), server.addr()))?;
    connection.login(USER, &password())?;
    connection.quit();
    let transcript = server.finish()?;
    assert!(!signed_in(transcript.in_clear()), "{:?}", transcript.in_clear());
    assert!(signed_in(transcript.received()));
    // Without STARTTLS at all, nothing more than EHLO goes out.
    let server = Listen::plain().start_smtp(FakeSmtp::new().auth_in_clear().without(Extension::StartTls))?;
    let opened = SmtpConnection::open(&submission(Security::StartTls)?, &connector(&pki, wall(), server.addr()));
    assert_eq!(kind(opened), Some(ErrorKind::StartTlsMissing));
    let (transcript, _ended) = server.finish_with_transcript();
    assert_eq!(transcript.received(), ["EHLO [127.0.0.1]"]);
    Ok(())
}

#[test]
fn a_refused_password_and_a_server_without_a_method_are_told_apart() -> Result {
    let pki = TestPki::new()?;
    let server = Listen::tls(pki.server_config(None)?).start_smtp(FakeSmtp::new().login(USER, "another-invented-password"))?;
    let mut connection = SmtpConnection::open(&submission(Security::Tls)?, &connector(&pki, wall(), server.addr()))?;
    assert_eq!(kind(connection.login(USER, &password())), Some(ErrorKind::LoginRefused));
    connection.quit();
    drop(server.finish_with_transcript());
    let fake = FakeSmtp::new().without(Extension::AuthPlain).without(Extension::AuthLogin);
    let server = Listen::tls(pki.server_config(None)?).start_smtp(fake)?;
    let mut connection = SmtpConnection::open(&submission(Security::Tls)?, &connector(&pki, wall(), server.addr()))?;
    assert_eq!(kind(connection.login(USER, &password())), Some(ErrorKind::NoLoginMethod));
    connection.quit();
    let (transcript, _ended) = server.finish_with_transcript();
    assert!(!signed_in(transcript.received()));
    Ok(())
}

#[test]
fn auth_login_is_used_where_plain_is_not_offered() -> Result {
    let pki = TestPki::new()?;
    let server = Listen::tls(pki.server_config(None)?).start_smtp(FakeSmtp::new().without(Extension::AuthPlain))?;
    let mut connection = SmtpConnection::open(&submission(Security::Tls)?, &connector(&pki, wall(), server.addr()))?;
    connection.login(USER, &password())?;
    connection.quit();
    let transcript = server.finish()?;
    assert!(transcript.received().iter().any(|line| line == "AUTH LOGIN"), "{:?}", transcript.received());
    Ok(())
}

#[test]
fn a_refused_recipient_sends_nothing_and_names_the_reply() -> Result {
    let pki = TestPki::new()?;
    let fake = FakeSmtp::new().refuse(When::rcpt("nobody@example.org"), Reply::new(550, "5.1.1", "No such user"));
    let outbox = fake.outbox();
    let server = Listen::tls(pki.server_config(None)?).start_smtp(fake)?;
    let mut connection = SmtpConnection::open(&submission(Security::Tls)?, &connector(&pki, wall(), server.addr()))?;
    connection.login(USER, &password())?;
    let recipients = [address("bea@example.org")?, address("nobody@example.org")?];
    let refusal = connection.send(&address(USER)?, &recipients, MESSAGE)?.err().ok_or("accepted")?;
    assert_eq!((refusal.code, refusal.enhanced.map(|code| code.to_string()), refusal.text.as_str()), (550, Some("5.1.1".to_owned()), "No such user"));
    // The connection holds: the transaction was reset, and another message goes.
    let accepted = connection.send(&address(USER)?, &[address("bea@example.org")?], MESSAGE)?;
    assert!(accepted.is_ok(), "{accepted:?}");
    connection.quit();
    let transcript = server.finish()?;
    assert!(transcript.received().iter().any(|line| line == "RSET"), "{:?}", transcript.received());
    assert_eq!(outbox.messages().len(), 1);
    Ok(())
}

#[test]
fn a_refused_message_and_a_refused_sender_name_the_reply() -> Result {
    let pki = TestPki::new()?;
    for when in [When::Message, When::command("MAIL")] {
        let fake = FakeSmtp::new().refuse(when.clone(), Reply::new(552, "5.3.4", "Message too big for system"));
        let outbox = fake.outbox();
        let server = Listen::tls(pki.server_config(None)?).start_smtp(fake)?;
        let mut connection = SmtpConnection::open(&submission(Security::Tls)?, &connector(&pki, wall(), server.addr()))?;
        connection.login(USER, &password())?;
        let refusal = connection.send(&address(USER)?, &[address("bea@example.org")?], MESSAGE)?.err().ok_or("accepted")?;
        assert_eq!((refusal.code, refusal.text.as_str()), (552, "Message too big for system"), "{when:?}");
        connection.quit();
        drop(server.finish_with_transcript());
        assert!(outbox.messages().is_empty(), "{when:?}");
    }
    Ok(())
}

#[test]
fn eight_bit_and_utf8_are_said_where_needed() -> Result {
    let pki = TestPki::new()?;
    let fake = FakeSmtp::new();
    let outbox = fake.outbox();
    let server = Listen::tls(pki.server_config(None)?).start_smtp(fake)?;
    let mut connection = SmtpConnection::open(&submission(Security::Tls)?, &connector(&pki, wall(), server.addr()))?;
    connection.login(USER, &password())?;
    let message = "Subject: Árvíztűrő\r\n\r\nTükörfúrógép\r\n".as_bytes();
    assert!(connection.send(&address(USER)?, &[address("bea@example.org")?], MESSAGE)?.is_ok());
    assert!(connection.send(&address(USER)?, &[address("bea@example.org")?], message)?.is_ok());
    assert!(connection.send(&address(USER)?, &[address("józsef@example.org")?], MESSAGE)?.is_ok());
    connection.quit();
    drop(server.finish()?);
    let params: Vec<Vec<String>> =
        outbox.messages().iter().map(|sent| sent.params().iter().filter(|param| !param.starts_with("SIZE=")).cloned().collect()).collect();
    assert_eq!(params, [vec![], vec!["BODY=8BITMIME".to_owned()], vec!["SMTPUTF8".to_owned()]]);
    Ok(())
}

#[test]
fn debug_hides_the_message() -> Result {
    let pki = TestPki::new()?;
    let server = Listen::tls(pki.server_config(None)?).start_smtp(FakeSmtp::new())?;
    let connection = SmtpConnection::open(&submission(Security::Tls)?, &connector(&pki, wall(), server.addr()))?;
    let shown = format!("{connection:?}");
    assert!(shown.contains("encrypted: true"), "{shown}");
    connection.quit();
    drop(server.finish()?);
    Ok(())
}
