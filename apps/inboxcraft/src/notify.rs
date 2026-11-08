// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Notifications on the desktop: the engine decides what to tell the owner (`inboxcraft_engine::notify`), and this
//! shows each notice the way the desktop does it.
//!
//! [`shown`] turns a [`Notice`] into the text and buttons of one notification, in English through `tr!`; the name
//! and subject in it are the sender's data and go out as plain text. On Linux ([`bus`]) the Flatpak goes through the
//! notification portal and everything else through `org.freedesktop.Notifications`, both over the session bus. A
//! click on the notification opens the message in the full-width reader, and its buttons (where the desktop shows
//! them) mark it done or read: each is a [`Click`] for the window to answer. Other systems show no notifications yet.

use inboxcraft_engine::notify::{About, Notice};
use inboxcraft_ui_egui::tr;

/// What the owner clicked in a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    /// Open the message (by id) in the full-width reader.
    Open(u64),
    /// Mark the message done.
    Done(u64),
    /// Mark the message read.
    Read(u64),
    /// Bring the window forward.
    Show,
}

/// A button of a notification, besides the click on the notification itself, which opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// Mark the message done.
    Done,
    /// Mark the message read.
    Read,
}

impl Button {
    /// The action's name on the bus.
    #[must_use]
    pub const fn action(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Read => "read",
        }
    }

    /// The button's text.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Done => tr!("Done").to_owned(),
            Self::Read => tr!("Mark read").to_owned(),
        }
    }
}

/// The action of a click on the notification itself.
pub const OPEN: &str = "open";

/// One notification as the desktop shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// What tells it apart: a later notification with the same key takes its place.
    pub key: String,
    /// The title.
    pub title: String,
    /// The text below it; plain text.
    pub body: String,
    /// Its buttons.
    pub buttons: Vec<Button>,
    /// Whether it is about new mail (the desktop's `email.arrived` category).
    pub mail: bool,
}

/// The notification of `notice`; `account` names the account of a count.
#[must_use]
pub fn shown(notice: &Notice, account: Option<&str>) -> Shown {
    let line = |about: &About| {
        let subject = if about.subject.is_empty() { tr!("(no subject)").to_owned() } else { about.subject.clone() };
        if about.who.is_empty() { subject } else { format!("{}: {subject}", about.who) }
    };
    match notice {
        Notice::Mail(about) => Shown {
            key: format!("mail-{}", about.message.get()),
            title: if about.who.is_empty() { tr!("New mail").to_owned() } else { about.who.clone() },
            body: if about.subject.is_empty() { tr!("(no subject)").to_owned() } else { about.subject.clone() },
            buttons: vec![Button::Done, Button::Read],
            mail: true,
        },
        Notice::Mails { account: id, count } => Shown {
            key: format!("mails-{}", id.get()),
            title: tr!("{count} new messages", count = count),
            body: account.unwrap_or_default().to_owned(),
            buttons: Vec::new(),
            mail: true,
        },
        Notice::Returned { about, missed } => Shown {
            key: format!("returned-{}", about.message.get()),
            title: if *missed { tr!("Back from snooze while InboxCraft was off").to_owned() } else { tr!("Back from snooze").to_owned() },
            body: line(about),
            buttons: vec![Button::Done],
            mail: false,
        },
        Notice::Reminder { about, missed } => Shown {
            key: format!("reminder-{}", about.message.get()),
            title: if *missed { tr!("No reply yet, reminded late").to_owned() } else { tr!("No reply yet").to_owned() },
            body: line(about),
            buttons: Vec::new(),
            mail: false,
        },
        Notice::SentLate { draft, subject, .. } => Shown {
            key: format!("sent-{}", draft.get()),
            title: tr!("Sent late: InboxCraft was not running at its time").to_owned(),
            body: if subject.is_empty() { tr!("(no subject)").to_owned() } else { subject.clone() },
            buttons: Vec::new(),
            mail: false,
        },
    }
}

/// The click that `action` on the notification `key` stands for; `None` for anything else. Both come over the bus,
/// so both are read as text from outside: the key counts only written exactly as [`shown`] writes it.
#[must_use]
pub fn clicked(key: &str, action: &str) -> Option<Click> {
    let (kind, id) = key.split_once('-')?;
    let id = id.parse::<u64>().ok().filter(|id| *id > 0 && format!("{kind}-{id}") == key)?;
    let open = action == OPEN || action == "default";
    match (kind, action) {
        ("mails", _) if open => Some(Click::Show),
        ("mail" | "returned" | "reminder", _) if open => Some(Click::Open(id)),
        ("mail" | "returned", "done") => Some(Click::Done(id)),
        ("mail", "read") => Some(Click::Read(id)),
        _ => None,
    }
}

/// `text` for a desktop that reads the body as markup: `&`, `<` and `>` as entities.
#[must_use]
pub fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// Where notifications go.
pub trait Deliver: Send {
    /// Shows `shown`.
    ///
    /// # Errors
    ///
    /// The desktop did not take it.
    fn deliver(&mut self, shown: &Shown) -> Result<(), String>;
}

#[cfg(target_os = "linux")]
pub mod bus;

#[cfg(test)]
#[path = "notify_tests.rs"]
mod tests;
