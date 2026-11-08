// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Notifications: what the owner is told of outside the window. The engine decides; the front end shows each
//! [`Notice`] it takes from [`crate::Session::take_notices`] the way the desktop does it.
//!
//! - New mail (a stage of the arriving-mail pipeline, after the screener): a message tells the owner when it is new in
//!   an inbox after the folder's first sync, unread, still in the inbox after the earlier stages (mail the screener
//!   holds or blocks tells nothing), not from one of the owner's own addresses, and not the owner's own move coming
//!   back: a server that names no UID for a moved message gives it back through the next sync, as accepting a sender
//!   does with the mail held, and that is no new mail. Its account's [`Level`] has the last word: `all`, `people`
//!   (a sender the owner knows or accepted, whose message is in People by the owner's choice or the classifier) or
//!   `none`. [`tells`] is the table.
//! - A snoozed message that comes back and a follow-up reminder that fires tell the owner unless they turned that off
//!   ([`Settings::returns`], [`Settings::reminders`]).
//! - More than [`MAX_SINGLE`] new messages of one account at once become one notice that counts them, and the queue
//!   keeps the newest [`MAX_PENDING`] for a front end that does not take them.
//! - A notice's name and subject are the sender's data: one line, without control characters or invisible marks, at
//!   most [`MAX_TEXT`] characters. The front end shows them as text, never as markup, and acts on nothing in them.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use inboxcraft_model::{AccountId, Address, DraftId, Group, Message, MessageId, Role, Screening, is_invisible_mark};
use inboxcraft_store::{Change, Storage};
use serde_json::{Value, json};

use crate::EngineError;
use crate::arrive::screener::sender_of;
use crate::arrive::{Arrival, group};
use crate::schedule::Fired;
use crate::session::State;
use crate::sync::Plan;

/// More new messages of one account than this, taken at once, become one notice that counts them.
pub const MAX_SINGLE: usize = 3;

/// The notices the session keeps for the front end; older ones go first.
pub const MAX_PENDING: usize = 100;

/// The longest name or subject in a notice, in characters.
pub const MAX_TEXT: usize = 120;

/// How many of the owner's own moves the session remembers, to keep them quiet when they come back.
const MAX_QUIET: usize = 256;

/// The most accounts [`Settings::from_value`] reads.
const MAX_ACCOUNTS: usize = 1000;

/// How much new mail of one account tells the owner.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Level {
    /// Every new message in the inbox.
    #[default]
    All,
    /// New mail from the people the owner knows or accepted.
    People,
    /// Nothing.
    None,
}

impl Level {
    /// Every level.
    pub const ALL: [Self; 3] = [Self::All, Self::People, Self::None];

    /// The level's name, as commands write it.
    ///
    /// ```
    /// use inboxcraft_engine::notify::Level;
    ///
    /// assert_eq!(Level::People.name(), "people");
    /// assert_eq!(Level::from_name("none"), Some(Level::None));
    /// ```
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::People => "people",
            Self::None => "none",
        }
    }

    /// The level named `name`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.name() == name)
    }
}

/// The owner's choices: a [`Level`] per account, and whether returned snoozes and follow-up reminders tell them. The
/// front end keeps them on disk ([`Settings::to_value`]) and hands them back at the start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    levels: BTreeMap<AccountId, Level>,
    /// A snoozed message that comes back tells the owner.
    pub returns: bool,
    /// A follow-up reminder that fires tells the owner.
    pub reminders: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { levels: BTreeMap::new(), returns: true, reminders: true }
    }
}

impl Settings {
    /// The level of `account`; [`Level::All`] until the owner chose another.
    #[must_use]
    pub fn level(&self, account: AccountId) -> Level {
        self.levels.get(&account).copied().unwrap_or_default()
    }

    /// Sets the level of `account`.
    pub fn set_level(&mut self, account: AccountId, level: Level) {
        if level == Level::default() {
            self.levels.remove(&account);
        } else {
            self.levels.insert(account, level);
        }
    }

    /// Forgets what was chosen for `account`.
    pub(crate) fn forget(&mut self, account: AccountId) {
        self.levels.remove(&account);
    }

    /// The choices as JSON, to keep.
    ///
    /// ```
    /// use inboxcraft_engine::notify::{Level, Settings};
    /// use inboxcraft_model::AccountId;
    ///
    /// let mut settings = Settings::default();
    /// settings.set_level(AccountId::new(2), Level::People);
    /// settings.returns = false;
    /// assert_eq!(Settings::from_value(&settings.to_value()), settings);
    /// ```
    #[must_use]
    pub fn to_value(&self) -> Value {
        let accounts: Vec<Value> = self.levels.iter().map(|(account, level)| json!({ "account": account.get(), "level": level.name() })).collect();
        json!({ "accounts": accounts, "returns": self.returns, "reminders": self.reminders })
    }

