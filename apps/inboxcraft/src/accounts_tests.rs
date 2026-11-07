// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::BTreeMap;
use std::net::TcpListener;
use std::num::NonZeroU16;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use inboxcraft_model::{
    Account, AccountId, CheckStep, EmailAddress, ErrorKind, Event, EventKind, Host, Job, JobId, JobKind, Security, Server, StepStatus,
};
use inboxcraft_net::detect::{Candidate, Proposal as Found, Source as Where};
use inboxcraft_net::{Connector, Credentials, Password, TlsSettings, Trust, WorkerSettings};
use inboxcraft_time::SystemClock;
use inboxcraft_ui_egui::onboard::{self, Keychain};
use inboxcraft_ui_egui::workers::Workers;

use super::{Entries, NetWorkers, OsKeychain, SERVICE, described, detect, entry_name, proposal};

const SECRET: &str = "maple-Harbour-42-lantern";

#[test]
fn every_password_sits_under_the_app_id_by_store_and_account() {
    assert_eq!(SERVICE, "io.github.qubernetic.inboxcraft");
    assert_eq!(entry_name("0a1b", 7), "account-0a1b-7");
}

/// A credential store in memory that records what was asked of it.
#[derive(Default)]
struct Recording(Mutex<BTreeMap<String, String>>);

impl Recording {
    fn names(&self) -> Vec<String> {
        self.0.lock().unwrap().keys().cloned().collect()
    }
}

impl Entries for Recording {
    fn set(&self, name: &str, password: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(name.to_owned(), password.to_owned());
        Ok(())
    }

    fn get(&self, name: &str) -> Option<String> {
        self.0.lock().unwrap().get(name).cloned()
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(name);
        Ok(())
    }
}

/// Account 1 of the store in `dir`, its password kept through a keychain bound to that store over `entries`.
fn add_in(dir: &std::path::Path, entries: &Arc<Recording>, password: &str) -> OsKeychain {
    let opened = crate::store::open_store(dir).unwrap();
    let keychain = OsKeychain::over(&opened.id, Arc::clone(entries) as Arc<dyn Entries>);
    keychain.store(1, password).unwrap();
    keychain
}

#[test]
fn two_stores_on_one_computer_keep_their_passwords_apart() {
    // The Flatpak and a build of its own, or another XDG_DATA_HOME: both number their accounts from 1.
    let entries = Arc::new(Recording::default());
    let (one, other) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let first = add_in(one.path(), &entries, SECRET);
    let second = add_in(other.path(), &entries, "another-password");
    let names = entries.names();
    assert_eq!(names.len(), 2, "two entries, not one overwritten: {names:?}");
    let kept = |keychain: &OsKeychain| entries.get(&keychain.entry(1));
    assert_eq!(kept(&first).as_deref(), Some(SECRET));
    assert_eq!(kept(&second).as_deref(), Some("another-password"));
    assert!(Credentials::password(&first, &account(993)).is_some(), "a worker reads its own store's entry");
    first.forget(1).unwrap();
    assert!(Credentials::password(&first, &account(993)).is_none());
    assert_eq!(kept(&second).as_deref(), Some("another-password"), "forgetting in one store leaves the other's");
    let again = add_in(one.path(), &entries, SECRET);
    assert_eq!(again.entry(1), first.entry(1), "a store keeps its id, so its entries, between runs");
    assert_eq!(entries.names().len(), 2);
}

#[test]
fn an_entry_not_bound_to_the_store_is_never_read() {
    // Left by an older build, which named the entry by the account alone.
    let entries = Arc::new(Recording::default());
    entries.set("account-1", SECRET).unwrap();
    let keychain = OsKeychain::over("0a1b", Arc::clone(&entries) as Arc<dyn Entries>);
    assert!(Credentials::password(&keychain, &account(993)).is_none());
}

#[test]
fn a_credential_store_refusal_reads_in_fixed_words_without_the_secret() {
    let platform = || Box::new(std::io::Error::other(format!("dbus said {SECRET}"))) as Box<dyn std::error::Error + Send + Sync>;
    let cases = [
        (keyring::Error::NoDefaultStore, "no credential store is running (Secret Service, Keychain or Credential Manager)"),
        (keyring::Error::NoStorageAccess(platform()), "the credential store is locked or cannot be reached"),
        (keyring::Error::NoEntry, "the credential store holds no password for this account"),
        (keyring::Error::PlatformFailure(platform()), "the credential store failed"),
        (keyring::Error::BadEncoding(SECRET.as_bytes().to_vec()), "the credential store refused"),
    ];
    for (error, words) in cases {
        let text = described(&error);
        assert_eq!(text, words);
        assert!(!text.contains(SECRET));
    }
}

