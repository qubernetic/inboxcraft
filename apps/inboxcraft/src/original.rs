// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reader's "Open the original in the browser" on the desktop ([`Originals`], the window's
//! [`Original`](inboxcraft_ui_egui::original::Original)).
//!
//! The page it is handed is the cleaned copy the reader made (no script, no event handler, no address). It is written
//! to a private file, then handed to the desktop:
//!
//! - The folder is [`DIR`] in the owner's runtime folder (`$XDG_RUNTIME_DIR`, a per-user `tmpfs` the session empties at
//!   logout), else [`CACHE_DIR`] in the owner's cache folder; never the shared `/tmp`. The sample has folders of its
//!   own ([`SAMPLE_DIR`], [`SAMPLE_CACHE_DIR`]), so neither run touches the other's copies. It is made `0700`; one that
//!   is a symbolic link, not a folder or open to anybody else is refused, not repaired.
//! - The file's name comes from the message's number alone ([`name`]), never from anything the message says. It is
//!   made new (`O_CREAT | O_EXCL`, which follows no symbolic link, a dangling one included) with mode `0600`; whatever
//!   stood at the name before (an earlier copy, or a link someone planted) is unlinked first, never followed, and a
//!   file that then turns out not to be the owner's is removed again.
//! - Old copies go: every copy at the start ([`sweep_all`]), and at each open those older than [`KEEP_FOR`], then the
//!   oldest beyond [`MAX_FILES`]. Only names of the [`name`] pattern are touched.
//! - The hand-off ([`Hand`]) asks the desktop portal's `OpenURI.OpenFile` with the file opened read-only, so the
//!   browser gets the file even when it runs in a sandbox of its own; without a portal, outside the Flatpak,
//!   `xdg-open` is run on the path (no shell, the path its one argument). The bus call gives up after
//!   [`BUS_PATIENCE`]. The whole open runs on a thread of its own, never on the frame.

use std::ffi::OsString;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{ErrorKind, Write as _};
use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use inboxcraft_time::{Clock as _, SystemClock};
use inboxcraft_ui_egui::original::Original;

/// The private folder's name in the owner's runtime folder.
pub const DIR: &str = "inboxcraft-originals";

/// The private folder's place in the owner's cache folder, where there is no runtime folder.
pub const CACHE_DIR: &str = "inboxcraft/originals";

/// The sample's private folder in the owner's runtime folder.
pub const SAMPLE_DIR: &str = "inboxcraft-sample-originals";

/// The sample's private folder in the owner's cache folder.
pub const SAMPLE_CACHE_DIR: &str = "inboxcraft/sample-originals";

/// Whose copies: the owner's mailbox's, or the sample's. Each has a folder of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whose {
    /// The owner's mailbox.
    Owner,
    /// The sample mailbox.
    Sample,
}

impl Whose {
    /// The folder's name in the runtime folder, and its place in the cache folder.
    const fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::Owner => (DIR, CACHE_DIR),
            Self::Sample => (SAMPLE_DIR, SAMPLE_CACHE_DIR),
        }
    }
}

/// How long a copy is kept after it was written: long enough for the browser to read it, and to reload it.
pub const KEEP_FOR: Duration = Duration::from_secs(10 * 60);

/// Most copies kept at a time.
pub const MAX_FILES: usize = 16;

/// Most folder entries looked at in one sweep.
const MAX_SWEPT: usize = 1024;

/// The name of message `id`'s copy: its number, nothing of its text.
#[must_use]
pub fn name(id: u64) -> String {
    format!("original-{id}.html")
}

/// Whether `file` is a name [`name`] makes.
fn is_copy(file: &str) -> bool {
    file.strip_prefix("original-")
        .and_then(|rest| rest.strip_suffix(".html"))
        .is_some_and(|number| !number.is_empty() && number.len() <= 20 && number.bytes().all(|b| b.is_ascii_digit()))
}

/// The private folder of `whose` from the owner's runtime folder `runtime`, cache folder `cache` and home `home` (each
/// taken only when absolute): `None` where there is none of them.
#[must_use]
pub fn folder_from(whose: Whose, runtime: Option<OsString>, cache: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    let (dir, cache_dir) = whose.names();
    let absolute = |dir: Option<OsString>| dir.map(PathBuf::from).filter(|dir| dir.is_absolute());
    absolute(runtime)
        .map(|runtime| runtime.join(dir))
        .or_else(|| absolute(cache).map(|cache| cache.join(cache_dir)))
        .or_else(|| absolute(home).map(|home| home.join(".cache").join(cache_dir)))
}

