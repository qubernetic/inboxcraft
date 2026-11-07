// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The local control channel (`docs/control-protocol.md`): the CLI and the MCP server run commands in the running app
//! through a socket in the user's runtime folder, one JSON line each way.
//!
//! - The token: [`ensure`] keeps one in the OS credential store under [`ENTRY`], made from the operating system's
//!   random source ([`fresh`]) when there is none; [`revoke`] (`inboxcraft --revoke-control`) puts a new one in its
//!   place. The server reads the store for every line, so a revoked token is refused from the next line on, on a
//!   connection that is already open too; with no token in the store, or no store, every line is refused.
//! - The server ([`listen`]): the folder (`0700`) and the socket (`0600`) are the owner's alone, whatever the umask. One
//!   thread a client, at most [`Limits::clients`] at a time; a client that says nothing for [`Limits::idle`] is let go.
//!   A line with a wrong, missing or revoked token is answered `unauthorized` and the connection closed; a line that is
//!   no request is answered `bad_line`; a line over the limit `too_long`, and the connection closed. A request with the
//!   right token goes to the window as a [`Call`], which runs it for an agent: only a `safe` command runs
//!   (`Mailbox::run_for_agent`). A window that does not answer within [`Limits::patience`] is answered `busy`.
//!
//! Unix only: elsewhere [`listen`] says the channel is not built (a gap, see `docs/gaps.md`).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::Duration;

use inboxcraft_control_key::{ENTRY, Failure, TOKEN_BYTES, Token};
use inboxcraft_ui_egui::fetch::Wake;
use serde_json::Value;

use crate::accounts::Entries;

/// How the server keeps clients in check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The most clients served at a time; one more is answered `busy` and let go.
    pub clients: usize,
    /// How long a client may say nothing before it is let go.
    pub idle: Duration,
    /// How long the window may take to answer a call before the client is answered `busy`.
    pub patience: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self { clients: 8, idle: Duration::from_secs(300), patience: Duration::from_secs(30) }
    }
}

/// One request with the right token, for the window to run and answer.
#[derive(Debug)]
pub struct Call {
    command: String,
    params: Value,
    answer: Sender<Result<Value, Failure>>,
}

impl Call {
    /// The call of `command` with `params`, answered on `answer`.
    #[must_use]
    pub fn new(command: String, params: Value, answer: Sender<Result<Value, Failure>>) -> Self {
        Self { command, params, answer }
    }

    /// The command id.
    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }

    /// The command's parameters: one JSON object.
    #[must_use]
    pub fn params(&self) -> &Value {
        &self.params
    }

    /// Hands `answer` back to the client; nothing when it has left.
    pub fn answer(self, answer: Result<Value, Failure>) {
        // A client that left needs no answer.
        let _ = self.answer.send(answer);
    }
}

/// 32 bytes from the operating system's random source, as a token.
///
/// # Errors
///
/// The random source could not be read.
pub fn fresh() -> std::io::Result<Token> {
    use std::io::Read;
    let mut bytes = [0_u8; TOKEN_BYTES];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(Token::from_bytes(bytes))
}

/// Keeps a token in `entries`: the one there when it reads as one, else a new one from `fresh`.
///
/// # Errors
///
/// Why no token could be made or kept, in words for the owner.
pub fn ensure(entries: &dyn Entries, fresh: &dyn Fn() -> std::io::Result<Token>) -> Result<(), String> {
    if entries.get(ENTRY).as_deref().and_then(Token::parse).is_some() {
        return Ok(());
    }
    replace(entries, fresh)
}

/// Puts a new token from `fresh` into `entries`.
fn replace(entries: &dyn Entries, fresh: &dyn Fn() -> std::io::Result<Token>) -> Result<(), String> {
    let token = fresh().map_err(|error| format!("no control token could be made: {error}"))?;
    entries.set(ENTRY, &token.text()).map_err(|error| format!("the control token could not be kept: {error}"))
}

