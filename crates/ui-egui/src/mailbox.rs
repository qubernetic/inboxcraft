// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the mail list shows, read from the engine's commands: the accounts, the unified views, the folders, the
//! list of the open place or of the search, and the selected row.
//!
//! Everything goes through [`Session::run`] by command id (`account.list`, `folder.list`, `mail.list`,
//! `mail.search`), as the menus, the CLI and MCP do. A command that fails leaves the list as it was and shows its
//! error until the next command succeeds or the person dismisses it.
//!
//! The acts on a message ([`Mailbox::act`]) run the engine's commands (`mail.seen`, `mail.flag`, `mail.archive`,
//! `mail.delete`, `mail.move`), on a real account as on the sample; opening a message marks it read (`mail.open`). Each
//! act that changes something goes into the engine's history, and [`Mailbox::undo`] takes acts back through
//! `history.undo`.
//!
//! The screener and the grouped inbox come from the engine too: `screener.list` gives the senders waiting and the
//! screener's mode, each row of `mail.list` says its group and whether its sender is new. "New senders"
//! ([`Place::Senders`]) lists the mail held, and reading it there is a preview (`mail.read`) that decides nothing.
//! Accept (`screener.accept`, with a group and "open"), Block (`screener.block`) and a sender's group (`group.set`)
//! are acts like the others: a toast with Undo, taken back through `history.undo`. The groups of the unified inbox
//! show as cards or a grouped list, or not at all in the simple list ([`InboxView`]).
//!
//! Done (`mail.done`), park (`mail.park`), pin (`mail.pin`), snooze (`mail.snooze`) and follow-up reminders
//! (`mail.follow_up`) are acts too. "Show done" lists the done mail in its old place; [`Place::Later`] shows a pile
//! of "Later": the snoozed folders, `pin.list` or `park.list`. The time picker's presets come from `schedule.presets`.
//!
//! The outbox comes from `outbox.list`, read with the rest: each send waiting for its hold to end or for the time the
//! owner picked, on its way, or refused with the server's reply. [`Place::Outbox`] lists it while anything is in it;
//! `mail.unsend` takes a waiting send back, `outbox.retry` sends a refused one again, and the hold is `send.set`. A
//! sent message a delivery report says did not get through is marked in its row (`bounced` in `mail.list`). A send
//! whose time passed while InboxCraft was not running is told once ([`Mailbox::take_late`]), besides the desktop's
//! notice.

use std::collections::BTreeMap;

use inboxcraft_engine::appearance::Appearance;
use inboxcraft_engine::notify::{Level, Notice, Settings as NotifySettings};
use inboxcraft_engine::{Applied, Caller, EngineError, Hold, MAX_HISTORY, MAX_LIST, ScreenerMode, Session};
use inboxcraft_model::{Account, AccountId, Role};
use inboxcraft_time::{CivilDateTime, Clock, Offset, Timestamp, Weekday};
use serde_json::{Value, json};

use crate::iconset::IconId;
use crate::workers::Workers;
use crate::{UiError, tr};

/// The most rounds of jobs out and events in [`Mailbox::pump`] makes in one frame.
pub const MAX_PUMP_ROUNDS: usize = 8;

/// How soon [`Mailbox::due_in`] asks again for a time that has passed without firing (a tick that failed): soon, but
/// not every frame.
pub const OVERDUE_WAKE: std::time::Duration = std::time::Duration::from_secs(1);

/// The longest search, in characters; the rest is cut off.
pub const MAX_QUERY: usize = 256;

/// The most characters of a name, subject or preview the list keeps.
pub const MAX_SHOWN: usize = 300;

/// The most notices the mailbox keeps for the desktop between two [`Mailbox::take_notices`]; older ones go.
pub const MAX_NOTICES: usize = 64;

/// The roles in the order the folder pane lists them.
const ROLES: [Role; 11] = [
    Role::Inbox,
    Role::Drafts,
    Role::Sent,
    Role::Archive,
    Role::Junk,
    Role::Trash,
    Role::All,
    Role::Flagged,
    Role::Screener,
    Role::Snoozed,
    Role::Blocked,
];

/// The role a command names (`inbox`, `sent` ...).
fn role(name: &str) -> Option<Role> {
    ROLES.into_iter().find(|role| role.label().eq_ignore_ascii_case(name))
}

/// The role's place in [`ROLES`].
fn rank(role: Role) -> usize {
    ROLES.iter().position(|known| *known == role).unwrap_or(ROLES.len())
}

/// The name of a role as the person reads it.
#[must_use]
pub fn role_text(role: Role) -> &'static str {
    match role {
        Role::Inbox => tr!("Inbox"),
        Role::Drafts => tr!("Drafts"),
        Role::Sent => tr!("Sent"),
        Role::Archive => tr!("Archive"),
        Role::Junk => tr!("Junk"),
        Role::Trash => tr!("Trash"),
        Role::All => tr!("All mail"),
        Role::Flagged => tr!("Flagged"),
        Role::Screener => tr!("Screener"),
        Role::Snoozed => tr!("Snoozed"),
        Role::Blocked => tr!("Blocked"),
    }
}

/// What the list shows: a unified view by role, one folder, the mail the screener holds, or a pile of "Later".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// The folders with this role across the accounts shown.
    View(Role),
    /// One folder.
    Folder(u64),
    /// "New senders": the mail held from senders the owner has not let in yet, in the screener folders and, while the
    /// screener marks them inside the inbox, in the inboxes.
    Senders,
    /// "Later": the snoozed mail, the pins or the park pile, across the accounts shown.
    Later(Pile),
    /// The outbox: what waits to be sent, what is on its way and what the server refused. It opens only while anything
    /// is in it.
    Outbox,
}

/// A pile of "Later": mail put away for now, reachable from the rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pile {
    /// The snoozed mail, by the time it comes back (`mail.list` of the snoozed folders).
    Snoozed,
    /// The pinned mail, wherever it is (`pin.list`).
    Pins,
    /// The park pile: mail set aside without a time (`park.list`).
    Parked,
}

impl Pile {
    /// Every pile, in the order "Later" offers them.
    pub const ALL: [Self; 3] = [Self::Snoozed, Self::Pins, Self::Parked];

    /// Its name in scripts.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Snoozed => "snoozed",
            Self::Pins => "pins",
            Self::Parked => "parked",
        }
    }

    /// The pile a script calls `name`.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|pile| pile.id() == name)
    }

    /// Its name as the person reads it.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::Snoozed => tr!("Snoozed"),
            Self::Pins => tr!("Pins"),
            Self::Parked => tr!("Parked"),
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Snoozed => 0,
            Self::Pins => 1,
            Self::Parked => 2,
        }
    }
}

/// A group of the grouped inbox, as the engine names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GroupKind {
    /// Mail from people.
    People,
    /// Notices sent by machines: receipts, reminders, reports.
    Updates,
    /// Mail through mailing lists.
    Newsletters,
    /// Calendar invitations.
    Invitations,
}

impl GroupKind {
    /// Every group, in the order the inbox shows them.
    pub const ALL: [Self; 4] = [Self::People, Self::Updates, Self::Newsletters, Self::Invitations];

    /// Its name in the engine's commands.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::People => "people",
            Self::Updates => "updates",
            Self::Newsletters => "newsletters",
            Self::Invitations => "invitations",
        }
    }

    /// The group the engine calls `name`.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.id() == name)
    }

    /// Its name as the person reads it.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::People => tr!("People"),
            Self::Updates => tr!("Updates"),
            Self::Newsletters => tr!("Newsletters"),
            Self::Invitations => tr!("Invitations"),
        }
    }

    /// The icon on its badge.
    #[must_use]
    pub fn icon(self) -> IconId {
        match self {
            Self::People => IconId::People,
            Self::Updates => IconId::Notify,
            Self::Newsletters => IconId::Newspaper,
            Self::Invitations => IconId::Calendar,
        }
    }
}

/// How the unified inbox shows its groups.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum InboxView {
    /// A card for each group, its first rows and "Show all".
    #[default]
    Cards,
    /// One list, each group under a heading.
    List,
    /// One list by date, without groups.
    Simple,
}

impl InboxView {
    /// Every view, in the order the Triage page offers them.
    pub const ALL: [Self; 3] = [Self::Cards, Self::List, Self::Simple];

    /// Its name where it is kept.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Cards => "cards",
            Self::List => "list",
            Self::Simple => "simple",
        }
    }

    /// The view kept as `name`.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|view| view.id() == name)
    }

    /// Its name as the person reads it.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::Cards => tr!("Cards"),
            Self::List => tr!("Grouped list"),
            Self::Simple => tr!("Simple list"),
        }
    }
}

/// What is selected after the open message leaves the list (an act or a sync removed it).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum After {
    /// The message below it, the next older one (the one above when it was the last).
    #[default]
    Next,
    /// The message above it, the next newer one (the one below when it was the first).
    Previous,
    /// Nothing: back to the list.
    List,
}

/// An account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRow {
    /// Its id.
    pub id: u64,
    /// The owner's name for it.
    pub label: String,
    /// Its address.
    pub address: String,
    /// The name its mail is sent under; empty when it has none.
    pub name: String,
    /// The signature its new drafts get; empty when it has none.
    pub signature: String,
    /// Its place in the account list, which picks its colour.
    pub index: usize,
    /// Whether its inbox is grouped.
    pub grouped: bool,
}

