// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The owner requests: `owner` commands an agent asked the owner to allow once, over the control channel, waiting for
//! the owner's answer in the window.
//!
//! One question at a time: only the request in front is asked, and only its two minutes ([`PATIENCE`]) run, from the
//! moment it came to the front. At most [`MOST_WAITING`] wait; one more is turned away at once. A request is settled
//! only by the owner's answer or by its time running out, which is a refusal: nothing here runs a command, and
//! nothing but [`OwnerRequests::settle_front`] takes a request out of the queue.

use std::collections::VecDeque;
use std::time::Duration;

use inboxcraft_engine::EngineError;
use inboxcraft_time::Timestamp;
use serde_json::Value;

/// The most requests that wait for the owner at a time.
pub const MOST_WAITING: usize = 8;

/// How long the owner has to answer the request in front; no answer by then is a refusal.
pub const PATIENCE: Duration = Duration::from_secs(120);

/// Which request a settlement answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ticket(u64);

/// How a request ended.
#[derive(Debug)]
pub enum Settled {
    /// The owner allowed it: the command ran, with this answer or refusal.
    Ran(Result<Value, EngineError>),
    /// The owner denied it; nothing ran.
    Denied,
    /// The owner did not answer within [`PATIENCE`]; nothing ran.
    TimedOut,
}

/// One request waiting for the owner.
#[derive(Debug)]
pub(crate) struct OwnerRequest {
    /// Which request it is.
    pub(crate) ticket: Ticket,
    /// The name the asking client gave itself, as the channel knows it: the agent's words, never the app's.
    #[cfg_attr(not(test), expect(dead_code, reason = "the question that shows who asks is drawn next (#443)"))]
    pub(crate) client: Option<String>,
    /// The `owner` command asked for.
    pub(crate) command: &'static str,
    /// Its parameters, as the agent sent them.
    pub(crate) params: Value,
    /// When it came to the front, so its two minutes run; `None` until [`OwnerRequests::expire`] first sees it there.
    pub(crate) since: Option<Timestamp>,
    /// Asked right before an Allow runs it: false when the asker withdrew it meanwhile, so it runs not.
    pub(crate) may_run: MayRun,
}

/// The requests waiting for the owner, in the order they came, and those settled but not yet answered.
/// Whether a request may still run, asked of the side that put it (see [`crate::InboxApp::ask_owner_if`]).
pub(crate) struct MayRun(Box<dyn Fn() -> bool + Send>);

impl MayRun {
    /// The request may run when `check` says so, at the moment it would.
    #[must_use]
    pub(crate) fn new(check: Box<dyn Fn() -> bool + Send>) -> Self {
        Self(check)
    }

    /// Whether the request may run now.
    #[must_use]
    pub(crate) fn check(&self) -> bool {
        (self.0)()
    }
}

impl Default for MayRun {
    /// Always.
    fn default() -> Self {
        Self(Box::new(|| true))
    }
}

impl std::fmt::Debug for MayRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MayRun")
    }
}

#[derive(Debug, Default)]
pub(crate) struct OwnerRequests {
    waiting: VecDeque<OwnerRequest>,
    settled: Vec<(Ticket, Settled)>,
    next: u64,
}

impl OwnerRequests {
    /// Queues the request of `client` for `command` with `params`; `None` when [`MOST_WAITING`] wait already.
    #[cfg(test)]
    pub(crate) fn push(&mut self, client: Option<&str>, command: &'static str, params: Value) -> Option<Ticket> {
        self.push_if(client, command, params, MayRun::default())
    }

    /// Queues the request of `client` for `command` with `params`, which runs on Allow only when `may_run` says so
    /// then; `None` when [`MOST_WAITING`] wait already.
    pub(crate) fn push_if(&mut self, client: Option<&str>, command: &'static str, params: Value, may_run: MayRun) -> Option<Ticket> {
        if self.waiting.len() >= MOST_WAITING {
            return None;
        }
        let ticket = Ticket(self.next);
        self.next = self.next.wrapping_add(1);
        self.waiting.push_back(OwnerRequest { ticket, client: client.map(str::to_owned), command, params, since: None, may_run });
        Some(ticket)
    }

    /// The request the owner is asked about.
    pub(crate) fn front(&self) -> Option<&OwnerRequest> {
        self.waiting.front()
    }

    /// Settles the request in front as `settled`; nothing when none waits.
    pub(crate) fn settle_front(&mut self, settled: Settled) {
        if let Some(request) = self.waiting.pop_front() {
            self.settled.push((request.ticket, settled));
        }
    }

    /// Takes the request `ticket` out of the queue without settling it: its asker is gone, and nobody is answered. One
    /// that comes to the front for it is dated on the next [`OwnerRequests::expire`]. Whether it was waiting.
    pub(crate) fn withdraw(&mut self, ticket: Ticket) -> bool {
        let Some(at) = self.waiting.iter().position(|request| request.ticket == ticket) else { return false };
        self.waiting.remove(at);
        if at == 0
            && let Some(next) = self.waiting.front_mut()
        {
            next.since = None;
        }
        true
    }

    /// Dates the request in front when it has no date yet (a clock that went back since dates it not again: it has
    /// waited nothing until the clock is past its date once more, so it never times out early), and
    /// settles it as [`Settled::TimedOut`] once [`PATIENCE`] has gone by since; the one behind it is dated `now`, so its
    /// two minutes run from then. One behind a request the owner answered is dated on the next call.
    pub(crate) fn expire(&mut self, now: Timestamp) {
        let Some(front) = self.waiting.front_mut() else { return };
        let Some(since) = front.since else {
            front.since = Some(now);
            return;
        };
        match now.duration_since(since) {
            Some(waited) if waited >= PATIENCE => {
                self.settle_front(Settled::TimedOut);
                // The one behind it comes to the front now: its two minutes start.
                if let Some(next) = self.waiting.front_mut() {
                    next.since = Some(now);
                }
            }
            Some(_) | None => {}
        }
    }

    /// How long until the request in front runs out of time: `Some(ZERO)` when it has no date yet, `None` when none
    /// waits.
    pub(crate) fn due_in(&self, now: Timestamp) -> Option<Duration> {
        let front = self.waiting.front()?;
        let Some(since) = front.since else { return Some(Duration::ZERO) };
        let waited = now.duration_since(since).unwrap_or_default();
        Some(PATIENCE.saturating_sub(waited))
    }

    /// The requests settled since the last call, oldest first.
    pub(crate) fn take_settled(&mut self) -> Vec<(Ticket, Settled)> {
        std::mem::take(&mut self.settled)
    }

    /// Whether a request waits for the owner.
    pub(crate) fn waiting(&self) -> bool {
        !self.waiting.is_empty()
    }
}

#[cfg(test)]
#[path = "owner_request_tests.rs"]
mod tests;
