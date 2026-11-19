// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendars' sync at the foot of the side panel: when the calendars were last read, and the owner's own sync
//! (`calendar.sync`, `safe`) from the press on "sync now" to its end.
//!
//! The engine does no I/O: `calendar.sync` queues a discovery job for each account with a CalDAV address, the worker
//! answers it with the calendars found, the engine then queues a sync job for each calendar, and the worker answers
//! each with what changed or why it failed. The [`Tracker`] follows the jobs of one press by their ids as the
//! [`crate::mailbox::Mailbox`] hands them out and hears their events: the sync is under way while one of them is out,
//! and it ends synced, or failed with the first failure heard. The press is one of the calendar's own presses: the
//! window runs the command for it ([`crate::mailbox::Mailbox::sync_calendars`]) unless a sync is under way
//! ([`Tracker::can_start`]), so there is never a second sync of the owner's at once.
//!
//! A failure is shown in the window's own words, one of a fixed set ([`Why`]); no text of a server reaches the panel.
//! An account's name is cleaned and capped when the sync starts ([`MOST_NAME`]); the jobs followed are capped too
//! ([`MOST_JOBS`]).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use inboxcraft_model::{CalendarJob, CalendarReport, ErrorKind, Event, EventKind, Job, JobId, JobKind};
use inboxcraft_time::{Offset, Timestamp};

use super::Moment;
use crate::tr;

/// The most characters of an account's name kept for the panel's line: the line is 222 points wide and cuts it there
/// anyway.
pub const MOST_NAME: usize = 64;

/// The most jobs of one sync followed at once: a discovery for each account and a sync for each calendar. A job over
/// the cap is not followed, so the sync can end before it does.
pub const MOST_JOBS: usize = 4096;

/// Why the owner's sync failed, in the window's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// No account has a CalDAV address.
    NoAddress,
    /// The server refused the sign-in.
    SignIn,
    /// The credential store holds no password for the account.
    Password,
    /// The server's name was not found.
    NotFound,
    /// No connection to the server, or it dropped.
    NoConnection,
    /// The encrypted connection or the server's certificate failed.
    Certificate,
    /// The server has no calendars at the address.
    NoCalendars,
    /// The server sent the program on to another server.
    Elsewhere,
    /// The server refused a command.
    Refused,
    /// The server's answer could not be read, or was too large.
    Unreadable,
    /// The command could not be run.
    NotStarted,
    /// Anything else.
    Failed,
}

impl Why {
    /// The window's reason for a worker's `kind` of failure.
    #[must_use]
    pub const fn of(kind: ErrorKind) -> Self {
        match kind {
            ErrorKind::LoginRefused | ErrorKind::LoginUnanswered | ErrorKind::NoLoginMethod => Self::SignIn,
            ErrorKind::CredentialMissing => Self::Password,
            ErrorKind::HostNotFound => Self::NotFound,
            ErrorKind::ConnectionRefused
            | ErrorKind::TimedOut
            | ErrorKind::PortTimedOut
            | ErrorKind::MailPortsBlocked
            | ErrorKind::ConnectionDropped => Self::NoConnection,
            ErrorKind::CertificateUnknownIssuer
            | ErrorKind::CertificateExpired
            | ErrorKind::CertificateNotYetValid
            | ErrorKind::CertificateWrongHost
            | ErrorKind::ClockFarOff
            | ErrorKind::Tls
            | ErrorKind::StartTlsMissing
            | ErrorKind::NotEncrypted => Self::Certificate,
            ErrorKind::NoCalendarServer => Self::NoCalendars,
            ErrorKind::OtherServer => Self::Elsewhere,
            ErrorKind::CommandRefused => Self::Refused,
            ErrorKind::Protocol | ErrorKind::TooLarge => Self::Unreadable,
            _ => Self::Failed,
        }
    }

