// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `sample.*`: what the owner can try on the invented sample mailbox (`--sample`) before the commands that act on a
//! real mailbox exist. They change the in-memory sample only, queue no job for a server and are refused in any other
//! session ([`EngineError::SampleOnly`]), so they are `safe`.

use inboxcraft_model::{Flag, FolderId, Message, MessageId, Summary, Uid};
use inboxcraft_store::Change;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::params::decode;
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome};

/// `sample.mail.set`: mark a sample message read or unread, move it to another folder of its account, or both at
/// once. The answer says where it was and whether it was read, so the front end can undo it.
pub(crate) const SET: CommandSpec = CommandSpec {
    id: "sample.mail.set",
    class: Some(AgentClass::Safe),
    summary: "In the sample mailbox: mark a message read or unread, or move it to another folder of its account",
    run: set,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetParams {
    /// The message.
    id: u64,
    /// Read (`true`) or unread.
    seen: Option<bool>,
    /// The folder it goes to.
    folder: Option<u64>,
}

fn set(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    if !context.state().sample {
        return Err(EngineError::SampleOnly(SET.id));
    }
    let params: SetParams = decode(SET.id, params)?;
    if params.seen.is_none() && params.folder.is_none() {
        return Err(EngineError::bad_params(SET.id, "give seen, folder or both"));
    }
    let store = context.store();
    let entry = store.message(MessageId::new(params.id))?.ok_or(EngineError::NotFound { what: "message", id: params.id })?;
    let message = entry.message();
    let summary = message.summary();
    let was_seen = summary.flags().contains(&Flag::Seen);
    let mut flags = summary.flags().clone();
    match params.seen {
        Some(true) => {
            flags.insert(Flag::Seen);
        }
        Some(false) => {
            flags.remove(&Flag::Seen);
        }
        None => {}
    }
    let seen = flags.contains(&Flag::Seen);
    let from = message.folder();
    let to = params.folder.map_or(from, FolderId::new);
    let change = if to == from {
        Change::SetFlags(message.id(), flags)
    } else {
        if !store.folders(message.account())?.iter().any(|folder| folder.id() == to) {
            let elsewhere = store.accounts()?.iter().map(|account| store.folders(account.id())).collect::<Result<Vec<_>, _>>()?;
            return Err(if elsewhere.iter().flatten().any(|folder| folder.id() == to) {
                EngineError::bad_params(SET.id, "the folder belongs to another account")
            } else {
                EngineError::NotFound { what: "folder", id: to.get() }
            });
        }
        let held = store.messages(to)?;
        let last = held.iter().map(|entry| entry.message().summary().uid().get()).max().unwrap_or(0);
        let uid = last.checked_add(1).and_then(Uid::new).ok_or_else(|| EngineError::bad_params(SET.id, "the folder has no UID left"))?;
        let moved = Summary::new(uid, summary.envelope().clone())
            .with_flags(flags)
            .with_size(summary.size())
            .with_received(summary.received())
            .with_preview(summary.preview())
            .with_modseq(summary.modseq());
        Change::PutMessage(Message::new(message.id(), message.account(), to, moved).with_thread(message.thread()))
    };
    let answer = json!({ "id": params.id, "folder": to.get(), "seen": seen, "was": { "folder": from.get(), "seen": was_seen } });
    Ok(Outcome::new(answer).with_change(change))
}
