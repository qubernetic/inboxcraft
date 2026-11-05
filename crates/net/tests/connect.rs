// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Connecting, encrypting and signing in against the fake IMAP server: each certificate fault and each socket fault
//! has its own error kind, and no password goes out before TLS.

mod support;

use std::net::TcpListener;
use std::time::Duration;

use inboxcraft_model::{ErrorKind, Security};
use inboxcraft_net::{Connector, ImapConnection, NetError, Password, Timeouts, TlsSettings, Trust};
use inboxcraft_proto::imap::State;
use inboxcraft_testkit::imap::{Action, Capability, FakeImap, PASSWORD, USER, When};
use inboxcraft_testkit::{CertFault, Listen, Script, TestPki};
use support::{Result, connector, far_off, server, wall};

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

#[test]
fn each_certificate_fault_has_its_own_kind_over_implicit_tls() -> Result {
    let pki = TestPki::new()?;
    let mut kinds = Vec::new();
    for fault in CertFault::ALL {
        let fake = Listen::tls(pki.server_config(Some(fault))?).start_imap(FakeImap::new())?;
        let opened = ImapConnection::open(&server(Security::Tls)?, &connector(&pki, clock_for(fault)?, fake.addr()));
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
        let fake = Listen::plain().start_imap(FakeImap::new().starttls(pki.server_config(Some(fault))?))?;
        let opened = ImapConnection::open(&server(Security::StartTls)?, &connector(&pki, clock_for(fault)?, fake.addr()));
        assert_eq!(kind(opened), Some(expected(fault)), "{fault:?}");
        let (transcript, _refused) = fake.finish_with_transcript();
        assert_eq!(transcript.received(), ["A0001 STARTTLS"], "{fault:?}");
    }
    Ok(())
}

#[test]
fn implicit_tls_signs_in() -> Result {
    let pki = TestPki::new()?;
    let fake = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
    let mut connection = ImapConnection::open(&server(Security::Tls)?, &connector(&pki, wall(), fake.addr()))?;
    assert!(connection.is_encrypted());
    assert_eq!(connection.state(), State::NotAuthenticated);
    assert!(connection.capabilities_known().is_some_and(|caps| caps.has("AUTH=PLAIN")));
    connection.login(USER, &password())?;
    assert_eq!(connection.state(), State::Authenticated);
    assert!(format!("{connection:?}").contains("encrypted: true"));
    connection.logout();
    let transcript = fake.finish()?;
    assert!(transcript.in_clear().is_empty());
    assert!(transcript.received().first().is_some_and(|line| line.starts_with("A0001 AUTHENTICATE PLAIN")), "{:?}", transcript.received());
    assert!(transcript.received().last().is_some_and(|line| line.ends_with("LOGOUT")));
    Ok(())
}

#[test]
fn starttls_comes_before_the_sign_in() -> Result {
    let pki = TestPki::new()?;
    let fake = Listen::plain().start_imap(FakeImap::new().starttls(pki.server_config(None)?))?;
    let mut connection = ImapConnection::open(&server(Security::StartTls)?, &connector(&pki, wall(), fake.addr()))?;
    assert!(connection.is_encrypted());
    connection.login(USER, &password())?;
    connection.logout();
    let transcript = fake.finish()?;
    assert_eq!(transcript.in_clear(), ["A0001 STARTTLS"]);
    assert_eq!(transcript.received().get(1).map(String::as_str), Some("A0002 CAPABILITY"));
    Ok(())
}

#[test]
fn a_server_that_offers_auth_before_starttls_gets_no_password() -> Result {
    let pki = TestPki::new()?;
    // AUTH=PLAIN and LOGIN in clear, and no STARTTLS: nothing is sent at all.
    let fake = Listen::plain().start_imap(FakeImap::new().without(Capability::LoginDisabled))?;
    let opened = ImapConnection::open(&server(Security::StartTls)?, &connector(&pki, wall(), fake.addr()));
    assert_eq!(kind(opened), Some(ErrorKind::StartTlsMissing));
    let (transcript, _ended) = fake.finish_with_transcript();
    assert!(transcript.received().is_empty(), "{:?}", transcript.received());

    // AUTH=PLAIN in clear and STARTTLS: the password goes after the handshake.
    let fake = Listen::plain().start_imap(FakeImap::new().without(Capability::LoginDisabled).starttls(pki.server_config(None)?))?;
    let mut connection = ImapConnection::open(&server(Security::StartTls)?, &connector(&pki, wall(), fake.addr()))?;
    connection.login(USER, &password())?;
    connection.logout();
    let transcript = fake.finish()?;
    assert_eq!(transcript.in_clear(), ["A0001 STARTTLS"]);
    Ok(())
}

#[test]
fn a_refused_starttls_sends_nothing_more() -> Result {
    let pki = TestPki::new()?;
    let script = Script::new().send("* OK [CAPABILITY IMAP4rev1 STARTTLS AUTH=PLAIN] hi").expect("A0001 STARTTLS").send("A0001 NO not now");
    let fake = Listen::plain().start(script)?;
    let opened = ImapConnection::open(&server(Security::StartTls)?, &connector(&pki, wall(), fake.addr()));
    assert_eq!(kind(opened), Some(ErrorKind::StartTlsMissing));
    assert_eq!(fake.finish()?.received(), ["A0001 STARTTLS"]);
    Ok(())
}

