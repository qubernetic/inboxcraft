// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The account worker's calendar jobs against the fake CalDAV server: discovery from the account's address, a first
//! sync and the next from its token, a server without the sync report, PUT and DELETE with entity tags and their
//! conflicts (412), the faults of a connection, and Basic auth that goes over verified TLS only, and not again once
//! refused.

mod support;

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use inboxcraft_model::{
    Account, AccountId, CalendarEventId, CalendarId, CalendarJob, CalendarListing, CalendarReport, CheckStep, EmailAddress, ErrorKind, Event,
    EventKind, FetchedEvent, Job, JobId, JobKind, KnownEvent, Security, StepStatus,
};
use inboxcraft_net::{Credentials, Password, Worker, WorkerSettings};
use inboxcraft_testkit::dav::{Action, FakeDav, HOME, PASSWORD, Reply, USER, WORK, When};
use inboxcraft_testkit::imap::FakeImap;
use inboxcraft_testkit::{CertFault, FakeServer, Fault, HOST, Listen, TestPki};
use support::{Result, connector, far_off, server, wall};

const ACCOUNT: AccountId = AccountId::new(1);
const CALENDAR: CalendarId = CalendarId::new(3);
const EVENT_ID: CalendarEventId = CalendarEventId::new(9);
const WAIT: Duration = Duration::from_secs(10);
const LUNCH: &str = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:lunch@example.org\r\nSUMMARY:Lunch with Bea\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
const TEA: &str = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:tea@example.org\r\nSUMMARY:Tea with Cy\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

/// The fake's address as the account has it: the certificate's host, over `https`.
fn dav() -> String {
    format!("https://{HOST}/")
}

/// A path on the fake as a URL.
fn url(path: &str) -> String {
    format!("https://{HOST}{path}")
}

fn work() -> String {
    url(WORK)
}

fn event_url(name: &str) -> String {
    url(&format!("{WORK}{name}"))
}

