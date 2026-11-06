// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Threads: messages linked by their `Message-ID`, `In-Reply-To` and `References` fields.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{Message, MessageId, ThreadId};

/// A thread: its messages, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thread {
    id: ThreadId,
    messages: Vec<MessageId>,
}

impl Thread {
    /// The thread `id` of `messages`, oldest first.
    #[must_use]
    pub fn new(id: ThreadId, messages: Vec<MessageId>) -> Self {
        Self { id, messages }
    }

    /// Our id.
    #[must_use]
    pub fn id(&self) -> ThreadId {
        self.id
    }

    /// The messages, oldest first.
    #[must_use]
    pub fn messages(&self) -> &[MessageId] {
        &self.messages
    }
}

/// Union-find over message positions, without recursion.
struct Sets(Vec<usize>);

impl Sets {
    fn root(&mut self, mut at: usize) -> usize {
        while let Some(&parent) = self.0.get(at) {
            if parent == at {
                break;
            }
            // Path halving: point at the grandparent on the way up.
            let grandparent = self.0.get(parent).copied().unwrap_or(parent);
            if let Some(slot) = self.0.get_mut(at) {
                *slot = grandparent;
            }
            at = parent;
        }
        at
    }

    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.root(a), self.root(b));
        if let Some(slot) = self.0.get_mut(a.max(b)) {
            *slot = a.min(b);
        }
    }
}

/// Groups `messages` into threads: two messages are in one thread when one names the other, or both name the same
/// message, in `Message-ID`, `In-Reply-To` or `References`. Replies and forwards that carry these fields join their
/// thread, across folders and accounts, so the sent copy and the received copy of one message share a thread.
///
/// Each thread lists its messages oldest first (by `Date`, else the arrival time; a message with neither first; ties
/// by our id); the threads come newest first, by their newest message. Subjects are not used.
#[must_use]
pub fn group_threads(messages: &[Message]) -> Vec<Vec<MessageId>> {
    let mut sets = Sets((0..messages.len()).collect());
    let mut first_with: HashMap<&str, usize> = HashMap::new();
    for (at, message) in messages.iter().enumerate() {
        let envelope = message.summary().envelope();
        let links = envelope.message_id().into_iter().chain(envelope.in_reply_to()).chain(envelope.references());
        for link in links {
            let first = *first_with.entry(link.as_str()).or_insert(at);
            sets.join(first, at);
        }
    }
    let mut by_root: HashMap<usize, Vec<&Message>> = HashMap::new();
    for (at, message) in messages.iter().enumerate() {
        by_root.entry(sets.root(at)).or_default().push(message);
    }
    let when = |message: &Message| {
        let summary = message.summary();
        (summary.envelope().date().or(summary.received()), message.id())
    };
    let mut threads: Vec<Vec<&Message>> = by_root.into_values().collect();
    for thread in &mut threads {
        thread.sort_by_key(|message| when(message));
    }
    threads.sort_by(|a, b| {
        let newest = |thread: &[&Message]| thread.last().map(|message| when(message));
        newest(b).cmp(&newest(a))
    });
    threads.into_iter().map(|thread| thread.into_iter().map(Message::id).collect()).collect()
}

#[cfg(test)]
#[path = "thread_tests.rs"]
mod tests;
