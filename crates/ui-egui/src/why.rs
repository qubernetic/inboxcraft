// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Why a message is here, in the window's words: the engine's `mail.why` gives at most four reasons, most specific
//! first, each a `text_id` from a fixed set with its `args`; each id the window knows becomes one sentence of its own
//! with the args filled in. An id the window does not know is left out, never shown as it came. No text of the message
//! reaches the words: the args are an address, a group's or a rule's name, a folder, a time or a count, and the
//! address and the folder's name are cleaned of controls and bidi overrides and capped as anything shown from outside.
//!
//! The reasons are asked once when a message opens and again only when its place in the mailbox changes
//! ([`Place`]), never per frame.

use inboxcraft_time::Timestamp;
use serde_json::{Value, json};

use crate::component::typed_time;
use crate::mailbox::{GroupKind, Mailbox, parse_date, role_text, shown};
use crate::tr;

/// The most reasons the line is given: the engine's cap, held to again here.
pub const MAX_REASONS: usize = 4;

/// Where a message stands as the list shows it: what `mail.why` reads changes only with one of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Place {
    folder: u64,
    group: Option<GroupKind>,
    new_sender: bool,
    done: bool,
    snoozed_until: Option<Timestamp>,
    returned: Option<Timestamp>,
}

/// Where message `id` stands, as the list shows it; none when the list does not hold it.
pub(crate) fn place(mailbox: &Mailbox, id: u64) -> Option<Place> {
    mailbox.messages().iter().find(|row| row.id == id).map(|row| Place {
        folder: row.folder,
        group: row.group,
        new_sender: row.new_sender,
        done: row.done,
        snoozed_until: row.snoozed_until,
        returned: row.returned,
    })
}

/// Why message `id` is here, in the window's words, at most [`MAX_REASONS`]; none when the engine cannot say (the
/// message is gone, the command failed).
pub(crate) fn ask(mailbox: &mut Mailbox, id: u64) -> Vec<String> {
    match mailbox.run("mail.why", &json!({ "id": id })) {
        Ok(answer) => words(mailbox, id, &answer),
        Err(_) => Vec::new(),
    }
}

/// The reasons of a `mail.why` answer for message `id` in the window's words, in the engine's order: the ids the window
/// does not know left out, at most [`MAX_REASONS`].
pub(crate) fn words(mailbox: &Mailbox, id: u64, answer: &Value) -> Vec<String> {
    let Some(reasons) = answer.get("reasons").and_then(Value::as_array) else { return Vec::new() };
    reasons
        .iter()
        .take(MAX_REASONS)
        .filter_map(|reason| {
            let text_id = reason.get("text_id").and_then(Value::as_str)?;
            sentence(mailbox, id, text_id, reason.get("args").unwrap_or(&Value::Null))
        })
        .collect()
}

/// The text arg `key`, made fit for one line: controls and bidi overrides out, capped.
fn text(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(shown).filter(|text| !text.trim().is_empty())
}

/// The time arg `key`, short on the window's clock.
fn when(mailbox: &Mailbox, args: &Value, key: &str) -> Option<String> {
    let at = args.get(key).and_then(Value::as_str).and_then(parse_date)?;
    Some(typed_time::short(at, mailbox.now(), mailbox.offset())).filter(|text| !text.is_empty())
}

/// The group arg as the list shows mail from `address` in it, by its name in the window's words: an invitation stands
/// with its sender's mail ([`GroupKind::shown_for`]), so the line names the group the list and the head's tag name.
/// Without an address, a group the list never shows is none.
fn group(args: &Value, address: Option<&str>) -> Option<&'static str> {
    let kind = args.get("group").and_then(Value::as_str).and_then(GroupKind::named)?;
    match address {
        Some(address) => Some(GroupKind::shown_for(kind, address).text()),
        None => GroupKind::SHOWN.contains(&kind).then(|| kind.text()),
    }
}

/// The name the index gives folder `id`: its role's, else its path.
fn folder(mailbox: &Mailbox, id: u64) -> Option<String> {
    let folder = mailbox.accounts().iter().find_map(|account| mailbox.folders_of(account.id).into_iter().find(|folder| folder.id == id))?;
    match folder.role {
        Some(role) => Some(role_text(role).to_owned()),
        None => Some(shown(&folder.name)).filter(|name| !name.trim().is_empty()),
    }
}

/// The one sentence of `text_id` with `args`; none for an id the window does not know or args it cannot read.
fn sentence(mailbox: &Mailbox, id: u64, text_id: &str, args: &Value) -> Option<String> {
    Some(match text_id {
        "why.snooze.returned" => tr!("back from a snooze, {when}", when = when(mailbox, args, "at")?),
        "why.snooze.until" => tr!("snoozed until {when}", when = when(mailbox, args, "until")?),
        "why.rule.group" => {
            let address = mailbox.messages().iter().find(|row| row.id == id).map(|row| row.address.as_str());
            let rule = args.get("rule").and_then(Value::as_str);
            let Some(group) = group(args, address) else {
                return (rule == Some("calendar")).then(|| tr!("it holds an invitation").to_owned());
            };
            match rule {
                Some("calendar") => tr!("grouped as {group}: it holds an invitation", group = group),
                Some("automatic") => tr!("grouped as {group}: sent automatically", group = group),
                Some("list") => tr!("grouped as {group}: sent through a list", group = group),
                Some("junk") => tr!("grouped as {group}: marked as bulk mail", group = group),
                Some("robot") => tr!("grouped as {group}: from an address that takes no reply", group = group),
                _ => tr!("grouped as {group}", group = group),
            }
        }
        "why.sender.group" => {
            let address = text(args, "address")?;
            tr!("you put {address} in {group}", group = group(args, Some(&address))?, address = address)
        }
        "why.sender.accepted" => tr!("let in by you").to_owned(),
        "why.sender.blocked" => tr!("blocked by you").to_owned(),
        "why.screener.held" => tr!("held back by the screener").to_owned(),
        "why.screener.blocked" => tr!("in Blocked").to_owned(),
        "why.screener.new" => tr!("first mail from this address").to_owned(),
        "why.screener.known" => tr!("a sender you know").to_owned(),
        "why.thread.yours" => tr!("in a thread you wrote in, {count} messages", count = args.get("messages").and_then(Value::as_u64)?),
        "why.thread.other" => tr!("in a thread of {count} messages", count = args.get("messages").and_then(Value::as_u64)?),
        "why.default" => match args.get("folder").and_then(Value::as_u64).and_then(|id| folder(mailbox, id)) {
            Some(name) => tr!("in {folder}", folder = name),
            None => tr!("in this folder").to_owned(),
        },
        _ => return None,
    })
}

#[cfg(test)]
#[path = "why_tests.rs"]
mod tests;
