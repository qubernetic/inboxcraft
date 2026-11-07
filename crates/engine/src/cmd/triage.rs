// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `triage`: the screener (`screener.*`), the grouped inbox (`group.*`), done, park and pin, snooze, and follow-up
//! reminders.
//! A new command goes into [`COMMANDS`] and its parameter names into `FIELDS`.
//!
//! The screener's mode is the owner's (`screener.mode`); turning it on learns every sender the owner has mail from or
//! wrote to (see `crate::arrive::screener`). `screener.list` shows the senders waiting, each with the mail held;
//! reading one (`mail.read`) is a preview that decides nothing. `screener.accept` lets the held mail in (or files it
//! as done, or opens the newest) and the sender's future mail too; `screener.block` files the held mail and the
//! future mail in `InboxCraft/Blocked`, made on the server the first time. Both are the owner's acts, and
//! `history.undo` takes either back: the sender as it was and the mail where it was.
//!
//! The grouped inbox (see `crate::arrive::group`): `group.set` (`owner`) puts a sender's mail, the kept and the
//! future, in a group of the owner's choice or back in the classifier's hands, and `history.undo` takes it back;
//! `group.account` (`owner`) turns an account's grouping on or off, kept with the account; `group.list` (`safe`)
//! counts the inbox's mail per group. `mail.list` says each row's group and lists one group.
//!
//! Done, park and pin are the owner's own state of a message, kept in the store and never on the server
//! ([`MessageState`]). `mail.done` (`owner`) files a message in its account's archive and keeps the folder it was in,
//! where "show done" (`mail.list` with `done`) lists it; done `false` brings it back there. A done message out of the
//! archive (its move failed) is filed again by the next done. `mail.park` (`safe`) puts a message on the park pile,
//! which `mail.list` leaves out and counts, and `park.list` lists; `mail.pin` (`safe`) pins it, and `pin.list` is the
//! pins view across the accounts. Done takes a pin off unless `pin.set` (`owner`, a setting) keeps it. `history.undo`
//! takes each back: the state as it was and, for done, the message where it was.
//!
//! `mail.snooze` (`safe`) puts a message away until a time (`until`, or a `preset` of `schedule.presets`): it goes to
//! the account's `InboxCraft/Snoozed`, made on the server the first time, in a new copy that carries the time in
//! InboxCraft's own header field, so another InboxCraft sees it (see `crate::arrive::snooze`). The old copy can go only
//! by UID EXPUNGE: on a server that has not said it can (`EventKind::Abilities`), and for a message whose body is not
//! kept yet, the message is moved there without the field (`written` in the answer). At the time [`crate::Session::tick`]
//! brings it back to the inbox unread, marked as returned and listed as if it arrived then; `mail.unsnooze` (`safe`)
//! brings it back at once. A snoozed message the owner moves on from the snoozed folder stays where it was put.
//! `history.undo` takes a snooze back: the message where it was, its time as it was.
//!
//! `mail.follow_up` (`safe`) sets a follow-up reminder on a message the owner sent (in a Sent folder, or from one of
//! the owner's own addresses): "remind me if no reply by" a time (`until`, or a `preset` of `schedule.presets`). It is
//! ours alone and nothing goes to the server. A reply that arrives before then cancels it (see
//! `crate::arrive::followup`); otherwise [`crate::Session::tick`] fires it at the time and the owner is told, the notice
//! opening the sent message. `mail.cancel_follow_up` (`safe`) takes it off; `history.undo` takes either back.

use std::collections::BTreeMap;

use inboxcraft_model::{
    AccountId, Due, EmailAddress, Flag, Folder, FolderId, Group, Message, MessageId, MessageState, Role, ScheduleId, Screening, Sender,
};
use inboxcraft_store::{Change, Storage};
use inboxcraft_time::Timestamp;
use serde::Deserialize;
use serde_json::{Value, json};

use super::mail::MAX_LIST;
use crate::act::{self as acts, Effects, Mover, mark_seen};
use crate::arrive::followup;
use crate::arrive::group::{self as grouping, Grouping};
use crate::arrive::screener::{self, ScreenerMode};
use crate::arrive::snooze as snoozing;
use crate::params::{NoParams, decode, parse_date};
use crate::schedule::{self, presets};
use crate::session::State;
use crate::undo::{Act, Inverse, Screened, Step};
use crate::view;
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] =
    &[ACCEPT, BLOCK, LIST, MODE, GROUP_SET, GROUP_ACCOUNT, GROUP_LIST, DONE, PARK, PIN, PARK_LIST, PIN_LIST, PIN_SET, SNOOZE, UNSNOOZE, FOLLOW_UP, CANCEL_FOLLOW_UP];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] =
    &["address", "group", "then", "mode", "account", "grouped", "id", "done", "parked", "pinned", "limit", "offset", "keep_on_done", "until", "preset"];

/// `screener.mode`: what the screener does with the first mail of a sender the owner does not know.
pub(crate) const MODE: CommandSpec = CommandSpec {
    id: "screener.mode",
    class: Some(AgentClass::Owner),
    summary: "Hold the first mail of unknown senders before the inbox, mark it inside the inbox, or let it in",
    run: mode,
};

