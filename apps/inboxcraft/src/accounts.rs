// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the window needs to add and run accounts: the OS credential store, the `net` workers and Detect.
//!
//! - [`OsKeychain`]: the passwords, in the system's credential store (Secret Service, Keychain, Credential Manager)
//!   through `keyring`, one entry per account under [`SERVICE`], named with the store's id and the account's
//!   ([`entry_name`]). The window writes an entry when it adds the account and removes it with the account; it stays
//!   in between, as the account stays in the store. The workers read them, each time they sign in. Never a file,
//!   never the log. ([`InboxApp::on_exit`](inboxcraft_ui_egui::InboxApp::on_exit) removes them all, for a window over
//!   a store that is not kept.)
//! - [`NetWorkers`]: one `net` worker per account, started at the account's first job; their events wake the window.
//! - [`detect`]: Detect over the system's name servers and HTTPS, as the form's proposal.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use inboxcraft_model::{Account, AccountId, EmailAddress, Event, EventKind, Job, Security};
use inboxcraft_net::detect::{self, DetectConfig, Proposal as Found, Source as Where};
use inboxcraft_net::dns::Resolver;
use inboxcraft_net::web::Https;
use inboxcraft_net::{Credentials, Password, TlsSettings, Worker, WorkerSettings};
use inboxcraft_ui_egui::fetch::Wake;
use inboxcraft_ui_egui::onboard::{self, Keychain, Proposal, Suggestion};
use inboxcraft_ui_egui::workers::Workers;

/// The service name of every password InboxCraft keeps in the credential store: the app id.
pub const SERVICE: &str = "io.github.qubernetic.inboxcraft";

/// How long Detect waits for one name server.
pub const DNS_TIMEOUT: Duration = Duration::from_secs(5);

/// How long Detect waits for one web request.
pub const WEB_TIMEOUT: Duration = Duration::from_secs(15);

/// The credential store entry's user name for `account` of the store whose id is `store`
/// ([`inboxcraft_store::Sqlite::id`]). Every store numbers its accounts from 1, and two stores can share one
/// credential store (the Flatpak and a build of its own, or another `XDG_DATA_HOME`): bound to its store, an entry is
/// never read, overwritten or removed for another store's account.
#[must_use]
pub fn entry_name(store: &str, account: u64) -> String {
    format!("account-{store}-{account}")
}

/// Why the credential store refused, in words for the owner. Fixed words: a `keyring` error can carry the bytes it
/// could not decode, which may be a password.
#[must_use]
pub fn described(error: &keyring::Error) -> String {
    match error {
        keyring::Error::NoDefaultStore => "no credential store is running (Secret Service, Keychain or Credential Manager)",
        keyring::Error::NoStorageAccess(_) => "the credential store is locked or cannot be reached",
        keyring::Error::NoEntry => "the credential store holds no password for this account",
        keyring::Error::PlatformFailure(_) => "the credential store failed",
        _ => "the credential store refused",
    }
    .to_owned()
}

/// The entries of a credential store under [`SERVICE`], by user name.
pub trait Entries: Send + Sync {
    /// Keeps `password` under `name`, replacing what was there.
    ///
    /// # Errors
    ///
    /// Why the credential store refused, in words for the owner.
    fn set(&self, name: &str, password: &str) -> Result<(), String>;

    /// The password under `name`, if there is one and it can be read.
    fn get(&self, name: &str) -> Option<String>;

    /// Removes the entry `name`; nothing when there is none.
    ///
    /// # Errors
    ///
    /// Why the credential store refused, in words for the owner.
    fn delete(&self, name: &str) -> Result<(), String>;
}

/// The system's credential store, through `keyring`.
#[derive(Debug, Default, Clone, Copy)]
pub struct OsEntries;

fn entry(name: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, name).map_err(|error| described(&error))
}

