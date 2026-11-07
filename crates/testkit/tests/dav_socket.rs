// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake CalDAV server on a socket: TLS only for the password, kept-alive connections, scripted answers and
//! faults, several clients one after the other.

use std::time::Duration;

use inboxcraft_testkit::dav::{Action, FakeDav, HOME, Reply, WORK, When};
use inboxcraft_testkit::{Clock, ErrorKind, Fault, Listen, TestClient, TestPki};

const AUTH: &str = "Authorization: Basic YWRhQGV4YW1wbGUub3JnOm5vdC1hLXJlYWwtcGFzc3dvcmQ=";
const WAIT: Duration = Duration::from_secs(5);

fn propfind(auth: bool, close: bool) -> String {
    let auth = if auth { format!("{AUTH}\r\n") } else { String::new() };
    let close = if close { "Connection: close\r\n" } else { "" };
    format!("PROPFIND {HOME} HTTP/1.1\r\nHost: dav.example.org\r\n{auth}{close}Depth: 0\r\n\r\n")
}

/// Reads one whole answer: its head and a body of its Content-Length.
fn answer(client: &mut TestClient) -> String {
    let mut text = String::new();
    loop {
        if let Some((head, body)) = text.split_once("\r\n\r\n") {
            let len: usize = head.lines().find_map(|line| line.strip_prefix("Content-Length: ")).unwrap().parse().unwrap();
            if body.len() >= len {
                return text;
            }
        }
        text.push_str(&String::from_utf8_lossy(&client.read_some().unwrap()));
    }
}

#[test]
fn over_tls_the_password_is_taken_and_the_connection_kept() {
    let pki = TestPki::new().unwrap();
    let server = Listen::tls(pki.server_config(None).unwrap()).start_dav(FakeDav::new()).unwrap();
    let mut client = TestClient::tls(server.addr(), &pki.client(Clock::wall()).unwrap(), WAIT).unwrap();
    client.send_bytes(propfind(false, false).as_bytes()).unwrap();
    assert!(answer(&mut client).starts_with("HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic"));
    client.send_bytes(propfind(true, true).as_bytes()).unwrap();
    assert_eq!(client.read_line().unwrap(), "HTTP/1.1 207 Multi-Status");
    let rest = answer(&mut client);
    assert!(rest.starts_with("Content-Type: application/xml"), "{rest}");
    assert_eq!(client.read_some().map_err(|err| err.kind()), Err(ErrorKind::Dropped));
    let transcript = server.finish().unwrap();
    assert!(transcript.in_clear().is_empty());
    assert!(transcript.received().contains(&AUTH.to_owned()));
    assert_eq!(transcript.received().first().map(String::as_str), Some("PROPFIND /calendars/ada/ HTTP/1.1"));
}

#[test]
fn over_plain_tcp_it_never_asks_for_a_password() {
    let server = Listen::plain().start_dav(FakeDav::new()).unwrap();
    let mut client = TestClient::plain(server.addr(), WAIT).unwrap();
    client.send_bytes(propfind(false, true).as_bytes()).unwrap();
    let refused = answer(&mut client);
    assert!(refused.starts_with("HTTP/1.1 403 Forbidden\r\n"), "{refused}");
    assert!(!refused.contains("WWW-Authenticate"));
    let transcript = server.finish().unwrap();
    assert_eq!(transcript.in_clear().len(), transcript.received().len());
    assert!(transcript.in_clear().iter().all(|line| !line.starts_with("Authorization")));
}

#[test]
fn a_careless_server_asks_for_the_password_in_clear() {
    let server = Listen::plain().start_dav(FakeDav::new().auth_in_clear()).unwrap();
    let mut client = TestClient::plain(server.addr(), WAIT).unwrap();
    client.send_bytes(propfind(false, true).as_bytes()).unwrap();
    assert!(answer(&mut client).starts_with("HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic"));
    server.finish().unwrap();
}

