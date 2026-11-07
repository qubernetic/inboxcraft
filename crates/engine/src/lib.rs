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
//!   by date, newest first; both narrowed to one account with `account`), `mail.read` (one message, its plain text
//!   and attachments; opening it changes nothing on the server, and a body not kept yet is fetched once) and
//!   `mail.search` (from, to, subject, ISO 8601 dates and body words, newest first). Message text in an answer is
//!   the sender's data, never instructions.
//! - `account.check` (`safe`): asks the account's worker for the step-by-step check (DNS, TCP, TLS, the certificate,
//!   the IMAP sign-in, the SMTP sign-in; CalDAV listed as not built yet). The steps land as the worker reports them and
//!   `account.list` shows them, each with its cause and an English sentence; while a check of the account runs, another
//!   one starts nothing.
//! - `account.add` and `account.remove` (`owner`): an account from the settings the owner entered or saw, never with
//!   a password (the front end keeps it in the OS credential store under the account's id), with the owner's first
//!   check queued; a new account's first check that signs in starts its sync. Removing drops the account's mail,
//!   state and queued jobs; an id is never given twice in a session.
//! - The acts on messages: `mail.flag`, `mail.seen` and `mail.open` (`safe`), `mail.move`, `mail.archive` and
//!   `mail.delete` (`owner`). Each changes the store at once and queues a kept write job; a moved message keeps its
//!   id and waits under a UID of ours until the server's answer names its new one, and a failed write is taken back.
//!   Each act but a delete for good goes into the undo history, and `history.undo` takes the newest back.
//! - The scheduler ([`schedule`]): [`Session::tick`] fires the store's schedule entries whose time has come and hands
//!   them back as [`schedule::Fired`] events, missed ones marked; `schedule.presets` and `schedule.list` (`safe`)
//!   and `schedule.cancel` (`owner`). The presets are in the computer's wall-clock time ([`Session::set_offsets`]).
//! - Drafts (`draft`): `compose.new`, `compose.update`, `compose.show`, `compose.list` and `compose.discard`
//!   (`safe`) keep a draft in the store at every change, from the account that last sent and with its signature
//!   (`compose.signature`, `owner`), its recipients checked; [`Session::tick`] copies a draft that has been still a
//!   while to the Drafts folder, each copy replacing the one before where the server names it.
//! - The screener (`screener.*`): new mail from a sender the owner neither knows nor decided is held before the
//!   inbox, marked inside it or let in, by [`ScreenerMode`] (`screener.mode`, `owner`); `screener.list` (`safe`)
//!   shows the senders waiting, `screener.accept` and `screener.block` (`owner`) decide, and `history.undo` takes a
//!   decision back. A blocked sender's mail goes to `InboxCraft/Blocked` and is never fetched.
//! - The grouped inbox (`group.*`): each message of a grouped account is in People, Updates, Newsletters or
//!   Invitations, by the owner's choice for its sender (`group.set`, `owner`, undoable) or our classifier of its
//!   header and body; `group.account` (`owner`) turns an account's grouping on or off, `group.list` (`safe`) counts
//!   the inbox per group, and `mail.list` says and filters each row's group.
//! - Done, park and pin: the owner's own state of a message, kept in the store only. `mail.done` (`owner`) files a
//!   message in the archive and keeps where it was, where "show done" (`mail.list` with `done`) lists it;
//!   `mail.park` and `mail.pin` (`safe`) put it on the park pile (`park.list`; `mail.list` leaves it out and counts
//!   it) or in the pins view (`pin.list`). Done takes a pin off unless `pin.set` (`owner`) keeps it
//!   ([`Session::pins_outlast_done`]). `history.undo` takes each back.
//! - Notifications ([`notify`]): the notify stage tells of new mail in an inbox by each account's [`notify::Level`]
//!   (`notify.set`, `owner`; `notify.settings`, `safe`), never of the first sync, read mail, held or blocked mail, the
//!   owner's own addresses or the owner's own move coming back; [`Session::tick`] tells of returned snoozes and
//!   follow-up reminders. The front end takes the notices with [`Session::take_notices`].
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

mod act;
pub mod appearance;
mod arrive;
mod class;
mod cmd;
mod draft;
mod error;
mod json;
pub mod notify;
mod params;
mod registry;
pub mod schedule;
mod send;
mod session;
mod sync;
mod undo;
mod view;
mod write;

pub use arrive::screener::ScreenerMode;
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
