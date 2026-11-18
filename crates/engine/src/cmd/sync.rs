// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `sync`: how the syncing of every account stands, for a status line. A new command goes into [`COMMANDS`] and its
//! parameter names into `FIELDS`.
//!
//! `sync.status` (`safe`, no parameters) answers `{ state, done, of, last_synced, failing_accounts }`. It is kept
//! from what the workers tell: the connection of each account (`EventKind::Connection`), its last failure
//! (`EventKind::Failed`, the `failure` of `account.list`), the progress of its sync round
//! (`EventKind::SyncProgress`) and the end of one (`EventKind::SyncFinished`). The state is the worst that holds:
//!
//! - `failing`, first: an account stopped by a failure only the owner can mend (a refused or unanswered sign-in, no
//!   password, no login method, a certificate or encryption refused), connected or not;
//! - `offline`: at least one account and none signed in (no connection told yet, connecting, offline, or backing
//!   off), when the failures are of the network or the server (no route, a timeout, a refused connection);
//! - `failing`: an account's last connection or job failed, and it has not been online or ended a sync round since;
//! - `syncing`: an account's sync round has folders left, or a signed-in account has not ended a round yet;
//! - `synced`: none of these, and always with no account.
//!
//! `done` and `of` add up the rounds under way of every account, as last told (each `done` at most its `of`, the sums
//! saturate at `u32::MAX`); a round counts no longer once it ended, synced or failed, once its connection went
//! offline, or once the account's settings changed, so a new worker counts from its own first progress. `last_synced` is the newest end of a
//! sync round over all accounts in ISO 8601 UTC, `null` before one. `failing_accounts` counts the accounts with a
//! failure, not the failures.
//!
//! The state is the session's and bounded by the accounts: one progress and one time each, replaced by every event,
//! dropped with the account. An event for an account the session does not hold is dropped before it gets here, and
//! an end of a round that lies more than [`FUTURE_SLACK`] (five minutes) past the engine's clock is junk and dropped;
//! one less far ahead is taken as the clock, so it cannot pin `last_synced` ahead of it.

use std::time::Duration;

use inboxcraft_model::{AccountId, ConnectionState, ErrorKind};
use inboxcraft_time::Timestamp;
use serde_json::{Value, json};

use crate::params::{NoParams, decode};
use crate::session::{AccountState, State};
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome, json};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[STATUS];

/// The parameter names these commands read, for `hostile_params_never_panic`: `sync.status` reads none, so the name
/// a client would most likely send, to be refused.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["account"];

/// How far past the engine's clock the end of a sync round may lie: the worker's clock is the same computer's, so
/// anything further is junk. An end within it is taken as the engine's clock.
pub(crate) const FUTURE_SLACK: Duration = Duration::from_secs(5 * 60);

/// `sync.status`: how the syncing of every account stands.
pub(crate) const STATUS: CommandSpec = CommandSpec {
    id: "sync.status",
    class: Some(AgentClass::Safe),
    summary: "Show how syncing stands: synced, syncing, offline or failing, the progress, the last sync",
    run: status,
};

/// Takes the progress a worker told of `account`'s sync round: a round with nothing left (`done` at or above `of`),
/// synced or failed, is over and counts no longer, so what is kept always has `done` below `of`.
pub(crate) fn progress(state: &mut State, account: AccountId, done: u32, of: u32) {
    if let Some(held) = state.accounts.get_mut(&account) {
        held.progress = (done < of).then_some((done, of));
    }
}

/// Takes the end of `account`'s sync round at `at`; whether the event is kept. An end past `now` and
/// [`FUTURE_SLACK`] is junk and dropped, one past `now` within it counts as `now`; otherwise the round's progress and
/// the account's failure end, and the newest end is kept.
pub(crate) fn finished(state: &mut State, account: AccountId, at: Timestamp, now: Timestamp) -> bool {
    if at > now.saturating_add(FUTURE_SLACK) {
        return false;
    }
    let at = at.min(now);
    let Some(held) = state.accounts.get_mut(&account) else { return false };
    held.progress = None;
    held.failure = None;
    held.synced = Some(held.synced.map_or(at, |synced| synced.max(at)));
    true
}

/// Whether the account is signed in: one on its way to a connection is not connected yet, so an account retrying in
/// its back-off reads the same at every attempt.
fn connected(held: Option<&AccountState>) -> bool {
    matches!(held.and_then(|held| held.connection), Some(ConnectionState::Online | ConnectionState::Watching))
}

/// Whether a failure of `kind` stops the account until the owner acts (a password or a setting to fix, a certificate
/// or encryption refused), rather than waiting for the network or the server to come back.
const fn stops(kind: ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::LoginRefused
            | ErrorKind::LoginUnanswered
            | ErrorKind::NoLoginMethod
            | ErrorKind::CredentialMissing
            | ErrorKind::CertificateUnknownIssuer
            | ErrorKind::CertificateExpired
            | ErrorKind::CertificateNotYetValid
            | ErrorKind::CertificateWrongHost
            | ErrorKind::ClockFarOff
            | ErrorKind::Tls
            | ErrorKind::StartTlsMissing
            | ErrorKind::NotEncrypted
            | ErrorKind::OtherServer
    )
}

fn status(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let NoParams {} = decode(STATUS.id, params)?;
    let accounts = context.store().accounts()?;
    let held: Vec<Option<&AccountState>> = accounts.iter().map(|account| context.state().accounts.get(&account.id())).collect();
    let (mut done, mut of, mut failing, mut syncing, mut online, mut stopped) = (0_u32, 0_u32, 0_u32, false, false, false);
    let mut last: Option<Timestamp> = None;
    for state in &held {
        let signed_in = connected(*state);
        online |= signed_in;
        let Some(state) = state else { continue };
        // Signed in and no round ended yet: the first sync is under way, even before the worker tells its round.
        syncing |= signed_in && state.synced.is_none();
        if let Some((kind, _)) = state.failure {
            failing = failing.saturating_add(1);
            stopped |= stops(kind);
        }
        if let Some((told, all)) = state.progress {
            done = done.saturating_add(told);
            of = of.saturating_add(all);
            syncing = true;
        }
        if let Some(synced) = state.synced {
            last = Some(last.map_or(synced, |last| last.max(synced)));
        }
    }
    let state = if accounts.is_empty() {
        "synced"
    } else if stopped {
        "failing"
    } else if !online {
        "offline"
    } else if failing > 0 {
        "failing"
    } else if syncing {
        "syncing"
    } else {
        "synced"
    };
    Ok(Outcome::new(json!({
        "state": state,
        "done": done,
        "of": of,
        "last_synced": last.map(json::date),
        "failing_accounts": failing,
    })))
}
