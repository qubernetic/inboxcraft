// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The access key and the lines of the local control channel (`docs/control-protocol.md`).
//!
//! Layer 0, standalone: it depends on no other workspace crate, so the app, which serves the channel, and the CLI and
//! the MCP server, which talk to it, share one rule for each of these:
//!
//! - [`Token`]: 32 random bytes the app keeps in the OS credential store under [`SERVICE`] and [`ENTRY`]; a client
//!   reads it from there and shows it on every line. It compares in constant time and never shows in `Debug` output.
//! - [`line`]: one JSON object a line each way, with its limits; everything a client writes is hostile.
//! - [`socket`]: where the socket lives, in the user's runtime folder.
//!
//! It does no I/O and draws no random bytes itself: the app hands it bytes from the operating system.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::{Path, PathBuf};

pub mod line;
mod token;

pub use line::{Failure, Request};
pub use token::{TOKEN_BYTES, Token};

/// The app id: the Flatpak's id and the credential store's service name.
pub const APP_ID: &str = "io.github.qubernetic.inboxcraft";

/// The service name of the credential store entry that holds the token (the same as the account passwords').
pub const SERVICE: &str = APP_ID;

/// The user name of the credential store entry that holds the token.
pub const ENTRY: &str = "control-token";

/// The socket's file name.
pub const SOCKET_FILE: &str = "control.sock";

/// The socket in the user's runtime folder (`$XDG_RUNTIME_DIR`): in a folder of its own, `inboxcraft`, or, for the
/// app `sandboxed` in the Flatpak, in the folder Flatpak shares with the host, `app/<app id>`.
#[must_use]
pub fn socket(runtime: &Path, sandboxed: bool) -> PathBuf {
    let folder = if sandboxed { runtime.join("app").join(APP_ID) } else { runtime.join("inboxcraft") };
    folder.join(SOCKET_FILE)
}

/// Where a client looks for the socket, in this order: a build of its own, then the Flatpak.
#[must_use]
pub fn sockets(runtime: &Path) -> [PathBuf; 2] {
    [socket(runtime, false), socket(runtime, true)]
}

/// The crate's package name.
///
/// ```
/// assert_eq!(inboxcraft_control_key::name(), "inboxcraft-control-key");
/// ```
#[must_use]
pub fn name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests;
