// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An IMAP client core without I/O (RFC 9051 and RFC 3501): the [`Client`] state machine and what it reads.
//!
//! The commands of the read path: CAPABILITY, STARTTLS, LOGIN and AUTHENTICATE PLAIN (encrypted connections only),
//! LIST with SPECIAL-USE, SELECT and EXAMINE, UID FETCH, UID SEARCH and LOGOUT; and to keep a mailbox in step, ENABLE
//! (RFC 5161), CONDSTORE and QRESYNC (RFC 7162), IDLE (RFC 2177) and NOTIFY (RFC 5465).

mod capability;
mod client;
mod command;
mod fetch;
mod frame;
mod list;
mod mailbox;
mod notify;
mod response;
mod sasl;
mod search;
mod sync;
mod text;
mod uidplus;
mod utf7;
mod value;

pub use capability::{Capabilities, MAX_CAPABILITIES, MAX_CAPABILITY_LEN};
pub use client::{Answer, Client, Event, Moved, Refusal, RefusalKind, Removal, State, Tag, Transport};
pub use fetch::{FetchAttributes, FetchItem, Section};
pub use frame::{MAX_LINE, MAX_LITERAL, MAX_LITERALS};
pub use list::{ListEntry, MAX_MAILBOX_NAME, MAX_MAILBOXES};
pub use mailbox::{MAX_FLAG_LEN, MAX_FLAGS, MailboxStatus};
pub use notify::{MAX_NOTIFY_MAILBOXES, MailboxCounts, Notify, NotifyMailboxes};
pub use sasl::MAX_CREDENTIALS;
pub use search::{MAX_SEARCH_DEPTH, MAX_SEARCH_KEYS, MAX_SET_RANGES, SearchDate, SearchKey};
pub use sync::{Extension, SelectOptions};
pub use text::MAX_TEXT_CHARS;
pub use uidplus::CopyUid;
pub use utf7::{decode_utf7, encode_utf7};
pub use value::MAX_DEPTH;
