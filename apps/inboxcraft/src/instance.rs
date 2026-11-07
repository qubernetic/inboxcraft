// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! One InboxCraft at a time on the owner's mailbox: the first start holds a lock in the store's folder and listens on a
//! socket beside it; a second start finds the lock held and asks the first, through the socket, to show its window
//! (or, with `--quit`, to end). Then it leaves, so two runs never open the same store.
//!
//! The lock is the operating system's lock on [`LOCK_FILE`]; it goes with the process however that ends, so a crash
//! never leaves the folder held. The socket ([`SOCKET_FILE`], Unix only) takes one short line ([`Request`]) and answers
//! `ok`; anything else is closed unanswered. The folder (`0700`), the lock (`0600`) and the socket (`0600`) are the
//! owner's alone, whatever the umask. On systems without Unix sockets the
//! lock still keeps a second run away, which then says that InboxCraft is already running.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The lock file in the store's folder.
pub const LOCK_FILE: &str = "instance.lock";

/// The socket in the store's folder.
pub const SOCKET_FILE: &str = "instance.sock";

/// The longest line the socket reads, line end included.
pub const MAX_LINE: usize = 16;

/// How long the first start waits for a second one to say what it wants.
pub const READ_TIMEOUT: Duration = Duration::from_secs(1);

/// How long a second start keeps trying to reach the first by default: the first may still be starting.
pub const PATIENCE: Duration = Duration::from_secs(5);

/// How long a second start waits between two tries.
const RETRY: Duration = Duration::from_millis(50);

/// What a second start asks of the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Show the window and bring it forward.
    Show,
    /// End.
    Quit,
}

impl Request {
    /// The line that carries it.
    #[must_use]
    pub fn line(self) -> &'static str {
        match self {
            Self::Show => "show\n",
            Self::Quit => "quit\n",
        }
    }

    /// The request in `line`, with or without its line end; `None` for anything else.
    #[must_use]
    pub fn parse(line: &[u8]) -> Option<Self> {
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        match line.strip_suffix(b"\r").unwrap_or(line) {
            b"show" => Some(Self::Show),
            b"quit" => Some(Self::Quit),
            _ => None,
        }
    }
}

/// Why the folder could not be claimed, or the running start not reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceError(String);

impl std::fmt::Display for InstanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for InstanceError {}

/// What [`claim`] found.
#[derive(Debug)]
pub enum Claim {
    /// No other start holds the folder; this one does now.
    First(Instance),
    /// Another start holds it.
    Running(Running),
}

/// Holds the folder for as long as it lives.
#[derive(Debug)]
pub struct Instance {
    lock: File,
    dir: PathBuf,
}

/// Claims `dir` (made if missing) for this start.
///
/// # Errors
///
/// A folder or lock file that cannot be made or locked.
pub fn claim(dir: &Path) -> Result<Claim, InstanceError> {
    let failed = |error: std::io::Error| InstanceError(format!("{} could not be claimed: {error}", dir.display()));
    let lock = private(dir).map_err(failed)?;
    match lock.try_lock() {
        Ok(()) => Ok(Claim::First(Instance { lock, dir: dir.to_path_buf() })),
        Err(TryLockError::WouldBlock) => Ok(Claim::Running(Running { socket: dir.join(SOCKET_FILE), patience: PATIENCE })),
        Err(TryLockError::Error(error)) => Err(failed(error)),
    }
}

impl Instance {
    /// Listens for later starts on a thread of its own and hands each request to `heard`. A socket that cannot be made
    /// leaves the folder held all the same; [`Listening::error`] says why.
    pub fn listen(self, heard: impl Fn(Request) + Send + 'static) -> Listening {
        let socket = self.dir.join(SOCKET_FILE);
        let error = listen(&socket, heard).err().map(|error| InstanceError(format!("{} could not be opened: {error}", socket.display())));
        Listening { _lock: self.lock, socket, error }
    }
}

/// The first start, listening; the folder stays held and the socket open until it is dropped.
#[derive(Debug)]
pub struct Listening {
    _lock: File,
    socket: PathBuf,
    error: Option<InstanceError>,
}

impl Listening {
    /// Why later starts cannot reach this one, if they cannot.
    #[must_use]
    pub fn error(&self) -> Option<&InstanceError> {
        self.error.as_ref()
    }
}

impl Drop for Listening {
    fn drop(&mut self) {
        if self.error.is_none() {
            // The next start replaces a socket left behind anyway.
            let _ = std::fs::remove_file(&self.socket);
        }
    }
}