/// `--revoke-control`: puts a new token in the place of the old one, so a client that kept the old one is refused. 0
/// when done, 1 when not; the token itself is never written out.
pub fn revoke(entries: &dyn Entries, fresh: &dyn Fn() -> std::io::Result<Token>, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let name = env!("CARGO_PKG_NAME");
    match replace(entries, fresh) {
        Ok(()) => {
            let _ = writeln!(out, "{name}: a new control token is in the credential store; clients that kept the old one are refused");
            0
        }
        Err(error) => {
            let _ = writeln!(err, "{name}: {error}");
            1
        }
    }
}

/// Starts the channel for the app: keeps a token in `entries` ([`ensure`]) and listens on the socket in `runtime`
/// (`$XDG_RUNTIME_DIR`, [`inboxcraft_control_key::socket`], the Flatpak's folder when `sandboxed`). The calls come
/// out of the receiver, for [`Desktop::with_control`](crate::desktop::Desktop::with_control).
///
/// # Errors
///
/// Why there is no channel, in words for the owner: no runtime folder, no token, or no socket.
pub fn start(
    runtime: Option<&Path>,
    sandboxed: bool,
    entries: std::sync::Arc<dyn Entries>,
    fresh: &dyn Fn() -> std::io::Result<Token>,
    wake: Wake,
) -> Result<(Listening, std::sync::mpsc::Receiver<Call>), String> {
    let runtime = runtime.filter(|runtime| runtime.is_absolute()).ok_or("XDG_RUNTIME_DIR is not set to a folder")?;
    ensure(entries.as_ref(), fresh)?;
    let socket = inboxcraft_control_key::socket(runtime, sandboxed);
    let (send, calls) = std::sync::mpsc::channel();
    let listening = listen(&socket, entries, send, wake).map_err(|error| format!("{} could not be opened: {error}", socket.display()))?;
    Ok((listening, calls))
}

/// The server, listening; the socket goes when it is dropped.
#[derive(Debug)]
pub struct Listening {
    socket: PathBuf,
}

impl Drop for Listening {
    fn drop(&mut self) {
        // The next start replaces a socket left behind anyway.
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// Listens on `socket` with the default [`Limits`]; see [`listen_with`].
///
/// # Errors
///
/// As [`listen_with`].
pub fn listen(socket: &Path, entries: std::sync::Arc<dyn Entries>, calls: Sender<Call>, wake: Wake) -> std::io::Result<Listening> {
    listen_with(socket, entries, calls, wake, Limits::default())
}

/// Listens on `socket` (its folder made `0700` if missing, narrowed if wider) on a thread of its own: every request
/// with the token `entries` holds goes to `calls`, and `wake` wakes the window to run it.
///
/// # Errors
///
/// The folder or the socket could not be made, another app serves `socket` already, or (outside Unix) the channel is
/// not built.
#[cfg(unix)]
pub fn listen_with(
    socket: &Path,
    entries: std::sync::Arc<dyn Entries>,
    calls: Sender<Call>,
    wake: Wake,
    limits: Limits,
) -> std::io::Result<Listening> {
    unix::listen(socket, entries, calls, wake, limits)
}

/// Not built outside Unix yet.
///
/// # Errors
///
/// Always [`std::io::ErrorKind::Unsupported`].
#[cfg(not(unix))]
pub fn listen_with(
    _socket: &Path,
    _entries: std::sync::Arc<dyn Entries>,
    _calls: Sender<Call>,
    _wake: Wake,
    _limits: Limits,
) -> std::io::Result<Listening> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "the control channel is not built on this system yet"))
}

#[cfg(unix)]
mod unix {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::{self, Sender};

    use inboxcraft_control_key::line::{MAX_LINE, answer_line, read_request};
    use inboxcraft_control_key::{ENTRY, Failure, Token};
    use inboxcraft_ui_egui::fetch::Wake;
    use serde_json::Value;

    use super::{Call, Limits, Listening};
    use crate::accounts::Entries;