#[test]
fn capabilities_are_asked_for_when_the_greeting_has_none() -> Result {
    let pki = TestPki::new()?;
    let script = Script::new().send("* OK hi").expect("A0001 CAPABILITY").send("* CAPABILITY IMAP4rev1 LOGINDISABLED").send("A0001 OK done");
    let fake = Listen::plain().start(script)?;
    let opened = ImapConnection::open(&server(Security::StartTls)?, &connector(&pki, wall(), fake.addr()));
    assert_eq!(kind(opened), Some(ErrorKind::StartTlsMissing));
    fake.finish()?;

    let script = Script::new().send("* OK hi").expect("A0001 CAPABILITY").send("A0001 NO no");
    let fake = Listen::plain().start(script)?;
    let opened = ImapConnection::open(&server(Security::StartTls)?, &connector(&pki, wall(), fake.addr()));
    assert_eq!(kind(opened), Some(ErrorKind::CommandRefused));
    Ok(())
}

#[test]
fn plaintext_injected_after_the_starttls_answer_ends_the_connection() -> Result {
    let pki = TestPki::new()?;
    let fake = FakeImap::new().starttls(pki.server_config(None)?).on(When::answered("STARTTLS"), [Action::Send("* OK injected".to_owned())]);
    let fake = Listen::plain().start_imap(fake)?;
    let opened = ImapConnection::open(&server(Security::StartTls)?, &connector(&pki, wall(), fake.addr()));
    assert!(matches!(kind(opened), Some(ErrorKind::Protocol | ErrorKind::Tls)));
    let (transcript, _ended) = fake.finish_with_transcript();
    assert_eq!(transcript.received(), ["A0001 STARTTLS"]);
    Ok(())
}

#[test]
fn a_wrong_password_is_refused() -> Result {
    let pki = TestPki::new()?;
    let fake = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new().login(USER, "another-invented-password"))?;
    let mut connection = ImapConnection::open(&server(Security::Tls)?, &connector(&pki, wall(), fake.addr()))?;
    let refused = connection.login(USER, &password());
    let error = refused.err().ok_or("signed in")?;
    assert_eq!(error.kind(), ErrorKind::LoginRefused);
    assert!(!format!("{error:?} {error}").contains(PASSWORD));
    assert_eq!(format!("{:?}", password()), "Password(hidden)");
    Ok(())
}

#[test]
fn login_without_auth_plain_uses_login() -> Result {
    let pki = TestPki::new()?;
    let fake = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new().without(Capability::AuthPlain))?;
    let mut connection = ImapConnection::open(&server(Security::Tls)?, &connector(&pki, wall(), fake.addr()))?;
    connection.login(USER, &password())?;
    connection.logout();
    assert!(fake.finish()?.received().first().is_some_and(|line| line.starts_with("A0001 LOGIN")));
    Ok(())
}

#[test]
fn a_server_without_a_login_method_gets_nothing() -> Result {
    let pki = TestPki::new()?;
    let fake = Listen::tls(pki.server_config(None)?).start(Script::new().send("* OK [CAPABILITY IMAP4rev1 LOGINDISABLED] hi"))?;
    let mut connection = ImapConnection::open(&server(Security::Tls)?, &connector(&pki, wall(), fake.addr()))?;
    assert_eq!(kind(connection.login(USER, &password())), Some(ErrorKind::NoLoginMethod));
    assert!(fake.finish()?.received().is_empty());
    Ok(())
}

#[test]
fn preauth_is_taken_only_over_tls() -> Result {
    let pki = TestPki::new()?;
    let fake = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new().preauth())?;
    let mut connection = ImapConnection::open(&server(Security::Tls)?, &connector(&pki, wall(), fake.addr()))?;
    assert_eq!(connection.state(), State::Authenticated);
    connection.login(USER, &password())?;
    connection.logout();
    assert!(fake.finish()?.received().iter().all(|line| !line.contains("LOGIN") && !line.contains("AUTHENTICATE")));

    let fake = Listen::plain().start_imap(FakeImap::new().preauth())?;
    let opened = ImapConnection::open(&server(Security::StartTls)?, &connector(&pki, wall(), fake.addr()));
    assert_eq!(kind(opened), Some(ErrorKind::Protocol));
    Ok(())
}

#[test]
fn socket_faults_have_their_kinds() -> Result {
    let pki = TestPki::new()?;
    let closed = TcpListener::bind("127.0.0.1:0")?.local_addr()?;
    assert_eq!(kind(ImapConnection::open(&server(Security::Tls)?, &connector(&pki, wall(), closed))), Some(ErrorKind::ConnectionRefused));

    let silent = TcpListener::bind("127.0.0.1:0")?;
    let short = Timeouts { connect: Duration::from_secs(5), io: Duration::from_millis(200) };
    let quiet = connector(&pki, wall(), silent.local_addr()?).with_timeouts(short);
    assert_eq!(kind(ImapConnection::open(&server(Security::Tls)?, &quiet)), Some(ErrorKind::TimedOut));
    assert_eq!(quiet.timeouts(), short);

    let fake = Listen::plain().start(Script::new().drop_connection())?;
    assert_eq!(kind(ImapConnection::open(&server(Security::StartTls)?, &connector(&pki, wall(), fake.addr()))), Some(ErrorKind::ConnectionDropped));

    let fake = Listen::plain().start(Script::new().garbage(b"\x00\x01 not imap\r\n"))?;
    assert_eq!(kind(ImapConnection::open(&server(Security::StartTls)?, &connector(&pki, wall(), fake.addr()))), Some(ErrorKind::Protocol));

    let nowhere = Connector::new(TlsSettings::new(Trust::Roots(vec![pki.ca()]), wall())).with_addresses(Vec::new());
    assert_eq!(kind(ImapConnection::open(&server(Security::Tls)?, &nowhere)), Some(ErrorKind::HostNotFound));
    Ok(())
}