/// A password, or none, counting how often it was asked for.
struct Fixed(Option<&'static str>, AtomicUsize);

impl Credentials for Fixed {
    fn password(&self, _account: &Account) -> Option<Password> {
        self.1.fetch_add(1, Ordering::SeqCst);
        self.0.map(|text| Password::new(text.to_owned()))
    }
}

/// A worker whose account has its CalDAV server at `caldav`; every connection goes to `addr`.
struct Calendars {
    worker: Worker,
    events: Receiver<Event>,
    credentials: Arc<Fixed>,
    next: u64,
}

impl Calendars {
    fn start(pki: &TestPki, addr: SocketAddr, caldav: Option<&str>, password: Option<&'static str>) -> Result<Self> {
        Self::with_clock(pki, addr, caldav, password, wall())
    }

    fn with_clock(
        pki: &TestPki,
        addr: SocketAddr,
        caldav: Option<&str>,
        password: Option<&'static str>,
        clock: Arc<dyn inboxcraft_time::Clock>,
    ) -> Result<Self> {
        let account = Account::new(ACCOUNT, "Ada", EmailAddress::new(USER)?, USER, server(Security::Tls)?)?.with_caldav(caldav)?;
        let (sender, events) = mpsc::channel();
        let credentials = Arc::new(Fixed(password, AtomicUsize::new(0)));
        let worker =
            Worker::spawn(account, WorkerSettings::new(connector(pki, clock, addr)), Arc::clone(&credentials) as Arc<dyn Credentials>, sender)?;
        Ok(Self { worker, events, credentials, next: 0 })
    }

    /// Runs `job` and returns its report; any other event is an error.
    fn run(&mut self, job: CalendarJob) -> Result<CalendarReport> {
        self.next += 1;
        let id = JobId::new(self.next);
        self.worker.send(Job::new(id, ACCOUNT, JobKind::Calendar(job))).map_err(|_| "job refused")?;
        let event = self.events.recv_timeout(WAIT)?;
        assert_eq!(event.job, Some(id));
        match event.kind {
            EventKind::Calendar(report) => Ok(report),
            other => Err(format!("not a calendar report: {other:?}").into()),
        }
    }

    fn asked(&self) -> usize {
        self.credentials.1.load(Ordering::SeqCst)
    }
}

/// `fake` over TLS for up to `connections` clients, one after the other.
fn serve(pki: &TestPki, fake: FakeDav, connections: usize) -> Result<FakeServer> {
    Ok(Listen::tls(pki.server_config(None)?).accept_timeout(Duration::from_millis(500)).start_dav_many(fake, connections)?)
}

fn sync(token: Option<String>, known: Vec<KnownEvent>) -> CalendarJob {
    CalendarJob::Sync { calendar: CALENDAR, url: work(), token, known }
}

fn put(name: &str, etag: Option<&str>, ical: &str) -> CalendarJob {
    CalendarJob::Put { calendar: CALENDAR, event: EVENT_ID, url: event_url(name), etag: etag.map(str::to_owned), ical: ical.to_owned() }
}

fn delete(name: &str, etag: Option<&str>) -> CalendarJob {
    CalendarJob::Delete { calendar: CALENDAR, event: EVENT_ID, url: event_url(name), etag: etag.map(str::to_owned) }
}

fn failed(kind: ErrorKind) -> CalendarReport {
    CalendarReport::Failed { calendar: Some(CALENDAR), kind }
}

/// The request lines (`PROPFIND /path HTTP/1.1`) the fake got.
fn requests(lines: &[String]) -> Vec<String> {
    lines.iter().filter(|line| line.ends_with(" HTTP/1.1")).cloned().collect()
}

fn authorizations(lines: &[String]) -> usize {
    lines.iter().filter(|line| line.to_ascii_lowercase().starts_with("authorization: basic ")).count()
}

#[test]
fn discovery_finds_the_calendars_from_the_account_address() -> Result {
    let pki = TestPki::new()?;
    let fake = FakeDav::new();
    let token = fake.calendars().token(WORK);
    let server = serve(&pki, fake, 1)?;
    let mut calendars = Calendars::start(&pki, server.addr(), Some(&dav()), Some(PASSWORD))?;
    let report = calendars.run(CalendarJob::Discover { url: None })?;
    let work = CalendarListing {
        url: work(),
        name: Some("Work".to_owned()),
        colour: Some("#3366CCFF".to_owned()),
        ctag: report_ctag(&report),
        sync_token: token,
        events: true,
    };
    assert_eq!(report, CalendarReport::Found { home: url(HOME), calendars: vec![work] });
    assert_eq!(calendars.asked(), 1);
    drop(calendars);
    let transcript = server.finish()?;
    let requests = requests(transcript.received());
    assert_eq!(requests.first().map(String::as_str), Some("PROPFIND /.well-known/caldav HTTP/1.1"));
    assert_eq!(authorizations(transcript.received()), requests.len(), "every request carries the password, over TLS");
    assert!(transcript.in_clear().is_empty());
    Ok(())
}

fn report_ctag(report: &CalendarReport) -> Option<String> {
    match report {
        CalendarReport::Found { calendars, .. } => calendars.first().and_then(|calendar| calendar.ctag.clone()),
        _ => None,
    }
}

#[test]
fn a_discovery_job_may_name_where_to_start() -> Result {
    let pki = TestPki::new()?;
    let server = serve(&pki, FakeDav::new().without_well_known(), 1)?;
    let mut calendars = Calendars::start(&pki, server.addr(), None, Some(PASSWORD))?;
    let report = calendars.run(CalendarJob::Discover { url: Some(dav()) })?;
    assert!(matches!(&report, CalendarReport::Found { calendars, .. } if calendars.len() == 1), "{report:?}");
    Ok(())
}

#[test]
fn without_an_address_no_password_is_asked_for() -> Result {
    let pki = TestPki::new()?;
    let unused: SocketAddr = ([127, 0, 0, 1], 9).into();
    let mut calendars = Calendars::start(&pki, unused, None, Some(PASSWORD))?;
    assert_eq!(calendars.run(CalendarJob::Discover { url: None })?, CalendarReport::Failed { calendar: None, kind: ErrorKind::NoCalendarServer });
    assert_eq!(calendars.asked(), 0);
    Ok(())
}

#[test]
fn without_a_stored_password_nothing_is_sent() -> Result {
    let pki = TestPki::new()?;
    let server = serve(&pki, FakeDav::new(), 1)?;
    let mut calendars = Calendars::start(&pki, server.addr(), Some(&dav()), None)?;
    assert_eq!(calendars.run(sync(None, Vec::new()))?, failed(ErrorKind::CredentialMissing));
    drop(calendars);
    let (transcript, _no_client) = server.finish_with_transcript();
    assert!(transcript.received().is_empty());
    Ok(())
}

#[test]
fn a_first_sync_brings_every_event_and_the_next_only_what_changed() -> Result {
    let pki = TestPki::new()?;
    let fake = FakeDav::new();
    let store = fake.calendars();
    let lunch_tag = store.put(&format!("{WORK}lunch.ics"), LUNCH).ok_or("put")?;
    let tea_tag = store.put(&format!("{WORK}tea.ics"), TEA).ok_or("put")?;
    let server = serve(&pki, fake, 2)?;
    let mut calendars = Calendars::start(&pki, server.addr(), Some(&dav()), Some(PASSWORD))?;
    let CalendarReport::Synced { calendar, changed, removed, token, full } = calendars.run(sync(None, Vec::new()))? else {
        return Err("no sync".into());
    };
    assert_eq!((calendar, full, removed), (CALENDAR, true, Vec::<String>::new()));
    assert_eq!(
        changed,
        [
            FetchedEvent { url: event_url("lunch.ics"), etag: Some(lunch_tag.clone()), ical: LUNCH.to_owned() },
            FetchedEvent { url: event_url("tea.ics"), etag: Some(tea_tag.clone()), ical: TEA.to_owned() },
        ]
    );
    assert_eq!(token, store.token(WORK));

    // Elsewhere: the lunch goes, the tea changes, a dinner comes.
    store.remove(&format!("{WORK}lunch.ics"));
    let tea_tag_now = store.put(&format!("{WORK}tea.ics"), &LUNCH.replace("lunch", "tea")).ok_or("put")?;
    let dinner_tag = store.put(&format!("{WORK}dinner.ics"), &TEA.replace("tea", "dinner")).ok_or("put")?;
    let known = vec![KnownEvent { url: event_url("lunch.ics"), etag: lunch_tag }, KnownEvent { url: event_url("tea.ics"), etag: tea_tag }];
    let CalendarReport::Synced { changed, removed, token: next, full, .. } = calendars.run(sync(token, known))? else {
        return Err("no second sync".into());
    };
    assert!(!full);
    assert_eq!(removed, [event_url("lunch.ics")]);
    let fetched: Vec<(String, Option<String>)> = changed.into_iter().map(|event| (event.url, event.etag)).collect();
    assert_eq!(fetched, [(event_url("dinner.ics"), Some(dinner_tag)), (event_url("tea.ics"), Some(tea_tag_now))]);
    assert_eq!(next, store.token(WORK));
    Ok(())
}

#[test]
fn a_server_without_the_sync_report_is_compared_by_entity_tags() -> Result {
    let pki = TestPki::new()?;
    let fake = FakeDav::new().without_sync();
    let tag = fake.calendars().put(&format!("{WORK}lunch.ics"), LUNCH).ok_or("put")?;
    fake.calendars().put(&format!("{WORK}tea.ics"), TEA).ok_or("put")?;
    let server = serve(&pki, fake, 1)?;
    let mut calendars = Calendars::start(&pki, server.addr(), Some(&dav()), Some(PASSWORD))?;
    let known = vec![KnownEvent { url: event_url("lunch.ics"), etag: tag }, KnownEvent { url: event_url("gone.ics"), etag: "\"0\"".to_owned() }];
    let CalendarReport::Synced { changed, removed, token, full, .. } = calendars.run(sync(Some("http://example.org/ns/sync/1".to_owned()), known))?
    else {
        return Err("no sync".into());
    };
    assert_eq!((token, full), (None, true));
    assert_eq!(changed.iter().map(|event| event.url.clone()).collect::<Vec<_>>(), [event_url("tea.ics")]);
    assert_eq!(removed, [event_url("gone.ics")]);
    Ok(())
}

#[test]
fn a_put_creates_and_then_replaces_by_entity_tag() -> Result {
    let pki = TestPki::new()?;
    let fake = FakeDav::new();
    let store = fake.calendars();
    let server = serve(&pki, fake, 2)?;
    let mut calendars = Calendars::start(&pki, server.addr(), Some(&dav()), Some(PASSWORD))?;
    let href = format!("{WORK}lunch.ics");
    let CalendarReport::Written { calendar, event, etag: Some(first) } = calendars.run(put("lunch.ics", None, LUNCH))? else {
        return Err("not written".into());
    };
    assert_eq!((calendar, event), (CALENDAR, EVENT_ID));
    assert_eq!(store.get(&href), Some((first.clone(), LUNCH.to_owned())));
    let CalendarReport::Written { etag: Some(second), .. } = calendars.run(put("lunch.ics", Some(&first), TEA))? else {
        return Err("not replaced".into());
    };
    assert_ne!(first, second);
    assert_eq!(store.get(&href), Some((second, TEA.to_owned())));
    drop(calendars);
    let transcript = server.finish()?;
    let lines = transcript.received();
    assert!(lines.iter().any(|line| line.eq_ignore_ascii_case("if-none-match: *")), "{lines:?}");
    assert!(lines.iter().any(|line| line.eq_ignore_ascii_case(&format!("if-match: {first}"))), "{lines:?}");
    Ok(())
}

#[test]
fn a_change_made_elsewhere_is_a_conflict_and_never_overwritten() -> Result {
    let pki = TestPki::new()?;
    let fake = FakeDav::new();
    let store = fake.calendars();
    let href = format!("{WORK}lunch.ics");
    let seen = store.put(&href, LUNCH).ok_or("put")?;
    let theirs = store.put(&href, TEA).ok_or("put")?;
    let server = serve(&pki, fake, 3)?;
    let mut calendars = Calendars::start(&pki, server.addr(), Some(&dav()), Some(PASSWORD))?;
    let conflict = CalendarReport::Conflict { calendar: CALENDAR, event: EVENT_ID };
    // A new event where one is already, a change over a copy changed since, and a removal of it.
    assert_eq!(calendars.run(put("lunch.ics", None, LUNCH))?, conflict);
    assert_eq!(calendars.run(put("lunch.ics", Some(&seen), LUNCH))?, conflict);
    assert_eq!(calendars.run(delete("lunch.ics", Some(&seen)))?, conflict);
    assert_eq!(store.get(&href), Some((theirs, TEA.to_owned())));
    Ok(())
}

#[test]
fn a_delete_removes_the_event_and_one_gone_already_is_deleted_too() -> Result {
    let pki = TestPki::new()?;
    let fake = FakeDav::new();
    let store = fake.calendars();
    let href = format!("{WORK}lunch.ics");
    let tag = store.put(&href, LUNCH).ok_or("put")?;
    let server = serve(&pki, fake, 2)?;
    let mut calendars = Calendars::start(&pki, server.addr(), Some(&dav()), Some(PASSWORD))?;
    let deleted = CalendarReport::Deleted { calendar: CALENDAR, event: EVENT_ID };
    assert_eq!(calendars.run(delete("lunch.ics", Some(&tag)))?, deleted);
    assert_eq!(store.get(&href), None);
    assert_eq!(calendars.run(delete("lunch.ics", None))?, deleted);
    Ok(())
}

#[test]
fn each_fault_of_the_server_has_its_own_kind() -> Result {
    let pki = TestPki::new()?;
    let cases = [
        (Action::Fault(Fault::Drop), ErrorKind::ConnectionDropped),
        (Action::Fault(Fault::Garbage(b"HTTP/1.1 banana\r\n\r\n".to_vec())), ErrorKind::Protocol),
        (Action::Reply(Reply::new(500)), ErrorKind::CommandRefused),
        (Action::Reply(Reply::new(404)), ErrorKind::NoCalendarServer),
        (Action::Reply(Reply::new(401)), ErrorKind::LoginRefused),
        (
            Action::Reply(Reply::new(207).field("Content-Type", "application/xml").body(b"<!DOCTYPE x [<!ENTITY a \"a\">]><x/>".to_vec())),
            ErrorKind::Protocol,
        ),
    ];
    for (action, kind) in cases {
        let server = serve(&pki, FakeDav::new().on(When::method("REPORT"), [action.clone()]), 1)?;
        let mut calendars = Calendars::start(&pki, server.addr(), Some(&dav()), Some(PASSWORD))?;
        assert_eq!(calendars.run(sync(None, Vec::new()))?, failed(kind), "{action:?}");
    }
    Ok(())
}

#[test]
fn a_refused_password_is_not_sent_again() -> Result {
    let pki = TestPki::new()?;
    let server = serve(&pki, FakeDav::new().login(USER, "another-password"), 2)?;
    let mut calendars = Calendars::start(&pki, server.addr(), Some(&dav()), Some(PASSWORD))?;
    assert_eq!(calendars.run(CalendarJob::Discover { url: None })?, CalendarReport::Failed { calendar: None, kind: ErrorKind::LoginRefused });
    assert_eq!(calendars.run(sync(None, Vec::new()))?, failed(ErrorKind::LoginRefused));
    assert_eq!(calendars.run(put("lunch.ics", None, LUNCH))?, failed(ErrorKind::LoginRefused));
    assert_eq!(calendars.asked(), 1);
    drop(calendars);
    let (transcript, _no_second_client) = server.finish_with_transcript();
    // The well-known redirect, then the refusal; nothing after it.
    assert_eq!(requests(transcript.received()), ["PROPFIND /.well-known/caldav HTTP/1.1", "PROPFIND / HTTP/1.1"]);
    Ok(())
}

#[test]
fn no_password_goes_to_an_address_that_is_not_https() -> Result {
    let pki = TestPki::new()?;
    let server = Listen::plain().accept_timeout(Duration::from_millis(500)).start_dav(FakeDav::new().auth_in_clear())?;
    let mut calendars = Calendars::start(&pki, server.addr(), None, Some(PASSWORD))?;
    let clear = CalendarJob::Discover { url: Some(format!("http://{HOST}/")) };
    assert_eq!(calendars.run(clear)?, CalendarReport::Failed { calendar: None, kind: ErrorKind::NotEncrypted });
    let clear = CalendarJob::Sync { calendar: CALENDAR, url: format!("http://{HOST}{WORK}"), token: None, known: Vec::new() };
    assert_eq!(calendars.run(clear)?, failed(ErrorKind::NotEncrypted));
    drop(calendars);
    let (transcript, _no_client) = server.finish_with_transcript();
    assert!(transcript.received().is_empty(), "{:?}", transcript.received());
    Ok(())
}

#[test]
fn a_redirect_to_another_server_is_not_followed() -> Result {
    let pki = TestPki::new()?;
    let moved = Reply::new(301).field("Location", "https://calendar.example.net/dav/");
    let server = serve(&pki, FakeDav::new().reply(When::at("PROPFIND", "/.well-known/caldav"), moved), 1)?;
    let mut calendars = Calendars::start(&pki, server.addr(), Some(&dav()), Some(PASSWORD))?;
    assert_eq!(calendars.run(CalendarJob::Discover { url: None })?, CalendarReport::Failed { calendar: None, kind: ErrorKind::OtherServer });
    drop(calendars);
    let transcript = server.finish()?;
    assert_eq!(requests(transcript.received()), ["PROPFIND /.well-known/caldav HTTP/1.1"]);
    Ok(())
}

#[test]
fn a_response_that_closes_the_connection_is_followed_on_a_new_one() -> Result {
    let pki = TestPki::new()?;
    let moved = Reply::new(301).field("Location", "/").field("Connection", "close");
    let server = serve(&pki, FakeDav::new().reply(When::at("PROPFIND", "/.well-known/caldav"), moved), 2)?;
    let mut calendars = Calendars::start(&pki, server.addr(), Some(&dav()), Some(PASSWORD))?;
    let report = calendars.run(CalendarJob::Discover { url: None })?;
    assert!(matches!(&report, CalendarReport::Found { calendars, .. } if calendars.len() == 1), "{report:?}");
    Ok(())
}

#[test]
fn each_certificate_fault_has_its_own_kind_and_no_password_goes_out() -> Result {
    let pki = TestPki::new()?;
    for fault in CertFault::ALL {
        let server = Listen::tls(pki.server_config(Some(fault))?).accept_timeout(Duration::from_millis(500)).start_dav(FakeDav::new())?;
        let clock = if fault == CertFault::ClockFarOff { far_off()? } else { wall() };
        let mut calendars = Calendars::with_clock(&pki, server.addr(), Some(&dav()), Some(PASSWORD), clock)?;
        let expected = match fault {
            CertFault::UnknownCa => ErrorKind::CertificateUnknownIssuer,
            CertFault::Expired => ErrorKind::CertificateExpired,
            CertFault::WrongHost => ErrorKind::CertificateWrongHost,
            CertFault::ClockFarOff => ErrorKind::ClockFarOff,
        };
        assert_eq!(calendars.run(CalendarJob::Discover { url: None })?, CalendarReport::Failed { calendar: None, kind: expected }, "{fault:?}");
        drop(calendars);
        let (transcript, _refused) = server.finish_with_transcript();
        assert!(transcript.received().is_empty(), "{fault:?}");
    }
    Ok(())
}

#[test]
fn an_event_too_large_to_write_is_refused_before_it_goes() -> Result {
    let pki = TestPki::new()?;
    let server = serve(&pki, FakeDav::new(), 1)?;
    let mut calendars = Calendars::start(&pki, server.addr(), Some(&dav()), Some(PASSWORD))?;
    let huge = "X".repeat(inboxcraft_dav::MAX_PUT + 1);
    assert_eq!(calendars.run(put("huge.ics", None, &huge))?, failed(ErrorKind::TooLarge));
    drop(calendars);
    let (transcript, _no_client) = server.finish_with_transcript();
    assert!(requests(transcript.received()).is_empty());
    Ok(())
}

#[test]
fn a_user_name_basic_auth_cannot_carry_is_refused_before_anything_goes() -> Result {
    let pki = TestPki::new()?;
    let imap = server(Security::Tls)?;
    let server = serve(&pki, FakeDav::new(), 1)?;
    let account = Account::new(ACCOUNT, "Ada", EmailAddress::new(USER)?, "ada:colon", imap)?.with_caldav(Some(&dav()))?;
    let (sender, events) = mpsc::channel();
    let worker = Worker::spawn(
        account,
        WorkerSettings::new(connector(&pki, wall(), server.addr())),
        Arc::new(Fixed(Some(PASSWORD), AtomicUsize::new(0))),
        sender,
    )?;
    worker.send(Job::new(JobId::new(1), ACCOUNT, JobKind::Calendar(CalendarJob::Discover { url: None }))).map_err(|_| "job refused")?;
    let event = events.recv_timeout(WAIT)?;
    assert_eq!(event.kind, EventKind::Calendar(CalendarReport::Failed { calendar: None, kind: ErrorKind::NoLoginMethod }));
    drop(worker);
    let (transcript, _no_client) = server.finish_with_transcript();
    assert!(transcript.received().is_empty());
    Ok(())
}

#[test]
fn after_a_refused_password_only_the_owners_check_sends_it_again() -> Result {
    let pki = TestPki::new()?;
    let imap = Listen::tls(pki.server_config(None)?).accept_timeout(Duration::from_millis(500)).start_imap_many(FakeImap::new(), 2)?;
    // The CalDAV server refuses the first request only, as after the owner fixed the password there.
    let fake = serve(&pki, FakeDav::new().reply(When::method("PROPFIND"), Reply::new(401)), 3)?;
    let account = Account::new(ACCOUNT, "Ada", EmailAddress::new(USER)?, USER, server(Security::Tls)?)?.with_caldav(Some(&dav()))?;
    let connector = connector(&pki, wall(), fake.addr()).with_addresses_on(993, vec![imap.addr()]);
    let (sender, events) = mpsc::channel();
    let worker = Worker::spawn(account, WorkerSettings::new(connector), Arc::new(Fixed(Some(PASSWORD), AtomicUsize::new(0))), sender)?;
    let mut calendars = Calendars { worker, events, credentials: Arc::new(Fixed(None, AtomicUsize::new(0))), next: 0 };
    assert_eq!(calendars.run(CalendarJob::Discover { url: None })?, CalendarReport::Failed { calendar: None, kind: ErrorKind::LoginRefused });
    assert_eq!(caldav_step(&mut calendars, false)?, StepStatus::NeedsOwner);
    assert_eq!(caldav_step(&mut calendars, true)?, StepStatus::Passed);
    let report = calendars.run(CalendarJob::Discover { url: None })?;
    assert!(matches!(report, CalendarReport::Found { .. }), "{report:?}");
    drop(calendars);
    drop(imap.finish_with_transcript());
    let transcript = fake.finish()?;
    assert_eq!(requests(transcript.received()).iter().filter(|line| line.starts_with("PROPFIND /.well-known/")).count(), 3);
    Ok(())
}

/// Runs a check and returns the status of its CalDAV step.
fn caldav_step(calendars: &mut Calendars, by_owner: bool) -> Result<StepStatus> {
    calendars.next += 1;
    let id = JobId::new(calendars.next);
    calendars.worker.send(Job::new(id, ACCOUNT, JobKind::Check { by_owner })).map_err(|_| "job refused")?;
    loop {
        let event = calendars.events.recv_timeout(WAIT)?;
        if let EventKind::CheckStep(report) = event.kind
            && report.step() == CheckStep::CaldavDiscovery
        {
            return Ok(report.status());
        }
    }
}