    pub(super) fn listen(socket: &Path, entries: Arc<dyn Entries>, calls: Sender<Call>, wake: Wake, limits: Limits) -> std::io::Result<Listening> {
        if let Some(folder) = socket.parent() {
            std::fs::DirBuilder::new().recursive(true).mode(0o700).create(folder)?;
            if std::fs::metadata(folder)?.permissions().mode() & 0o777 != 0o700 {
                std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        if UnixStream::connect(socket).is_ok() {
            return Err(std::io::Error::new(std::io::ErrorKind::AddrInUse, "another InboxCraft serves the control channel already"));
        }
        match std::fs::remove_file(socket) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
        let listener = UnixListener::bind(socket)?;
        let listening = Listening { socket: socket.to_path_buf() };
        std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
        let served = Arc::new(AtomicUsize::new(0));
        std::thread::Builder::new().name("inboxcraft-control".to_owned()).spawn(move || {
            for stream in listener.incoming().flatten() {
                welcome(stream, &served, &entries, &calls, &wake, limits);
            }
        })?;
        Ok(listening)
    }

    /// Holds one of the clients' places while it lives.
    struct Place(Arc<AtomicUsize>);

    impl Drop for Place {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Serves `stream` on a thread of its own, or answers `busy` when every place is taken.
    fn welcome(mut stream: UnixStream, served: &Arc<AtomicUsize>, entries: &Arc<dyn Entries>, calls: &Sender<Call>, wake: &Wake, limits: Limits) {
        if served.fetch_add(1, Ordering::SeqCst) >= limits.clients {
            served.fetch_sub(1, Ordering::SeqCst);
            let _ = stream.write_all(answer_line(&Err(Failure::new("busy", "too many clients are connected"))).as_bytes());
            return;
        }
        let place = Place(Arc::clone(served));
        let (entries, calls, wake) = (Arc::clone(entries), calls.clone(), Arc::clone(wake));
        let spawned = std::thread::Builder::new().name("inboxcraft-control-client".to_owned()).spawn(move || {
            let _place = place;
            serve(&stream, entries.as_ref(), &calls, &wake, limits);
        });
        // A client without a thread is let go; its place went with the closure.
        drop(spawned);
    }

    /// Answers the lines of one client until it leaves, says nothing for too long, or is refused.
    fn serve(stream: &UnixStream, entries: &dyn Entries, calls: &Sender<Call>, wake: &Wake, limits: Limits) {
        let Ok(reading) = stream.try_clone() else { return };
        if stream.set_read_timeout(Some(limits.idle)).is_err() {
            return;
        }
        let mut reader = BufReader::new(reading);
        let mut writer = stream;
        let mut line = Vec::new();
        loop {
            line.clear();
            let cap = u64::try_from(MAX_LINE).unwrap_or(u64::MAX).saturating_add(1);
            match (&mut reader).take(cap).read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => return,
                Ok(_) => {}
            }
            let (answer, goes_on) = answer(&line, entries, calls, wake, limits);
            if writer.write_all(answer_line(&answer).as_bytes()).is_err() || !goes_on {
                return;
            }
        }
    }

    /// The answer to one line, and whether the connection goes on after it.
    fn answer(line: &[u8], entries: &dyn Entries, calls: &Sender<Call>, wake: &Wake, limits: Limits) -> (Result<Value, Failure>, bool) {
        let request = match read_request(line) {
            Ok(request) => request,
            Err(failure) => {
                let goes_on = failure.code != "too_long";
                return (Err(failure), goes_on);
            }
        };
        let expected = entries.get(ENTRY).as_deref().and_then(Token::parse);
        match (request.token, expected) {
            (Some(shown), Some(expected)) if shown == expected => {}
            _ => return (Err(Failure::new("unauthorized", "the control token is wrong or revoked")), false),
        }
        let (send, answered) = mpsc::channel();
        if calls.send(Call::new(request.command, request.params, send)).is_err() {
            return (Err(Failure::new("closing", "InboxCraft is closing")), false);
        }
        wake();
        match answered.recv_timeout(limits.patience) {
            Ok(answer) => (answer, true),
            Err(_) => (Err(Failure::new("busy", "InboxCraft did not answer in time")), true),
        }
    }
}

#[cfg(test)]
#[cfg(unix)]
#[path = "control_tests.rs"]
mod tests;