fn found() -> Found {
    let server = |host: &str, port: u16, security| Server::new(Host::new(host).unwrap(), NonZeroU16::new(port).unwrap(), security);
    Found {
        mx: vec![Host::new("mx.example.org").unwrap()],
        imap: vec![
            Candidate { server: server("imap.example.org", 993, Security::Tls), username: "carol@example.org".to_owned(), source: Where::Autoconfig },
            Candidate { server: server("mail.example.org", 143, Security::StartTls), username: "carol".to_owned(), source: Where::Guess },
        ],
        submission: Vec::new(),
        asked: vec!["dns MX example.org".to_owned()],
        failed: vec![("dns SRV _imaps._tcp.example.org".to_owned(), ErrorKind::TimedOut)],
    }
}

#[test]
fn without_a_credential_store_nothing_is_kept_and_no_password_comes_back() {
    // A test never writes into a credential store the computer runs: this one asks only where there is none.
    if keyring::Entry::store_status().is_ok() {
        eprintln!("SKIP: this computer runs a credential store");
        return;
    }
    let none = "no credential store is running (Secret Service, Keychain or Credential Manager)";
    let keychain = OsKeychain::new("0a1b");
    assert_eq!(keychain.store(7, SECRET), Err(none.to_owned()));
    assert_eq!(keychain.forget(7), Err(none.to_owned()));
    let account = account(993);
    assert!(Credentials::password(&keychain, &account).is_none());
}

#[test]
fn a_detected_proposal_reads_as_the_form_shows_it() {
    let shown = proposal(&found());
    assert_eq!(shown.imap.len(), 2);
    let first = &shown.imap[0];
    assert_eq!(
        (first.host.as_str(), first.port, first.security, first.source),
        ("imap.example.org", 993, onboard::Security::Tls, onboard::Source::Autoconfig)
    );
    assert_eq!(first.username, "carol@example.org");
    assert_eq!((shown.imap[1].security, shown.imap[1].source), (onboard::Security::StartTls, onboard::Source::Guess));
    assert_eq!(shown.asked, ["dns MX example.org"]);
    assert_eq!(shown.failed, [format!("dns SRV _imaps._tcp.example.org: {}", ErrorKind::TimedOut)]);
    let mut every = found();
    for (candidate, source) in every.imap.iter_mut().zip([Where::AutoconfigRedirect, Where::Ispdb]) {
        candidate.source = source;
    }
    every.imap.push(Candidate { source: Where::Srv, ..every.imap[0].clone() });
    let sources: Vec<onboard::Source> = proposal(&every).imap.iter().map(|suggestion| suggestion.source).collect();
    assert_eq!(sources, [onboard::Source::AutoconfigRedirect, onboard::Source::Ispdb, onboard::Source::Srv]);
}

#[test]
fn detect_refuses_what_is_not_an_address_without_asking_anyone() {
    let tls = TlsSettings::new(Trust::Platform, Arc::new(SystemClock::new()));
    let shown = detect(&tls, "not an address", false);
    assert!(shown.imap.is_empty() && shown.asked.is_empty());
    assert_eq!(shown.failed, ["the address is not a mail address"]);
}

/// No password, ever: the check below fails long before a sign-in.
struct NoPassword;

impl Credentials for NoPassword {
    fn password(&self, _account: &Account) -> Option<Password> {
        None
    }
}

fn closed_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn account(port: u16) -> Account {
    let imap = Server::new(Host::new("127.0.0.1").unwrap(), NonZeroU16::new(port).unwrap(), Security::Tls);
    Account::new(AccountId::new(1), "", EmailAddress::new("carol@example.org").unwrap(), "carol", imap).unwrap()
}

fn wait_for(workers: &mut NetWorkers, done: impl Fn(&[Event]) -> bool) -> Vec<Event> {
    let mut seen = Vec::new();
    for _ in 0..1000 {
        if done(&seen) {
            break;
        }
        seen.extend(workers.events());
        std::thread::sleep(Duration::from_millis(20));
    }
    seen
}

