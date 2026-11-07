// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The arriving-mail pipeline: every message new to us passes each stage in order, once the sync has put it in the
//! store's batch (`crate::sync`).
//!
//! A stage is a file of its own in `arrive/` with a `pub(crate) fn stage` of the type [`Stage`], and one more name in
//! the `stages!` line below; the order of that line is the order the stages run in. A stage reads the message and
//! where it arrived, and adds to the plan: store changes, kept jobs, or an act of `crate::act` such as a move to
//! another folder. What a stage adds lands with the sync's own changes in one batch, so a later stage sees an earlier
//! one's decision only in the plan.

use inboxcraft_model::{AccountId, FolderId, Message, Role};
use inboxcraft_store::Storage;
use inboxcraft_time::Timestamp;

use crate::EngineError;
use crate::sync::Plan;

/// Where messages arrived.
pub(crate) struct Arrival<'a> {
    /// The store as it was before the sync's batch.
    pub(crate) store: &'a dyn Storage,
    /// The time the event is applied at.
    #[cfg_attr(not(test), expect(dead_code, reason = "the stages that read it come with the outbox and calendar slices"))]
    pub(crate) now: Timestamp,
    /// The account.
    pub(crate) account: AccountId,
    /// The folder.
    pub(crate) folder: FolderId,
    /// The folder's role, if it has one.
    pub(crate) role: Option<Role>,
    /// Whether this is the folder's first sync, which brings every message it holds and not only new mail.
    pub(crate) first_sync: bool,
}

/// One stage: what it does with one arriving message.
pub(crate) type Stage = fn(&Arrival<'_>, &Message, &mut Plan) -> Result<(), EngineError>;

/// Declares the stage modules and [`STAGES`], in the order they run.
macro_rules! stages {
    ($($name:ident),* $(,)?) => {
        $(pub(crate) mod $name;)*

        /// Every stage, in order.
        pub(crate) const STAGES: &[Stage] = &[$($name::stage),*];
    };
}

stages!(screener);

/// Runs every message of `messages`, which arrived as `arrival` says, through `stages` in order.
pub(crate) fn arrived(stages: &[Stage], arrival: &Arrival<'_>, messages: &[Message], plan: &mut Plan) -> Result<(), EngineError> {
    for message in messages {
        for stage in stages {
            stage(arrival, message, plan)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "arrive_tests.rs"]
mod tests;