/// A unified view and how many unread messages its folders hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewRow {
    /// The role.
    pub role: Role,
    /// The unread messages in it.
    pub unread: u64,
}

/// A folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderRow {
    /// Its id.
    pub id: u64,
    /// Its account.
    pub account: u64,
    /// Its full path on the server.
    pub name: String,
    /// The last part of the path.
    pub leaf: String,
    /// How deep it sits under other folders.
    pub depth: usize,
    /// What it is for, when known.
    pub role: Option<Role>,
    /// The unread messages in it.
    pub unread: u64,
    /// All messages in it.
    pub messages: u64,
    /// Whether it can hold mail: a `\Noselect` or `\NonExistent` folder cannot. Fails closed: `false` when
    /// `folder.list` leaves it out, so a move is never offered into a folder the engine would refuse.
    pub selectable: bool,
}

/// A row of the message list. Sender, subject and preview are the sender's text: data, shown and never run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRow {
    /// Its id.
    pub id: u64,
    /// Its account.
    pub account: u64,
    /// The folder it is in.
    pub folder: u64,
    /// The sender's name, or the address without one.
    pub sender: String,
    /// The sender's address, which picks the avatar's colour.
    pub address: String,
    /// The subject.
    pub subject: String,
    /// The first words of the text.
    pub preview: String,
    /// When it was sent or came in.
    pub date: Option<Timestamp>,
    /// Read.
    pub seen: bool,
    /// Flagged.
    pub flagged: bool,
    /// The group of the grouped inbox it is in; `None` in an account that is not grouped.
    pub group: Option<GroupKind>,
    /// Its sender is new: the screener holds it, or marks it inside the inbox.
    pub new_sender: bool,
    /// Done: filed in its account's archive, listed in its old place only while "show done" is on.
    pub done: bool,
    /// On the park pile.
    pub parked: bool,
    /// Pinned.
    pub pinned: bool,
    /// Snoozed until this time.
    pub snoozed_until: Option<Timestamp>,
    /// When it came back from a snooze.
    pub returned: Option<Timestamp>,
    /// The time its follow-up reminder is due, when it has one.
    pub follow_up: Option<Timestamp>,
    /// Why it did not get through, when a delivery report says so of a message the owner sent.
    pub bounced: Option<String>,
}

/// Where a send in the outbox stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendState {
    /// Waiting for its hold to end or for the time the owner picked; it can be taken back until then. The time it
    /// goes, when the engine gave one.
    Waiting(Option<Timestamp>),
    /// Handed to the server, which has not answered yet.
    Sending,
    /// Not sent: the server's reply, or why it could not go; the draft is kept.
    Refused {
        /// The server's reply, fit for one line.
        reply: String,
        /// Whether the server would refuse it again as it is.
        permanent: bool,
    },
}

/// A send in the outbox. Its subject and recipients are the owner's own text, shown and never run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxRow {
    /// Its draft.
    pub draft: u64,
    /// The account it goes from.
    pub account: u64,
    /// The subject.
    pub subject: String,
    /// Whom it goes to: each recipient's name, or the address without one.
    pub to: Vec<String>,
    /// The first recipient's address, which picks the avatar's colour.
    pub address: String,
    /// Where it stands.
    pub state: SendState,
    /// It answers an invitation: sent again as it is, never edited.
    pub invite_answer: bool,
}

/// A sender the screener holds back until the owner lets them in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiting {
    /// Their name, or the address without one.
    pub name: String,
    /// Their address, which picks the avatar's colour.
    pub address: String,
    /// Their mail held, by id.
    pub messages: Vec<u64>,
}

/// A group of the grouped inbox: its badge's icon, its name and its messages, by id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// The icon on its badge.
    pub icon: IconId,
    /// Its name.
    pub name: String,
    /// Its messages, by id; those not in the list are left out.
    pub messages: Vec<u64>,
}

/// Invented senders and groups `ui_shot` shows in place of the engine's, to picture the cards over any list. In the app
/// it stays empty, and the screener card and the groups come from the engine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Showcase {
    /// The senders waiting in the screener; its card shows while there are any.
    pub screener: Vec<Waiting>,
    /// The groups of the unified inbox.
    pub groups: Vec<Group>,
}

impl Showcase {
    /// Whether it shows nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.screener.is_empty() && self.groups.is_empty()
    }
}

/// What a hover action, a toolbar button or its key does to a message, through the engine's commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// Mark it read (`mail.seen`).
    Read,
    /// Mark it unread (`mail.seen`).
    Unread,
    /// Flag it (`mail.flag`).
    Flag,
    /// Take its flag off (`mail.flag`).
    Unflag,
    /// Mark it read and file it as done in its account's Archive folder (`mail.seen`, `mail.done`): it leaves the
    /// inbox, and "show done" lists it there again, dimmed.
    Done,
    /// Bring a done message back to where it was done from (`mail.done`).
    Undone,
    /// Put it on the park pile (`true`) or take it off (`mail.park`).
    Park(bool),
    /// Pin it (`true`) or take its pin off (`mail.pin`).
    Pin(bool),
    /// Snooze it until a time (`mail.snooze`): it waits in the snoozed folder and comes back unread then.
    Snooze(Timestamp),
    /// Bring a snoozed message back to the inbox now (`mail.unsnooze`), which the history does not hold.
    Unsnooze,
    /// Remind the owner at a time if a message they sent gets no reply by then (`mail.follow_up`).
    FollowUp(Timestamp),
    /// Take the follow-up reminder off (`mail.cancel_follow_up`).
    CancelFollowUp,
    /// Move it to its account's Archive folder as it is (`mail.archive`).
    Archive,
    /// Move it to its account's Trash folder, or delete it for good when it is in the Trash (`mail.delete`).
    Delete,
    /// Move it to the folder with this id, of its account (`mail.move`).
    Move(u64),
    /// A change to a calendar event, made on the calendar screen ([`Mailbox::event_act`]); no act on a message.
    Event,
    /// Let its sender in (`screener.accept`): their held mail and their future mail come in, into `group` when one is
    /// chosen; with `open`, the message opens in the inbox.
    Accept {
        /// The group their mail goes to.
        group: Option<GroupKind>,
        /// Whether the message opens in the inbox.
        open: bool,
    },
    /// Block its sender (`screener.block`): their held and future mail go to the blocked folder.
    Block,
    /// Put its sender's mail, kept and future, in a group, or back in the classifier's hands with `None`
    /// (`group.set`).
    Group(Option<GroupKind>),
}

impl Act {
    /// Whether the row slides out of the list first: the message leaves the list.
    #[must_use]
    pub fn slides(self) -> bool {
        matches!(self, Self::Done | Self::Archive | Self::Delete | Self::Move(_) | Self::Park(true) | Self::Snooze(_))
    }
}

/// The last act, while its toast shows: what it was, what the toast says, and how far back Undo takes the engine's
/// history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undo {
    /// What was done.
    pub act: Act,
    /// To which message.
    pub id: u64,
    /// The how-manyth act of the session, so a toast knows a new act from the one it shows.
    pub serial: u64,
    /// What the toast says.
    pub text: String,
    /// Whether it went into the history, so Undo can take it back: an act that changed nothing or a delete for good
    /// did not.
    pub undoable: bool,
    /// How many acts the history held before it.
    depth: usize,
    selected: bool,
}

/// The mail list's state over a session.
pub struct Mailbox {
    session: Session,
    clock: Box<dyn Clock>,
    offset: Offset,
    accounts: Vec<AccountRow>,
    views: Vec<ViewRow>,
    folders: Vec<FolderRow>,
    narrowed: Option<u64>,
    place: Place,
    query: String,
    messages: Vec<MessageRow>,
    selected: Option<u64>,
    /// Whether the selection came from the keyboard.
    keyed: bool,
    showcase: Showcase,
    /// The senders the screener holds back, from `screener.list`.
    waiting: Vec<Waiting>,
    /// The screener's mode, from `screener.list`.
    screener: ScreenerMode,
    inbox_view: InboxView,
    /// The message an accept-and-open opens in the inbox.
    opening: Option<u64>,
    /// The message sliding out of the list, and what is done to it once it is out.
    leaving: Option<(u64, Act)>,
    /// The act the toast shows.
    undo: Option<Undo>,
    acts: u64,
    /// How many acts the engine's history holds, as counted from the answers: each act that changed something adds
    /// one, `history.undo` takes one off.
    history: usize,
    /// The message an undo brought back, which opens without being marked read.
    restored: Option<u64>,
    /// "Show done": the done mail is listed in its old place, dimmed.
    show_done: bool,
    /// How many messages of the place shown are on the park pile, from `mail.list`.
    parked: u64,
    /// How many messages each pile of "Later" holds, read while "Later" shows.
    piles: [u64; 3],
    /// The time presets still ahead, as the time picker names them, from `schedule.presets`.
    presets: Vec<(String, Timestamp)>,
    /// The outbox, from `outbox.list`.
    outbox: Vec<OutboxRow>,
    /// The subjects of the sends that went late since [`Mailbox::take_late`] was last called.
    late: Vec<String>,
    /// The engine's notices since [`Mailbox::take_notices`] was last called.
    notices: Vec<Notice>,
    after: After,
    error: Option<String>,
    runs: u64,
    /// The id of the last command run through [`Mailbox::run`].
    last: Option<&'static str>,
}

