// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Our own ids, and the numbers an IMAP server gives a message.

use std::fmt;
use std::num::{NonZeroU32, NonZeroU64};

use serde::{Deserialize, Serialize};

/// One of our own ids: a number the local store hands out, never one a server chose.
macro_rules! own_id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        ///
        #[doc = concat!("Shown with a one-letter prefix in logs (`", $prefix, "7`); serialised as the bare number.")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(u64);

        impl $name {
            /// The id with the number `value`.
            #[must_use]
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            /// The number.
            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($prefix, "{}"), self.0)
            }
        }
    };
}

own_id!(
    /// An account.
    AccountId,
    "a"
);
own_id!(
    /// A folder of one account.
    FolderId,
    "f"
);
own_id!(
    /// A message: one copy in one folder.
    MessageId,
    "m"
);
own_id!(
    /// A thread of messages, across folders and accounts.
    ThreadId,
    "t"
);
own_id!(
    /// A job the engine gave a `net` worker; the events it causes carry it back.
    JobId,
    "j"
);

/// A number the server gives, never zero (RFC 9051 `nz-number`).
macro_rules! server_number {
    ($(#[$doc:meta])* $name:ident, $inner:ty, $plain:ty) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name($inner);

        impl $name {
            /// The number `value`, or `None` for zero.
            #[must_use]
            pub const fn new(value: $plain) -> Option<Self> {
                match <$inner>::new(value) {
                    Some(value) => Some(Self(value)),
                    None => None,
                }
            }

            /// The number.
            #[must_use]
            pub const fn get(self) -> $plain {
                self.0.get()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

server_number!(
    /// A message's UID in its folder (RFC 9051 section 2.3.1.1); it means something only together with the folder's
    /// [`UidValidity`].
    Uid,
    NonZeroU32,
    u32
);
server_number!(
    /// A folder's UIDVALIDITY: when it changes, every UID the folder had before is void.
    UidValidity,
    NonZeroU32,
    u32
);
server_number!(
    /// A modification sequence (RFC 7162, CONDSTORE).
    ModSeq,
    NonZeroU64,
    u64
);

#[cfg(test)]
#[path = "id_tests.rs"]
mod tests;