impl Entries for OsEntries {
    fn set(&self, name: &str, password: &str) -> Result<(), String> {
        entry(name)?.set_password(password).map_err(|error| described(&error))
    }

    fn get(&self, name: &str) -> Option<String> {
        entry(name).ok()?.get_password().ok()
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        match entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(described(&error)),
        }
    }
}

/// The passwords of one store's accounts in the system's credential store, each under [`entry_name`].
#[derive(Clone)]
pub struct OsKeychain {
    store: String,
    entries: Arc<dyn Entries>,
}

impl std::fmt::Debug for OsKeychain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OsKeychain").field("store", &self.store).finish_non_exhaustive()
    }
}

impl OsKeychain {
    /// The passwords of the accounts of the store whose id is `store`, in the system's credential store.
    #[must_use]
    pub fn new(store: &str) -> Self {
        Self::over(store, Arc::new(OsEntries))
    }

    /// The passwords of the accounts of the store whose id is `store`, in `entries`.
    #[must_use]
    pub fn over(store: &str, entries: Arc<dyn Entries>) -> Self {
        Self { store: store.to_owned(), entries }
    }

    /// The entry's user name for `account` of this store.
    #[must_use]
    pub fn entry(&self, account: u64) -> String {
        entry_name(&self.store, account)
    }
}

impl Keychain for OsKeychain {
    fn store(&self, account: u64, password: &str) -> Result<(), String> {
        self.entries.set(&self.entry(account), password)
    }

    fn forget(&self, account: u64) -> Result<(), String> {
        self.entries.delete(&self.entry(account))
    }
}

impl Credentials for OsKeychain {
    fn password(&self, account: &Account) -> Option<Password> {
        self.entries.get(&self.entry(account.id().get())).map(Password::new)
    }
}

/// One `net` worker per account; their events wake the window.
pub struct NetWorkers {
    settings: WorkerSettings,
    credentials: Arc<dyn Credentials>,
    workers: BTreeMap<AccountId, Worker>,
    /// The account each worker was started with: it signs in to these servers until it ends.
    started: BTreeMap<AccountId, Account>,
    /// What the workers send on; each worker gets a copy.
    sender: Sender<Event>,
    events: Receiver<Event>,
    /// Events of jobs no worker took.
    failed: Vec<Event>,
}

impl std::fmt::Debug for NetWorkers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetWorkers").field("workers", &self.workers).finish_non_exhaustive()
    }
}

impl NetWorkers {
    /// Workers with `settings`, reading the passwords from `credentials`, calling `wake` after each event.
    #[must_use]
    pub fn new(settings: WorkerSettings, credentials: Arc<dyn Credentials>, wake: Option<Wake>) -> Self {
        let (sender, from_workers) = mpsc::channel::<Event>();
        let (to_window, events) = mpsc::channel();
        // Passes each event on and wakes the window; it ends with the last worker and this value.
        let spawned = thread::Builder::new().name("inboxcraft-events".to_owned()).spawn(move || {
            for event in from_workers {
                if to_window.send(event).is_err() {
                    break;
                }
                if let Some(wake) = &wake {
                    wake();
                }
            }
        });
        let mut workers = Self { settings, credentials, workers: BTreeMap::new(), started: BTreeMap::new(), sender, events, failed: Vec::new() };
        if spawned.is_err() {
            // Without the thread no event reaches the window: the workers send straight to it, unwoken.
            let (sender, events) = mpsc::channel();
            workers.sender = sender;
            workers.events = events;
        }
        workers
    }

    /// Lets the worker of `id` end, off the window's thread: a worker signs out before it ends, which can take a while.
    fn stop(&mut self, id: AccountId) {
        self.started.remove(&id);
        if let Some(worker) = self.workers.remove(&id) {
            let _ = thread::Builder::new().name("inboxcraft-stop".to_owned()).spawn(move || worker.stop());
        }
    }