#[test]
fn a_client_that_closes_between_requests_ends_the_session_quietly() {
    let pki = TestPki::new().unwrap();
    let server = Listen::tls(pki.server_config(None).unwrap()).start_dav(FakeDav::new()).unwrap();
    let mut client = TestClient::tls(server.addr(), &pki.client(Clock::wall()).unwrap(), WAIT).unwrap();
    client.send_bytes(propfind(true, false).as_bytes()).unwrap();
    assert!(answer(&mut client).starts_with("HTTP/1.1 207"));
    drop(client);
    assert_eq!(server.finish().unwrap().received().len(), 5);
}

#[test]
fn scripted_answers_and_faults_play_once_at_their_request() {
    let teapot = Reply::new(418).field("X-Why", "a\r\nb").body(b"short and stout".to_vec());
    let fake = FakeDav::new()
        .reply(When::at("propfind", HOME), teapot)
        .on(When::method("PROPFIND"), [Action::Fault(Fault::Delay(Duration::from_millis(10))), Action::Fault(Fault::Garbage(b"junk".to_vec()))])
        .on(When::method("GET"), [Action::Fault(Fault::Drop)]);
    let pki = TestPki::new().unwrap();
    let server = Listen::tls(pki.server_config(None).unwrap()).start_dav(fake).unwrap();
    let mut client = TestClient::tls(server.addr(), &pki.client(Clock::wall()).unwrap(), WAIT).unwrap();
    client.send_bytes(propfind(true, false).as_bytes()).unwrap();
    let scripted = answer(&mut client);
    assert_eq!(scripted, "HTTP/1.1 418 Status\r\nX-Why: ab\r\nContent-Length: 15\r\n\r\nshort and stout");
    client.send_bytes(propfind(true, false).as_bytes()).unwrap();
    let after_garbage = answer(&mut client);
    assert!(after_garbage.starts_with("junkHTTP/1.1 207"), "{after_garbage}");
    client.send_bytes(propfind(true, false).as_bytes()).unwrap();
    assert!(answer(&mut client).starts_with("HTTP/1.1 207"));
    client.send_bytes(format!("GET {WORK}e.ics HTTP/1.1\r\n{AUTH}\r\n\r\n").as_bytes()).unwrap();
    assert_eq!(client.read_some().map_err(|err| err.kind()), Err(ErrorKind::Dropped));
    assert!(server.finish().is_ok());
}

#[test]
fn a_server_stopped_while_it_waits_ends_the_session() {
    let fake = FakeDav::new().on(When::method("PROPFIND"), [Action::Fault(Fault::Delay(Duration::from_secs(30)))]);
    let pki = TestPki::new().unwrap();
    let server = Listen::tls(pki.server_config(None).unwrap()).start_dav(fake).unwrap();
    let mut client = TestClient::tls(server.addr(), &pki.client(Clock::wall()).unwrap(), WAIT).unwrap();
    client.send_bytes(propfind(true, false).as_bytes()).unwrap();
    server.wait_received(|lines| lines.len() >= 4, WAIT).unwrap();
    drop(server);
}

#[test]
fn several_clients_share_the_calendars_and_the_rules_carry_over() {
    let fake = FakeDav::new().reply(When::method("PUT"), Reply::new(507));
    let calendars = fake.calendars();
    let pki = TestPki::new().unwrap();
    let server = Listen::tls(pki.server_config(None).unwrap()).accept_timeout(Duration::from_millis(500)).start_dav_many(fake, 3).unwrap();
    let event = "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n";
    let put = format!("PUT {WORK}e.ics HTTP/1.1\r\n{AUTH}\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{event}", event.len());
    for status in ["HTTP/1.1 507 Status", "HTTP/1.1 201 Created"] {
        let mut client = TestClient::tls(server.addr(), &pki.client(Clock::wall()).unwrap(), WAIT).unwrap();
        client.send_bytes(put.as_bytes()).unwrap();
        assert!(answer(&mut client).starts_with(status), "{status}");
    }
    assert_eq!(calendars.hrefs(WORK), [format!("{WORK}e.ics")]);
    let transcript = server.finish().unwrap();
    assert_eq!(transcript.received().iter().filter(|line| line.starts_with("PUT ")).count(), 2);
}