    /// The choices kept in `value` ([`Settings::to_value`]); what cannot be read is left at its default.
    #[must_use]
    pub fn from_value(value: &Value) -> Self {
        let mut settings = Self::default();
        let accounts = value.get("accounts").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
        for entry in accounts.iter().take(MAX_ACCOUNTS) {
            let account = entry.get("account").and_then(Value::as_u64).filter(|id| *id > 0);
            let level = entry.get("level").and_then(Value::as_str).and_then(Level::from_name);
            if let (Some(account), Some(level)) = (account, level) {
                settings.set_level(AccountId::new(account), level);
            }
        }
        let flag = |key: &str| value.get(key).and_then(Value::as_bool).unwrap_or(true);
        settings.returns = flag("returns");
        settings.reminders = flag("reminders");
        settings
    }
}

/// The message a notice is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct About {
    /// Its account.
    pub account: AccountId,
    /// The message, which a click opens.
    pub message: MessageId,
    /// Who it is from (for a reminder: to), by name or else address; the sender's data.
    pub who: String,
    /// Its subject; the sender's data, empty when it has none.
    pub subject: String,
}

/// Something to tell the owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// A new message.
    Mail(About),
    /// More than [`MAX_SINGLE`] new messages of one account at once.
    Mails {
        /// The account.
        account: AccountId,
        /// How many.
        count: usize,
    },
    /// A snoozed message came back.
    Returned {
        /// The message.
        about: About,
        /// Its time passed while InboxCraft was off or asleep.
        missed: bool,
    },
    /// A sent message got no reply by the time the owner set.
    Reminder {
        /// The sent message.
        about: About,
        /// Its time passed while InboxCraft was off or asleep.
        missed: bool,
    },
    /// A send whose time passed while InboxCraft was off or asleep went late: at the start, or when it woke.
    SentLate {
        /// The account it went from.
        account: AccountId,
        /// Its draft, gone once the server took it.
        draft: DraftId,
        /// Its subject, as the owner wrote it.
        subject: String,
    },
}

impl Notice {
    /// The account it is about.
    #[must_use]
    pub fn account(&self) -> AccountId {
        match self {
            Self::Mail(about) | Self::Returned { about, .. } | Self::Reminder { about, .. } => about.account,
            Self::Mails { account, .. } | Self::SentLate { account, .. } => *account,
        }
    }

    /// The message a click opens; `None` for a count.
    #[must_use]
    pub fn message(&self) -> Option<MessageId> {
        match self {
            Self::Mail(about) | Self::Returned { about, .. } | Self::Reminder { about, .. } => Some(about.message),
            Self::Mails { .. } | Self::SentLate { .. } => None,
        }
    }
}

/// What is known of one arriving message, for [`tells`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Facts {
    /// Its account's level.
    pub(crate) level: Level,
    /// It arrived in an inbox.
    pub(crate) inbox: bool,
    /// The folder's first sync brought it, with every other message the folder holds.
    pub(crate) first_sync: bool,
    /// It is still where it arrived after the earlier stages.
    pub(crate) stays: bool,
    /// It is read already.
    pub(crate) seen: bool,
    /// It is from one of the owner's own addresses.
    pub(crate) own: bool,
    /// It is from a sender the owner knows or accepted, in People: the owner's group for the sender, or else the
    /// grouped inbox's classifier.
    pub(crate) person: bool,
    /// It is the owner's own move, back from the server.
    pub(crate) quiet: bool,
}

/// Whether a message with `facts` tells the owner: the decision table.
pub(crate) fn tells(facts: &Facts) -> bool {
    let new = facts.inbox && !facts.first_sync && facts.stays && !facts.seen && !facts.own && !facts.quiet;
    new && match facts.level {
        Level::All => true,
        Level::People => facts.person,
        Level::None => false,
    }
}

/// The stage: a new message that [`tells`] the owner becomes a [`Notice::Mail`].
pub(crate) fn arrived(arrival: &Arrival<'_>, message: &Message, plan: &mut Plan) -> Result<(), EngineError> {
    let store = arrival.store;
    let from = sender_of(message);
    let own = match from {
        Some(address) => store.accounts()?.iter().any(|account| account.address().as_str().eq_ignore_ascii_case(address.as_str())),
        None => false,
    };
    let record = match from {
        Some(address) => store.sender(address)?,
        None => None,
    };
    let person = record.is_some_and(|record| {
        let met = record.known || record.screening == Some(Screening::Accepted);
        met && record.group.unwrap_or_else(|| group::classify(message.summary().envelope())) == Group::People
    });
    let mark = mark(message);
    let quiet = match plan.state.quiet.iter().position(|held| *held == mark) {
        Some(at) => {
            plan.state.quiet.remove(at);
            true
        }
        None => false,
    };
    let facts = Facts {
        level: plan.state.notify.level(arrival.account),
        inbox: arrival.role == Some(Role::Inbox),
        first_sync: arrival.first_sync,
        stays: stays(plan, message, arrival),
        seen: message.summary().flags().is_seen(),
        own,
        person,
        quiet,
    };
    if tells(&facts) {
        push(&mut plan.state, Notice::Mail(about(message, from_whom(message))));
    }
    Ok(())
}

