// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the mail list shows, read from the engine's commands: the accounts, the unified views, the folders, the
//! list of the open place or of the search, and the selected row.
//!
//! Everything goes through [`Session::run`] by command id (`account.list`, `folder.list`, `mail.list`,
//! `mail.search`), as the menus, the CLI and MCP do. A command that fails leaves the list as it was and shows its
//! error until the next command succeeds or the person dismisses it. On the sample mailbox the hover actions run
//! `sample.mail.set` ([`Mailbox::act`]) and the last one can be undone.

use std::collections::BTreeMap;

use inboxcraft_engine::appearance::Appearance;
use inboxcraft_engine::{Applied, EngineError, MAX_LIST, Session};
use inboxcraft_model::{Account, Role};
use inboxcraft_time::{CivilDateTime, Clock, Offset, Timestamp, Weekday};
use serde_json::{Value, json};

use crate::iconset::IconId;
use crate::workers::Workers;
use crate::{UiError, tr};

/// The most rounds of jobs out and events in [`Mailbox::pump`] makes in one frame.
pub const MAX_PUMP_ROUNDS: usize = 8;

/// The longest search, in characters; the rest is cut off.
pub const MAX_QUERY: usize = 256;

/// The most characters of a name, subject or preview the list keeps.
pub const MAX_SHOWN: usize = 300;

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

/// What the list shows: a unified view by role, or one folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// The folders with this role across the accounts shown.
    View(Role),
    /// One folder.
    Folder(u64),
}

/// What is selected after the open message leaves the list (a sync removed it; later also archive and delete).
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
    /// Its place in the account list, which picks its colour.
    pub index: usize,
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
}

/// A row of the message list. Sender, subject and preview are the sender's text: data, shown and never run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRow {
    /// Its id.
    pub id: u64,
    /// Its account.
    pub account: u64,
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
}

/// A sender the screener holds back until the owner lets them in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiting {
    /// Their name.
    pub name: String,
    /// Their address, which picks the avatar's colour.
    pub address: String,
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

/// The parts of the list whose data does not exist yet (the screener and the groups come with E3.1 and E3.2), filled
/// with invented data by `ui_shot` only. In the app it stays empty, so none of them is drawn.
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

/// What a hover action or its key does to a message. Only the sample mailbox takes them (`sample.mail.set`), until
/// the commands that act on a real mailbox exist (E2.1, E3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// Mark it read.
    Read,
    /// Mark it unread.
    Unread,
    /// Mark it read and move it to its account's Archive folder: it leaves the inbox.
    Done,
    /// Move it to its account's Trash folder.
    Delete,
}

impl Act {
    /// Whether the row slides out of the list first: the message leaves the list.
    #[must_use]
    pub fn slides(self) -> bool {
        matches!(self, Self::Done | Self::Delete)
    }

    /// What the toast says after it.
    #[must_use]
    pub fn told(self) -> &'static str {
        match self {
            Self::Read => tr!("Marked as read"),
            Self::Unread => tr!("Marked as unread"),
            Self::Done => tr!("Marked as done"),
            Self::Delete => tr!("Moved to Trash"),
        }
    }
}

