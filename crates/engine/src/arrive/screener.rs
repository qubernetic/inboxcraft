// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The screener: who the owner knows, and where the first mail of anyone else goes.
//!
//! A sender is known, across the accounts, once the owner received mail from it, wrote to it or accepted it:
//! the first sync of a folder learns the senders of the mail it holds (not of the junk, the trash, the drafts, the
//! virtual folders or our own), every message in the sent folder learns its recipients, and mail that comes into the
//! inbox while the screener is off learns its sender. The owner's own addresses and delivery reports (`MAILER-DAEMON`,
//! `postmaster`, and any message the bounce stage before this one found to be a report) are never screened and teach
//! nothing.
//!
//! Later mail into the inbox from a sender neither known nor decided is, by [`ScreenerMode`]: held in our own
//! `InboxCraft/Screener` folder (made on the server the first time), left in the inbox and marked, or let in. Mail
//! from a blocked sender goes to `InboxCraft/Blocked` in every mode; the reader never fetches its body or its remote
//! content.

use inboxcraft_model::{
    Account, EmailAddress, Envelope, Folder, FolderId, FolderListing, FolderPath, JobKind, Message, Role, RoleMap, Screening, Sender,
};
use inboxcraft_store::{Change, Storage};

use super::Arrival;
use crate::EngineError;
use crate::act;
use crate::sync::Plan;

/// What the screener does with the first mail of a sender the owner does not know.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ScreenerMode {
    /// Held in `InboxCraft/Screener` until the owner accepts or blocks the sender.
    Before,
    /// Left in the inbox, marked as from a new sender.
    Inside,
    /// Let in; its sender is known from then on.
    #[default]
    Off,
}

impl ScreenerMode {
    /// Every mode.
    pub const ALL: [Self; 3] = [Self::Before, Self::Inside, Self::Off];

    /// The mode's name, as commands write it.
    ///
    /// ```
    /// use inboxcraft_engine::ScreenerMode;
    ///
    /// assert_eq!(ScreenerMode::Before.name(), "before");
    /// assert_eq!(ScreenerMode::from_name("inside"), Some(ScreenerMode::Inside));
    /// ```
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::Inside => "inside",
            Self::Off => "off",
        }
    }

    /// The mode named `name`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.name() == name)
    }
}

/// The roles whose mail teaches the screener nothing.
pub(crate) const SILENT: [Role; 8] = [Role::Drafts, Role::Junk, Role::Trash, Role::All, Role::Flagged, Role::Screener, Role::Snoozed, Role::Blocked];

/// The local parts of the senders of delivery reports.
const REPORTERS: [&str; 2] = ["mailer-daemon", "postmaster"];

/// The stage: learns the senders the owner knows and screens the inbox's new mail.
pub(crate) fn stage(arrival: &Arrival<'_>, message: &Message, plan: &mut Plan) -> Result<(), EngineError> {
    // A delivery report, as the bounce stage found: let in, and its sender teaches nothing.
    if plan.reports.contains(&message.id()) {
        return Ok(());
    }
    let store = arrival.store;
    let accounts = store.accounts()?;
    match arrival.role {
        Some(Role::Sent) => {
            for address in recipients(message.summary().envelope()) {
                if teaches(&accounts, address) {
                    learn(plan, address, store.sender(address)?);
                }
            }
            Ok(())
        }
        Some(Role::Inbox) => inbox(arrival, &accounts, message, plan),
        Some(role) if SILENT.contains(&role) => Ok(()),
        _ => {
            if arrival.first_sync
                && let Some(address) = sender_of(message).filter(|address| teaches(&accounts, address))
            {
                learn(plan, address, store.sender(address)?);
            }
            Ok(())
        }
    }
}

/// New mail in the inbox.
fn inbox(arrival: &Arrival<'_>, accounts: &[Account], message: &Message, plan: &mut Plan) -> Result<(), EngineError> {
    let Some(address) = sender_of(message).filter(|address| teaches(accounts, address)) else { return Ok(()) };
    let record = arrival.store.sender(address)?;
    let screening = record.as_ref().and_then(|record| record.screening);
    if screening == Some(Screening::Blocked) {
        return hold(arrival, message, plan, Role::Blocked);
    }
    if screening.is_some() || record.as_ref().is_some_and(|record| record.known) {
        return Ok(());
    }
    match plan.state.screener {
        _ if arrival.first_sync => learn(plan, address, record),
        ScreenerMode::Off => learn(plan, address, record),
        ScreenerMode::Inside => {}
        ScreenerMode::Before => return hold(arrival, message, plan, Role::Screener),
    }
    Ok(())
}