impl std::fmt::Debug for Mailbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mailbox")
            .field("narrowed", &self.narrowed)
            .field("place", &self.place)
            .field("messages", &self.messages.len())
            .finish_non_exhaustive()
    }
}

fn shape(command: &'static str, detail: &str) -> UiError {
    UiError::Answer { command, detail: detail.to_owned() }
}

fn array<'a>(command: &'static str, answer: &'a Value, key: &str) -> Result<&'a Vec<Value>, UiError> {
    answer.get(key).and_then(Value::as_array).ok_or_else(|| shape(command, &format!("no {key} list")))
}

fn number(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn text(value: &Value, key: &str) -> String {
    shown(value.get(key).and_then(Value::as_str).unwrap_or_default())
}

/// Text from a server or a sender made fit for one line: control characters (line breaks, tabs, NUL) become spaces,
/// the bidi controls that could reorder what is shown go, and it is cut to [`MAX_SHOWN`] characters.
pub(crate) fn shown(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}'))
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_SHOWN)
        .collect()
}

/// The first sender's name, or the address when it has none.
pub(crate) fn sender(from: &Value) -> String {
    let Some(first) = from.as_array().and_then(|from| from.first()) else { return tr!("(no sender)").to_owned() };
    match first.get("name").and_then(Value::as_str).filter(|name| !name.is_empty()) {
        Some(name) => shown(name),
        None => text(first, "address"),
    }
}

/// An ISO 8601 date as the engine writes it (`2026-11-05T09:30:00Z`).
pub(crate) fn parse_date(text: &str) -> Option<Timestamp> {
    let number = |from: usize, to: usize| text.get(from..to).filter(|digits| digits.bytes().all(|b| b.is_ascii_digit()))?.parse::<u16>().ok();
    let small = |from: usize, to: usize| number(from, to).and_then(|n| u8::try_from(n).ok());
    if text.len() != 20 || !text.ends_with('Z') {
        return None;
    }
    CivilDateTime::new(i32::from(number(0, 4)?), small(5, 7)?, small(8, 10)?, small(11, 13)?, small(14, 16)?, small(17, 19)?)?
        .to_timestamp(Offset::UTC)
}

fn message_row(value: &Value) -> MessageRow {
    MessageRow {
        id: number(value, "id"),
        account: number(value, "account"),
        folder: number(value, "folder"),
        sender: sender(value.get("from").unwrap_or(&Value::Null)),
        address: value.get("from").and_then(Value::as_array).and_then(|from| from.first()).map(|first| text(first, "address")).unwrap_or_default(),
        subject: text(value, "subject"),
        preview: text(value, "preview"),
        date: value.get("date").and_then(Value::as_str).and_then(parse_date),
        seen: value.get("seen").and_then(Value::as_bool).unwrap_or(false),
        flagged: value.get("flagged").and_then(Value::as_bool).unwrap_or(false),
        group: value.get("group").and_then(Value::as_str).and_then(GroupKind::named),
        new_sender: value.get("new_sender").and_then(Value::as_bool).unwrap_or(false),
        done: value.get("done").and_then(Value::as_bool).unwrap_or(false),
        parked: value.get("parked").and_then(Value::as_bool).unwrap_or(false),
        pinned: value.get("pinned").and_then(Value::as_bool).unwrap_or(false),
        snoozed_until: value.get("snoozed_until").and_then(Value::as_str).and_then(parse_date),
        returned: value.get("returned").and_then(Value::as_str).and_then(parse_date),
        follow_up: value.get("follow_up").and_then(Value::as_str).and_then(parse_date),
        bounced: value.get("bounced").and_then(|bounce| bounce.get("reason")).and_then(Value::as_str).map(shown),
    }
}

/// The recipient `text` (`Name <address>`, or an address) as the outbox shows it: the name, or the address without one;
/// and the address.
fn recipient(text: &str) -> (String, String) {
    let text = text.trim();
    let named =
        text.strip_suffix('>').and_then(|rest| rest.rfind('<').map(|at| (rest.get(..at).unwrap_or_default(), rest.get(at.saturating_add(1)..))));
    match named {
        Some((name, Some(address))) => {
            let name = name.trim().trim_matches('"').trim();
            (shown(if name.is_empty() { address } else { name }), shown(address.trim()))
        }
        _ => (shown(text), shown(text)),
    }
}

fn outbox_row(value: &Value) -> OutboxRow {
    let to: Vec<(String, String)> =
        value.get("to").and_then(Value::as_array).map(|to| to.iter().filter_map(Value::as_str).map(recipient).collect()).unwrap_or_default();
    let state = match value.get("state").and_then(Value::as_str) {
        Some("sending") => SendState::Sending,
        Some("refused") => {
            SendState::Refused { reply: text(value, "reply"), permanent: value.get("permanent").and_then(Value::as_bool).unwrap_or(false) }
        }
        _ => SendState::Waiting(value.get("at").and_then(Value::as_str).and_then(parse_date)),
    };
    OutboxRow {
        draft: number(value, "draft"),
        account: number(value, "account"),
        subject: text(value, "subject"),
        address: to.first().map(|(_, address)| address.clone()).unwrap_or_default(),
        to: to.into_iter().map(|(name, _)| name).collect(),
        state,
        invite_answer: value.get("invite_answer").and_then(Value::as_bool).unwrap_or(false),
    }
}

/// `at` as the engine reads a time (`2026-11-05T09:30:00Z`).
pub(crate) fn engine_date(at: Timestamp) -> String {
    match at.to_civil(Offset::UTC) {
        Some(civil) => {
            format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", civil.year(), civil.month(), civil.day(), civil.hour(), civil.minute(), civil.second())
        }
        None => String::new(),
    }
}

/// The name of the engine's time preset `name` (`schedule.presets`), as the time picker shows it.
fn preset_text(name: &str) -> Option<&'static str> {
    match name {
        "later_today" => Some(tr!("Later today")),
        "this_evening" => Some(tr!("This evening")),
        "tomorrow_morning" => Some(tr!("Tomorrow morning")),
        _ => None,
    }
}

pub(crate) fn month_text(month: u8) -> &'static str {
    match month {
        1 => tr!("Jan"),
        2 => tr!("Feb"),
        3 => tr!("Mar"),
        4 => tr!("Apr"),
        5 => tr!("May"),
        6 => tr!("Jun"),
        7 => tr!("Jul"),
        8 => tr!("Aug"),
        9 => tr!("Sep"),
        10 => tr!("Oct"),
        11 => tr!("Nov"),
        _ => tr!("Dec"),
    }
}

pub(crate) fn weekday_text(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => tr!("Monday"),
        Weekday::Tuesday => tr!("Tuesday"),
        Weekday::Wednesday => tr!("Wednesday"),
        Weekday::Thursday => tr!("Thursday"),
        Weekday::Friday => tr!("Friday"),
        Weekday::Saturday => tr!("Saturday"),
        Weekday::Sunday => tr!("Sunday"),
    }
}

/// The day number of a civil date, counted from an arbitrary start: only differences mean anything.
pub(crate) fn day_number(civil: CivilDateTime) -> Option<i64> {
    CivilDateTime::new(civil.year(), civil.month(), civil.day(), 0, 0, 0)?.to_timestamp(Offset::UTC).map(|t| t.unix_seconds().div_euclid(86_400))
}

/// `date` as the list shows it at `now` in `offset`: the time today, "Yesterday", the weekday within the last week,
/// the day and month this year, the full date before.
#[must_use]
pub fn when(date: Timestamp, now: Timestamp, offset: Offset) -> String {
    let (Some(day), Some(today)) = (date.to_civil(offset), now.to_civil(offset)) else { return String::new() };
    let ago = day_number(today).zip(day_number(day)).map_or(i64::MAX, |(today, day)| today.saturating_sub(day));
    match ago {
        0 => format!("{:02}:{:02}", day.hour(), day.minute()),
        1 => tr!("Yesterday").to_owned(),
        2..=6 => weekday_text(day.weekday()).to_owned(),
        _ if day.year() == today.year() => tr!("{day} {month}", day = day.day(), month = month_text(day.month())),
        _ => tr!("{day} {month} {year}", day = day.day(), month = month_text(day.month()), year = day.year()),
    }
}

impl Mailbox {
    /// The list over `session`, on the unified inbox of every account; `clock` dates the rows.
    #[must_use]
    pub fn new(session: Session, clock: Box<dyn Clock>) -> Self {
        let mut mailbox = Self {
            session,
            clock,
            offset: Offset::UTC,
            accounts: Vec::new(),
            views: Vec::new(),
            folders: Vec::new(),
            narrowed: None,
            place: Place::View(Role::Inbox),
            query: String::new(),
            messages: Vec::new(),
            selected: None,
            keyed: false,
            showcase: Showcase::default(),
            waiting: Vec::new(),
            screener: ScreenerMode::Off,
            inbox_view: InboxView::default(),
            opening: None,
            leaving: None,
            undo: None,
            acts: 0,
            history: 0,
            restored: None,
            show_done: false,
            parked: 0,
            piles: [0; 3],
            presets: Vec::new(),
            outbox: Vec::new(),
            late: Vec::new(),
            notices: Vec::new(),
            after: After::Next,
            error: None,
            runs: 0,
            last: None,
        };
        mailbox.refresh();
        mailbox
    }

