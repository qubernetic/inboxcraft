// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The client against the fake CalDAV server over a socket: discovery, sync with tokens and without, PUT and DELETE
//! with conflicts, and Basic auth that never goes in the clear.

use std::time::Duration;

use inboxcraft_dav::{
    Changes, Credentials, DavError, Deleted, Discovery, Precondition, Request, Response, ResponseReader, Sync, Written, delete, delete_outcome, put,
    put_outcome,
};
use inboxcraft_testkit::dav::{FakeDav, PASSWORD, USER, WORK};
use inboxcraft_testkit::{Clock, FakeServer, Listen, TestClient, TestPki};

const WAIT: Duration = Duration::from_secs(5);
const EVENT: &str = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:1@example.org\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

/// The server and a client connected to `fake` over TLS; bound in this order, the client is dropped first, so the
/// server does not wait for it to go.
fn connect(fake: FakeDav) -> (FakeServer, TestClient) {
    let pki = TestPki::new().unwrap();
    let server = Listen::tls(pki.server_config(None).unwrap()).start_dav(fake).unwrap();
    let client = TestClient::tls(server.addr(), &pki.client(Clock::wall()).unwrap(), WAIT).unwrap();
    (server, client)
}

fn credentials() -> Credentials {
    Credentials::new(USER, PASSWORD).unwrap()
}

/// Sends `request` and reads the answer.
fn exchange(client: &mut TestClient, request: &Request, credentials: Option<&Credentials>) -> Response {
    client.send_bytes(&request.encode(credentials).unwrap()).unwrap();
    let mut reader = ResponseReader::new();
    loop {
        if let Some(response) = reader.receive(&client.read_some().unwrap()).unwrap() {
            return response;
        }
    }
}

fn discover(client: &mut TestClient, credentials: Option<&Credentials>) -> Result<inboxcraft_dav::Account, DavError> {
    let mut discovery = Discovery::new("https://dav.example.org")?;
    loop {
        let request = discovery.request()?;
        if let Some(account) = discovery.receive(&exchange(client, &request, credentials))? {
            return Ok(account);
        }
    }
}

fn sync(client: &mut TestClient, token: Option<&str>, known: &[(String, String)]) -> Changes {
    let origin = inboxcraft_dav::Origin::parse("https://dav.example.org/").unwrap().0;
    let mut sync = Sync::new(&origin, WORK, token, known.to_vec()).unwrap();
    loop {
        let request = sync.request().unwrap();
        if let Some(changes) = sync.receive(&exchange(client, &request, Some(&credentials()))).unwrap() {
            return changes;
        }
    }
}

#[test]
fn discovery_finds_the_work_calendar() {
    let (server, mut client) = connect(FakeDav::new());
    let account = discover(&mut client, Some(&credentials())).unwrap();
    assert_eq!((account.principal.as_str(), account.home.as_str()), ("/principals/ada/", "/calendars/ada/"));
    assert_eq!(account.calendars.len(), 1);
    let work = &account.calendars[0];
    assert_eq!((work.href.as_str(), work.name.as_deref()), (WORK, Some("Work")));
    assert!(work.holds_events() && work.sync_token.is_some() && work.ctag.is_some());
    drop(client);
    let transcript = server.finish().unwrap();
    assert!(transcript.in_clear().is_empty());
}

#[test]
fn a_wrong_password_is_unauthorized() {
    let (_server, mut client) = connect(FakeDav::new().login(USER, "another-password"));
    assert_eq!(discover(&mut client, Some(&credentials())), Err(DavError::Unauthorized));
}

