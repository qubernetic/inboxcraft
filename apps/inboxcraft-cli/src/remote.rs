// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The running app, reached through the control channel (`docs/control-protocol.md`): the socket in the user's runtime
//! folder, the token from the OS credential store. Each command opens a connection of its own, so a long `mcp` session
//! never holds one while it waits for its client.

use std::path::{Path, PathBuf};
use std::time::Duration;

use inboxcraft_control_key::{Failure, Token};
use serde_json::Value;

/// How long the CLI waits for the app's answer: longer than the app waits for its window.
pub const PATIENCE: Duration = Duration::from_secs(60);

/// Where the CLI looks for the running app, and how it gets the token.
pub struct Reach {
    runtime: Option<PathBuf>,
    token: Box<dyn Fn() -> Result<Token, Failure>>,
}

impl std::fmt::Debug for Reach {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reach").field("runtime", &self.runtime).finish_non_exhaustive()
    }
}

impl Reach {
    /// The app in `runtime` (the user's runtime folder), with the token `token` reads.
    #[must_use]
    pub fn new(runtime: Option<PathBuf>, token: Box<dyn Fn() -> Result<Token, Failure>>) -> Self {
        Self { runtime, token }
    }

    /// The user's app: `$XDG_RUNTIME_DIR`, and the token in the OS credential store.
    #[must_use]
    pub fn of_user() -> Self {
        Self::new(std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from), Box::new(os_token))
    }

    /// The runtime folder looked in.
    #[must_use]
    pub fn runtime(&self) -> Option<&Path> {
        self.runtime.as_deref()
    }
}

/// The token in the OS credential store.
///
/// # Errors
///
/// `no_token`, in fixed words: a `keyring` error can carry the bytes it could not decode.
pub fn os_token() -> Result<Token, Failure> {
    let entry = keyring::Entry::new(inboxcraft_control_key::SERVICE, inboxcraft_control_key::ENTRY)
        .map_err(|_| Failure::new("no_token", "the credential store cannot be reached, so there is no control token"))?;
    let text = entry.get_password().map_err(|_| Failure::new("no_token", "there is no control token in the credential store; start InboxCraft"))?;
    Token::parse(&text)
        .ok_or_else(|| Failure::new("no_token", "the control token in the credential store is broken; run inboxcraft --revoke-control"))
}

/// The running app, found.
pub(crate) struct Remote {
    socket: PathBuf,
    token: Token,
}

impl Remote {
    /// The app `reach` finds: the first socket that the user alone could have made and that answers a connection, then
    /// the token. Nothing is written to a socket in a folder another account could write, so it never sees the token.
    pub(crate) fn find(reach: &Reach) -> Result<Self, Failure> {
        let runtime = reach.runtime().filter(|runtime| runtime.is_absolute()).ok_or_else(|| {
            Failure::new("not_running", "XDG_RUNTIME_DIR is not set, so the running InboxCraft cannot be reached; give --store <dir>")
        })?;
        let uid = user(runtime)?;
        let socket = inboxcraft_control_key::sockets(runtime)
            .into_iter()
            .find(|socket| trusted(socket, uid) && connect(socket).is_ok())
            .ok_or_else(|| Failure::new("not_running", "InboxCraft is not running (no control socket answers); start it, or give --store <dir>"))?;
        Ok(Self { socket, token: (reach.token)()? })
    }

    /// Runs `id` with `params` in the app and reads its answer.
    pub(crate) fn run(&self, id: &str, params: &Value) -> Result<Value, Failure> {
        exchange(&self.socket, &inboxcraft_control_key::line::request_line(&self.token, id, params))
    }
}

impl inboxcraft_mcp::Commands for Remote {
    fn run(&mut self, id: &str, params: &Value) -> Result<Value, Failure> {
        Remote::run(self, id, params)
    }
}

/// The effective user, when `runtime` is a folder of theirs alone (`0700`).
///
/// The app's peer is not checked after connecting: the standard library cannot read it yet, and the channel adds no
/// crate for it (a gap); the owner and mode of the folders stand in for it.
#[cfg(unix)]
fn user(runtime: &Path) -> Result<u32, Failure> {
    let uid = inboxcraft_control_key::folder::effective_uid().ok_or_else(|| {
        Failure::new(
            "not_running",
            "the user this program runs as is not known on this system, so the running InboxCraft cannot be trusted; give --store <dir>",
        )
    })?;
    if private(runtime, uid) {
        Ok(uid)
    } else {
        Err(Failure::new(
            "not_running",
            "XDG_RUNTIME_DIR is not a folder of this user's alone (theirs, mode 0700), so the running InboxCraft is not trusted there; give --store <dir>",
        ))
    }
}

#[cfg(not(unix))]
fn user(_runtime: &Path) -> Result<u32, Failure> {
    Ok(0)
}

/// Whether `folder` is a real folder of `uid`'s that nobody else may enter.
#[cfg(unix)]
fn private(folder: &Path, uid: u32) -> bool {
    use inboxcraft_control_key::folder::{PRIVATE, kept, look};
    look(folder).is_ok_and(|seen| kept(seen, uid, PRIVATE))
}

/// Whether `socket` is a socket of `uid`'s in a folder of `uid`'s alone, so no other account could have put it there.
#[cfg(unix)]
fn trusted(socket: &Path, uid: u32) -> bool {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    socket.parent().is_some_and(|folder| private(folder, uid))
        && std::fs::symlink_metadata(socket).is_ok_and(|meta| meta.file_type().is_socket() && meta.uid() == uid)
}

#[cfg(not(unix))]
fn trusted(_socket: &Path, _uid: u32) -> bool {
    false
}

#[cfg(unix)]
fn connect(socket: &Path) -> std::io::Result<std::os::unix::net::UnixStream> {
    std::os::unix::net::UnixStream::connect(socket)
}

#[cfg(not(unix))]
fn connect(_socket: &Path) -> std::io::Result<std::convert::Infallible> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "the control channel is not built on this system yet"))
}

/// Writes `line` on a new connection to `socket` and reads one answer.
#[cfg(unix)]
fn exchange(socket: &Path, line: &str) -> Result<Value, Failure> {
    use std::io::{BufRead, BufReader, Read, Write};
    let closed = |error: std::io::Error| Failure::new("closed", format!("the connection to InboxCraft failed: {error}"));
    let mut stream = connect(socket).map_err(closed)?;
    stream.set_read_timeout(Some(PATIENCE)).map_err(closed)?;
    stream.write_all(line.as_bytes()).map_err(closed)?;
    let cap = u64::try_from(inboxcraft_control_key::line::MAX_ANSWER).unwrap_or(u64::MAX);
    let mut answer = Vec::new();
    BufReader::new(stream).take(cap).read_until(b'\n', &mut answer).map_err(closed)?;
    inboxcraft_control_key::line::read_answer(&answer)
}

#[cfg(not(unix))]
fn exchange(_socket: &Path, _line: &str) -> Result<Value, Failure> {
    Err(Failure::new("not_running", "the control channel is not built on this system yet; give --store <dir>"))
}

#[cfg(test)]
#[path = "remote_tests.rs"]
mod tests;
