// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `Session`, the command registry, undo and the triage rules.
//!
//! Layer 4 (see `docs/architecture.md`): it emits jobs and applies events, and does no network I/O.
//!
//! - **Everything is a command.** A [`CommandSpec`] has a stable id (`mail.list`), an [`AgentClass`] and a function;
//!   menus, shortcuts, the palette, the CLI, the control channel and MCP all dispatch by id through
//!   [`Session::run`]. The [`Registry`] refuses a command without a class, so an unclassified command never runs.
//! - A command only reads ([`Context`]) and returns an [`Outcome`]: its JSON answer, the store changes and the jobs
//!   it asks for. The session makes them after the command returned, so a failed command changes nothing, and a
//!   command that panics despite the rules becomes [`EngineError::Panicked`] with the session as it was.
//! - Jobs out, events in: [`Session::start`] asks each account's `net` worker for its folder list,
//!   [`Session::take_jobs`] hands the queued [`inboxcraft_model::Job`]s over, and [`Session::apply`] turns each
//!   [`inboxcraft_model::Event`] into store changes and the next jobs (a folder list brings the folder syncs, the
//!   inbox first, and the watch on the inbox). Events for something the session does not hold, or with UIDs of an
//!   older UIDVALIDITY, are dropped.
//! - The read commands, all `safe`: `account.list`, `folder.list` (folders with their roles, the unified views by
//!   role and the roles to ask the owner about) and `mail.list` (one folder, or one role across every account merged
//!   by date, newest first; both narrowed to one account with `account`).
//! - Parameters are JSON from outside and hostile: unknown fields are refused, ids that name nothing are
//!   [`EngineError::NotFound`], and every command is run with junk by the `hostile_params_never_panic` test.
//!
//! ```
//! use inboxcraft_engine::{Caller, Session};
//! use inboxcraft_store::{Memory, StoreConfig};
//! use inboxcraft_time::{FakeClock, Timestamp};
//! use serde_json::json;
//!
//! let mut session = Session::new(Box::new(Memory::new(StoreConfig::default())), Box::new(FakeClock::new(Timestamp::UNIX_EPOCH)))?;
//! assert_eq!(session.run_as(Caller::Agent, "account.list", &json!({}))?, json!({ "accounts": [] }));
//! assert_eq!(session.run("account.list", &json!({ "typo": 1 })).unwrap_err().code(), "bad_params");
//! # Ok::<(), inboxcraft_engine::EngineError>(())
//! ```

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod class;
mod cmd;
mod error;
mod json;
mod params;
mod registry;
mod session;
mod sync;
mod view;

pub use class::{AgentClass, Caller};
pub use cmd::MAX_LIST;
pub use error::{EngineError, MAX_ERROR_TEXT};
pub use registry::{CommandSpec, Registry, Run, is_command_id};
pub use session::{Applied, Context, Outcome, Session};

/// The crate's package name.
///
/// ```
/// assert_eq!(inboxcraft_engine::name(), "inboxcraft-engine");
/// ```
#[must_use]
pub fn name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