/// The private folder of `whose` in this process ([`folder_from`] over its environment).
#[must_use]
pub fn folder(whose: Whose) -> Option<PathBuf> {
    folder_from(whose, std::env::var_os("XDG_RUNTIME_DIR"), std::env::var_os("XDG_CACHE_HOME"), std::env::var_os("HOME"))
}

/// Hands a written copy to the desktop, which opens it in the browser.
pub trait Hand: Send + Sync {
    /// Opens `file` in the browser. Called on the open's own thread: it may wait.
    ///
    /// # Errors
    ///
    /// Why the desktop did not take it, as the owner reads it.
    fn hand(&self, file: &Path) -> Result<(), String>;
}

/// Writes the copies into a private folder and hands them to the desktop.
pub struct Originals {
    folder: Option<PathBuf>,
    hand: Box<dyn Hand>,
}

impl Originals {
    /// Copies written into `folder` (none: nowhere to write, every open fails) and handed on through `hand`.
    #[must_use]
    pub fn new(folder: Option<PathBuf>, hand: Box<dyn Hand>) -> Self {
        Self { folder, hand }
    }

    /// `whose`'s: its [`folder`], handed to the desktop's portal or `xdg-open` ([`Desktop`]).
    #[cfg(target_os = "linux")]
    #[must_use]
    pub fn of(whose: Whose) -> Self {
        Self::new(folder(whose), Box::new(Desktop { sandboxed: crate::autostart::sandboxed() }))
    }

    /// Writes `page` as message `id`'s copy and says where; old copies go first.
    ///
    /// # Errors
    ///
    /// No folder, a folder that is not private, or a file that could not be written.
    pub fn write(&self, id: u64, page: &str) -> Result<PathBuf, String> {
        let folder = self.folder.as_deref().ok_or_else(|| "there is no private folder to write it to".to_owned())?;
        let owner = private_folder(folder)?;
        sweep(folder, KEEP_FOR, MAX_FILES.saturating_sub(1));
        let path = folder.join(name(id));
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(format!("an earlier copy could not be removed: {error}")),
        }
        let failed = |error: std::io::Error| format!("the copy could not be written: {error}");
        let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path).map_err(failed)?;
        // The folder is the owner's only if the file just made in it has the same owner.
        if file.metadata().map_err(failed)?.uid() != owner {
            let _ = fs::remove_file(&path);
            return Err("the private folder is not the owner's".to_owned());
        }
        if let Err(error) = file.write_all(page.as_bytes()) {
            let _ = fs::remove_file(&path);
            return Err(failed(error));
        }
        Ok(path)
    }
}

impl Original for Originals {
    fn open(&self, id: u64, page: &str) -> Result<(), String> {
        let path = self.write(id, page)?;
        self.hand.hand(&path)
    }
}

/// Makes `folder` (`0700`) where it is missing and checks that it is a private folder: a real folder, not a symbolic
/// link, closed to everybody but its owner. Says its owner's user id.
fn private_folder(folder: &Path) -> Result<u32, String> {
    if let Some(parent) = folder.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("the private folder could not be made: {error}"))?;
    }
    match DirBuilder::new().mode(0o700).create(folder) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(format!("the private folder could not be made: {error}")),
    }
    let meta = fs::symlink_metadata(folder).map_err(|error| format!("the private folder could not be read: {error}"))?;
    if !meta.file_type().is_dir() {
        return Err("the private folder is not a folder".to_owned());
    }
    if meta.mode() & 0o077 != 0 {
        return Err("the private folder is open to others".to_owned());
    }
    Ok(meta.uid())
}