#[test]
fn a_check_runs_on_the_accounts_worker_and_its_steps_wake_the_window() {
    let woken = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&woken);
    let wake: inboxcraft_ui_egui::fetch::Wake = Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    let tls = TlsSettings::new(Trust::Platform, Arc::new(SystemClock::new()));
    let mut workers = NetWorkers::new(WorkerSettings::new(Connector::new(tls)), Arc::new(NoPassword), Some(wake));
    assert!(format!("{workers:?}").starts_with("NetWorkers"));
    let account = account(closed_port());
    workers.send(&account, Job::new(JobId::new(1), account.id(), JobKind::Check { by_owner: true }));
    let last = |events: &[Event]| {
        events.iter().any(|event| matches!(&event.kind, EventKind::CheckStep(report) if report.step() == CheckStep::CaldavDiscovery))
    };
    let events = wait_for(&mut workers, last);
    let steps: Vec<(CheckStep, StepStatus)> = events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::CheckStep(report) => Some((report.step(), report.status())),
            _ => None,
        })
        .collect();
    assert_eq!(steps.first(), Some(&(CheckStep::Dns, StepStatus::Passed)), "{steps:?}");
    assert_eq!(steps.get(1), Some(&(CheckStep::Tcp, StepStatus::Failed(ErrorKind::ConnectionRefused))), "{steps:?}");
    assert!(steps.iter().all(|(step, status)| *step < CheckStep::Tls || matches!(status, StepStatus::Skipped | StepStatus::NotBuilt)), "{steps:?}");
    assert!(woken.load(Ordering::SeqCst) >= steps.len());
    // The account is gone: its worker stops, off the window's thread, and the next job starts another.
    workers.keep(&[]);
    workers.send(&account, Job::new(JobId::new(2), account.id(), JobKind::Check { by_owner: true }));
    let again = wait_for(&mut workers, last);
    let steps_of = |job| again.iter().filter(|event| event.job == Some(JobId::new(job)) && matches!(event.kind, EventKind::CheckStep(_))).count();
    assert_eq!((steps_of(1), steps_of(2)), (0, CheckStep::ALL.len()));
}

#[test]
fn a_job_for_another_account_than_its_worker_fails_and_starts_over() {
    let tls = TlsSettings::new(Trust::Platform, Arc::new(SystemClock::new()));
    let mut workers = NetWorkers::new(WorkerSettings::new(Connector::new(tls)), Arc::new(NoPassword), None);
    let account = account(closed_port());
    // A job whose account is not the worker's comes back refused.
    workers.send(&account, Job::new(JobId::new(5), AccountId::new(9), JobKind::SyncFolderList));
    let events = workers.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::Failed { kind: ErrorKind::ConnectionDropped, folder: None });
    assert_eq!(events[0].job, Some(JobId::new(5)));
}

#[test]
fn once_the_window_is_gone_the_events_stop_waking_it() {
    // The first wake holds the event thread until the window is gone; the wake itself lives as long as that thread.
    let (woken, first_wake) = mpsc::channel::<()>();
    let (release, released) = mpsc::channel::<()>();
    let (woken, released) = (Mutex::new(woken), Mutex::new(released));
    let wakes = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&wakes);
    let wake: inboxcraft_ui_egui::fetch::Wake = Arc::new(move || {
        if counter.fetch_add(1, Ordering::SeqCst) == 0 {
            let _ = woken.lock().map(|woken| woken.send(()));
            let _ = released.lock().map(|released| released.recv_timeout(Duration::from_secs(10)));
        }
    });
    let tls = TlsSettings::new(Trust::Platform, Arc::new(SystemClock::new()));
    let mut workers = NetWorkers::new(WorkerSettings::new(Connector::new(tls)), Arc::new(NoPassword), Some(wake));
    let account = account(closed_port());
    workers.send(&account, Job::new(JobId::new(1), account.id(), JobKind::Check { by_owner: true }));
    first_wake.recv_timeout(Duration::from_secs(10)).unwrap();
    // The window closes: its worker ends after the check, every step of which is now waiting for the event thread.
    drop(workers);
    release.send(()).unwrap();
    for _ in 0..500 {
        if Arc::strong_count(&wakes) == 1 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(Arc::strong_count(&wakes), 1, "the event thread ended");
    assert_eq!(wakes.load(Ordering::SeqCst), 1, "no step after the first woke the window that is gone");
}