/// Whether `message` is still where it arrived after the earlier stages of `plan`.
fn stays(plan: &Plan, message: &Message, arrival: &Arrival<'_>) -> bool {
    let placed = plan.batch.changes().iter().rev().find_map(|change| match change {
        Change::PutMessage(placed) if placed.id() == message.id() => Some(placed.folder()),
        _ => None,
    });
    placed.is_none_or(|folder| folder == arrival.folder)
}

/// Remembers that the owner's own move of `message` comes back through a sync, so it tells nothing.
pub(crate) fn quiet(state: &mut State, message: &Message) {
    if state.quiet.len() >= MAX_QUIET {
        state.quiet.pop_front();
    }
    state.quiet.push_back(mark(message));
}

/// What tells two copies of a message apart from other mail: its Message-ID, or without one its sender, subject and
/// date.
fn mark(message: &Message) -> String {
    let envelope = message.summary().envelope();
    match envelope.message_id() {
        Some(id) => format!("id {}", id.as_str()),
        None => {
            let from = sender_of(message).map(|address| address.as_str().to_lowercase()).unwrap_or_default();
            let date = envelope.date().map(|date| date.unix_seconds());
            format!("from {from} {date:?} {}", envelope.subject())
        }
    }
}

/// The notices for the schedule entries that fired, as the owner's [`Settings`] want them; a send that went late,
/// one of `sent`, is told whatever they are.
pub(crate) fn fired(store: &dyn Storage, settings: &Settings, fired: &[Fired], sent: &[DraftId]) -> Result<Vec<Notice>, EngineError> {
    let mut notices = Vec::new();
    for entry in fired {
        let missed = entry.missed;
        let notice = match entry.entry.what {
            inboxcraft_model::Due::Unsnooze(id) if settings.returns => {
                held(store, id)?.map(|message| Notice::Returned { about: about(&message, from_whom(&message)), missed })
            }
            inboxcraft_model::Due::FollowUp(id) if settings.reminders => {
                held(store, id)?.map(|message| Notice::Reminder { about: about(&message, to_whom(&message)), missed })
            }
            inboxcraft_model::Due::Send(id) if missed && sent.contains(&id) => {
                store.draft(id)?.map(|draft| Notice::SentLate { account: draft.account, draft: id, subject: draft.subject })
            }
            _ => None,
        };
        notices.extend(notice);
    }
    Ok(notices)
}

/// The message `id`, if the store still holds it.
fn held(store: &dyn Storage, id: MessageId) -> Result<Option<Message>, EngineError> {
    Ok(store.message(id)?.map(|entry| entry.message().clone()))
}

/// Puts `notice` in the queue, the oldest leaving when it is full.
pub(crate) fn push(state: &mut State, notice: Notice) {
    if state.notices.len() >= MAX_PENDING {
        state.notices.pop_front();
    }
    state.notices.push_back(notice);
}

/// `notices` with the new messages of an account that has more than [`MAX_SINGLE`] of them as one count, where the
/// first of them was.
pub(crate) fn collapse(notices: VecDeque<Notice>) -> Vec<Notice> {
    let mut counts: BTreeMap<AccountId, usize> = BTreeMap::new();
    for notice in &notices {
        if let Notice::Mail(about) = notice {
            let count = counts.entry(about.account).or_default();
            *count = count.saturating_add(1);
        }
    }
    let mut counted = BTreeSet::new();
    let mut told = Vec::new();
    for notice in notices {
        match notice {
            Notice::Mail(about) => {
                let count = counts.get(&about.account).copied().unwrap_or_default();
                if count <= MAX_SINGLE {
                    told.push(Notice::Mail(about));
                } else if counted.insert(about.account) {
                    told.push(Notice::Mails { account: about.account, count });
                }
            }
            other => told.push(other),
        }
    }
    told
}

/// The notice's view of `message`, from or to `who`.
fn about(message: &Message, who: Option<&Address>) -> About {
    let who = who.map(|address| address.name().filter(|name| !name.trim().is_empty()).unwrap_or(address.email().as_str())).unwrap_or_default();
    About { account: message.account(), message: message.id(), who: line(who), subject: line(message.summary().envelope().subject()) }
}

fn from_whom(message: &Message) -> Option<&Address> {
    message.summary().envelope().from().first()
}

fn to_whom(message: &Message) -> Option<&Address> {
    message.summary().envelope().to().first()
}

/// `text` as one short line: white space as single spaces, without control characters and invisible marks, at most
/// [`MAX_TEXT`] characters.
pub(crate) fn line(text: &str) -> String {
    let mut cleaned = String::new();
    let mut kept = 0_usize;
    let mut space = false;
    for c in text.chars() {
        if kept >= MAX_TEXT {
            break;
        }
        if c.is_whitespace() {
            space = !cleaned.is_empty();
            continue;
        }
        if c.is_control() || is_invisible_mark(c) {
            continue;
        }
        if space {
            cleaned.push(' ');
            kept = kept.saturating_add(1);
            space = false;
        }
        cleaned.push(c);
        kept = kept.saturating_add(1);
    }
    cleaned.chars().take(MAX_TEXT).collect::<String>().trim_end().to_owned()
}

#[cfg(test)]
#[path = "notify_tests.rs"]
mod tests;
