// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Waiting for changes in a watched folder: IDLE (RFC 2177) where the server offers it, a resync every
//! [`WatchConfig::poll`] otherwise.
//!
//! A worker cannot wait on its socket and its job queue at once, so during IDLE it reads with a short timeout
//! ([`WatchConfig::tick`]) and looks at the queue in between: a job ends the IDLE within a tick. Whatever the server
//! tells of the open folder (new, removed or changed messages, a new UIDVALIDITY) ends the IDLE too, and the worker
//! resyncs the folder before it idles again: the resync, not the untagged responses, reports the change, so a change
//! is reported the same way whether it came while idling or while the worker was offline. IDLE is renewed every
//! [`WatchConfig::renew`], before servers end it on their side (RFC 2177 asks for at most 29 minutes).

use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::Duration;

use inboxcraft_model::Job;
use inboxcraft_proto::imap::{Client, Event};

use crate::{ImapConnection, NetError};

/// How a worker watches a folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchConfig {
    /// How long one read waits during IDLE before the worker looks for a job: the most a job waits for an IDLE to end.
    pub tick: Duration,
    /// How long one IDLE lasts before it is renewed.
    pub renew: Duration,
    /// How often a server without IDLE is asked again (a resync).
    pub poll: Duration,
}

impl Default for WatchConfig {
    /// A quarter of a second, 25 minutes, 5 minutes.
    fn default() -> Self {
        Self { tick: Duration::from_millis(250), renew: Duration::from_secs(25 * 60), poll: Duration::from_secs(5 * 60) }
    }
}

/// Why a wait ended, and what came meanwhile.
#[derive(Debug, Default)]
pub(crate) struct Wake {
    /// A job came: it goes to the queue.
    pub(crate) job: Option<Job>,
    /// The engine stopped the worker.
    pub(crate) closed: bool,
    /// The server told of a change, ended the IDLE itself, or the poll time is up: resync.
    pub(crate) changed: bool,
    /// The server refused IDLE: poll from now on.
    pub(crate) refused: bool,
}

/// One IDLE in the open folder, until a job comes, the engine stops the worker, the server tells of a change, or it is
/// time to renew it; then DONE and its answer.
pub(crate) fn idle(connection: &mut ImapConnection, config: &WatchConfig, queue: &Receiver<Job>) -> Result<Wake, NetError> {
    let clock = Arc::clone(connection.clock());
    let started = clock.now();
    let tag = connection.start(Client::idle)?;
    let mut wake = Wake::default();
    let mut ended = false;
    loop {
        connection.listen(config.tick, &mut |event| match event {
            Event::Completed { tag: done, outcome } if done == tag => {
                ended = true;
                // Ended by the server: something may have been missed. Refused: IDLE does not work here.
                wake.refused = outcome.is_err();
                wake.changed = outcome.is_ok();
            }
            event => wake.changed |= is_change(&event),
        })?;
        if ended {
            return Ok(wake);
        }
        if wake.changed {
            break;
        }
        match queue.try_recv() {
            Ok(job) => {
                wake.job = Some(job);
                break;
            }
            Err(TryRecvError::Disconnected) => {
                wake.closed = true;
                break;
            }
            Err(TryRecvError::Empty) => {}
        }
        if clock.now().duration_since(started).is_none_or(|idled| idled >= config.renew) {
            break;
        }
    }
    connection.done()?;
    // A change told between the last read and DONE still counts.
    let _answer = connection.wait(&tag, &mut |event| wake.changed |= is_change(&event))?;
    Ok(wake)
}

/// Waits [`WatchConfig::poll`] for a job, for a server without IDLE; a wait that ends without one asks for a resync.
pub(crate) fn poll(config: &WatchConfig, queue: &Receiver<Job>) -> Wake {
    match queue.recv_timeout(config.poll) {
        Ok(job) => Wake { job: Some(job), ..Wake::default() },
        Err(RecvTimeoutError::Timeout) => Wake { changed: true, ..Wake::default() },
        Err(RecvTimeoutError::Disconnected) => Wake { closed: true, ..Wake::default() },
    }
}

/// Whether `event` tells of a change in the open folder.
fn is_change(event: &Event) -> bool {
    matches!(event, Event::Exists(_) | Event::Expunged(_) | Event::Vanished { .. } | Event::Fetched(_) | Event::UidValidityChanged { .. })
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