/// The start that holds the folder, as a later one reaches it.
#[derive(Debug)]
pub struct Running {
    socket: PathBuf,
    patience: Duration,
}

impl Running {
    /// The same, trying for `patience` instead of [`PATIENCE`].
    #[must_use]
    pub fn with_patience(self, patience: Duration) -> Self {
        Self { patience, ..self }
    }

    /// Hands `request` to the running start and waits for its `ok`.
    ///
    /// # Errors
    ///
    /// The running start did not answer within the patience, or answered something else.
    pub fn send(&self, request: Request) -> Result<(), InstanceError> {
        // Tries again every RETRY until the patience is spent: a refused or missing socket fails at once.
        let tries = self.patience.as_millis().checked_div(RETRY.as_millis()).unwrap_or(0);
        let mut tried = 0;
        loop {
            let error = match send(&self.socket, request, self.patience) {
                Ok(()) => return Ok(()),
                Err(error) => error,
            };
            tried += 1;
            if tried > tries || error.kind() == std::io::ErrorKind::Unsupported {
                return Err(InstanceError(format!("InboxCraft is already running but did not answer: {error}")));
            }
            std::thread::sleep(RETRY);
        }
    }
}

/// Makes `dir` (`0700`, the folders above it that are missing too) and opens its lock file (`0600`), whatever the
/// umask; a folder or lock file found wider is narrowed. A lock others could open would let them hold the folder.
#[cfg(unix)]
fn private(dir: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    if std::fs::metadata(dir)?.permissions().mode() & 0o777 != 0o700 {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let lock = OpenOptions::new().create(true).truncate(false).write(true).mode(0o600).open(dir.join(LOCK_FILE))?;
    if lock.metadata()?.permissions().mode() & 0o777 != 0o600 {
        lock.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(lock)
}

/// Elsewhere the folder and the lock inherit the ACL of the owner's data folder.
#[cfg(not(unix))]
fn private(dir: &Path) -> std::io::Result<File> {
    std::fs::create_dir_all(dir)?;
    OpenOptions::new().create(true).truncate(false).write(true).open(dir.join(LOCK_FILE))
}

#[cfg(unix)]
fn listen(socket: &Path, heard: impl Fn(Request) + Send + 'static) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    // Only the holder of the lock gets here: a socket file is one a run that ended left behind.
    match std::fs::remove_file(socket) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let listener = UnixListener::bind(socket)?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    std::thread::Builder::new().name("inboxcraft-instance".to_owned()).spawn(move || {
        for stream in listener.incoming().flatten() {
            serve(stream, &heard);
        }
    })?;
    Ok(())
}

/// Reads one request from `stream`, hands it on and answers `ok`; anything else is closed unanswered.
#[cfg(unix)]
fn serve(mut stream: std::os::unix::net::UnixStream, heard: &impl Fn(Request)) {
    use std::io::{Read, Write};
    if stream.set_read_timeout(Some(READ_TIMEOUT)).is_err() {
        return;
    }
    let mut line = Vec::new();
    let mut byte = [0_u8];
    while line.len() < MAX_LINE && !line.ends_with(b"\n") {
        match stream.read(&mut byte) {
            Ok(1) => line.push(byte[0]),
            _ => break,
        }
    }
    if let Some(request) = Request::parse(&line) {
        heard(request);
        // A second start that left before the answer needs none.
        let _ = stream.write_all(b"ok\n");
    }
}

#[cfg(unix)]
fn send(socket: &Path, request: Request, patience: Duration) -> std::io::Result<()> {
    use std::io::{Read, Write};
    let mut stream = std::os::unix::net::UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(patience))?;
    stream.write_all(request.line().as_bytes())?;
    let mut answer = [0_u8; 3];
    stream.read_exact(&mut answer)?;
    if &answer == b"ok\n" { Ok(()) } else { Err(std::io::Error::other("an answer that is not ok")) }
}

#[cfg(not(unix))]
fn listen(_socket: &Path, _heard: impl Fn(Request) + Send + 'static) -> std::io::Result<()> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "no Unix sockets on this system"))
}

#[cfg(not(unix))]
fn send(_socket: &Path, _request: Request, _patience: Duration) -> std::io::Result<()> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "no Unix sockets on this system"))
}

#[cfg(test)]
#[path = "instance_tests.rs"]
mod tests;