    /// Reads the accounts, the folders and the list again. When the selected message has left the list, the
    /// next-message rule ([`After`]) picks what is selected instead.
    pub fn refresh(&mut self) {
        let was = self.selected.and_then(|id| self.messages.iter().position(|row| row.id == id));
        let result = self
            .load_accounts()
            .and_then(|()| self.load_folders())
            .and_then(|()| self.load_screener())
            .and_then(|()| self.load_outbox())
            .and_then(|()| self.load_list(self.place));
        if let (Some(at), None) = (was, self.selected) {
            let last = self.messages.len().checked_sub(1);
            let pick = match self.after {
                After::Next => last.map(|last| at.min(last)),
                After::Previous => last.map(|last| at.saturating_sub(1).min(last)),
                After::List => None,
            };
            self.selected = pick.and_then(|at| self.messages.get(at)).map(|row| row.id);
        }
        self.settle(result);
    }

    /// The next-message rule.
    #[must_use]
    pub fn after(&self) -> After {
        self.after
    }

    /// Sets the next-message rule.
    pub fn set_after(&mut self, after: After) {
        self.after = after;
    }

    /// Selects the row below the selected one (`forward`) or above it; the first row when none is selected. Stays at
    /// either end.
    pub fn step(&mut self, forward: bool) {
        let at = self.selected.and_then(|id| self.messages.iter().position(|row| row.id == id));
        let next = match at {
            None => 0,
            Some(at) if forward => at.saturating_add(1).min(self.messages.len().saturating_sub(1)),
            Some(at) => at.saturating_sub(1),
        };
        if let Some(row) = self.messages.get(next) {
            self.selected = Some(row.id);
            self.keyed = true;
        }
    }

    /// Whether the selection came from the keyboard ([`Mailbox::step`]): the selected row then shows the hover
    /// actions, as a hovered row does.
    #[must_use]
    pub fn keyed(&self) -> bool {
        self.keyed
    }

    /// The invented parts `ui_shot` shows; empty in the app.
    #[must_use]
    pub fn showcase(&self) -> &Showcase {
        &self.showcase
    }

    /// Shows `showcase` in the list, for `ui_shot`.
    pub fn set_showcase(&mut self, showcase: Showcase) {
        self.showcase = showcase;
    }

    /// The senders the screener holds back: the showcase's while it has any, else the engine's.
    #[must_use]
    pub fn waiting(&self) -> &[Waiting] {
        if self.showcase.screener.is_empty() { &self.waiting } else { &self.showcase.screener }
    }

    /// Whether this is the sample mailbox.
    #[must_use]
    pub fn is_sample(&self) -> bool {
        self.session.is_sample()
    }

    /// The screener's mode.
    #[must_use]
    pub fn screener_mode(&self) -> ScreenerMode {
        self.screener
    }

    /// Sets the screener's mode (`screener.mode`). Turning it on learns every sender the kept mail names.
    pub fn set_screener_mode(&mut self, mode: ScreenerMode) {
        let result = self.run("screener.mode", &json!({ "mode": mode.name() })).map(|_| ());
        self.refresh();
        if let Err(error) = result {
            self.fail(error);
        }
    }

    /// Takes the screener's mode kept from an earlier run, without learning anyone.
    pub(crate) fn restore_screener(&mut self, mode: ScreenerMode) {
        self.session.set_screener(mode);
        self.refresh();
    }

    /// Turns the grouped inbox of `account` on or off (`group.account`).
    pub fn set_grouped(&mut self, account: u64, grouped: bool) {
        let result = self.run("group.account", &json!({ "account": account, "grouped": grouped })).map(|_| ());
        self.refresh();
        if let Err(error) = result {
            self.fail(error);
        }
    }

    /// How the unified inbox shows its groups.
    #[must_use]
    pub fn inbox_view(&self) -> InboxView {
        self.inbox_view
    }

    /// Shows the unified inbox's groups as `view` does.
    pub fn set_inbox_view(&mut self, view: InboxView) {
        self.inbox_view = view;
    }

    /// The groups the list shows: in the unified inbox without a search, unless the view is the simple list; the
    /// showcase's while it has any, else each group of the engine's that holds a row, in the order of
    /// [`GroupKind::ALL`]. The rows no group holds are left out.
    #[must_use]
    pub fn groups(&self) -> Vec<Group> {
        if self.place != Place::View(Role::Inbox) || !self.query.is_empty() || self.inbox_view == InboxView::Simple {
            return Vec::new();
        }
        if !self.showcase.groups.is_empty() {
            return self.showcase.groups.clone();
        }
        GroupKind::ALL
            .into_iter()
            .map(|kind| Group {
                icon: kind.icon(),
                name: kind.text().to_owned(),
                messages: self.messages.iter().filter(|row| row.group == Some(kind)).map(|row| row.id).collect(),
            })
            .filter(|group| !group.messages.is_empty())
            .collect()
    }

    /// Does `act` to the message `id` through the engine's commands, at once, and shows its toast; Undo takes it back
    /// ([`Mailbox::undo`]). An act that takes the message out of the list lets the next-message rule pick what is
    /// selected. The engine changes the store at once and sends the change to the server as a kept job; on the sample
    /// mailbox nothing goes out.
    pub fn act(&mut self, id: u64, act: Act) {
        let (before, selected) = (self.history, self.selected == Some(id));
        let result = self.acted(id, act);
        if let Some(open) = self.opening.take().filter(|_| result.is_ok()) {
            self.query.clear();
            self.place = Place::View(Role::Inbox);
            self.selected = Some(open);
        }
        if result.is_ok() || self.history != before {
            self.refresh();
        }
        match result {
            Ok(text) => {
                self.acts = self.acts.saturating_add(1);
                self.undo = Some(Undo { act, id, serial: self.acts, text, undoable: self.history > before, depth: before, selected });
            }
            Err(error) => self.fail(error),
        }
    }

    /// Marks every message of `ids` read and done ("Done all" on a group's card), with one toast whose Undo brings them
    /// all back, or, when they took more acts than the engine's history holds, a toast that says they cannot be undone.
    /// The first refusal stops it, and the messages done before it stay done.
    pub fn done_all(&mut self, ids: &[u64]) {
        let Some(first) = ids.first().copied() else { return };
        let (before, selected) = (self.history, self.selected.is_some_and(|id| ids.contains(&id)));
        let mut result = Ok(());
        for id in ids {
            if let Err(error) = self.acted(*id, Act::Done) {
                result = Err(error);
                break;
            }
        }
        if self.history != before {
            self.refresh();
        }
        match result {
            Ok(()) => {
                self.acts = self.acts.saturating_add(1);
                // The engine's history keeps the newest MAX_HISTORY acts only: past that, Undo could bring back only
                // some of the group, so the toast offers none.
                let held = self.history.saturating_sub(before) <= MAX_HISTORY;
                let text = if held {
                    tr!("Marked {count} as done", count = ids.len())
                } else {
                    tr!("Marked {count} as done, too many to undo", count = ids.len())
                };
                let undoable = held && self.history > before;
                self.undo = Some(Undo { act: Act::Done, id: first, serial: self.acts, text, undoable, depth: before, selected });
            }
            Err(error) => self.fail(error),
        }
    }

    /// Runs the commands of `act` on the message `id`; what its toast says.
    fn acted(&mut self, id: u64, act: Act) -> Result<String, UiError> {
        let one = json!({ "id": id });
        let text = match act {
            Act::Read | Act::Unread => {
                self.write("mail.seen", &json!({ "id": id, "seen": act == Act::Read }))?;
                if act == Act::Read { tr!("Marked as read") } else { tr!("Marked as unread") }
            }
            Act::Flag | Act::Unflag => {
                self.write("mail.flag", &json!({ "id": id, "flagged": act == Act::Flag }))?;
                if act == Act::Flag { tr!("Flagged") } else { tr!("Flag taken off") }
            }
            Act::Done => {
                self.write("mail.seen", &json!({ "id": id, "seen": true }))?;
                self.write("mail.done", &json!({ "id": id, "done": true }))?;
                tr!("Marked as done")
            }
            Act::Undone => {
                self.write("mail.done", &json!({ "id": id, "done": false }))?;
                tr!("Back in its place")
            }
            Act::Park(parked) => {
                self.write("mail.park", &json!({ "id": id, "parked": parked }))?;
                if parked { tr!("Parked") } else { tr!("Taken off the park pile") }
            }
            Act::Pin(pinned) => {
                self.write("mail.pin", &json!({ "id": id, "pinned": pinned }))?;
                if pinned { tr!("Pinned") } else { tr!("Pin taken off") }
            }
            Act::Snooze(until) => {
                self.write("mail.snooze", &json!({ "id": id, "until": engine_date(until) }))?;
                return Ok(tr!("Snoozed until {when}", when = self.long_date(until)));
            }
            Act::Unsnooze => {
                // Bringing it back early is no act of the history: Undo has nothing to take back.
                self.run("mail.unsnooze", &one)?;
                tr!("Back in the inbox")
            }
            Act::FollowUp(until) => {
                self.write("mail.follow_up", &json!({ "id": id, "until": engine_date(until) }))?;
                return Ok(tr!("Reminder set for {when}", when = self.long_date(until)));
            }
            Act::CancelFollowUp => {
                self.write("mail.cancel_follow_up", &one)?;
                tr!("Reminder taken off")
            }
            Act::Archive => {
                self.write("mail.archive", &one)?;
                tr!("Moved to Archive")
            }
            Act::Delete => {
                let answer = self.write("mail.delete", &one)?;
                if answer.get("permanent").and_then(Value::as_bool) == Some(true) { tr!("Deleted for good") } else { tr!("Moved to Trash") }
            }
            Act::Event => return Err(UiError::Answer { command: "event", detail: "an event changes on the calendar screen".to_owned() }),
            Act::Move(folder) => {
                self.write("mail.move", &json!({ "id": id, "folder": folder }))?;
                let leaf = self.folders.iter().find(|row| row.id == folder).map(|row| row.leaf.clone()).unwrap_or_default();
                return Ok(tr!("Moved to {folder}", folder = leaf));
            }
            Act::Accept { group, open } => {
                let (name, address) = self.sender_of(id)?;
                let mut params = json!({ "address": address, "group": group.map(GroupKind::id) });
                if let (true, Value::Object(fields)) = (open, &mut params) {
                    fields.insert("then".to_owned(), json!("open"));
                }
                let answer = self.screen("screener.accept", &params)?;
                self.opening = answer.get("open").and_then(Value::as_u64);
                return Ok(match group {
                    Some(group) => tr!("Accepted {sender} into {group}", sender = name, group = group.text()),
                    None => tr!("Accepted {sender}", sender = name),
                });
            }
            Act::Block => {
                let (name, address) = self.sender_of(id)?;
                self.screen("screener.block", &json!({ "address": address }))?;
                return Ok(tr!("Blocked {sender}", sender = name));
            }
            Act::Group(group) => {
                let (name, address) = self.sender_of(id)?;
                self.screen("group.set", &json!({ "address": address, "group": group.map_or("auto", GroupKind::id) }))?;
                return Ok(match group {
                    Some(group) => tr!("{sender} now goes to {group}", sender = name, group = group.text()),
                    None => tr!("{sender} goes where InboxCraft sorts it", sender = name),
                });
            }
        };
        Ok(text.to_owned())
    }

