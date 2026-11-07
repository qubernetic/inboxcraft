// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `act`: acting on messages (flag, read and unread, open, move, archive and delete) and taking the acts back.
//!
//! An act shows at once and goes to the server as a kept job (see `crate::act`); each one but a delete for good
//! stores its inverse, and `history.undo` takes the newest back, again and again.

use inboxcraft_model::{Flag, Folder, FolderId, Message, MessageId, Role};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::act::{self as acts, Effects};
use crate::params::{NoParams, decode};
use crate::undo::{Act, Inverse, Step};
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[ARCHIVE, DELETE, FLAG, MOVE, OPEN, SEEN, UNDO];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["id", "flagged", "seen", "folder"];

/// `mail.flag`: flag a message, or take its flag off.
pub(crate) const FLAG: CommandSpec =
    CommandSpec { id: "mail.flag", class: Some(AgentClass::Safe), summary: "Flag a message or take its flag off", run: flag };

/// `mail.seen`: mark a message read or unread.
pub(crate) const SEEN: CommandSpec =
    CommandSpec { id: "mail.seen", class: Some(AgentClass::Safe), summary: "Mark a message read or unread", run: seen };

/// `mail.open`: what the window does when the owner opens a message: `mail.read`, and the message marked read.
pub(crate) const OPEN: CommandSpec =
    CommandSpec { id: "mail.open", class: Some(AgentClass::Safe), summary: "Open a message: read it and mark it read", run: open };

/// `mail.move`: move a message to another folder of its account.
pub(crate) const MOVE: CommandSpec =
    CommandSpec { id: "mail.move", class: Some(AgentClass::Owner), summary: "Move a message to another folder of its account", run: move_to };

/// `mail.archive`: move a message to its account's archive folder.
pub(crate) const ARCHIVE: CommandSpec =
    CommandSpec { id: "mail.archive", class: Some(AgentClass::Owner), summary: "Move a message to its account's archive folder", run: archive };

/// `mail.delete`: move a message to its account's trash; a message in the trash goes for good, with no undo.
pub(crate) const DELETE: CommandSpec = CommandSpec {
    id: "mail.delete",
    class: Some(AgentClass::Owner),
    summary: "Move a message to the trash, or delete it for good when it is in the trash",
    run: delete,
};