    /// The reason in a few words, after "calendars:".
    #[must_use]
    pub fn words(self) -> &'static str {
        match self {
            Self::NoAddress => tr!("no CalDAV address"),
            Self::SignIn => tr!("sign-in refused"),
            Self::Password => tr!("no password kept"),
            Self::NotFound => tr!("server not found"),
            Self::NoConnection => tr!("no connection"),
            Self::Certificate => tr!("not secure"),
            Self::NoCalendars => tr!("no calendars there"),
            Self::Elsewhere => tr!("on another server"),
            Self::Refused => tr!("refused"),
            Self::Unreadable => tr!("answer unreadable"),
            Self::NotStarted => tr!("not started"),
            Self::Failed => tr!("failed"),
        }
    }
}

/// How the owner's sync stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncState {
    /// No sync of the owner's: when the calendars were last read, and the control to sync.
    Idle,
    /// Under way: `done` of the `of` calendars found so far read, `account` the name of the account whose calendars
    /// are read. A discovery is no calendar: the count starts once the calendars are found, and never goes back.
    Syncing {
        /// The calendars read.
        done: usize,
        /// The calendars found so far.
        of: usize,
        /// The account whose calendars are read now, cleaned and capped.
        account: String,
    },
    /// Every job heard back without a failure, at `at` (the computer's clock).
    Synced {
        /// When it ended.
        at: Option<Moment>,
    },
    /// A job failed: the account's name (cleaned and capped), when one was named, and why.
    Failed {
        /// The account whose calendars failed.
        account: Option<String>,
        /// Why.
        why: Why,
    },
}

/// What the side panel's foot shows: the state of the owner's sync and when the calendars were last read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncView {
    /// The owner's sync.
    pub state: SyncState,
    /// When a calendar was last read, by anyone's sync, on the computer's clock; `None` before the first.
    pub read: Option<Moment>,
}

impl Default for SyncView {
    fn default() -> Self {
        Self { state: SyncState::Idle, read: None }
    }
}

/// The owner's sync under way.
#[derive(Debug, Clone, Default)]
struct Running {
    /// The accounts asked, by id, with their names.
    accounts: BTreeMap<u64, String>,
    /// The jobs out, with their accounts and whether each reads a calendar (a sync, not a discovery).
    out: BTreeMap<JobId, (u64, bool)>,
    /// The calendars whose sync was handed out.
    of: usize,
    /// The calendars whose sync came back.
    done: usize,
    /// The first failure heard: its account and why.
    failure: Option<(u64, Why)>,
    /// The account of the last job heard back or handed out.
    current: Option<u64>,
}

/// The calendars' sync, kept by the mailbox and read by the side panel ([`SyncHandle`]).
#[derive(Debug, Default)]
pub struct Tracker {
    running: Option<Running>,
    view: SyncView,
    /// How many times the command ran for a press.
    started: u64,
}

/// The tracker shared between the mailbox, which runs the sync and hears its events, and the calendar's side panel,
/// which draws it and notes the press.
pub type SyncHandle = Arc<Mutex<Tracker>>;

/// Does `f` with `handle`'s tracker; a tracker left half changed by a panic elsewhere is still read (it holds plain
/// data, every state of it can be drawn).
pub fn with<R>(handle: &SyncHandle, f: impl FnOnce(&mut Tracker) -> R) -> R {
    let mut tracker = handle.lock().unwrap_or_else(PoisonError::into_inner);
    f(&mut tracker)
}

impl Tracker {
    /// Whether the owner's press may start a sync: none is under way.
    #[must_use]
    pub fn can_start(&self) -> bool {
        self.running.is_none()
    }

    /// The command ran and asked `accounts` (ids with their names) to sync: their jobs are followed from now on.
    /// Without an account there is nothing to sync: no account has a CalDAV address.
    pub fn start(&mut self, accounts: &[(u64, &str)]) {
        self.started = self.started.saturating_add(1);
        if accounts.is_empty() {
            self.running = None;
            self.view.state = SyncState::Failed { account: None, why: Why::NoAddress };
            return;
        }
        let accounts = accounts.iter().map(|(id, name)| (*id, clean(name))).collect();
        let running = Running { accounts, ..Running::default() };
        self.view.state = syncing(&running);
        self.running = Some(running);
    }