    /// The sender of the row `id`: the name the list shows and the address.
    fn sender_of(&self, id: u64) -> Result<(String, String), UiError> {
        self.messages
            .iter()
            .find(|row| row.id == id && !row.address.is_empty())
            .map(|row| (row.sender.clone(), row.address.clone()))
            .ok_or(UiError::Engine(EngineError::NotFound { what: "message", id }))
    }

    /// Runs the screening act `command` (accept, block, a group), which always goes into the engine's history.
    fn screen(&mut self, command: &'static str, params: &Value) -> Result<Value, UiError> {
        let answer = self.run(command, params)?;
        self.history = self.history.saturating_add(1);
        Ok(answer)
    }

    /// Runs the act `command`: one act more in the engine's history when it changed something.
    fn write(&mut self, command: &'static str, params: &Value) -> Result<Value, UiError> {
        let answer = self.run(command, params)?;
        if answer.get("changed").and_then(Value::as_bool) == Some(true) {
            self.history = self.history.saturating_add(1);
        }
        Ok(answer)
    }

    /// Reads the message `id` the reader opens, through `mail.open`, which marks it read: an act in the history, shown
    /// in the list at once. The message an undo has just brought back is read through `mail.read` and stays as it was.
    ///
    /// # Errors
    ///
    /// The command's refusal.
    pub(crate) fn open_message(&mut self, id: u64) -> Result<Value, UiError> {
        let held = self.held(id);
        if self.restored.take() == Some(id) || held {
            return self.run("mail.read", &json!({ "id": id }));
        }
        let answer = self.run("mail.open", &json!({ "id": id }))?;
        if answer.get("marked_seen").and_then(Value::as_bool) == Some(true) {
            self.history = self.history.saturating_add(1);
            self.refresh();
        }
        Ok(answer)
    }

    /// Whether the message `id` is held by the screener: listed in "New senders", or marked as from a new sender. Its
    /// reader asks whether to let the sender in, and reading it is a preview.
    #[must_use]
    pub fn held(&self, id: u64) -> bool {
        self.messages.iter().any(|row| row.id == id && (row.new_sender || self.place == Place::Senders))
    }

    /// The folders the message `id` can be moved to: those of its account that hold mail, but the one it is in.
    #[must_use]
    pub fn move_targets(&self, id: u64) -> Vec<&FolderRow> {
        let Some(row) = self.messages.iter().find(|row| row.id == id) else { return Vec::new() };
        self.folders.iter().filter(|folder| folder.account == row.account && folder.id != row.folder && folder.selectable).collect()
    }

    /// Starts `act` on the message `id`: an act that takes the row out of the list lets it slide out first
    /// ([`Mailbox::leaving`], then [`Mailbox::left`]); the others are done at once. A row still sliding is done first.
    pub fn start(&mut self, id: u64, act: Act) {
        self.left();
        // With "show done" a done message stays where it is, dimmed.
        if act.slides() && !(act == Act::Done && self.show_done) {
            self.leaving = Some((id, act));
        } else {
            self.act(id, act);
        }
    }

    /// The row sliding out of the list and what is done to it once it is out.
    #[must_use]
    pub fn leaving(&self) -> Option<(u64, Act)> {
        self.leaving
    }

    /// The sliding row is out: its act is done.
    pub fn left(&mut self) {
        if let Some((id, act)) = self.leaving.take() {
            self.act(id, act);
        }
    }

    /// The last act while its toast shows.
    #[must_use]
    pub fn undoable(&self) -> Option<&Undo> {
        self.undo.as_ref()
    }