/// `screener.list`: the senders waiting, with the mail held.
pub(crate) const LIST: CommandSpec =
    CommandSpec { id: "screener.list", class: Some(AgentClass::Safe), summary: "List the new senders waiting, with their mail", run: list };

/// `screener.accept`: let a sender's held and future mail in.
pub(crate) const ACCEPT: CommandSpec = CommandSpec {
    id: "screener.accept",
    class: Some(AgentClass::Owner),
    summary: "Accept a sender: its held mail and its future mail come in",
    run: accept,
};

/// `screener.block`: file a sender's held and future mail in the blocked folder.
pub(crate) const BLOCK: CommandSpec = CommandSpec {
    id: "screener.block",
    class: Some(AgentClass::Owner),
    summary: "Block a sender: its held mail and its future mail go to the blocked folder",
    run: block,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModeParams {
    /// `before`, `inside` or `off`.
    mode: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptParams {
    /// The sender's address.
    address: String,
    /// The group its mail goes to: `people`, `updates`, `newsletters` or `invitations`.
    group: Option<String>,
    /// What happens to the held mail besides: `open` (the newest is marked read, to open) or `done`.
    then: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockParams {
    /// The sender's address.
    address: String,
}

/// What `screener.accept` does with the held mail besides letting it in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Then {
    /// Nothing.
    Nothing,
    /// The newest is marked read, for the front end to open.
    Open,
    /// Filed as done: in the archive, listed by "show done" in the inbox.
    Done,
}

/// The address `text` names, as `command` reads it.
fn address(command: &'static str, text: &str) -> Result<EmailAddress, EngineError> {
    EmailAddress::new(text.trim()).map_err(|error| EngineError::bad_params(command, &error.to_string()))
}

/// The group named `name`, as `command` reads it.
fn group(command: &'static str, name: &str) -> Result<Group, EngineError> {
    grouping::named(name).ok_or_else(|| EngineError::bad_params(command, "the group is people, updates, newsletters or invitations"))
}

/// The folder of `account` with `role`.
fn role_folder(store: &dyn Storage, state: &State, account: AccountId, role: Role) -> Result<Option<Folder>, EngineError> {
    match state.accounts.get(&account).and_then(|held| held.roles.folder(role)) {
        Some(id) => acts::folder(store, account, id),
        None => Ok(None),
    }
}

fn mode(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: ModeParams = decode(MODE.id, params)?;
    let mode = ScreenerMode::from_name(&params.mode).ok_or_else(|| EngineError::bad_params(MODE.id, "the mode is before, inside or off"))?;
    let learned = if context.state().screener == ScreenerMode::Off && mode != ScreenerMode::Off { learn(context)? } else { Vec::new() };
    let mut outcome = Outcome::new(json!({ "mode": mode.name(), "learned": learned.len() })).with_screener(mode);
    for sender in learned {
        outcome = outcome.with_change(Change::PutSender(sender));
    }
    Ok(outcome)
}

/// Every sender the store's mail says the owner knows and the store does not know yet: the senders of the mail kept
/// and the recipients of the mail sent.
fn learn(context: &Context<'_>) -> Result<Vec<Sender>, EngineError> {
    let store = context.store();
    let accounts = store.accounts()?;
    let mut seen: BTreeMap<String, Option<Sender>> = BTreeMap::new();
    for account in &accounts {
        let roles = context.state().accounts.get(&account.id()).map(|held| &held.roles);
        for folder in store.folders(account.id())? {
            let role = roles.and_then(|roles| roles.role_of(folder.id()));
            if role.is_some_and(|role| screener::SILENT.contains(&role)) {
                continue;
            }
            for entry in store.messages(folder.id())? {
                let message = entry.message();
                let addresses: Vec<&EmailAddress> = if role == Some(Role::Sent) {
                    screener::recipients(message.summary().envelope()).collect()
                } else {
                    screener::sender_of(message).into_iter().collect()
                };
                for address in addresses {
                    let key = address.as_str().to_lowercase();
                    if !seen.contains_key(&key) && screener::teaches(&accounts, address) {
                        seen.insert(key, screener::learned(address, store.sender(address)?));
                    }
                }
            }
        }
    }
    Ok(seen.into_values().flatten().collect())
}

/// The mail held from senders: in the screener folders, and in the inboxes while marked as new.
fn waiting(context: &Context<'_>) -> Result<Vec<Message>, EngineError> {
    let store = context.store();
    let state = context.state();
    let accounts = store.accounts()?;
    let mut held = Vec::new();
    for account in &accounts {
        for role in [Role::Screener, Role::Inbox] {
            let Some(folder) = role_folder(store, state, account.id(), role)? else { continue };
            for entry in store.messages(folder.id())? {
                if screener::is_new(store, state.screener, Some(role), &accounts, entry.message())? || role == Role::Screener {
                    held.push(entry.message().clone());
                }
            }
        }
    }
    held.sort_by_cached_key(view::order);
    Ok(held)
}

/// The mail held from `address`, newest first: what [`waiting`] lists, and with `blocked` the mail in the blocked
/// folders.
fn held_from(context: &Context<'_>, address: &EmailAddress, blocked: bool) -> Result<Vec<Message>, EngineError> {
    let store = context.store();
    let from = |message: &Message| screener::sender_of(message).is_some_and(|sender| sender.as_str().eq_ignore_ascii_case(address.as_str()));
    let mut held: Vec<Message> = waiting(context)?.into_iter().filter(|message| from(message)).collect();
    if blocked {
        for account in store.accounts()? {
            if let Some(folder) = role_folder(store, context.state(), account.id(), Role::Blocked)? {
                held.extend(store.messages(folder.id())?.into_iter().map(|entry| entry.message().clone()).filter(|message| from(message)));
            }
        }
    }
    held.sort_by_cached_key(view::order);
    Ok(held)
}

/// One sender waiting, as `screener.list` shows it.
struct Waiting {
    address: String,
    name: Option<String>,
    accounts: Vec<u64>,
    messages: Vec<u64>,
}

fn list(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let NoParams {} = decode(LIST.id, params)?;
    let most = super::mail::limit(Some(MAX_LIST));
    let mut senders: Vec<Waiting> = Vec::new();
    for message in waiting(context)? {
        let Some(first) = message.summary().envelope().from().first() else { continue };
        let address = first.email().as_str().to_lowercase();
        let index = match senders.iter().position(|held| held.address == address) {
            Some(index) => index,
            None if senders.len() < most => {
                senders.push(Waiting { address, name: first.name().map(str::to_owned), accounts: Vec::new(), messages: Vec::new() });
                senders.len().saturating_sub(1)
            }
            None => continue,
        };
        if let Some(sender) = senders.get_mut(index) {
            let account = message.account().get();
            if !sender.accounts.contains(&account) {
                sender.accounts.push(account);
            }
            if sender.messages.len() < most {
                sender.messages.push(message.id().get());
            }
        }
    }
    let senders: Vec<Value> = senders
        .into_iter()
        .map(|sender| json!({ "address": sender.address, "name": sender.name, "accounts": sender.accounts, "messages": sender.messages }))
        .collect();
    Ok(Outcome::new(json!({ "mode": context.state().screener.name(), "senders": senders })))
}

/// What an accept or a block did, to take back.
#[derive(Debug, Default)]
struct Done {
    moves: Vec<(inboxcraft_model::MessageId, FolderId)>,
    /// The message the accept marked read; `None` when it was read already.
    marked: Option<inboxcraft_model::MessageId>,
    states: Vec<(inboxcraft_model::MessageId, MessageState)>,
}

/// The outcome of a screening act on `address`: `sender` kept, `effects` made, `done` to take back.
fn screened(spec: &CommandSpec, address: &EmailAddress, before: Option<Sender>, effects: Effects, done: Done, opened: Option<u64>) -> Outcome {
    let key = address.as_str().to_lowercase();
    let answer = if spec.id == ACCEPT.id {
        json!({ "address": key, "moved": done.moves.len(), "open": opened })
    } else {
        json!({ "address": key, "moved": done.moves.len() })
    };
    let inverse = Inverse::Screen(Box::new(Screened {
        address: address.clone(),
        before,
        moves: done.moves,
        opened: done.marked,
        states: done.states,
    }));
    Outcome::new(answer).with_effects(effects).with_undo(Step::Push(Act { command: spec.id, class: AgentClass::Owner, inverse }))
}

fn accept(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: AcceptParams = decode(ACCEPT.id, params)?;
    let address = address(ACCEPT.id, &params.address)?;
    let group = params.group.as_deref().map(|name| group(ACCEPT.id, name)).transpose()?;
    let then = match params.then.as_deref() {
        None => Then::Nothing,
        Some("open") => Then::Open,
        Some("done") => Then::Done,
        Some(_) => return Err(EngineError::bad_params(ACCEPT.id, "then is open or done")),
    };
    let (store, state) = (context.store(), context.state());
    let before = store.sender(&address)?;
    let mut sender = before.clone().unwrap_or_else(|| Sender::new(address.clone()));
    sender.known = true;
    sender.screening = Some(Screening::Accepted);
    sender.group = group.or(sender.group);
    let mut effects = Effects::default();
    effects.change(Change::PutSender(sender));
    let mut mover = Mover::default();
    let mut done = Done::default();
    let mut opened = None;
    for message in held_from(context, &address, true)? {
        let account = message.account();
        let Some(inbox) = role_folder(store, state, account, Role::Inbox)? else { continue };
        let mut message = message;
        if then == Then::Open && opened.is_none() {
            opened = Some(message.id().get());
            if !message.summary().flags().contains(&Flag::Seen) {
                done.marked = Some(message.id());
            }
            message = mark_seen(store, state, message, true, &mut effects)?;
        }
        let to = match then {
            Then::Done => {
                let archive = role_folder(store, state, account, Role::Archive)?.ok_or(EngineError::NotFound { what: "archive folder of account", id: account.get() })?;
                let was = store.message_state(message.id())?;
                effects.change(Change::SetMessageState(message.id(), MessageState { done: Some(inbox.id()), ..was.clone() }));
                done.states.push((message.id(), was));
                archive
            }
            Then::Nothing | Then::Open => inbox,
        };
        if let Some(moved) = mover.relocate(store, state, &message, &to)? {
            done.moves.push((message.id(), message.folder()));
            effects.extend(moved);
        }
    }
    Ok(screened(&ACCEPT, &address, before, effects, done, opened))
}

fn block(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: BlockParams = decode(BLOCK.id, params)?;
    let address = address(BLOCK.id, &params.address)?;
    let (store, state) = (context.store(), context.state());
    let before = store.sender(&address)?;
    let mut sender = before.clone().unwrap_or_else(|| Sender::new(address.clone()));
    sender.screening = Some(Screening::Blocked);
    let mut effects = Effects::default();
    effects.change(Change::PutSender(sender));
    let mut mover = Mover::default();
    let mut done = Done::default();
    let mut blocked: BTreeMap<AccountId, Option<Folder>> = BTreeMap::new();
    let mut next = state.next_folder;
    for message in held_from(context, &address, false)? {
        let account = message.account();
        let to = match blocked.get(&account) {
            Some(folder) => folder.clone(),
            None => {
                let folder = own_folder(store, state, account, Role::Blocked, &mut next, &mut effects)?;
                blocked.insert(account, folder.clone());
                folder
            }
        };
        let Some(to) = to else { continue };
        if let Some(moved) = mover.relocate(store, state, &message, &to)? {
            done.moves.push((message.id(), message.folder()));
            effects.extend(moved);
        }
    }
    Ok(screened(&BLOCK, &address, before, effects, done, None))
}

/// Our own folder of `account` with `role` (blocked, snoozed), made (with an id from `next`) when it has none, on the
/// server too unless this is the sample mailbox; `None` when it cannot be.
fn own_folder(store: &dyn Storage, state: &State, account: AccountId, role: Role, next: &mut u64, effects: &mut Effects) -> Result<Option<Folder>, EngineError> {
    let Some(held) = state.accounts.get(&account) else { return Ok(None) };
    let Some(inbox) = held.roles.folder(Role::Inbox) else { return Ok(None) };
    let folders = store.folders(account)?;
    let made = screener::own_folder(&folders, &held.roles, inbox, role, || {
        let id = FolderId::new(*next);
        *next = next.saturating_add(1);
        id
    });
    Ok(made.map(|(folder, job)| {
        if let Some(job) = job {
            effects.change(Change::PutFolder(folder.clone()));
            if !state.sample {
                effects.job(account, job);
            }
        }
        folder
    }))
}

/// Takes a screening act or a group choice back: the sender as it was, the mail moved back where it was, the read
/// mark and the done state as they were. A message gone meanwhile is skipped.
pub(crate) fn undo(context: &Context<'_>, screened: &Screened) -> Result<Effects, EngineError> {
    let Screened { address, before, moves, opened, states } = screened;
    let (store, state) = (context.store(), context.state());
    let mut effects = Effects::default();
    effects.change(before.clone().map_or_else(|| Change::RemoveSender(address.clone()), Change::PutSender));
    let mut mover = Mover::default();
    let mut unmarked = false;
    for (id, folder) in moves {
        let Some(entry) = store.message(*id)? else { continue };
        let mut message = entry.message().clone();
        if *opened == Some(*id) {
            message = mark_seen(store, state, message, false, &mut effects)?;
            unmarked = true;
        }
        if let Some(to) = acts::folder(store, message.account(), *folder)?
            && let Some(moved) = mover.relocate(store, state, &message, &to)?
        {
            effects.extend(moved);
        }
    }
    if let Some(id) = opened.filter(|_| !unmarked)
        && let Some(entry) = store.message(id)?
    {
        mark_seen(store, state, entry.message().clone(), false, &mut effects)?;
    }
    for (id, was) in states {
        if store.message(*id)?.is_some() {
            effects.change(Change::SetMessageState(*id, was.clone()));
        }
    }
    Ok(effects)
}

/// `group.set`: the group a sender's mail goes to, kept and future.
pub(crate) const GROUP_SET: CommandSpec = CommandSpec {
    id: "group.set",
    class: Some(AgentClass::Owner),
    summary: "Put a sender's mail, kept and future, in a group, or back in the classifier's hands",
    run: group_set,
};

/// `group.account`: an account's grouping on or off.
pub(crate) const GROUP_ACCOUNT: CommandSpec = CommandSpec {
    id: "group.account",
    class: Some(AgentClass::Owner),
    summary: "Turn the grouped inbox of an account on or off",
    run: group_account,
};

/// `group.list`: the inbox's mail counted per group.
pub(crate) const GROUP_LIST: CommandSpec =
    CommandSpec { id: "group.list", class: Some(AgentClass::Safe), summary: "Count the inbox's mail and its unread mail per group", run: group_list };

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupSetParams {
    /// The sender's address.
    address: String,
    /// `people`, `updates`, `newsletters`, `invitations`, or `auto` for the classifier's choice.
    group: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupAccountParams {
    /// The account.
    account: u64,
    /// Whether its inbox is grouped.
    grouped: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupListParams {
    /// Only this account.
    account: Option<u64>,
}

fn group_set(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: GroupSetParams = decode(GROUP_SET.id, params)?;
    let address = address(GROUP_SET.id, &params.address)?;
    let chosen = match params.group.as_str() {
        "auto" => None,
        name => Some(group(GROUP_SET.id, name)?),
    };
    let before = context.store().sender(&address)?;
    let mut sender = before.clone().unwrap_or_else(|| Sender::new(address.clone()));
    sender.group = chosen;
    let answer = json!({ "address": address.as_str().to_lowercase(), "group": chosen.map(grouping::name) });
    let inverse = Inverse::Screen(Box::new(Screened { address, before, moves: Vec::new(), opened: None, states: Vec::new() }));
    Ok(Outcome::new(answer)
        .with_change(Change::PutSender(sender))
        .with_undo(Step::Push(Act { command: GROUP_SET.id, class: AgentClass::Owner, inverse })))
}

fn group_account(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: GroupAccountParams = decode(GROUP_ACCOUNT.id, params)?;
    let account = context
        .store()
        .accounts()?
        .into_iter()
        .find(|account| account.id().get() == params.account)
        .ok_or(EngineError::NotFound { what: "account", id: params.account })?;
    let answer = json!({ "account": params.account, "grouped": params.grouped });
    Ok(Outcome::new(answer).with_change(Change::PutAccount(account.with_grouped(params.grouped))))
}

/// A count of messages and of the unread among them.
#[derive(Default)]
struct Count {
    total: u64,
    unread: u64,
}

impl Count {
    fn add(&mut self, message: &Message) {
        self.total = self.total.saturating_add(1);
        if !message.summary().flags().contains(&Flag::Seen) {
            self.unread = self.unread.saturating_add(1);
        }
    }

}

fn group_list(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: GroupListParams = decode(GROUP_LIST.id, params)?;
    let account = params.account.map(AccountId::new);
    let entries = view::merged(context, &view::folders(context, view::View::Role(Role::Inbox), account)?)?;
    let mut grouping = Grouping::new(context.store())?;
    let mut counts: BTreeMap<&'static str, Count> = BTreeMap::new();
    let mut ungrouped = Count::default();
    for entry in &entries {
        match grouping.of(entry.message())? {
            Some(group) => counts.entry(grouping::name(group)).or_default().add(entry.message()),
            None => ungrouped.add(entry.message()),
        }
    }
    let groups: Vec<Value> = grouping::ALL
        .into_iter()
        .map(|group| {
            let name = grouping::name(group);
            let count = counts.remove(name).unwrap_or_default();
            json!({ "group": name, "total": count.total, "unread": count.unread })
        })
        .collect();
    let ungrouped = json!({ "total": ungrouped.total, "unread": ungrouped.unread });
    Ok(Outcome::new(json!({ "account": params.account, "groups": groups, "ungrouped": ungrouped })))
}

/// `mail.done`: file a message as done, or bring it back.
pub(crate) const DONE: CommandSpec = CommandSpec {
    id: "mail.done",
    class: Some(AgentClass::Owner),
    summary: "File a message as done in its account's archive, or bring it back to where it was",
    run: done,
};

/// `mail.park`: put a message on the park pile, or take it off.
pub(crate) const PARK: CommandSpec =
    CommandSpec { id: "mail.park", class: Some(AgentClass::Safe), summary: "Put a message on the park pile or take it off", run: park };

/// `mail.pin`: pin a message, or take its pin off.
pub(crate) const PIN: CommandSpec = CommandSpec { id: "mail.pin", class: Some(AgentClass::Safe), summary: "Pin a message or take its pin off", run: pin };

/// `park.list`: the park pile, newest first.
pub(crate) const PARK_LIST: CommandSpec =
    CommandSpec { id: "park.list", class: Some(AgentClass::Safe), summary: "List the park pile across the accounts, newest first", run: park_list };

/// `pin.list`: the pins view, newest first.
pub(crate) const PIN_LIST: CommandSpec =
    CommandSpec { id: "pin.list", class: Some(AgentClass::Safe), summary: "List the pinned messages across the accounts, newest first", run: pin_list };

/// `pin.set`: whether done leaves a pin on.
pub(crate) const PIN_SET: CommandSpec =
    CommandSpec { id: "pin.set", class: Some(AgentClass::Owner), summary: "Keep a pin when its message is done, or take it off", run: pin_set };

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DoneParams {
    /// The message.
    id: u64,
    /// Done (`true`) or back where it was.
    done: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ParkParams {
    /// The message.
    id: u64,
    /// On the park pile (`true`) or off it.
    parked: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PinParams {
    /// The message.
    id: u64,
    /// Pinned (`true`) or not.
    pinned: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PileParams {
    /// Only this account.
    account: Option<u64>,
    /// At most this many, 1 to [`MAX_LIST`].
    limit: Option<u64>,
    /// Skip this many first.
    offset: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PinSetParams {
    /// Done leaves a pin on.
    keep_on_done: bool,
}

/// The message `id`.
fn message(context: &Context<'_>, id: u64) -> Result<Message, EngineError> {
    let entry = context.store().message(MessageId::new(id))?.ok_or(EngineError::NotFound { what: "message", id })?;
    Ok(entry.message().clone())
}

/// The folder `id` of `message`'s account, when it is still there and holds mail.
fn mail_folder(store: &dyn Storage, message: &Message, id: FolderId) -> Result<Option<Folder>, EngineError> {
    Ok(acts::folder(store, message.account(), id)?.filter(|folder| folder.listing().is_selectable()))
}

/// The outcome of `spec` putting `message` in `state`, moved by `moved`; `history.undo` puts it back in `was`, and
/// in the folder `from` when it moved.
fn stated(spec: &CommandSpec, answer: Value, message: &Message, was: MessageState, state: MessageState, moved: Option<Effects>) -> Outcome {
    let from = moved.is_some().then_some(message.folder());
    let mut effects = moved.unwrap_or_default();
    effects.change(Change::SetMessageState(message.id(), state));
    let class = spec.class.unwrap_or(AgentClass::Owner);
    let act = Act { command: spec.id, class, inverse: Inverse::State { message: message.id(), to: from, state: was } };
    Outcome::new(answer).with_effects(effects).with_undo(Step::Push(act))
}

fn done(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: DoneParams = decode(DONE.id, params)?;
    let message = message(context, params.id)?;
    let (store, state) = (context.store(), context.state());
    let was = store.message_state(message.id())?;
    let here = message.folder().get();
    if !params.done {
        let Some(back) = was.done else { return Ok(Outcome::new(json!({ "id": params.id, "done": false, "folder": here, "changed": false }))) };
        let moved = match mail_folder(store, &message, back)? {
            Some(to) => acts::relocate(store, state, &message, &to)?,
            None => None,
        };
        let folder = if moved.is_some() { back.get() } else { here };
        let answer = json!({ "id": params.id, "done": false, "folder": folder, "changed": true });
        return Ok(stated(&DONE, answer, &message, was.clone(), MessageState { done: None, ..was }, moved));
    }
    let account = message.account();
    let archive = role_folder(store, state, account, Role::Archive)?.filter(|folder| folder.listing().is_selectable());
    // Done already, and in the archive: nothing to do. Done but elsewhere (its move failed, or it was moved on by
    // hand): it is filed again, from where it is now.
    if let Some(from) = was.done
        && archive.as_ref().is_none_or(|archive| archive.id() == message.folder())
    {
        let answer = json!({ "id": params.id, "done": true, "folder": here, "was": from.get(), "changed": false, "pinned": was.pinned });
        return Ok(Outcome::new(answer));
    }
    let archive = archive.ok_or(EngineError::NotFound { what: "archive folder of account", id: account.get() })?;
    let moved = acts::relocate(store, state, &message, &archive)?;
    let pinned = was.pinned && state.pins_outlast_done;
    let answer = json!({ "id": params.id, "done": true, "folder": archive.id().get(), "was": here, "changed": true, "pinned": pinned });
    let now = MessageState { done: Some(message.folder()), pinned, ..was.clone() };
    Ok(stated(&DONE, answer, &message, was, now, moved))
}

/// Sets `field` of `message`'s state to `set` as `spec` does, with `apply` changing the state.
fn mark_state(context: &Context<'_>, spec: &CommandSpec, id: u64, field: &str, set: bool, apply: fn(&mut MessageState, bool) -> bool) -> Result<Outcome, EngineError> {
    let message = message(context, id)?;
    let was = context.store().message_state(message.id())?;
    let mut now = was.clone();
    let changed = apply(&mut now, set);
    let answer = Value::Object(serde_json::Map::from_iter([
        ("id".to_owned(), json!(id)),
        (field.to_owned(), json!(set)),
        ("changed".to_owned(), json!(changed)),
    ]));
    if !changed {
        return Ok(Outcome::new(answer));
    }
    Ok(stated(spec, answer, &message, was, now, None))
}

fn park(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: ParkParams = decode(PARK.id, params)?;
    mark_state(context, &PARK, params.id, "parked", params.parked, |state, set| std::mem::replace(&mut state.parked, set) != set)
}

fn pin(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: PinParams = decode(PIN.id, params)?;
    mark_state(context, &PIN, params.id, "pinned", params.pinned, |state, set| std::mem::replace(&mut state.pinned, set) != set)
}

/// The messages whose state `keep` takes, of `account` or every account, newest first, as a list answers them.
fn pile(context: &Context<'_>, params: &PileParams, keep: fn(&MessageState) -> bool) -> Result<Value, EngineError> {
    let account = params.account.map(AccountId::new);
    let accounts = view::accounts(context, account)?;
    let store = context.store();
    let mut entries = Vec::new();
    let mut states = BTreeMap::new();
    for (id, state) in store.message_states()? {
        if keep(&state)
            && let Some(entry) = store.message(id)?
            && accounts.contains(&entry.message().account())
        {
            entries.push(entry);
            states.insert(id, state);
        }
    }
    entries.sort_by_cached_key(|entry| view::order(entry.message()));
    let limit = super::mail::limit(params.limit);
    let offset = usize::try_from(params.offset.unwrap_or(0)).unwrap_or(usize::MAX);
    let mut grouping = Grouping::new(store)?;
    let held = store.accounts()?;
    let mut messages = Vec::new();
    for entry in entries.iter().skip(offset).take(limit) {
        messages.push(super::mail::row(context, &mut grouping, &held, entry, states.get(&entry.message().id()))?);
    }
    Ok(json!({ "account": params.account, "total": entries.len(), "offset": offset, "limit": limit, "messages": messages }))
}

fn park_list(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: PileParams = decode(PARK_LIST.id, params)?;
    Ok(Outcome::new(pile(context, &params, |state| state.parked)?))
}

fn pin_list(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: PileParams = decode(PIN_LIST.id, params)?;
    let mut answer = pile(context, &params, |state| state.pinned)?;
    if let Value::Object(fields) = &mut answer {
        fields.insert("keep_on_done".into(), json!(context.state().pins_outlast_done));
    }
    Ok(Outcome::new(answer))
}

fn pin_set(_: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: PinSetParams = decode(PIN_SET.id, params)?;
    Ok(Outcome::new(json!({ "keep_on_done": params.keep_on_done })).with_pins_outlast_done(params.keep_on_done))
}

/// Takes done, park, pin, snooze or a follow-up reminder back on `message`: back in the folder `to` when it is still
/// there, and in `state`, with the schedule bringing it back and reminding of it at the times `state` says and no other.
pub(crate) fn restore(context: &Context<'_>, message: &Message, to: Option<FolderId>, state: &MessageState) -> Result<Effects, EngineError> {
    let store = context.store();
    let mut effects = Effects::default();
    if let Some(to) = to
        && let Some(folder) = mail_folder(store, message, to)?
        && let Some(moved) = acts::relocate(store, context.state(), message, &folder)?
    {
        effects.extend(moved);
    }
    effects.change(Change::SetMessageState(message.id(), state.clone()));
    let next = context.next_schedule_id();
    effects.changes.extend(schedule::reschedule(store, Due::Unsnooze(message.id()), state.snoozed_until, next)?);
    let after = ScheduleId::new(next.get().saturating_add(1));
    effects.changes.extend(schedule::reschedule(store, Due::FollowUp(message.id()), state.follow_up, after)?);
    Ok(effects)
}

/// `mail.snooze`: put a message away until a time.
pub(crate) const SNOOZE: CommandSpec = CommandSpec {
    id: "mail.snooze",
    class: Some(AgentClass::Safe),
    summary: "Snooze a message until a time: it leaves for the snoozed folder and comes back to the inbox unread then",
    run: snooze,
};

/// `mail.unsnooze`: bring a snoozed message back now.
pub(crate) const UNSNOOZE: CommandSpec =
    CommandSpec { id: "mail.unsnooze", class: Some(AgentClass::Safe), summary: "Bring a snoozed message back to the inbox now", run: unsnooze };

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SnoozeParams {
    /// The message.
    id: u64,
    /// The time it comes back: `2026-11-05T18:00:00Z`, with an offset, or a date (midnight UTC).
    until: Option<String>,
    /// Or a preset of `schedule.presets`: `later_today`, `this_evening`, `tomorrow_morning`.
    preset: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UnsnoozeParams {
    /// The message.
    id: u64,
}

/// The time `command` is given, as `until` or a `preset`, ahead of the context's time.
fn until(context: &Context<'_>, command: &'static str, until: Option<&str>, preset: Option<&str>) -> Result<Timestamp, EngineError> {
    let now = context.now();
    let until = match (until, preset) {
        (Some(text), None) => {
            parse_date(text.trim()).ok_or_else(|| EngineError::bad_params(command, "until is a time such as 2026-11-05T18:00:00Z"))?
        }
        (None, Some(name)) => presets(now, context.offsets())
            .into_iter()
            .find(|(preset, _)| preset.name() == name)
            .map(|(_, at)| at)
            .ok_or_else(|| EngineError::bad_params(command, "the preset is one of schedule.presets still ahead"))?,
        _ => return Err(EngineError::bad_params(command, "give until or a preset")),
    };
    if until <= now {
        return Err(EngineError::bad_params(command, "the time has passed"));
    }
    Ok(until)
}

fn snooze(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: SnoozeParams = decode(SNOOZE.id, params)?;
    let until = until(context, SNOOZE.id, params.until.as_deref(), params.preset.as_deref())?;
    let message = message(context, params.id)?;
    let (store, state) = (context.store(), context.state());
    let account = message.account();
    let mut effects = Effects::default();
    let mut next = state.next_folder;
    let folder = own_folder(store, state, account, Role::Snoozed, &mut next, &mut effects)?
        .filter(|folder| folder.listing().is_selectable())
        .ok_or(EngineError::NotFound { what: "snoozed folder of account", id: account.get() })?;
    // The old copy can go only by UID EXPUNGE: without it, a new copy would leave the message twice on the server.
    let removes_one = state.accounts.get(&account).is_some_and(|held| held.uid_expunge);
    let copy = match store.body(message.id())?.filter(|_| removes_one) {
        Some(body) => snoozing::with_field(body.source(), until),
        None => None,
    };
    let rewritten = match copy {
        Some(raw) => acts::rewrite(store, state, &message, &folder, raw)?,
        None => None,
    };
    let written = rewritten.is_some();
    let moved = match rewritten {
        Some(effects) => Some(effects),
        None => acts::relocate(store, state, &message, &folder)?,
    };
    let from = moved.is_some().then_some(message.folder());
    effects.extend(moved.unwrap_or_default());
    let was = store.message_state(message.id())?;
    let now = MessageState { snoozed_until: Some(until), returned: None, ..was.clone() };
    effects.change(Change::SetMessageState(message.id(), now));
    effects.changes.extend(schedule::reschedule(store, Due::Unsnooze(message.id()), Some(until), context.next_schedule_id())?);
    let answer = json!({
        "id": params.id,
        "until": crate::json::date(until),
        "folder": folder.id().get(),
        "was": message.folder().get(),
        "changed": true,
        "written": written,
    });
    let act = Act { command: SNOOZE.id, class: AgentClass::Safe, inverse: Inverse::State { message: message.id(), to: from, state: was } };
    Ok(Outcome::new(answer).with_effects(effects).with_undo(Step::Push(act)))
}

fn unsnooze(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: UnsnoozeParams = decode(UNSNOOZE.id, params)?;
    let message = message(context, params.id)?;
    let (store, state) = (context.store(), context.state());
    let was = store.message_state(message.id())?;
    if was.snoozed_until.is_none() && !snoozing::is_snoozed(state, &message) {
        return Ok(Outcome::new(json!({ "id": params.id, "folder": message.folder().get(), "changed": false })));
    }
    let mut effects = Effects::default();
    let inbox = snoozing::back(store, state, &message, &mut Mover::default(), &mut effects)?;
    let folder = inbox.map_or(message.folder(), |inbox| inbox.id());
    effects.change(Change::SetMessageState(message.id(), MessageState { snoozed_until: None, returned: Some(context.now()), ..was }));
    effects.changes.extend(schedule::reschedule(store, Due::Unsnooze(message.id()), None, context.next_schedule_id())?);
    Ok(Outcome::new(json!({ "id": params.id, "folder": folder.get(), "changed": true })).with_effects(effects))
}

/// `mail.follow_up`: remind the owner if a sent message gets no reply by a time.
pub(crate) const FOLLOW_UP: CommandSpec = CommandSpec {
    id: "mail.follow_up",
    class: Some(AgentClass::Safe),
    summary: "Remind me if a message I sent gets no reply by a time",
    run: follow_up,
};

/// `mail.cancel_follow_up`: take a follow-up reminder off.
pub(crate) const CANCEL_FOLLOW_UP: CommandSpec = CommandSpec {
    id: "mail.cancel_follow_up",
    class: Some(AgentClass::Safe),
    summary: "Take the follow-up reminder off a message I sent",
    run: cancel_follow_up,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FollowUpParams {
    /// The sent message.
    id: u64,
    /// The time to remind at: `2026-11-05T18:00:00Z`, with an offset, or a date (midnight UTC).
    until: Option<String>,
    /// Or a preset of `schedule.presets`: `later_today`, `this_evening`, `tomorrow_morning`.
    preset: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CancelFollowUpParams {
    /// The sent message.
    id: u64,
}

fn follow_up(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: FollowUpParams = decode(FOLLOW_UP.id, params)?;
    let until = until(context, FOLLOW_UP.id, params.until.as_deref(), params.preset.as_deref())?;
    let message = message(context, params.id)?;
    let store = context.store();
    if !followup::is_sent(store, context.state(), &message)? {
        return Err(EngineError::bad_params(FOLLOW_UP.id, "a follow-up reminder goes on a message the owner sent"));
    }
    let was = store.message_state(message.id())?;
    let answer = json!({ "id": params.id, "until": crate::json::date(until), "changed": was.follow_up != Some(until) });
    if was.follow_up == Some(until) {
        return Ok(Outcome::new(answer));
    }
    reminded(context, &FOLLOW_UP, answer, &message, was, Some(until))
}

fn cancel_follow_up(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: CancelFollowUpParams = decode(CANCEL_FOLLOW_UP.id, params)?;
    let message = message(context, params.id)?;
    let was = context.store().message_state(message.id())?;
    let answer = json!({ "id": params.id, "changed": was.follow_up.is_some() });
    if was.follow_up.is_none() {
        return Ok(Outcome::new(answer));
    }
    reminded(context, &CANCEL_FOLLOW_UP, answer, &message, was, None)
}

/// The outcome of `spec` setting `message`'s follow-up reminder to `until`, from `was`: the state, the schedule, and
/// the undo that puts both back.
fn reminded(context: &Context<'_>, spec: &CommandSpec, answer: Value, message: &Message, was: MessageState, until: Option<Timestamp>) -> Result<Outcome, EngineError> {
    let mut effects = Effects::default();
    effects.change(Change::SetMessageState(message.id(), MessageState { follow_up: until, ..was.clone() }));
    effects.changes.extend(schedule::reschedule(context.store(), Due::FollowUp(message.id()), until, context.next_schedule_id())?);
    let act = Act { command: spec.id, class: AgentClass::Safe, inverse: Inverse::State { message: message.id(), to: None, state: was } };
    Ok(Outcome::new(answer).with_effects(effects).with_undo(Step::Push(act)))
}