    /// The command was refused: the sync never started.
    pub fn not_started(&mut self) {
        self.started = self.started.saturating_add(1);
        self.running = None;
        self.view.state = SyncState::Failed { account: None, why: Why::NotStarted };
    }

    /// The jobs the engine just handed out: the calendar jobs of the accounts asked are followed. Then, with every
    /// job followed heard back (or its account gone, `alive` not holding it), the sync ends at `now` on the clock of
    /// `offset`. Whether the view changed.
    pub fn sent(&mut self, jobs: &[Job], alive: &[u64], now: Timestamp, offset: Offset) -> bool {
        let Some(running) = self.running.as_mut() else { return false };
        let before = self.view.clone();
        for job in jobs {
            let account = job.account.get();
            let (followed, reads) = match job.kind {
                JobKind::Calendar(CalendarJob::Discover { .. }) => (true, false),
                JobKind::Calendar(CalendarJob::Sync { .. }) => (true, true),
                _ => (false, false),
            };
            if followed
                && running.accounts.contains_key(&account)
                && running.out.len() < MOST_JOBS
                && running.out.insert(job.id, (account, reads)).is_none()
            {
                running.of = running.of.saturating_add(usize::from(reads));
                running.current = running.current.or(Some(account));
            }
        }
        // A job of an account that is gone has no worker to answer it.
        running.out.retain(|_, (account, _)| alive.contains(account));
        if running.out.is_empty() {
            self.view.state = match running.failure {
                Some((account, why)) => SyncState::Failed { account: running.accounts.get(&account).cloned(), why },
                None => SyncState::Synced { at: super::moment(now, offset) },
            };
            self.running = None;
        } else {
            self.view.state = syncing(running);
        }
        self.view != before
    }

    /// Hears `event` after the engine applied it (`dropped` when the engine left it out), at `now` on the clock of
    /// `offset`: a calendar read notes the time, and the end of a job followed counts it done (a failure noted, the
    /// first one kept). Calendars found that the engine dropped (listed on another server than the account's) read
    /// nothing: that is the account's failure, [`Why::Elsewhere`]. Whether the event was one of the sync's.
    pub fn heard(&mut self, event: &Event, dropped: bool, now: Timestamp, offset: Offset) -> bool {
        if matches!(event.kind, EventKind::Calendar(CalendarReport::Synced { .. })) {
            self.view.read = super::moment(now, offset).or(self.view.read);
        }
        let Some(running) = self.running.as_mut() else { return false };
        let ends = matches!(event.kind, EventKind::Calendar(_) | EventKind::Failed { .. });
        let Some((account, reads)) = event.job.filter(|_| ends).and_then(|job| running.out.remove(&job)) else { return false };
        running.done = running.done.saturating_add(usize::from(reads));
        running.current = running.out.values().next().map(|(account, _)| *account).or(Some(account));
        let why = match &event.kind {
            EventKind::Calendar(CalendarReport::Failed { kind, .. }) | EventKind::Failed { kind, .. } => Some(Why::of(*kind)),
            EventKind::Calendar(CalendarReport::Found { .. }) if dropped => Some(Why::Elsewhere),
            _ => None,
        };
        if let Some(why) = why {
            running.failure = running.failure.or(Some((account, why)));
        }
        self.view.state = syncing(running);
        true
    }

    /// What the panel's foot shows.
    #[must_use]
    pub fn view(&self) -> SyncView {
        self.view.clone()
    }

    /// How many times the command ran for a press of the owner's.
    #[must_use]
    pub fn started(&self) -> u64 {
        self.started
    }
}

/// The state of a sync under way.
fn syncing(running: &Running) -> SyncState {
    let account = running.current.and_then(|id| running.accounts.get(&id)).cloned().unwrap_or_default();
    SyncState::Syncing { done: running.done, of: running.of, account }
}

/// An account's name for the panel: no control character or bidi override, at most [`MOST_NAME`] characters.
fn clean(name: &str) -> String {
    let plain = crate::component::plain(name);
    crate::component::capped(&plain, MOST_NAME).0.trim().to_owned()
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