    fn fail(&mut self, job: &Job, kind: inboxcraft_model::ErrorKind) {
        self.failed.push(Event::new(job.account, Some(job.id), EventKind::Failed { kind, folder: None }));
    }
}

impl Workers for NetWorkers {
    fn send(&mut self, account: &Account, job: Job) {
        let id = account.id();
        // The servers or the user name changed (a CalDAV address set, cleared or moved): the worker that holds the
        // account as it was ends after the jobs it has, and another one starts, so the password goes to no server the
        // account no longer uses.
        if self.started.get(&id).is_some_and(|held| !same_servers(held, account)) {
            self.stop(id);
        }
        if !self.workers.contains_key(&id) {
            match Worker::spawn(account.clone(), self.settings.clone(), Arc::clone(&self.credentials), self.sender.clone()) {
                Ok(worker) => {
                    self.workers.insert(id, worker);
                    self.started.insert(id, account.clone());
                }
                Err(error) => return self.fail(&job, error.kind()),
            }
        }
        let refused = self.workers.get(&id).map(|worker| worker.send(job));
        if let Some(Err(job)) = refused {
            // The worker ended: the next job starts another.
            self.workers.remove(&id);
            self.started.remove(&id);
            self.fail(&job, inboxcraft_model::ErrorKind::ConnectionDropped);
        }
    }

    fn keep(&mut self, accounts: &[AccountId]) {
        let gone: Vec<AccountId> = self.workers.keys().filter(|id| !accounts.contains(id)).copied().collect();
        for id in gone {
            self.stop(id);
        }
    }

    fn events(&mut self) -> Vec<Event> {
        let mut events: Vec<Event> = std::mem::take(&mut self.failed);
        events.extend(self.events.try_iter());
        events
    }
}

/// Whether `a` and `b` sign in alike: the same user name, IMAP, submission and CalDAV servers.
fn same_servers(a: &Account, b: &Account) -> bool {
    a.username() == b.username() && a.imap() == b.imap() && a.submission() == b.submission() && a.caldav() == b.caldav()
}

/// `found` as the form shows it.
#[must_use]
pub fn proposal(found: &Found) -> Proposal {
    let imap = found
        .imap
        .iter()
        .map(|candidate| Suggestion {
            host: candidate.server.host().as_str().to_owned(),
            port: candidate.server.port().get(),
            security: match candidate.server.security() {
                Security::Tls => onboard::Security::Tls,
                Security::StartTls => onboard::Security::StartTls,
            },
            username: candidate.username.clone(),
            source: match candidate.source {
                Where::Autoconfig => onboard::Source::Autoconfig,
                Where::AutoconfigRedirect => onboard::Source::AutoconfigRedirect,
                Where::Ispdb => onboard::Source::Ispdb,
                Where::Srv => onboard::Source::Srv,
                Where::Guess => onboard::Source::Guess,
            },
        })
        .collect();
    let failed = found.failed.iter().map(|(what, kind)| format!("{what}: {kind}")).collect();
    Proposal { imap, asked: found.asked.clone(), failed }
}

/// Detect for `address` over the system's name servers and HTTPS with `tls`; the ISPDB only when `ispdb`.
#[must_use]
pub fn detect(tls: &TlsSettings, address: &str, ispdb: bool) -> Proposal {
    let Ok(address) = EmailAddress::new(address) else {
        return Proposal { failed: vec!["the address is not a mail address".to_owned()], ..Proposal::default() };
    };
    let config = DetectConfig { ispdb, ..DetectConfig::default() };
    // Without the system's configuration there is no name server to ask: Detect goes on with the web only.
    let dns = Resolver::system(DNS_TIMEOUT).unwrap_or_else(|_| Resolver::new(Vec::new(), DNS_TIMEOUT));
    let web = Https::new(tls.clone(), WEB_TIMEOUT);
    proposal(&detect::detect(&address, &config, &dns, &web))
}

#[cfg(test)]
#[path = "accounts_tests.rs"]
mod tests;