/// Moves `message` to our own folder with `role`, made first when the account has none.
fn hold(arrival: &Arrival<'_>, message: &Message, plan: &mut Plan, role: Role) -> Result<(), EngineError> {
    let store = arrival.store;
    let account = arrival.account;
    let mut folders = store.folders(account)?;
    for change in plan.batch.changes() {
        if let Change::PutFolder(folder) = change
            && folder.account() == account
            && !folders.iter().any(|held| held.id() == folder.id())
        {
            folders.push(folder.clone());
        }
    }
    let held = plan.state.accounts.entry(account).or_default();
    let next = &mut plan.state.next_folder;
    let Some((to, made)) = own_folder(&folders, &held.roles, arrival.folder, role, || {
        let id = FolderId::new(*next);
        *next = next.saturating_add(1);
        id
    }) else {
        return Ok(());
    };
    if let Some(job) = made {
        folders.push(to.clone());
        let questions = held.roles.detect(&folders);
        held.ask.extend(questions);
        held.ask.retain(|role| held.roles.folder(*role).is_none());
        plan.change(Change::PutFolder(to.clone()));
        plan.kept_job(account, job);
    }
    let mut taken = act::taken(store, to.id())?;
    for change in plan.batch.changes() {
        if let Change::PutMessage(placed) = change
            && placed.folder() == to.id()
        {
            taken.insert(placed.summary().uid());
        }
    }
    if let Some(effects) = act::relocate_beside(store, &plan.state, message, &to, &mut taken)? {
        plan.effects(effects);
    }
    Ok(())
}

/// The folder of `folders` with our own `role`, or a new one to make (with the job that makes it on the server) under
/// the id `id` gives, beside the inbox `inbox`. `None` when the account's folders have no hierarchy to put it in.
pub(crate) fn own_folder(
    folders: &[Folder],
    roles: &RoleMap,
    inbox: FolderId,
    role: Role,
    id: impl FnOnce() -> FolderId,
) -> Option<(Folder, Option<JobKind>)> {
    if let Some(folder) = roles.folder(role).and_then(|id| folders.iter().find(|folder| folder.id() == id)) {
        return Some((folder.clone(), None));
    }
    let inbox = folders.iter().find(|folder| folder.id() == inbox)?;
    let path = own_path(folders, inbox.listing().path(), role)?;
    let folder = Folder::new(id(), inbox.account(), FolderListing::new(path.clone(), Vec::new(), true));
    Some((folder, Some(JobKind::CreateFolder { path })))
}

/// `InboxCraft/<role>` with the inbox's delimiter, under the inbox when every other folder is there (a server whose
/// personal folders all live below `INBOX.`).
fn own_path(folders: &[Folder], inbox: &FolderPath, role: Role) -> Option<FolderPath> {
    let delimiter = inbox.delimiter()?;
    let under = format!("{}{delimiter}", inbox.name());
    let mut others = folders.iter().map(|folder| folder.listing().path()).filter(|path| !path.is_inbox()).peekable();
    let below = others.peek().is_some() && others.all(|path| path.name().get(..under.len()).is_some_and(|start| start.eq_ignore_ascii_case(&under)));
    let prefix = if below { under.as_str() } else { "" };
    FolderPath::new(&format!("{prefix}InboxCraft{delimiter}{}", role.label()), Some(delimiter)).ok()
}

/// `record` of `address`, known from now on.
fn learn(plan: &mut Plan, address: &EmailAddress, record: Option<Sender>) {
    if let Some(sender) = learned(address, record) {
        plan.change(Change::PutSender(sender));
    }
}

/// `record` of `address` with `known` set; `None` when it was known already.
pub(crate) fn learned(address: &EmailAddress, record: Option<Sender>) -> Option<Sender> {
    let mut sender = record.unwrap_or_else(|| Sender::new(address.clone()));
    (!sender.known).then(|| {
        sender.known = true;
        sender
    })
}

/// The address a message is from: its first sender's.
pub(crate) fn sender_of(message: &Message) -> Option<&EmailAddress> {
    message.summary().envelope().from().first().map(inboxcraft_model::Address::email)
}

/// Every recipient of a message: To, Cc and Bcc.
pub(crate) fn recipients(envelope: &Envelope) -> impl Iterator<Item = &EmailAddress> {
    envelope.to().iter().chain(envelope.cc()).chain(envelope.bcc()).map(inboxcraft_model::Address::email)
}

/// Whether mail from or to `address` says something about a sender: it is not one of `accounts`' own addresses and
/// not the sender of delivery reports.
pub(crate) fn teaches(accounts: &[Account], address: &EmailAddress) -> bool {
    let own = accounts.iter().any(|account| account.address().as_str().eq_ignore_ascii_case(address.as_str()));
    let reporter = REPORTERS.iter().any(|name| address.local_part().eq_ignore_ascii_case(name));
    !own && !reporter
}

/// Whether the owner has decided nothing about `address` and does not know it.
pub(crate) fn undecided(store: &dyn Storage, address: &EmailAddress) -> Result<bool, EngineError> {
    Ok(store.sender(address)?.is_none_or(|record| !record.known && record.screening.is_none()))
}

/// Whether `message` is from a new sender, as a list marks it: the screener is on, the message is in the inbox or the
/// screener folder, and its sender is one the screener would hold.
pub(crate) fn is_new(store: &dyn Storage, mode: ScreenerMode, role: Option<Role>, accounts: &[Account], message: &Message) -> Result<bool, EngineError> {
    if mode == ScreenerMode::Off || !matches!(role, Some(Role::Inbox | Role::Screener)) {
        return Ok(false);
    }
    match sender_of(message) {
        Some(address) if teaches(accounts, address) => undecided(store, address),
        _ => Ok(false),
    }
}

#[cfg(test)]
#[path = "screener_tests.rs"]
pub(crate) mod tests;
