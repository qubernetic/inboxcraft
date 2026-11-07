// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `triage`: the screener now (`screener.*`); the groups, done, park, pin, snooze and follow-up reminders come later.
//! A new command goes into [`COMMANDS`] and its parameter names into `FIELDS`.
//!
//! The screener's mode is the owner's (`screener.mode`); turning it on learns every sender the owner has mail from or
//! wrote to (see `crate::arrive::screener`). `screener.list` shows the senders waiting, each with the mail held;
//! reading one (`mail.read`) is a preview that decides nothing. `screener.accept` lets the held mail in (or files it
//! as done, or opens the newest) and the sender's future mail too; `screener.block` files the held mail and the
//! future mail in `InboxCraft/Blocked`, made on the server the first time. Both are the owner's acts, and
//! `history.undo` takes either back: the sender as it was and the mail where it was.

use std::collections::BTreeMap;

use inboxcraft_model::{AccountId, EmailAddress, Flag, Folder, FolderId, Group, Message, MessageState, Role, Screening, Sender};
use inboxcraft_store::{Change, Storage};
use serde::Deserialize;
use serde_json::{Value, json};

use super::mail::MAX_LIST;
use crate::act::{self as acts, Effects, Mover};
use crate::arrive::screener::{self, ScreenerMode};
use crate::params::{NoParams, decode};
use crate::session::State;
use crate::undo::{Act, Inverse, Screened, Step};
use crate::view;
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[ACCEPT, BLOCK, LIST, MODE];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["address", "group", "then", "mode"];

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

/// The group named `name`.
fn group(name: &str) -> Result<Group, EngineError> {
    match name {
        "people" => Ok(Group::People),
        "updates" => Ok(Group::Updates),
        "newsletters" => Ok(Group::Newsletters),
        "invitations" => Ok(Group::Invitations),
        _ => Err(EngineError::bad_params(ACCEPT.id, "the group is people, updates, newsletters or invitations")),
    }
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

/// `message` with `flags`.
fn flagged(message: &Message, flags: inboxcraft_model::Flags) -> Message {
    Message::new(message.id(), message.account(), message.folder(), message.summary().clone().with_flags(flags))
        .with_thread(message.thread())
        .with_moving(message.moving().cloned())
}

/// Sets or clears the read mark on `message`: the changes into `effects`, the message as it now is back.
fn mark_seen(store: &dyn Storage, state: &State, message: Message, seen: bool, effects: &mut Effects) -> Result<Message, EngineError> {
    match acts::mark(store, state, &message, &Flag::Seen, seen)? {
        Some(marked) => {
            let mut flags = message.summary().flags().clone();
            if seen {
                flags.insert(Flag::Seen);
            } else {
                flags.remove(&Flag::Seen);
            }
            effects.extend(marked);
            Ok(flagged(&message, flags))
        }
        None => Ok(message),
    }
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
    let group = params.group.as_deref().map(group).transpose()?;
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
                let folder = blocked_folder(store, state, account, &mut next, &mut effects)?;
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

/// The blocked folder of `account`, made (with an id from `next`) when it has none; `None` when it cannot be.
fn blocked_folder(store: &dyn Storage, state: &State, account: AccountId, next: &mut u64, effects: &mut Effects) -> Result<Option<Folder>, EngineError> {
    let Some(held) = state.accounts.get(&account) else { return Ok(None) };
    let Some(inbox) = held.roles.folder(Role::Inbox) else { return Ok(None) };
    let folders = store.folders(account)?;
    let made = screener::own_folder(&folders, &held.roles, inbox, Role::Blocked, || {
        let id = FolderId::new(*next);
        *next = next.saturating_add(1);
        id
    });
    Ok(made.map(|(folder, job)| {
        if let Some(job) = job {
            effects.change(Change::PutFolder(folder.clone()));
            effects.job(account, job);
        }
        folder
    }))
}

/// Takes a screening act back: the sender as it was, the mail moved back where it was, the read mark and the done
/// state as they were. A message gone meanwhile is skipped.
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
