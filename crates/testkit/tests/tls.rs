// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The implicit TLS listener, the test CA and the certificate faults.

use std::collections::HashSet;
use std::time::Duration;

use inboxcraft_testkit::{CertFault, Clock, Error, ErrorKind, FAR_OFF, Listen, Script, TestClient, TestPki};

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn a_valid_chain_carries_the_script() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let script = Script::new().send("* OK fake ready").expect("a1 LOGOUT").send("a1 OK bye");
    let server = Listen::tls(pki.server_config(None)?).start(script)?;
    let mut client = TestClient::tls(server.addr(), &pki.client(Clock::wall())?, TIMEOUT)?;
    assert_eq!(client.read_line()?, "* OK fake ready");
    client.send_line("a1 LOGOUT")?;
    assert_eq!(client.read_line()?, "a1 OK bye");
    assert_eq!(client.read_line().map_err(|err| err.kind()), Err(ErrorKind::Dropped));
    assert_eq!(server.finish()?.received(), ["a1 LOGOUT"]);
    Ok(())
}

fn connect_error(pki: &TestPki, fault: Option<CertFault>, clock: Clock) -> Result<ErrorKind, Box<dyn std::error::Error>> {
    let server = Listen::tls(pki.server_config(fault)?).start(Script::new().send("* OK fake ready"))?;
    let err = TestClient::tls(server.addr(), &pki.client(clock)?, TIMEOUT).err().ok_or("the handshake passed")?;
    assert!(!err.to_string().is_empty());
    let server_side = server.finish().err().ok_or("the server played its script")?;
    assert!(matches!(server_side, Error::Io(_)), "{server_side}");
    Ok(err.kind())
}

#[test]
fn each_certificate_fault_gives_its_own_error_kind() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let mut kinds = Vec::new();
    for fault in CertFault::ALL {
        kinds.push(connect_error(&pki, Some(fault), fault.clock())?);
    }
    assert_eq!(kinds, [ErrorKind::UnknownCa, ErrorKind::Expired, ErrorKind::WrongHost, ErrorKind::ClockFarOff]);
    let others = [ErrorKind::Dropped, ErrorKind::Timeout, ErrorKind::Garbage, ErrorKind::Tls, ErrorKind::Io];
    let distinct: HashSet<ErrorKind> = kinds.iter().chain(&others).copied().collect();
    assert_eq!(distinct.len(), kinds.len() + others.len());
    Ok(())
}

#[test]
fn a_clock_far_in_the_past_is_far_off_too() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    assert_eq!(connect_error(&pki, None, Clock::offset(-FAR_OFF))?, ErrorKind::ClockFarOff);
    Ok(())
}

#[test]
fn an_expired_certificate_seen_by_a_clock_far_off_names_the_clock() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    assert_eq!(connect_error(&pki, Some(CertFault::Expired), Clock::offset(FAR_OFF))?, ErrorKind::ClockFarOff);
    Ok(())
}

#[test]
fn a_certificate_not_yet_valid_at_a_clock_near_the_wall_is_a_tls_error() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    assert_eq!(connect_error(&pki, None, Clock::offset(-3600 * 3))?, ErrorKind::Tls);
    Ok(())
}

#[test]
fn a_clock_before_the_epoch_is_a_tls_error() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    assert_eq!(connect_error(&pki, None, Clock::offset(i64::MIN))?, ErrorKind::Tls);
    Ok(())
}

#[test]
fn a_host_that_is_not_a_dns_name_is_a_tls_error() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let server = Listen::tls(pki.server_config(None)?).accept_timeout(Duration::from_millis(20)).start(Script::new())?;
    let err = TestClient::tls(server.addr(), &pki.client(Clock::wall())?.host("not a host!"), TIMEOUT).err();
    assert_eq!(err.map(|err| err.kind()), Some(ErrorKind::Tls));
    Ok(())
}

#[test]
fn another_host_name_must_match_the_certificate() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let server = Listen::tls(pki.server_config(Some(CertFault::WrongHost))?).start(Script::new().send("* OK"))?;
    let mut client = TestClient::tls(server.addr(), &pki.client(Clock::wall())?.host(inboxcraft_testkit::WRONG_HOST), TIMEOUT)?;
    assert_eq!(client.read_line()?, "* OK");
    server.finish()?;
    Ok(())
}

fn tls_fault(script: Script, timeout: Duration) -> Result<ErrorKind, Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let server = Listen::tls(pki.server_config(None)?).start(Script::new().send("* OK fake ready").then(script))?;
    let mut client = TestClient::tls(server.addr(), &pki.client(Clock::wall())?, timeout)?;
    assert_eq!(client.read_line()?, "* OK fake ready");
    Ok(client.read_line().err().ok_or("the second line arrived")?.kind())
}

#[test]
fn the_line_faults_work_over_tls() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(tls_fault(Script::new().drop_connection(), TIMEOUT)?, ErrorKind::Dropped);
    assert_eq!(tls_fault(Script::new().delay(Duration::from_secs(2)), Duration::from_millis(100))?, ErrorKind::Timeout);
    assert_eq!(tls_fault(Script::new().garbage(b"\xff\x00\r\n"), TIMEOUT)?, ErrorKind::Garbage);
    Ok(())
}

#[test]
fn a_tls_client_against_a_plain_server_is_a_tls_error() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let server = Listen::plain().start(Script::new().send("* OK this is not TLS"))?;
    let err = TestClient::tls(server.addr(), &pki.client(Clock::wall())?, TIMEOUT).err();
    assert_eq!(err.map(|err| err.kind()), Some(ErrorKind::Tls));
    Ok(())
}

#[test]
fn a_plain_client_against_a_tls_server_fails_the_script() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let server = Listen::tls(pki.server_config(None)?).start(Script::new().expect("a1 HELLO"))?;
    let mut client = TestClient::plain(server.addr(), TIMEOUT)?;
    client.send_line("a1 HELLO")?;
    assert!(server.finish().is_err());
    Ok(())
}

#[test]
fn a_tls_client_to_a_closed_port_is_an_io_error() -> Result<(), Box<dyn std::error::Error>> {
    let pki = TestPki::new()?;
    let addr = std::net::TcpListener::bind("127.0.0.1:0")?.local_addr()?;
    let err = TestClient::tls(addr, &pki.client(Clock::wall())?, TIMEOUT).err();
    assert_eq!(err.map(|err| err.kind()), Some(ErrorKind::Io));
    Ok(())
}

#[test]
fn only_a_clock_more_than_a_day_away_is_far_off() {
    assert!(!Clock::wall().is_far_off());
    assert!(!Clock::offset(86_400).is_far_off());
    assert!(Clock::offset(86_401).is_far_off());
    assert!(Clock::offset(i64::MIN).is_far_off());
    assert!(CertFault::ClockFarOff.clock().is_far_off());
    assert!(CertFault::ALL.iter().filter(|fault| fault.clock().is_far_off()).eq([&CertFault::ClockFarOff]));
}

#[test]
fn the_tls_errors_name_their_cause() {
    let tls = Error::from(rustls::Error::General("refused".to_owned()));
    assert_eq!(tls.to_string(), "TLS configuration error: unexpected error: refused");
    assert!(std::error::Error::source(&tls).is_some());
    let certificate = Error::from(rcgen::Error::CouldNotParseCertificate);
    assert!(certificate.to_string().starts_with("test certificate error: "));
    assert!(std::error::Error::source(&certificate).is_some());
}