    /// Whether the engine's history holds an act to take back.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history > 0
    }

    /// Takes the toast's act back through `history.undo`, with every act after it (the next message marked read when
    /// it opened), so its message is back where it was, read or unread as it was, and selected again when it was.
    /// Without a toast that can be undone it takes the newest act of the history back: again and again, it walks back
    /// through the session.
    pub fn undo(&mut self) {
        let toast = self.undo.take();
        let floor = match &toast {
            Some(undo) if undo.undoable => undo.depth,
            _ => self.history.saturating_sub(1),
        };
        let before = self.history;
        let mut result = Ok(());
        while self.history > floor {
            match self.run("history.undo", &json!({})) {
                Ok(_) => self.history = self.history.saturating_sub(1),
                // The count was ahead of the engine's (it keeps the newest acts only): it starts again.
                Err(UiError::Engine(EngineError::NothingToUndo)) => self.history = 0,
                Err(error) => {
                    result = Err(error);
                    break;
                }
            }
        }
        if self.history != before {
            self.refresh();
            if let Some(undo) = toast.filter(|undo| undo.undoable && undo.selected && self.history <= undo.depth)
                && self.messages.iter().any(|row| row.id == undo.id)
            {
                self.selected = Some(undo.id);
                self.restored = Some(undo.id);
            }
        }
        if let Err(error) = result {
            self.fail(error);
        }
    }

    /// Runs the event command `command` (`event.create`, `event.update`, `event.delete`), an act of the engine's
    /// history, and shows `text` in its toast with Undo, which takes it back like a message's act.
    ///
    /// # Errors
    ///
    /// The command's refusal; no toast then.
    pub(crate) fn event_act(&mut self, command: &'static str, params: &Value, text: &str) -> Result<Value, UiError> {
        let before = self.history;
        let answer = self.run(command, params)?;
        self.history = self.history.saturating_add(1);
        self.acts = self.acts.saturating_add(1);
        let id = answer.get("id").and_then(Value::as_u64).unwrap_or(0);
        self.undo = Some(Undo { act: Act::Event, id, serial: self.acts, text: text.to_owned(), undoable: true, depth: before, selected: false });
        Ok(answer)
    }

    /// Forgets the toast: its Undo has gone. The act stays in the history, where Ctrl+Z still reaches it.
    pub fn forget_undo(&mut self) {
        self.undo = None;
    }

    /// Hands the engine's queued jobs to `workers`, stops the workers of accounts that are gone, and applies the
    /// events that came back; reads everything again when an event changed something. Applying an event can queue
    /// the next jobs (the folder syncs after the folder list), so it goes round again, at most [`MAX_PUMP_ROUNDS`]
    /// times a frame. First it lets the engine do what is due ([`Session::tick_if_due`]): a send whose hold is over,
    /// a snooze that ends, a draft's copy. Whether anything changed.
    pub fn pump(&mut self, workers: &mut dyn Workers) -> bool {
        let mut changed = match self.session.tick_if_due() {
            Ok(ticked) => ticked.is_some(),
            Err(error) => {
                self.error = Some(UiError::from(error).to_string());
                false
            }
        };
        for _ in 0..MAX_PUMP_ROUNDS {
            if !self.pump_once(workers) {
                break;
            }
            changed = true;
        }
        self.hear();
        if changed {
            self.refresh();
        }
        changed
    }

    /// Takes the engine's notices into the mailbox, for the desktop, and the subjects of the sends that went late, for
    /// the window; at most [`MAX_NOTICES`] of each wait.
    fn hear(&mut self) {
        for notice in self.session.take_notices() {
            if let Notice::SentLate { subject, .. } = &notice {
                self.late.push(shown(subject));
            }
            self.notices.push(notice);
        }
        let over = self.notices.len().saturating_sub(MAX_NOTICES);
        self.notices.drain(..over);
        let over = self.late.len().saturating_sub(MAX_NOTICES);
        self.late.drain(..over);
    }

    /// The subjects of the sends that went late, their time passed while InboxCraft was not running, since the last
    /// call: the window says so once.
    pub fn take_late(&mut self) -> Vec<String> {
        self.hear();
        std::mem::take(&mut self.late)
    }

    /// How long until the engine next has something to do ([`Session::due_at`]): a send whose hold ends, a send later,
    /// a snooze that ends, a reminder, a draft's copy. The window asks for a frame then, since nothing else draws one
    /// while it is idle. [`OVERDUE_WAKE`] for a time that has passed; `None` when nothing waits, or when the store
    /// cannot say (the pump shows that error).
    pub fn due_in(&mut self) -> Option<std::time::Duration> {
        let due = self.session.due_at().ok().flatten()?;
        Some(due.duration_since(self.clock.now()).filter(|left| !left.is_zero()).unwrap_or(OVERDUE_WAKE))
    }

    /// What the engine has to tell the owner since the last call ([`Session::take_notices`]), for the desktop to show.
    pub fn take_notices(&mut self) -> Vec<Notice> {
        self.hear();
        std::mem::take(&mut self.notices)
    }

    fn pump_once(&mut self, workers: &mut dyn Workers) -> bool {
        let jobs = self.session.take_jobs();
        let accounts = match self.session.store().accounts() {
            Ok(accounts) => accounts,
            Err(error) => {
                self.error = Some(UiError::from(EngineError::from(error)).to_string());
                return false;
            }
        };
        for job in jobs {
            // A job for an account that is gone has nobody to run it.
            if let Some(account) = accounts.iter().find(|account| account.id() == job.account) {
                workers.send(account, job);
            }
        }
        workers.keep(&accounts.iter().map(Account::id).collect::<Vec<_>>());
        let mut changed = false;
        for event in workers.events() {
            match self.session.apply(&event) {
                Ok(Applied::Changed) => changed = true,
                Ok(Applied::Dropped) => {}
                Err(error) => self.error = Some(UiError::from(error).to_string()),
            }
        }
        changed
    }

    fn settle(&mut self, result: Result<(), UiError>) {
        self.error = result.err().map(|error| error.to_string());
    }

    /// Shows `error` in the error banner until it is dismissed or the next command settles.
    pub(crate) fn fail(&mut self, error: UiError) {
        self.settle(Err(error));
    }

    /// The owner's appearance choices, as the `ui.*.set` commands left them.
    #[must_use]
    pub fn appearance(&self) -> &Appearance {
        self.session.appearance()
    }

    /// Takes the appearance choices kept from an earlier run.
    pub(crate) fn restore_appearance(&mut self, appearance: Appearance) {
        self.session.set_appearance(appearance);
    }

    pub(crate) fn run(&mut self, command: &'static str, params: &Value) -> Result<Value, UiError> {
        self.runs = self.runs.saturating_add(1);
        self.last = Some(command);
        Ok(self.session.run(command, params)?)
    }

    /// Runs the command `id` with `params` for an agent (the control channel): only a `safe` command runs. The list is
    /// read again. An act of the agent's (an answer that `changed` or `undone` something) ends the window's undo for
    /// the acts before it, so the window's Undo never takes the agent's act back in place of the owner's.
    ///
    /// # Errors
    ///
    /// As [`Session::run_as`]: [`EngineError::NotAllowed`] for an `owner` or `never` command.
    pub fn run_for_agent(&mut self, id: &str, params: &Value) -> Result<Value, EngineError> {
        let answer = self.session.run_as(Caller::Agent, id, params)?;
        if ["changed", "undone"].iter().any(|field| answer.get(field).and_then(Value::as_bool) == Some(true)) {
            self.undo = None;
            self.history = 0;
        }
        self.refresh();
        Ok(answer)
    }

    /// The id of the last command run through [`Mailbox::run`], answered or refused.
    #[must_use]
    pub fn last_run(&self) -> Option<&'static str> {
        self.last
    }

    /// How many commands ran through [`Mailbox::run`] so far.
    #[cfg(test)]
    pub(crate) fn runs(&self) -> u64 {
        self.runs
    }

    fn load_accounts(&mut self) -> Result<(), UiError> {
        let answer = self.run("account.list", &json!({}))?;
        self.accounts = array("account.list", &answer, "accounts")?
            .iter()
            .enumerate()
            .map(|(index, account)| AccountRow {
                id: number(account, "id"),
                label: text(account, "label"),
                address: text(account, "address"),
                name: text(account, "display_name"),
                signature: account.get("signature").and_then(Value::as_str).unwrap_or_default().to_owned(),
                index,
                grouped: account.get("grouped").and_then(Value::as_bool).unwrap_or(false),
            })
            .collect();
        if self.narrowed.is_some_and(|id| self.account(id).is_none()) {
            self.narrowed = None;
        }
        Ok(())
    }

    fn load_folders(&mut self) -> Result<(), UiError> {
        let answer = self.run("folder.list", &json!({ "account": self.narrowed }))?;
        let mut folders: Vec<FolderRow> = array("folder.list", &answer, "folders")?
            .iter()
            .map(|folder| {
                let name = text(folder, "name");
                let delimiter = folder.get("delimiter").and_then(Value::as_str).and_then(|d| d.chars().next());
                let depth = delimiter.map_or(0, |d| name.matches(d).count());
                FolderRow {
                    id: number(folder, "id"),
                    account: number(folder, "account"),
                    leaf: text(folder, "leaf"),
                    name,
                    depth,
                    role: folder.get("role").and_then(Value::as_str).and_then(role),
                    unread: number(folder, "unread"),
                    messages: number(folder, "messages"),
                    selectable: folder.get("selectable").and_then(Value::as_bool) == Some(true),
                }
            })
            .collect();
        folders.sort_by(|a, b| {
            let key = |f: &FolderRow| (f.account, f.role.map_or(ROLES.len(), rank));
            key(a).cmp(&key(b)).then_with(|| a.name.cmp(&b.name))
        });
        let unread: BTreeMap<u64, u64> = folders.iter().map(|folder| (folder.id, folder.unread)).collect();
        self.views = array("folder.list", &answer, "views")?
            .iter()
            .filter_map(|view| {
                let role = view.get("role").and_then(Value::as_str).and_then(role)?;
                let ids = view.get("folders").and_then(Value::as_array)?;
                let unread = ids.iter().filter_map(Value::as_u64).filter_map(|id| unread.get(&id)).fold(0_u64, |sum, n| sum.saturating_add(*n));
                Some(ViewRow { role, unread })
            })
            .collect();
        self.folders = folders;
        Ok(())
    }

    fn load_screener(&mut self) -> Result<(), UiError> {
        let answer = self.run("screener.list", &json!({}))?;
        self.screener = answer.get("mode").and_then(Value::as_str).and_then(ScreenerMode::from_name).unwrap_or(ScreenerMode::Off);
        self.waiting = array("screener.list", &answer, "senders")?
            .iter()
            .map(|sender| {
                let address = text(sender, "address");
                let name = sender.get("name").and_then(Value::as_str).filter(|name| !name.is_empty()).map_or_else(|| address.clone(), shown);
                let messages =
                    sender.get("messages").and_then(Value::as_array).map(|ids| ids.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
                Waiting { name, address, messages }
            })
            .collect();
        Ok(())
    }

    /// Reads the outbox (`outbox.list`).
    fn load_outbox(&mut self) -> Result<(), UiError> {
        let answer = self.run("outbox.list", &json!({}))?;
        self.outbox = array("outbox.list", &answer, "items")?.iter().map(outbox_row).collect();
        Ok(())
    }

    fn load_list(&mut self, place: Place) -> Result<(), UiError> {
        // The outbox shows while anything is in it, and gives way to the inbox after.
        let place = if place == Place::Outbox && self.outbox.is_empty() { Place::View(Role::Inbox) } else { place };
        let rows = if self.query.is_empty() { self.list(place)? } else { self.found()? };
        self.place = place;
        self.messages = rows;
        if self.selected.is_some_and(|id| self.messages.iter().all(|row| row.id != id)) {
            self.selected = None;
        }
        Ok(())
    }

    fn list(&mut self, place: Place) -> Result<Vec<MessageRow>, UiError> {
        let mut params = json!({ "account": self.narrowed, "limit": MAX_LIST });
        if let Value::Object(fields) = &mut params {
            match place {
                Place::View(role) => fields.insert("role".to_owned(), Value::String(role.label().to_ascii_lowercase())),
                Place::Folder(id) => fields.insert("folder".to_owned(), json!(id)),
                Place::Senders => return self.senders(),
                Place::Later(pile) => return self.later(pile),
                // The outbox lists sends, not messages.
                Place::Outbox => return Ok(Vec::new()),
            };
            if self.show_done {
                fields.insert("done".to_owned(), Value::Bool(true));
            }
        }
        let answer = self.run("mail.list", &params)?;
        self.parked = number(&answer, "parked");
        Ok(array("mail.list", &answer, "messages")?.iter().map(message_row).collect())
    }

    /// The rows of `pile`, and how many each pile holds.
    fn later(&mut self, pile: Pile) -> Result<Vec<MessageRow>, UiError> {
        let mut shown = Vec::new();
        for each in Pile::ALL {
            let (command, params) = match each {
                Pile::Snoozed => ("mail.list", json!({ "account": self.narrowed, "role": "snoozed", "limit": MAX_LIST })),
                Pile::Pins => ("pin.list", json!({ "account": self.narrowed, "limit": MAX_LIST })),
                Pile::Parked => ("park.list", json!({ "account": self.narrowed, "limit": MAX_LIST })),
            };
            let answer = self.run(command, &params)?;
            if let Some(count) = self.piles.get_mut(each.index()) {
                *count = number(&answer, "total");
            }
            if each == pile {
                shown = array(command, &answer, "messages")?.iter().map(message_row).collect();
            }
        }
        if pile == Pile::Snoozed {
            // The one coming back first on top.
            shown.sort_by_key(|row: &MessageRow| (row.snoozed_until.is_none(), row.snoozed_until));
        }
        self.parked = 0;
        Ok(shown)
    }

    /// The mail held from new senders: the screener folders', and the inbox rows marked as from a new sender; newest
    /// first.
    fn senders(&mut self) -> Result<Vec<MessageRow>, UiError> {
        let mut rows = Vec::new();
        for role in ["screener", "inbox"] {
            let answer = self.run("mail.list", &json!({ "account": self.narrowed, "role": role, "limit": MAX_LIST }))?;
            rows.extend(array("mail.list", &answer, "messages")?.iter().map(message_row).filter(|row| role == "screener" || row.new_sender));
        }
        rows.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.id.cmp(&a.id)));
        Ok(rows)
    }

    /// The search: every message whose subject, sender or recipient holds the query, newest first.
    fn found(&mut self) -> Result<Vec<MessageRow>, UiError> {
        let mut rows: BTreeMap<u64, MessageRow> = BTreeMap::new();
        for field in ["subject", "from", "to"] {
            let mut params = json!({ "account": self.narrowed, "limit": MAX_LIST });
            if let Value::Object(fields) = &mut params {
                fields.insert(field.to_owned(), Value::String(self.query.clone()));
            }
            let answer = self.run("mail.search", &params)?;
            for row in array("mail.search", &answer, "messages")?.iter().map(message_row) {
                rows.insert(row.id, row);
            }
        }
        let mut rows: Vec<MessageRow> = rows.into_values().collect();
        rows.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.id.cmp(&a.id)));
        Ok(rows)
    }

    /// Shows one account (`Some`) or all of them (`None`). An account that is not there narrows nothing; an open
    /// folder of another account gives way to the inbox.
    pub fn narrow(&mut self, account: Option<u64>) {
        let account = account.filter(|id| self.account(*id).is_some());
        self.narrowed = account;
        let place = match self.place {
            Place::Folder(id) if account.is_some() && self.folders.iter().any(|f| f.id == id && Some(f.account) != account) => {
                Place::View(Role::Inbox)
            }
            place => place,
        };
        let result = self.load_folders().and_then(|()| self.load_list(place));
        self.settle(result);
    }

    /// Opens a unified view or a folder, which ends a search.
    pub fn open(&mut self, place: Place) {
        let query = std::mem::take(&mut self.query);
        let result = self.load_list(place);
        if result.is_err() {
            self.query = query;
        }
        self.settle(result);
    }

    /// Searches the mail of the accounts shown; an empty query goes back to the open place.
    pub fn search(&mut self, query: &str) {
        self.query = query.trim().chars().take(MAX_QUERY).collect();
        let result = self.load_list(self.place);
        self.settle(result);
    }

    /// Selects a row of the list, or none. A message not in the list is not selected.
    pub fn select(&mut self, message: Option<u64>) {
        self.keyed = false;
        match message {
            Some(id) if self.messages.iter().any(|row| row.id == id) => self.selected = Some(id),
            Some(_) => {}
            None => self.selected = None,
        }
    }

    /// Hides the error until the next one.
    pub fn dismiss_error(&mut self) {
        self.error = None;
    }

    /// The accounts.
    #[must_use]
    pub fn accounts(&self) -> &[AccountRow] {
        &self.accounts
    }

    /// The account `id`.
    #[must_use]
    pub fn account(&self, id: u64) -> Option<&AccountRow> {
        self.accounts.iter().find(|account| account.id == id)
    }

    /// The unified views of the accounts shown.
    #[must_use]
    pub fn views(&self) -> &[ViewRow] {
        &self.views
    }

    /// The folders of `account`: those with a role first, then by path.
    #[must_use]
    pub fn folders_of(&self, account: u64) -> Vec<&FolderRow> {
        self.folders.iter().filter(|folder| folder.account == account).collect()
    }

    /// The account shown alone, if one is.
    #[must_use]
    pub fn narrowed(&self) -> Option<u64> {
        self.narrowed
    }

    /// What the list shows when there is no search.
    #[must_use]
    pub fn place(&self) -> Place {
        self.place
    }

    /// The search, empty when there is none.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The rows of the list.
    #[must_use]
    pub fn messages(&self) -> &[MessageRow] {
        &self.messages
    }

    /// The selected row.
    #[must_use]
    pub fn selected(&self) -> Option<u64> {
        self.selected
    }

    /// The last error, until the next command succeeds or it is dismissed.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The name of what the list shows.
    #[must_use]
    pub fn title(&self) -> String {
        if !self.query.is_empty() {
            return tr!("Search").to_owned();
        }
        match self.place {
            Place::View(role) => role_text(role).to_owned(),
            Place::Folder(id) => self.folders.iter().find(|folder| folder.id == id).map(|folder| folder.leaf.clone()).unwrap_or_default(),
            Place::Senders => tr!("New senders").to_owned(),
            Place::Later(_) => tr!("Later").to_owned(),
            Place::Outbox => tr!("Outbox").to_owned(),
        }
    }

    /// Whether "show done" lists the done mail in its old place.
    #[must_use]
    pub fn show_done(&self) -> bool {
        self.show_done
    }

    /// Turns "show done" on or off and reads the list again.
    pub fn set_show_done(&mut self, show: bool) {
        self.show_done = show;
        let result = self.load_list(self.place);
        self.settle(result);
    }

    /// How many messages of the place shown wait on the park pile.
    #[must_use]
    pub fn parked(&self) -> u64 {
        self.parked
    }

    /// How many messages `pile` held when "Later" was last read.
    #[must_use]
    pub fn pile_count(&self, pile: Pile) -> u64 {
        self.piles.get(pile.index()).copied().unwrap_or(0)
    }

    /// The time presets still ahead, by the name the picker shows, as last read by [`Mailbox::load_presets`].
    #[must_use]
    pub fn presets(&self) -> &[(String, Timestamp)] {
        &self.presets
    }

    /// Reads the time presets still ahead (`schedule.presets`), when a time picker opens.
    pub fn load_presets(&mut self) {
        match self.run("schedule.presets", &json!({})) {
            Ok(answer) => {
                self.presets = answer
                    .get("presets")
                    .and_then(Value::as_array)
                    .map(|presets| {
                        presets
                            .iter()
                            .filter_map(|preset| {
                                let name = preset_text(preset.get("preset").and_then(Value::as_str)?)?;
                                Some((name.to_owned(), parse_date(preset.get("at").and_then(Value::as_str)?)?))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
            }
            Err(error) => self.fail(error),
        }
    }

    /// The time now, on the list's clock.
    #[must_use]
    pub fn now(&self) -> Timestamp {
        self.clock.now()
    }

    /// The wall clock's offset the list shows times in.
    #[must_use]
    pub fn offset(&self) -> Offset {
        self.offset
    }

    /// `at` in full, as the time picker reads a time out: "Saturday 7 November, 09:00".
    fn long_date(&self, at: Timestamp) -> String {
        crate::component::typed_time::long(at, self.clock.now(), self.offset)
    }

    /// The date a row shows: when it comes back in the snoozed pile, else when it came.
    #[must_use]
    pub fn row_date(&self, row: &MessageRow) -> String {
        match (self.place, row.snoozed_until) {
            (Place::Later(Pile::Snoozed), Some(until)) if self.query.is_empty() => {
                crate::component::typed_time::short(until, self.clock.now(), self.offset)
            }
            _ => self.when(row),
        }
    }

    /// Whether the message `id` is one the owner sent: it is in a folder for sent mail. A follow-up reminder goes on
    /// such a message only.
    #[must_use]
    pub fn is_sent(&self, id: u64) -> bool {
        let Some(row) = self.messages.iter().find(|row| row.id == id) else { return false };
        self.folders.iter().any(|folder| folder.id == row.folder && folder.role == Some(Role::Sent))
    }

    /// Whether the message `id` is snoozed: it has a time to come back, or it is in a snoozed folder.
    #[must_use]
    pub fn is_snoozed(&self, id: u64) -> bool {
        let Some(row) = self.messages.iter().find(|row| row.id == id) else { return false };
        row.snoozed_until.is_some() || self.folders.iter().any(|folder| folder.id == row.folder && folder.role == Some(Role::Snoozed))
    }

    /// Whether `row` shows the returned mark: it came back from a snooze and is still unread.
    #[must_use]
    pub fn returned(&self, row: &MessageRow) -> bool {
        row.returned.is_some() && !row.seen
    }

    /// Whether a pin stays on a message when it is done.
    #[must_use]
    pub fn pins_outlast_done(&self) -> bool {
        self.session.pins_outlast_done()
    }

    /// Sets whether a pin stays on a message when it is done (`pin.set`).
    pub fn set_pins_outlast_done(&mut self, keep: bool) {
        if let Err(error) = self.run("pin.set", &json!({ "keep_on_done": keep })) {
            self.fail(error);
        }
    }

    /// Takes the pin choice kept from an earlier run.
    pub(crate) fn restore_pins(&mut self, keep: bool) {
        self.session.set_pins_outlast_done(keep);
    }

    /// Whose mail the list shows: the narrowed account's address, or all accounts.
    #[must_use]
    pub fn subtitle(&self) -> String {
        match self.narrowed.and_then(|id| self.account(id)) {
            Some(account) => account.address.clone(),
            None => tr!("All accounts").to_owned(),
        }
    }

    /// The row's date as the list shows it.
    #[must_use]
    pub fn when(&self, row: &MessageRow) -> String {
        row.date.map(|date| when(date, self.clock.now(), self.offset)).unwrap_or_default()
    }

    /// `date` in full, as the expanded header shows it: `Thursday 5 Nov 2026, 08:35`.
    #[must_use]
    pub fn full_date(&self, date: Timestamp) -> String {
        let Some(civil) = date.to_civil(self.offset) else { return String::new() };
        tr!(
            "{weekday} {day} {month} {year}, {time}",
            weekday = weekday_text(civil.weekday()),
            day = civil.day(),
            month = month_text(civil.month()),
            year = civil.year(),
            time = format!("{:02}:{:02}", civil.hour(), civil.minute())
        )
    }

    /// `date` as the list shows it.
    #[must_use]
    pub fn short_date(&self, date: Timestamp) -> String {
        when(date, self.clock.now(), self.offset)
    }

    /// The row as a screen reader says it: sender, subject, the date it shows, and whether it is unread, flagged, from a
    /// new sender, pinned, done or back from a snooze.
    #[must_use]
    pub fn spoken(&self, row: &MessageRow) -> String {
        let subject = if row.subject.is_empty() { tr!("(no subject)") } else { row.subject.as_str() };
        let mut parts = vec![row.sender.clone(), subject.to_owned()];
        let date = self.row_date(row);
        if !date.is_empty() {
            parts.push(date);
        }
        if !row.seen {
            parts.push(tr!("unread").to_owned());
        }
        if row.flagged {
            parts.push(tr!("flagged").to_owned());
        }
        if row.new_sender {
            parts.push(tr!("new sender").to_owned());
        }
        if row.pinned {
            parts.push(tr!("pinned").to_owned());
        }
        if row.done {
            parts.push(tr!("done").to_owned());
        }
        if self.returned(row) {
            parts.push(tr!("returned").to_owned());
        }
        if row.bounced.is_some() {
            parts.push(tr!("not delivered").to_owned());
        }
        parts.join(", ")
    }

    /// The outbox: each send waiting, on its way or refused.
    #[must_use]
    pub fn outbox(&self) -> &[OutboxRow] {
        &self.outbox
    }

    /// What the outbox's header says under its title: how many wait, and how many were not sent; `None` when it is
    /// empty.
    #[must_use]
    pub fn outbox_summary(&self) -> Option<String> {
        let refused = self.outbox.iter().filter(|row| matches!(row.state, SendState::Refused { .. })).count();
        let waiting = self.outbox.len().saturating_sub(refused);
        match (waiting, refused) {
            (0, 0) => None,
            (waiting, 0) => Some(tr!("{count} waiting", count = waiting)),
            (0, refused) => Some(tr!("{count} not sent", count = refused)),
            (waiting, refused) => Some(tr!("{waiting} waiting · {refused} not sent", waiting = waiting, refused = refused)),
        }
    }

    /// The whole seconds left before the send of `draft` goes, a second begun counting as one, while it waits for a
    /// hold of at most [`Hold::MAX`] seconds; `None` for a send later, a send no longer waiting, or a time behind.
    #[must_use]
    pub fn seconds_left(&self, draft: u64) -> Option<u64> {
        let row = self.outbox.iter().find(|row| row.draft == draft)?;
        let SendState::Waiting(Some(at)) = row.state else { return None };
        let left = at.duration_since(self.clock.now()).filter(|left| !left.is_zero())?;
        let seconds = left.as_secs().saturating_add(u64::from(left.subsec_nanos() > 0));
        (seconds <= u64::from(Hold::MAX)).then_some(seconds)
    }

    /// Where the send `row` stands, as its row says it: "Sending in 4 s", "Scheduled · Sat 08:00", "Sending…" or
    /// "Not sent".
    #[must_use]
    pub fn standing(&self, row: &OutboxRow) -> String {
        match &row.state {
            SendState::Waiting(at) => match (self.seconds_left(row.draft), at) {
                (Some(seconds), _) => tr!("Sending in {seconds} s", seconds = seconds),
                (None, Some(at)) if *at > self.clock.now() => {
                    tr!("Scheduled · {when}", when = crate::component::typed_time::short(*at, self.clock.now(), self.offset))
                }
                _ => tr!("Sending…").to_owned(),
            },
            SendState::Sending => tr!("Sending…").to_owned(),
            SendState::Refused { .. } => tr!("Not sent").to_owned(),
        }
    }

    /// The send `row` as a screen reader says it: whom it goes to, the subject, and where it stands, with the server's
    /// reply when it was refused.
    #[must_use]
    pub fn spoken_outbox(&self, row: &OutboxRow) -> String {
        let subject = if row.subject.is_empty() { tr!("(no subject)") } else { row.subject.as_str() };
        let standing = match &row.state {
            SendState::Refused { reply, .. } => tr!("Not sent: {reply}", reply = reply),
            _ => self.standing(row),
        };
        tr!("To {whom}, {subject}, {standing}", whom = self.recipients(row), subject = subject, standing = standing)
    }

    /// Whom the send `row` goes to, as its row says it.
    #[must_use]
    pub fn recipients(&self, row: &OutboxRow) -> String {
        if row.to.is_empty() { tr!("(no recipient)").to_owned() } else { row.to.join(", ") }
    }

    /// Takes the waiting send of `draft` back (`mail.unsend`): out of the outbox, its draft as it was, to be opened
    /// again. The lists are read again.
    ///
    /// # Errors
    ///
    /// The command's refusal: the send went, or there is none.
    pub fn unsend(&mut self, draft: u64) -> Result<(), UiError> {
        let result = self.run("mail.unsend", &json!({ "id": draft })).map(|_| ());
        self.refresh();
        result
    }

    /// Sends the refused draft `draft` again (`outbox.retry`), held as every send is; the time it goes. The lists are
    /// read again.
    ///
    /// # Errors
    ///
    /// The command's refusal: the draft cannot be sent as it is, or it was not refused.
    pub fn retry(&mut self, draft: u64) -> Result<Option<Timestamp>, UiError> {
        let answer = self.run("outbox.retry", &json!({ "id": draft }));
        self.refresh();
        Ok(answer?.get("at").and_then(Value::as_str).and_then(parse_date))
    }

    /// How long every send is held before it goes, in seconds.
    #[must_use]
    pub fn hold_seconds(&self) -> u8 {
        self.session.send_hold().seconds()
    }

    /// Holds every send `seconds` from now on (`send.set`); a value out of range is refused, in the error banner.
    pub fn set_hold(&mut self, seconds: u8) {
        if let Err(error) = self.run("send.set", &json!({ "hold_seconds": seconds })) {
            self.fail(error);
        }
    }

    /// Takes the hold kept from an earlier run; one out of range is left out.
    pub(crate) fn restore_hold(&mut self, seconds: u8) {
        if let Some(hold) = Hold::new(u64::from(seconds)) {
            self.session.set_send_hold(hold);
        }
    }

    /// What tells the owner: each account's level, returned snoozes and follow-up reminders.
    #[must_use]
    pub fn notify_settings(&self) -> &NotifySettings {
        self.session.notify_settings()
    }

    /// Which new mail of `account` notifies the owner.
    #[must_use]
    pub fn notify_level(&self, account: u64) -> Level {
        self.session.notify_settings().level(AccountId::new(account))
    }

    /// Notifies the owner of `level` of the new mail of `account` (`notify.set`); an account that is gone is refused,
    /// in the error banner.
    pub fn set_notify_level(&mut self, account: u64, level: Level) {
        self.notify_set(&json!({ "account": account, "level": level.name() }));
    }

    /// Whether a snoozed message that comes back notifies the owner (`notify.set`).
    pub fn set_notify_returns(&mut self, on: bool) {
        self.notify_set(&json!({ "returns": on }));
    }

    /// Whether a follow-up reminder that fires notifies the owner (`notify.set`).
    pub fn set_notify_reminders(&mut self, on: bool) {
        self.notify_set(&json!({ "reminders": on }));
    }

    fn notify_set(&mut self, params: &Value) {
        if let Err(error) = self.run("notify.set", params) {
            self.fail(error);
        }
    }

    /// Takes the notification choices kept from an earlier run.
    pub(crate) fn restore_notify(&mut self, settings: NotifySettings) {
        self.session.set_notify_settings(settings);
    }
}

#[cfg(test)]
#[path = "later_tests.rs"]
mod later_tests;
#[cfg(test)]
#[path = "notify_tests.rs"]
mod notify_tests;
#[cfg(test)]
#[path = "outbox_tests.rs"]
mod outbox_tests;
#[cfg(test)]
#[path = "mailbox_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "triage_tests.rs"]
mod triage_tests;
