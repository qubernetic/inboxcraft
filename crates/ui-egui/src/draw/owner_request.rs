// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window's side of the owner requests ([`crate::owner_request`]): an agent's request for an `owner` command
//! waits in the window's queue, and runs, for the owner, only on the owner's answer.
//!
//! Nothing here runs a command but [`InboxApp::answer_owner_request`] with `allow`, which the window's Allow button
//! calls: no time out, no second request and no headless store is an approval.

use std::time::Duration;

use inboxcraft_engine::AgentClass;
use serde_json::Value;

use super::InboxApp;
use crate::owner_request::{Settled, Ticket};
use crate::tr;

impl InboxApp {
    /// Queues an agent's request to run the `owner` command `command` with `params`, from the client that named
    /// itself `client`, for the owner to answer. `None` when [`MOST_WAITING`](crate::owner_request::MOST_WAITING)
    /// wait already, or when `command` is no `owner` command (a `safe` one needs no asking, a `never` one is never
    /// offered, an unknown one is none).
    pub fn ask_owner(&mut self, client: Option<&str>, command: &str, params: &Value) -> Option<Ticket> {
        let (command, class) = self.mailbox.class_of(command)?;
        if class != AgentClass::Owner {
            return None;
        }
        self.owner_requests.push(client, command, params.clone())
    }

    /// The owner's answer to the request in front: with `allow`, its command runs for the owner, one step of the
    /// window's undo, and is settled with its answer or refusal; without, it is settled [`Settled::Denied`] and
    /// nothing runs. A request whose two minutes are over is settled [`Settled::TimedOut`] instead, and the answer
    /// goes to no other request: the one behind it was not what the owner saw.
    pub fn answer_owner_request(&mut self, allow: bool) {
        let Some(asked) = self.owner_requests.front().map(|front| front.ticket) else { return };
        self.expire_owner_requests();
        if self.owner_requests.front().map(|front| front.ticket) != Some(asked) {
            return;
        }
        if !allow {
            self.owner_requests.settle_front(Settled::Denied);
            return;
        }
        let Some((command, params)) = self.owner_requests.front().map(|front| (front.command, front.params.clone())) else { return };
        let text = tr!("An agent ran {command}", command = command);
        let ran = self.mailbox.run_for_owner_request(command, &params, &text);
        self.owner_requests.settle_front(Settled::Ran(ran));
    }

    /// Settles the request in front as [`Settled::TimedOut`] once the owner has not answered it for two minutes, by
    /// the mailbox's clock; it starts the two minutes of a request that has just come to the front.
    pub fn expire_owner_requests(&mut self) {
        let now = self.mailbox.now();
        self.owner_requests.expire(now);
    }

    /// How long until the request in front runs out of time, so the window looks again then; `None` when none
    /// waits.
    #[must_use]
    pub fn owner_request_due_in(&self) -> Option<Duration> {
        self.owner_requests.due_in(self.mailbox.now())
    }

    /// The requests settled since the last call, oldest first, for their askers' answers.
    pub fn take_settled(&mut self) -> Vec<(Ticket, Settled)> {
        self.owner_requests.take_settled()
    }

    /// Whether a request waits for the owner's answer.
    #[must_use]
    pub fn asks_owner(&self) -> bool {
        self.owner_requests.waiting()
    }
}

#[cfg(test)]
#[path = "owner_request_tests.rs"]
mod tests;