/// The last act, until it is undone or forgotten: where its message was and whether it was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Undo {
    /// What was done.
    pub act: Act,
    /// To which message.
    pub id: u64,
    /// The how-manyth act of the session, so a toast knows a new act from the one it shows.
    pub serial: u64,
    folder: u64,
    seen: bool,
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
    /// Whether the session is over the sample mailbox, where the hover actions act.
    sample: bool,
    /// The message sliding out of the list, and what is done to it once it is out.
    leaving: Option<(u64, Act)>,
    undo: Option<Undo>,
    acts: u64,
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
        sender: sender(value.get("from").unwrap_or(&Value::Null)),
        address: value.get("from").and_then(Value::as_array).and_then(|from| from.first()).map(|first| text(first, "address")).unwrap_or_default(),
        subject: text(value, "subject"),
        preview: text(value, "preview"),
        date: value.get("date").and_then(Value::as_str).and_then(parse_date),
        seen: value.get("seen").and_then(Value::as_bool).unwrap_or(false),
        flagged: value.get("flagged").and_then(Value::as_bool).unwrap_or(false),
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
        let sample = session.is_sample();
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
            sample,
            leaving: None,
            undo: None,
            acts: 0,
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
        let result = self.load_accounts().and_then(|()| self.load_folders()).and_then(|()| self.load_list(self.place));
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

    /// Whether the rows show the hover actions and the action keys work: on the sample mailbox only, until the
    /// commands that act on a real mailbox exist.
    #[must_use]
    pub fn can_act(&self) -> bool {
        self.sample
    }

    /// Does `act` to the message `id` of the sample mailbox through `sample.mail.set`, at once; it can be undone
    /// ([`Mailbox::undo`]) until the next act. Done and Delete move it to its account's Archive or Trash folder, so
    /// it leaves the list and the next-message rule picks what is selected. Nothing outside the sample.
    pub fn act(&mut self, id: u64, act: Act) {
        if !self.sample {
            return;
        }
        let result = self.acted(id, act);
        if result.is_ok() {
            self.refresh();
        }
        self.settle(result);
    }

    fn acted(&mut self, id: u64, act: Act) -> Result<(), UiError> {
        let account = self.messages.iter().find(|row| row.id == id).map(|row| row.account);
        let folder = |role: Role| {
            let found = self.folders.iter().find(|folder| Some(folder.account) == account && folder.role == Some(role)).map(|folder| folder.id);
            found.ok_or_else(|| shape("sample.mail.set", &format!("no {} folder", role.label())))
        };
        let params = match act {
            Act::Read => json!({ "id": id, "seen": true }),
            Act::Unread => json!({ "id": id, "seen": false }),
            Act::Done => json!({ "id": id, "seen": true, "folder": folder(Role::Archive)? }),
            Act::Delete => json!({ "id": id, "folder": folder(Role::Trash)? }),
        };
        let selected = self.selected == Some(id);
        let answer = self.run("sample.mail.set", &params)?;
        let was = answer.get("was");
        let (Some(folder), Some(seen)) =
            (was.and_then(|was| was.get("folder")).and_then(Value::as_u64), was.and_then(|was| was.get("seen")).and_then(Value::as_bool))
        else {
            return Err(shape("sample.mail.set", "no folder or read state it was in"));
        };
        self.acts = self.acts.saturating_add(1);
        self.undo = Some(Undo { act, id, serial: self.acts, folder, seen, selected });
        Ok(())
    }

    /// Starts `act` on the message `id`: an act that takes the row out of the list lets it slide out first
    /// ([`Mailbox::leaving`], then [`Mailbox::left`]); the others are done at once. A row still sliding is done first.
    pub fn start(&mut self, id: u64, act: Act) {
        if !self.sample {
            return;
        }
        self.left();
        if act.slides() {
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

    /// The last act while it can be undone.
    #[must_use]
    pub fn undoable(&self) -> Option<Undo> {
        self.undo
    }

    /// Undoes the last act: its message goes back to its folder with its read state, selected again when it was.
    pub fn undo(&mut self) {
        let Some(undo) = self.undo.take() else { return };
        let result = self.run("sample.mail.set", &json!({ "id": undo.id, "seen": undo.seen, "folder": undo.folder })).map(|_| ());
        if result.is_ok() {
            self.refresh();
            if undo.selected && self.messages.iter().any(|row| row.id == undo.id) {
                self.selected = Some(undo.id);
            }
        }
        self.settle(result);
    }

    /// Forgets the last act: it can no longer be undone (its toast has gone).
    pub fn forget_undo(&mut self) {
        self.undo = None;
    }

    /// Hands the engine's queued jobs to `workers`, stops the workers of accounts that are gone, and applies the
    /// events that came back; reads everything again when an event changed something. Applying an event can queue
    /// the next jobs (the folder syncs after the folder list), so it goes round again, at most [`MAX_PUMP_ROUNDS`]
    /// times a frame. Whether anything changed.
    pub fn pump(&mut self, workers: &mut dyn Workers) -> bool {
        let mut changed = false;
        for _ in 0..MAX_PUMP_ROUNDS {
            if !self.pump_once(workers) {
                break;
            }
            changed = true;
        }
        if changed {
            self.refresh();
        }
        changed
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
            .map(|(index, account)| AccountRow { id: number(account, "id"), label: text(account, "label"), address: text(account, "address"), index })
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

    fn load_list(&mut self, place: Place) -> Result<(), UiError> {
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
            };
        }
        let answer = self.run("mail.list", &params)?;
        Ok(array("mail.list", &answer, "messages")?.iter().map(message_row).collect())
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
        }
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

    /// The row as a screen reader says it: sender, subject, date, and whether it is unread or flagged.
    #[must_use]
    pub fn spoken(&self, row: &MessageRow) -> String {
        let subject = if row.subject.is_empty() { tr!("(no subject)") } else { row.subject.as_str() };
        let mut parts = vec![row.sender.clone(), subject.to_owned()];
        let date = self.when(row);
        if !date.is_empty() {
            parts.push(date);
        }
        if !row.seen {
            parts.push(tr!("unread").to_owned());
        }
        if row.flagged {
            parts.push(tr!("flagged").to_owned());
        }
        parts.join(", ")
    }
}

#[cfg(test)]
#[path = "mailbox_tests.rs"]
mod tests;