/// `history.undo`: take the newest act back. An agent takes back only an act an agent may make.
pub(crate) const UNDO: CommandSpec =
    CommandSpec { id: "history.undo", class: Some(AgentClass::Safe), summary: "Take the newest act on a message back", run: undo };

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FlagParams {
    /// The message.
    id: u64,
    /// Flagged (`true`) or not.
    flagged: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeenParams {
    /// The message.
    id: u64,
    /// Read (`true`) or unread.
    seen: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdParams {
    /// The message.
    id: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveParams {
    /// The message.
    id: u64,
    /// The folder it goes to.
    folder: u64,
}

/// The message `id`.
fn message(context: &Context<'_>, id: u64) -> Result<Message, EngineError> {
    let entry = context.store().message(MessageId::new(id))?.ok_or(EngineError::NotFound { what: "message", id })?;
    Ok(entry.message().clone())
}

/// The outcome of `effects`, with `act` in the history when there were any.
fn acted(answer: Value, effects: Option<Effects>, act: Act) -> Outcome {
    match effects {
        Some(effects) => Outcome::new(answer).with_effects(effects).with_undo(Step::Push(act)),
        None => Outcome::new(answer),
    }
}

/// Sets or clears `flag` as `spec` does; the answer names the flag `field`.
fn mark(context: &Context<'_>, spec: &CommandSpec, id: u64, flag: Flag, set: bool, field: &str) -> Result<Outcome, EngineError> {
    let message = message(context, id)?;
    let effects = acts::mark(context.store(), context.state(), &message, &flag, set)?;
    let answer = Value::Object(Map::from_iter([
        ("id".to_owned(), json!(id)),
        (field.to_owned(), json!(set)),
        ("changed".to_owned(), json!(effects.is_some())),
    ]));
    let act = Act { command: spec.id, class: AgentClass::Safe, inverse: Inverse::Mark { message: message.id(), flag, set: !set } };
    Ok(acted(answer, effects, act))
}

fn flag(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: FlagParams = decode(FLAG.id, params)?;
    mark(context, &FLAG, params.id, Flag::Flagged, params.flagged, "flagged")
}

fn seen(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: SeenParams = decode(SEEN.id, params)?;
    mark(context, &SEEN, params.id, Flag::Seen, params.seen, "seen")
}

fn open(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: IdParams = decode(OPEN.id, params)?;
    let mut outcome = super::mail::read_one(context, params.id)?;
    let message = message(context, params.id)?;
    let effects = acts::mark(context.store(), context.state(), &message, &Flag::Seen, true)?;
    let marked = effects.is_some();
    if let Value::Object(answer) = outcome.value_mut() {
        answer.insert("marked_seen".into(), json!(marked));
        if marked && let Some(Value::Object(shown)) = answer.get_mut("message") {
            shown.insert("seen".into(), json!(true));
            if let Some(Value::Array(flags)) = shown.get_mut("flags") {
                flags.insert(0, json!(Flag::Seen.to_string()));
            }
        }
    }
    if let Some(effects) = effects {
        let act = Act { command: OPEN.id, class: AgentClass::Safe, inverse: Inverse::Mark { message: message.id(), flag: Flag::Seen, set: false } };
        outcome = outcome.with_effects(effects).with_undo(Step::Push(act));
    }
    Ok(outcome)
}

/// The folder `id` of `message`'s account, to move it to: it must hold mail.
fn target(context: &Context<'_>, spec: &CommandSpec, message: &Message, id: FolderId) -> Result<Folder, EngineError> {
    let store = context.store();
    match acts::folder(store, message.account(), id)? {
        Some(folder) if folder.listing().is_selectable() => Ok(folder),
        Some(_) => Err(EngineError::bad_params(spec.id, "the folder holds no mail")),
        None => {
            for account in store.accounts()? {
                if store.folders(account.id())?.iter().any(|folder| folder.id() == id) {
                    return Err(EngineError::bad_params(spec.id, "the folder belongs to another account"));
                }
            }
            Err(EngineError::NotFound { what: "folder", id: id.get() })
        }
    }
}

/// The folder of `message`'s account with `role`.
fn role_folder(context: &Context<'_>, message: &Message, role: Role) -> Option<FolderId> {
    context.state().accounts.get(&message.account()).and_then(|held| held.roles.folder(role))
}

/// Moves `message` to `to` as `spec` does.
fn relocate(context: &Context<'_>, spec: &CommandSpec, message: &Message, to: &Folder) -> Result<Outcome, EngineError> {
    let from = message.folder();
    let effects = acts::relocate(context.store(), context.state(), message, to)?;
    let answer = json!({ "id": message.id().get(), "folder": to.id().get(), "was": from.get(), "changed": effects.is_some() });
    let act = Act { command: spec.id, class: AgentClass::Owner, inverse: Inverse::Move { message: message.id(), to: from } };
    Ok(acted(answer, effects, act))
}

/// Moves `message` to its account's folder with `role`, `what` when there is none.
fn to_role(context: &Context<'_>, spec: &CommandSpec, message: &Message, role: Role, what: &'static str) -> Result<Outcome, EngineError> {
    let folder = role_folder(context, message, role).ok_or(EngineError::NotFound { what, id: message.account().get() })?;
    let to = target(context, spec, message, folder)?;
    relocate(context, spec, message, &to)
}

fn move_to(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: MoveParams = decode(MOVE.id, params)?;
    let message = message(context, params.id)?;
    let to = target(context, &MOVE, &message, FolderId::new(params.folder))?;
    relocate(context, &MOVE, &message, &to)
}

fn archive(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: IdParams = decode(ARCHIVE.id, params)?;
    let message = message(context, params.id)?;
    to_role(context, &ARCHIVE, &message, Role::Archive, "archive folder of account")
}

fn delete(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: IdParams = decode(DELETE.id, params)?;
    let message = message(context, params.id)?;
    if role_folder(context, &message, Role::Trash) == Some(message.folder()) {
        let effects = acts::expunge(context.store(), context.state(), &message)?;
        return Ok(Outcome::new(json!({ "id": params.id, "permanent": true })).with_effects(effects));
    }
    let mut outcome = to_role(context, &DELETE, &message, Role::Trash, "trash folder of account")?;
    if let Value::Object(answer) = outcome.value_mut() {
        answer.insert("permanent".into(), json!(false));
    }
    Ok(outcome)
}

fn undo(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let _: NoParams = decode(UNDO.id, params)?;
    let act = context.state().history.last().ok_or(EngineError::NothingToUndo)?;
    if !act.class.allows(context.caller()) {
        return Err(EngineError::NotAllowed { command: act.command, class: act.class });
    }
    let id = act.inverse.message();
    let answer = |undone: bool| json!({ "command": act.command, "id": id.get(), "undone": undone });
    let (store, state) = (context.store(), context.state());
    let Some(entry) = store.message(id)? else { return Ok(Outcome::new(answer(false)).with_undo(Step::Pop)) };
    let message = entry.message();
    let effects = match &act.inverse {
        Inverse::Mark { flag, set, .. } => acts::mark(store, state, message, flag, *set)?,
        Inverse::Move { to, .. } => match acts::folder(store, message.account(), *to)? {
            Some(folder) => acts::relocate(store, state, message, &folder)?,
            None => return Ok(Outcome::new(answer(false)).with_undo(Step::Pop)),
        },
    };
    let outcome = Outcome::new(answer(true)).with_undo(Step::Pop);
    Ok(match effects {
        Some(effects) => outcome.with_effects(effects),
        None => outcome,
    })
}