/// Removes the copies in `folder` older than `keep_for`, then the oldest beyond `keep`. Never follows a link: a link
/// at a copy's name is unlinked itself. Looks at no more than [`MAX_SWEPT`] entries.
fn sweep(folder: &Path, keep_for: Duration, keep: usize) {
    let Ok(entries) = fs::read_dir(folder) else { return };
    let now = wall_now();
    let mut kept: Vec<(SystemTime, PathBuf)> = Vec::new();
    for entry in entries.take(MAX_SWEPT).flatten() {
        if !entry.file_name().to_str().is_some_and(is_copy) {
            continue;
        }
        let path = entry.path();
        let written = fs::symlink_metadata(&path).and_then(|meta| meta.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
        let age = now.duration_since(written).unwrap_or(Duration::ZERO);
        if age >= keep_for {
            let _ = fs::remove_file(&path);
        } else {
            kept.push((written, path));
        }
    }
    kept.sort();
    let beyond = kept.len().saturating_sub(keep);
    for (_, path) in kept.into_iter().take(beyond) {
        let _ = fs::remove_file(path);
    }
}

/// The wall-clock time, as the files' times are kept.
pub(crate) fn wall_now() -> SystemTime {
    let seconds = u64::try_from(SystemClock::new().now().unix_seconds()).unwrap_or(0);
    SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
}

/// Removes every copy an earlier run left in `folder`.
pub fn sweep_folder(folder: &Path) {
    sweep(folder, Duration::ZERO, 0);
}

/// Removes every copy in `whose`'s [`folder`]: at the start, what an earlier run left.
pub fn sweep_all(whose: Whose) {
    if let Some(folder) = folder(whose) {
        sweep_folder(&folder);
    }
}

/// The desktop's hand-off: the portal, else (outside the Flatpak) `xdg-open`.
#[cfg(target_os = "linux")]
pub struct Desktop {
    /// Whether InboxCraft runs in the Flatpak, where only the portal reaches the browser.
    pub sandboxed: bool,
}

#[cfg(target_os = "linux")]
impl Hand for Desktop {
    fn hand(&self, file: &Path) -> Result<(), String> {
        let portal = zbus::blocking::connection::Builder::session()
            .map(|builder| builder.method_timeout(BUS_PATIENCE))
            .and_then(zbus::blocking::connection::Builder::build)
            .map_err(|error| format!("the desktop portal could not be reached: {error}"))
            .and_then(|bus| portal::open_file(&bus, file));
        match portal {
            Ok(()) => Ok(()),
            Err(why) if self.sandboxed => Err(why),
            Err(_) => xdg_open(Path::new("xdg-open"), file, XDG_OPEN_PATIENCE),
        }
    }
}

/// How long a call on the session bus (the portal) is waited for.
pub const BUS_PATIENCE: Duration = Duration::from_secs(4);

/// How long `xdg-open` is waited for: one that ends at once with a failure says why; one still running after this
/// has started the browser.
pub const XDG_OPEN_PATIENCE: Duration = Duration::from_millis(800);

/// Runs `program` (`xdg-open`) on `file`, with nothing on its standard streams, and waits up to `patience`.
///
/// # Errors
///
/// The program could not be started, or ended with a failure: no browser is set up.
pub fn xdg_open(program: &Path, file: &Path, patience: Duration) -> Result<(), String> {
    use std::process::{Command, Stdio};
    let mut child = Command::new(program)
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("no program to open web pages was found: {error}"))?;
    let clock = SystemClock::new();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(_)) => return Err("no browser is set up to open web pages".to_owned()),
            Ok(None) if clock.monotonic() < patience => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                // Still running: it started the browser and waits on it. Reaped on a thread of its own.
                std::thread::spawn(move || child.wait());
                return Ok(());
            }
            Err(error) => return Err(format!("the browser could not be started: {error}")),
        }
    }
}

/// The OpenURI portal (`org.freedesktop.portal.OpenURI`), over the session bus.
#[cfg(target_os = "linux")]
pub mod portal {
    use std::collections::HashMap;
    use std::fs::File;
    use std::path::Path;

    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::{Fd, OwnedObjectPath, Value};

    pub use crate::autostart::portal::{DESTINATION, PATH};

    /// Hands `file`, opened read-only, to the portal on `bus`, which opens it in the owner's browser; the portal is
    /// told the file is not to be written.
    ///
    /// # Errors
    ///
    /// The file could not be opened, or the portal could not be asked.
    pub fn open_file(bus: &Connection, file: &Path) -> Result<(), String> {
        let failed = |error: zbus::Error| format!("the desktop portal could not be asked: {error}");
        let read = File::open(file).map_err(|error| format!("the copy could not be read: {error}"))?;
        let options: HashMap<&str, Value<'_>> = HashMap::from([("writable", Value::from(false)), ("ask", Value::from(false))]);
        let proxy = Proxy::new(bus, DESTINATION, PATH, "org.freedesktop.portal.OpenURI").map_err(failed)?;
        let _: OwnedObjectPath = proxy.call("OpenFile", &("", Fd::from(&read), options)).map_err(failed)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "original_tests.rs"]
mod tests;
