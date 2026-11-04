// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! NOTIFY (RFC 5465): which mailboxes the client hears of without asking, and of which events.

use crate::imap::reader::Token;
use crate::imap::session::{Session, State};
use crate::imap::update::status;
use crate::imap::write::{no, ok};

/// The events the fake knows; the others are refused with BADEVENT.
const EVENTS: &str = "(MessageNew MessageExpunge FlagChange MailboxName SubscriptionChange)";

/// What a NOTIFY SET asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Notify {
    /// The events of the open mailbox, when SELECTED or SELECTED-DELAYED was asked for.
    pub(super) selected: Option<Events>,
    others: Vec<(Filter, Events)>,
}

impl Notify {
    /// Whether a change to the mailbox called `name`, when it is not open, gets a STATUS line.
    pub(super) fn covers(&self, name: &str) -> bool {
        self.others.iter().any(|(filter, events)| events.new && filter.covers(name))
    }
}

/// The events of one filter. MessageNew and MessageExpunge always come together, so one flag stands for both.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Events {
    new: bool,
    /// The FETCH items (UID, FLAGS, MODSEQ) to send with each new message of the open mailbox.
    pub(super) fetch: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Filter {
    Inboxes,
    /// PERSONAL and SUBSCRIBED: the fake's mailboxes are all personal and all subscribed.
    All,
    Subtree(Vec<String>),
    Mailboxes(Vec<String>),
}

impl Filter {
    fn covers(&self, name: &str) -> bool {
        match self {
            Self::Inboxes => name == "INBOX",
            Self::All => true,
            Self::Subtree(roots) => {
                roots.iter().any(|root| name == root || name.strip_prefix(root.as_str()).is_some_and(|rest| rest.starts_with('/')))
            }
            Self::Mailboxes(names) => names.iter().any(|own| own == name),
        }
    }
}

/// Why a NOTIFY SET is refused: a syntax error (`BAD`) or an event the fake does not know (`NO [BADEVENT]`).
enum Refusal {
    Bad(&'static str),
    BadEvent,
}

const BAD: Refusal = Refusal::Bad("Bad NOTIFY");

impl Session {
    pub(super) fn notify_set(&mut self, tag: &str, args: &[Token], out: &mut Vec<u8>) {
        let word = |token: Option<&Token>, wanted: &str| token.and_then(Token::atom).is_some_and(|word| word.eq_ignore_ascii_case(wanted));
        if word(args.first(), "NONE") && args.len() == 1 {
            self.notify = None;
            return ok(out, tag, "NOTIFY completed");
        }
        if !word(args.first(), "SET") {
            return self.bad(out, Some(tag), "Bad NOTIFY");
        }
        let statuses = word(args.get(1), "STATUS");
        let groups = args.get(if statuses { 2 } else { 1 }..).unwrap_or_default();
        let notify = match read(groups) {
            Ok(notify) => notify,
            Err(Refusal::Bad(reason)) => return self.bad(out, Some(tag), reason),
            Err(Refusal::BadEvent) => return no(out, tag, &format!("[BADEVENT {EVENTS}] Unsupported event")),
        };
        let open = match &self.state {
            State::Selected(selected) => Some(selected.index),
            _ => None,
        };
        for (index, mailbox) in self.fake.mailboxes.iter().enumerate().filter(|_| statuses) {
            if Some(index) != open && notify.covers(&mailbox.name) {
                status(mailbox, self.condstore, out);
            }
        }
        self.notify = Some(notify);
        ok(out, tag, "NOTIFY completed");
    }
}

fn read(groups: &[Token]) -> Result<Notify, Refusal> {
    if groups.is_empty() {
        return Err(BAD);
    }
    let mut notify = Notify::default();
    for group in groups {
        let Token::List(parts) = group else { return Err(BAD) };
        let [Token::Atom(filter), rest @ ..] = parts.as_slice() else { return Err(BAD) };
        let filter = filter.to_ascii_uppercase();
        let (filter, events) = match filter.as_str() {
            "SELECTED" | "SELECTED-DELAYED" => (None, rest),
            "INBOXES" => (Some(Filter::Inboxes), rest),
            "PERSONAL" | "SUBSCRIBED" => (Some(Filter::All), rest),
            "SUBTREE" | "MAILBOXES" => {
                let [names, events @ ..] = rest else { return Err(BAD) };
                let names = mailbox_names(names).ok_or(BAD)?;
                (Some(if filter == "SUBTREE" { Filter::Subtree(names) } else { Filter::Mailboxes(names) }), events)
            }
            _ => return Err(BAD),
        };
        let [events] = events else { return Err(BAD) };
        let events = read_events(events, filter.is_none())?;
        match filter {
            None => notify.selected = Some(events),
            Some(filter) => notify.others.push((filter, events)),
        }
    }
    Ok(notify)
}

/// One mailbox name or a list of them.
fn mailbox_names(token: &Token) -> Option<Vec<String>> {
    let name = |token: &Token| token.astring().map(|name| String::from_utf8_lossy(name).into_owned());
    match token {
        Token::List(names) => names.iter().map(name).collect(),
        single => name(single).map(|name| vec![name]),
    }
}

fn read_events(token: &Token, selected: bool) -> Result<Events, Refusal> {
    let items = match token {
        Token::Atom(none) if none.eq_ignore_ascii_case("NONE") => return Ok(Events::default()),
        Token::List(items) => items,
        _ => return Err(BAD),
    };
    let (mut new, mut expunge, mut flag_change) = (false, false, false);
    let mut fetch = Vec::new();
    let mut items = items.iter().peekable();
    while let Some(item) = items.next() {
        let Some(event) = item.atom() else { return Err(BAD) };
        match event.to_ascii_lowercase().as_str() {
            "messagenew" => {
                new = true;
                if let Some(Token::List(wanted)) = items.peek() {
                    items.next();
                    fetch = fetch_items(wanted, selected).ok_or(BAD)?;
                }
            }
            "messageexpunge" => expunge = true,
            "flagchange" => flag_change = true,
            "mailboxname" | "subscriptionchange" => {}
            _ => return Err(Refusal::BadEvent),
        }
    }
    if new != expunge || (flag_change && !new) {
        return Err(Refusal::Bad("MessageNew, MessageExpunge and FlagChange go together"));
    }
    Ok(Events { new, fetch })
}

/// The FETCH items of MessageNew: only for the open mailbox, and only UID, FLAGS and MODSEQ.
fn fetch_items(wanted: &[Token], selected: bool) -> Option<Vec<String>> {
    if !selected {
        return None;
    }
    wanted.iter().map(|item| item.atom().map(str::to_ascii_uppercase).filter(|item| matches!(item.as_str(), "UID" | "FLAGS" | "MODSEQ"))).collect()
}