#[test]
fn sync_put_and_delete_keep_the_calendar_in_step() {
    let fake = FakeDav::new();
    let calendars = fake.calendars();
    let elsewhere = format!("{WORK}elsewhere.ics");
    calendars.put(&elsewhere, EVENT).unwrap();
    let (_server, mut client) = connect(fake);

    let first = sync(&mut client, None, &[]);
    assert!(first.full);
    assert_eq!(first.changed.iter().map(|resource| resource.href.as_str()).collect::<Vec<_>>(), [elsewhere.as_str()]);
    assert_eq!(first.changed[0].data, EVENT);
    let mut known: Vec<(String, String)> = first.changed.iter().map(|resource| (resource.href.clone(), resource.etag.clone().unwrap())).collect();

    let mine = format!("{WORK}mine.ics");
    let origin = inboxcraft_dav::Origin::parse("https://dav.example.org/").unwrap().0;
    let created = put_outcome(&exchange(&mut client, &put(&origin, &mine, EVENT, &Precondition::Create).unwrap(), Some(&credentials()))).unwrap();
    let Written::Stored { etag: Some(etag) } = created else { panic!("{created:?}") };
    assert_eq!(calendars.get(&mine).map(|(stored, _)| stored), Some(etag.clone()));
    let again = put_outcome(&exchange(&mut client, &put(&origin, &mine, EVENT, &Precondition::Create).unwrap(), Some(&credentials()))).unwrap();
    assert_eq!(again, Written::Conflict);
    known.push((mine.clone(), etag.clone()));

    calendars.put(&mine, &EVENT.replace("UID:1", "UID:2")).unwrap();
    let stale = put_outcome(&exchange(&mut client, &put(&origin, &mine, EVENT, &Precondition::Replace(etag.clone())).unwrap(), Some(&credentials())))
        .unwrap();
    assert_eq!(stale, Written::Conflict, "a change made elsewhere is never overwritten");
    assert!(calendars.remove(&elsewhere));

    let second = sync(&mut client, first.token.as_deref(), &known);
    assert!(!second.full);
    assert_eq!(second.removed, std::slice::from_ref(&elsewhere));
    assert_eq!(second.changed.len(), 1);
    assert!(second.changed[0].data.contains("UID:2"));
    let fresh = second.changed[0].etag.clone().unwrap();

    let conflict = delete_outcome(&exchange(&mut client, &delete(&origin, &mine, Some(&etag)).unwrap(), Some(&credentials()))).unwrap();
    assert_eq!(conflict, Deleted::Conflict);
    let removed = delete_outcome(&exchange(&mut client, &delete(&origin, &mine, Some(&fresh)).unwrap(), Some(&credentials()))).unwrap();
    assert_eq!(removed, Deleted::Removed);
    let gone = delete_outcome(&exchange(&mut client, &delete(&origin, &mine, None).unwrap(), Some(&credentials()))).unwrap();
    assert_eq!(gone, Deleted::Gone);
    assert!(calendars.hrefs(WORK).is_empty());
}

#[test]
fn a_forgotten_token_and_a_server_without_sync_still_bring_the_calendar_in_step() {
    let fake = FakeDav::new();
    let calendars = fake.calendars();
    let (a, b) = (format!("{WORK}a.ics"), format!("{WORK}b.ics"));
    let etag = calendars.put(&a, EVENT).unwrap();
    let (_server, mut client) = connect(fake);
    let token = sync(&mut client, None, &[]).token.unwrap();
    calendars.forget_tokens();
    calendars.put(&b, EVENT).unwrap();
    let known = [(a.clone(), etag.clone()), (format!("{WORK}lost.ics"), "\"x\"".to_owned())];
    let restarted = sync(&mut client, Some(&token), &known);
    assert!(restarted.full);
    assert_eq!(restarted.changed.iter().map(|resource| resource.href.as_str()).collect::<Vec<_>>(), [b.as_str()]);
    assert_eq!(restarted.removed, [format!("{WORK}lost.ics")]);

    let fake = FakeDav::new().without_sync();
    fake.calendars().put(&a, EVENT).unwrap();
    let (_server, mut client) = connect(fake);
    let listed = sync(&mut client, Some(&token), &[]);
    assert!(listed.full && listed.token.is_none());
    assert_eq!(listed.changed.len(), 1);
}

#[test]
fn a_paged_server_is_followed_to_the_end() {
    let fake = FakeDav::new().page(2);
    for n in 0..5 {
        fake.calendars().put(&format!("{WORK}{n}.ics"), EVENT).unwrap();
    }
    let token = fake.calendars().token(WORK);
    let (_server, mut client) = connect(fake);
    let changes = sync(&mut client, None, &[]);
    assert_eq!(changes.changed.len(), 5);
    assert_eq!(changes.token, token);
}

#[test]
fn basic_auth_never_goes_in_the_clear() {
    for fake in [FakeDav::new(), FakeDav::new().auth_in_clear()] {
        let server = Listen::plain().start_dav(fake).unwrap();
        let mut client = TestClient::plain(server.addr(), WAIT).unwrap();
        let discovery = Discovery::new("http://dav.example.org/principals/ada/").unwrap();
        let request = discovery.request().unwrap();
        assert_eq!(request.encode(Some(&credentials())), Err(DavError::NotEncrypted));
        let origin = inboxcraft_dav::Origin::parse("http://dav.example.org/").unwrap().0;
        assert_eq!(put(&origin, "/e.ics", EVENT, &Precondition::Create).unwrap().encode(Some(&credentials())), Err(DavError::NotEncrypted));
        let mut discovery = discovery;
        let answer = exchange(&mut client, &request, None);
        let outcome = discovery.receive(&answer);
        assert!(matches!(outcome, Err(DavError::Refused { status: 403 }) | Err(DavError::Unauthorized)), "{outcome:?}");
        drop(client);
        let transcript = server.finish().unwrap();
        assert!(!transcript.in_clear().is_empty());
        assert!(transcript.received().iter().all(|line| !line.to_ascii_lowercase().starts_with("authorization")), "{:?}", transcript.received());
    }
}
